# Gukhanmun: Builds and verifies the Python package through installed wheels.
# Copyright (C) 2026  Hong Minhee
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

"""Local Python checks; invoked only through the corresponding mise tasks."""

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "target" / "python"
STATE = OUTPUT / "installed.json"


def run(command, *, cwd=ROOT, capture=False, env=None):
    print("+", " ".join(map(str, command)), flush=True)
    return subprocess.run(
        list(map(str, command)), cwd=cwd, text=True,
        capture_output=capture, check=not capture, env=env,
    )


def build():
    wheel_dir = OUTPUT / "wheels"
    if wheel_dir.exists():
        shutil.rmtree(wheel_dir)
    wheel_dir.mkdir(parents=True)
    command = [sys.executable, "-m", "maturin", "build", "--locked", "--release",
               "--interpreter", sys.executable, "--out", str(wheel_dir)]
    if sys.platform.startswith("linux"):
        command += ["--compatibility", "linux"]
    run(command, cwd=ROOT / "python" / "gukhanmun")
    wheels = list(wheel_dir.glob("*.whl"))
    if len(wheels) != 1:
        raise RuntimeError(f"expected one fresh wheel, got {wheels}")
    with zipfile.ZipFile(wheels[0]) as wheel:
        names = wheel.namelist()
        for name in ("gukhanmun/py.typed", "gukhanmun/_native.pyi"):
            if name not in names:
                raise RuntimeError(f"wheel omits {name}")
        extensions = [name for name in names if name.startswith("gukhanmun/_native.")
                      and name.endswith((".so", ".pyd"))]
        if len(extensions) != 1:
            raise RuntimeError("wheel must contain exactly one private extension")
        if sys.platform.startswith("linux"):
            with tempfile.TemporaryDirectory(prefix="gukhanmun-elf-") as temporary:
                extension = Path(temporary) / Path(extensions[0]).name
                extension.write_bytes(wheel.read(extensions[0]))
                result = run(["readelf", "-d", extension], capture=True)
                result.check_returncode()
                for line in result.stdout.splitlines():
                    if "RPATH" in line or "RUNPATH" in line:
                        if str(ROOT) in line or sys.base_prefix in line or "/home/" in line:
                            raise RuntimeError(f"wheel leaks build-machine rpath: {line}")
                    if "NEEDED" in line and "libpython" in line:
                        raise RuntimeError(f"extension links libpython: {line}")
    print(f"Verified wheel: {wheels[0]}")


def wheel_env():
    wheels = list((OUTPUT / "wheels").glob("*.whl"))
    if len(wheels) != 1:
        raise RuntimeError("run py-build before py-wheel-env")
    directory = Path(tempfile.mkdtemp(prefix="gukhanmun-wheel-"))
    run(["uv", "venv", "--python", sys.executable, directory])
    (directory / ".gukhanmun-check-env").write_text(str(ROOT), encoding="utf-8")
    python = directory / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    requirements = directory / "test-requirements.txt"
    run(["uv", "export", "--quiet", "--locked", "--only-group", "test", "--no-emit-workspace",
         "--output-file", requirements])
    run(["uv", "pip", "install", "--python", python, "--require-hashes", "-r", requirements])
    run(["uv", "pip", "install", "--python", python, "--no-deps", wheels[0]])
    inspected = run([python, "-I", "-c",
                     "import gukhanmun,json,sys; from pathlib import Path; "
                     "p=Path(gukhanmun.__file__).resolve(); "
                     "assert p.is_relative_to(Path(sys.prefix).resolve()); "
                     "print(json.dumps({'package':str(p.parent),'prefix':sys.prefix}))"],
                    cwd=directory, capture=True)
    inspected.check_returncode()
    data = json.loads(inspected.stdout)
    data.update(directory=str(directory), python=str(python), wheel=str(wheels[0]))
    STATE.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print(f"Installed-wheel environment: {directory}")


def installed():
    data = json.loads(STATE.read_text(encoding="utf-8"))
    directory = Path(data["directory"])
    if not (directory / ".gukhanmun-check-env").is_file():
        raise RuntimeError("installed-wheel environment is missing; run py-wheel-env")
    return data


def test():
    data = installed()
    directory = Path(data["directory"])
    tests = directory / "tests"
    if tests.exists():
        shutil.rmtree(tests)
    shutil.copytree(ROOT / "tests" / "python", tests)
    environment = {k: v for k, v in os.environ.items() if k not in ("PYTHONPATH", "MYPYPATH")}
    environment["GUKHANMUN_REPO_ROOT"] = str(ROOT)
    environment["GUKHANMUN_FIXTURE_DIR"] = str(OUTPUT / "fixtures")
    run([data["python"], "-I", "-m", "pytest", "-q", tests], cwd=directory, env=environment)


def diagnostics(checker, output):
    if checker in ("pyright", "basedpyright", "pyrefly"):
        start = output.find("{")
        if start < 0:
            raise RuntimeError(f"{checker} produced no diagnostic JSON: {output}")
        data, _ = json.JSONDecoder().raw_decode(output[start:])
        if checker == "pyrefly":
            return [(Path(d["path"]).name, d["line"], d["name"])
                    for d in data["errors"] if d.get("severity") in ("error", "warn", "warning")]
        return [(Path(d["file"]).name, d["range"]["start"]["line"] + 1, d.get("rule", ""))
                for d in data["generalDiagnostics"] if d["severity"] in ("error", "warning")]
    if checker == "mypy":
        return [(Path(m[1]).name, int(m[2]), m[3]) for line in output.splitlines()
                if (m := re.match(r"^(.*?):(\d+): error: .*\[([^]]+)\]$", line))]
    return [(Path(m[1]).name, int(m[2]), m[3]) for line in output.splitlines()
            if (m := re.match(r"^(.*?):(\d+):\d+: (?:error|warning)\[([^]]+)\]", line))]


RULES = {
    "mypy": {"enum": {"arg-type"}, "required": {"call-arg"}, "immutable": {"misc"}},
    "pyright": {"enum": {"reportArgumentType"}, "required": {"reportCallIssue"},
                "immutable": {"reportAttributeAccessIssue", "reportAssignmentType"}},
    "basedpyright": {"enum": {"reportArgumentType"}, "required": {"reportCallIssue"},
                     "immutable": {"reportAttributeAccessIssue", "reportAssignmentType"}},
    "ty": {"enum": {"invalid-argument-type"}, "required": {"missing-argument"},
           "immutable": {"invalid-assignment", "invalid-attribute-access", "read-only"}},
    "pyrefly": {"enum": {"bad-argument-type"}, "required": {"missing-argument"},
                "immutable": {"read-only", "read-only-attribute", "bad-assignment"}},
}


def typecheck():
    data = installed()
    directory = Path(data["directory"])
    consumers = directory / "consumers"
    if consumers.exists():
        shutil.rmtree(consumers)
    shutil.copytree(ROOT / "tests" / "python" / "typing", consumers)
    positive, negative = consumers / "positive.py", consumers / "negative.py"
    expected = {line: match[1] for line, text in enumerate(negative.read_text().splitlines(), 1)
                if (match := re.search(r"# expected: ([a-z-]+)", text))}
    package = Path(data["package"])
    python = data["python"]
    environment = {k: v for k, v in os.environ.items()
                   if k not in ("PYTHONPATH", "MYPYPATH", "VIRTUAL_ENV")}
    environment["NO_COLOR"] = "1"
    config = consumers / "pyrightconfig.json"
    # Boundary validation also protects untyped callers. Internal helpers may
    # cross private modules; neither diagnostic describes a missing type contract.
    config.write_text(json.dumps({"typeCheckingMode": "strict", "pythonVersion": "3.11",
                                  "reportUnnecessaryIsInstance": False,
                                  "reportPrivateUsage": False, "extraPaths": []}), encoding="utf-8")
    commands = {
        "mypy": [sys.executable, "-m", "mypy", "--strict", "--show-error-codes", "--no-incremental",
                 "--python-version", "3.11", "--python-executable", python],
        "pyright": ["pyright", "--outputjson", "--project", str(config), "--pythonpath", python],
        "basedpyright": ["basedpyright", "--outputjson", "--project", str(config), "--pythonpath", python],
        "ty": ["ty", "check", "--python", python, "--python-version", "3.11",
               "--output-format", "concise", "--error-on-warning"],
        # @override is a Python 3.12 addition; decorator style is not part of
        # the Python 3.11 runtime contract. Override signatures remain checked.
        "pyrefly": ["pyrefly", "check", "--preset", "strict", "--python-interpreter-path", python,
                    "--python-version", "3.11", "--output-format", "json",
                    "--min-severity", "warn",
                    "--ignore", "missing-override-decorator"],
    }
    summary = {}
    for checker, command in commands.items():
        clean_inputs = (["--package", "gukhanmun", "--module", "positive"]
                        if checker == "mypy" else [package, positive])
        clean = run([*command, *clean_inputs], cwd=consumers, capture=True, env=environment)
        (directory / f"{checker}-positive.log").write_text(clean.stdout + clean.stderr)
        if clean.returncode != 0 or diagnostics(checker, clean.stdout):
            raise RuntimeError(f"{checker} positive/package check failed:\n{clean.stdout}{clean.stderr}")
        rejected = run([*command, negative], cwd=consumers, capture=True, env=environment)
        (directory / f"{checker}-negative.log").write_text(rejected.stdout + rejected.stderr)
        findings = diagnostics(checker, rejected.stdout)
        seen = set()
        for filename, line, rule in findings:
            category = expected.get(line)
            if filename != "negative.py" or category is None or rule not in RULES[checker][category]:
                raise RuntimeError(f"{checker} unexpected diagnostic {(filename, line, rule)}:\n{rejected.stdout}")
            seen.add(line)
        if rejected.returncode == 0 or seen != set(expected):
            raise RuntimeError(f"{checker} missing intended diagnostics {set(expected) - seen}:\n{rejected.stdout}")
        summary[checker] = {"positive": "passed", "negative_lines": sorted(seen)}
        print(f"{checker}: installed package/positive types and {len(seen)} intended negative lines passed")
    run([python, "-I", "-m", "mypy.stubtest", "gukhanmun._native"], cwd=directory, env=environment)
    (OUTPUT / "typing-results.json").write_text(json.dumps(summary, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "wheel-env", "test", "typecheck"))
    args = parser.parse_args()
    {"build": build, "wheel-env": wheel_env, "test": test, "typecheck": typecheck}[args.command]()


if __name__ == "__main__":
    main()
