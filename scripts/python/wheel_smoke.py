# Gukhanmun: Installs Python artifacts from a local index and smoke-tests them.
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

"""Install built Python artifacts the way users would, then smoke-test them.

This script uses only the standard library, so it runs on bare interpreters
such as those of the musllinux images, and it never needs Rust.  `run` copies
the artifacts into a PEP 503 simple index under a temporary directory and
installs them with each venv's own pip, from that index alone and from wheels
only, in an environment without Cargo, rustc or inherited pip settings:

 -  the core package alone, followed by *tests/python-wheel/smoke.py* in base
    mode;
 -  a dry-run resolution of every extra, which must select exactly the core
    package and the data distribution that extra pins;
 -  with `--official`, the core package with every extra, followed by the
    smoke checks over the official dictionary corpus.

Every installation report is checked against the staged files and hashes.
The run writes a JSON evidence record naming the exact files it tested.
"""

import argparse
import email.parser
import hashlib
import html
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
from urllib.parse import urlsplit
from urllib.request import url2pathname
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SMOKE = ROOT / "tests" / "python-wheel" / "smoke.py"
SCHEMA = 1
# Variables that could redirect pip, the interpreter, the dynamic loader or a
# source build.  The runtime environment is rebuilt from a small allowlist, so
# this list only documents what must never leak through that allowlist.
KEEP = ("SYSTEMROOT", "SYSTEMDRIVE", "WINDIR", "COMSPEC", "PATHEXT", "TEMP", "TMP", "TMPDIR",
        "HOME", "USERPROFILE", "LOCALAPPDATA", "APPDATA", "LANG", "LC_ALL", "LC_CTYPE",
        "PROCESSOR_ARCHITECTURE", "NUMBER_OF_PROCESSORS")


def normalize(name):
    """PEP 503 project name normalization."""
    return re.sub(r"[-_.]+", "-", name).lower()


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def artifact_project(filename):
    """Return the normalized project name of a wheel or sdist filename."""
    if filename.endswith(".whl"):
        return normalize(filename.split("-", 1)[0])
    if filename.endswith(".tar.gz"):
        return normalize(filename[:-len(".tar.gz")].rsplit("-", 1)[0])
    raise ValueError(f"not a Python distribution file: {filename}")


def collect(directories):
    """Find every distribution file, refusing two different files of one name."""
    found = {}
    for directory in directories:
        for path in sorted(Path(directory).iterdir()):
            if not path.is_file() or not path.name.endswith((".whl", ".tar.gz")):
                continue
            if path.name in found and sha256(found[path.name]) != sha256(path):
                raise RuntimeError(f"two different artifacts are named {path.name}")
            found[path.name] = path
    if not found:
        raise RuntimeError(f"no wheels or sdists in {', '.join(map(str, directories))}")
    return found


def build_index(artifacts, root):
    """Copy the artifacts into a PEP 503 index and return the staged files.

    The result maps each filename to its staged path and SHA-256.  Links are
    relative, so the index stays valid wherever pip reads it from.
    """
    simple = root / "simple"
    projects = {}
    staged = {}
    for name, source in sorted(artifacts.items()):
        project = artifact_project(name)
        target = simple / project / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        staged[name] = {"path": target, "sha256": sha256(target)}
        projects.setdefault(project, []).append(name)
    page = "<!DOCTYPE html>\n<html><body>\n{}</body></html>\n"
    for project, names in projects.items():
        links = "".join(
            f'<a href="{html.escape(name, quote=True)}#sha256={staged[name]["sha256"]}">'
            f"{html.escape(name)}</a><br>\n" for name in names)
        (simple / project / "index.html").write_text(page.format(links), encoding="utf-8")
    links = "".join(f'<a href="{project}/">{project}</a><br>\n' for project in sorted(projects))
    (simple / "index.html").write_text(page.format(links), encoding="utf-8")
    return simple, staged


def scripts_dir(venv):
    return venv / ("Scripts" if os.name == "nt" else "bin")


def venv_python(venv):
    return scripts_dir(venv) / ("python.exe" if os.name == "nt" else "python")


def runtime_environment(venv=None):
    """Return an environment with only the venv on PATH and no tool overrides.

    Cargo, rustc and inherited pip, Python and loader settings are all absent,
    so neither an accidental source build nor a stray library can make a test
    pass.  Windows keeps the variables its loader and temporary files need.
    """
    environment = {key: value for key, value in os.environ.items() if key.upper() in KEEP}
    environment["PATH"] = str(scripts_dir(venv)) if venv is not None else ""
    environment.update(PIP_CONFIG_FILE=os.devnull, PIP_NO_INPUT="1",
                       PIP_DISABLE_PIP_VERSION_CHECK="1", PYTHONUTF8="1",
                       PYTHONDONTWRITEBYTECODE="1")
    for tool in ("cargo", "rustc", "rustup", "maturin"):
        if shutil.which(tool, path=environment["PATH"]) is not None:
            raise RuntimeError(f"{tool} is reachable from the test environment")
    return environment


def run(command, *, cwd, env, capture=False):
    print("+", " ".join(map(str, command)), flush=True)
    result = subprocess.run(list(map(str, command)), cwd=cwd, env=env, text=True,
                            encoding="utf-8", capture_output=capture)
    if result.returncode != 0:
        output = (result.stdout or "") + (result.stderr or "")
        raise RuntimeError(f"command failed with exit code {result.returncode}: {command[0]} ...\n{output}")
    return result


# PyPy 8.0's Python 3.12 build on Windows has no cffi binding for
# GetLongPathNameW, which its venv module calls through _winapi, so
# `python -m venv` fails there.  On Windows, venv runs with a ctypes
# implementation of that one function, which every interpreter supports.
VENV_SHIM = """
import ctypes, sys, _winapi, venv
from ctypes import wintypes

def long_path_name(path):
    function = ctypes.WinDLL("kernel32", use_last_error=True).GetLongPathNameW
    function.argtypes = (wintypes.LPCWSTR, wintypes.LPWSTR, wintypes.DWORD)
    function.restype = wintypes.DWORD
    size = function(path, None, 0)
    while True:
        if size == 0:
            raise ctypes.WinError(ctypes.get_last_error())
        buffer = ctypes.create_unicode_buffer(size)
        needed = function(path, buffer, size)
        if needed == 0:
            raise ctypes.WinError(ctypes.get_last_error())
        if needed < size:
            return buffer.value
        size = needed

_winapi.GetLongPathName = long_path_name
sys.exit(venv.main(sys.argv[1:]))
"""


def venv_command(python, directory, windows=os.name == "nt"):
    if windows:
        return [python, "-I", "-c", VENV_SHIM, directory]
    return [python, "-I", "-m", "venv", directory]


def create_venv(python, directory, work):
    run(venv_command(python, directory), cwd=work, env=runtime_environment())
    executable = venv_python(directory)
    if not executable.is_file():
        raise RuntimeError(f"{python} created no venv interpreter at {executable}")
    # ensurepip ships pip with the interpreter; never upgrade it from a network.
    version = run([executable, "-I", "-m", "pip", "--version"], cwd=work,
                  env=runtime_environment(directory), capture=True).stdout.split()[1]
    return executable, version


def pip_install(executable, venv, index, requirement, report, work, *, dry_run=False):
    command = [executable, "-I", "-m", "pip", "--isolated", "--disable-pip-version-check",
               "install", "--no-cache-dir", "--only-binary=:all:", "--no-input",
               "--index-url", index.as_uri(), "--report", report]
    if dry_run:
        command.append("--dry-run")
    run([*command, requirement], cwd=work, env=runtime_environment(venv))
    return json.loads(report.read_text(encoding="utf-8"))


def report_path(item):
    """Return the local path of a pip report item's archive."""
    url = item["download_info"]["url"]
    parts = urlsplit(url)
    if parts.scheme != "file":
        raise RuntimeError(f"pip resolved {url}, which is not a staged file")
    if parts.netloc not in ("", "localhost"):
        raise RuntimeError(f"pip resolved {url}, which is not a local file")
    # url2pathname decodes percent escapes and Windows drive letters.
    return Path(url2pathname(parts.path))


def check_report(report, staged, index, expected):
    """Require the install to use exactly the expected staged distributions."""
    installed = {}
    root = Path(os.path.realpath(index))
    for item in report.get("install", []):
        name = normalize(item["metadata"]["name"])
        version = item["metadata"]["version"]
        path = Path(os.path.realpath(report_path(item)))
        if not path.is_relative_to(root):
            raise RuntimeError(f"pip installed {name} from {path}, outside the staged index")
        entry = staged.get(path.name)
        if entry is None or Path(os.path.realpath(entry["path"])) != path:
            raise RuntimeError(f"pip installed {path.name}, which was not staged")
        hashes = item["download_info"].get("archive_info", {}).get("hashes", {})
        if hashes.get("sha256", entry["sha256"]) != entry["sha256"]:
            raise RuntimeError(f"pip reported a different hash for {path.name}")
        if not path.name.endswith(".whl"):
            raise RuntimeError(f"pip selected the source distribution {path.name}")
        installed[name] = {"version": version, "filename": path.name, "sha256": entry["sha256"]}
    if {name: data["version"] for name, data in installed.items()} != expected:
        raise RuntimeError(f"expected to install {expected}, pip installed "
                           + json.dumps({name: data["version"] for name, data in installed.items()}))
    return installed


def extras(staged, version):
    """Read the core wheel's extras and the data distribution each one pins."""
    wheels = [entry["path"] for name, entry in staged.items()
              if name.endswith(".whl") and artifact_project(name) == "gukhanmun"
              and name.split("-")[1] == version]
    if not wheels:
        raise RuntimeError(f"no gukhanmun {version} wheel was staged")
    tables = set()
    for wheel in wheels:
        with zipfile.ZipFile(wheel) as archive:
            metadata = next(name for name in archive.namelist() if name.endswith(".dist-info/METADATA"))
            fields = email.parser.Parser().parsestr(archive.read(metadata).decode("utf-8"))
        table = {}
        for line in fields.get_all("Requires-Dist") or []:
            match = re.fullmatch(r"""([A-Za-z0-9._-]+)\s*==\s*([^\s;]+)\s*;\s*extra\s*==\s*["']([^"']+)["']""",
                                 line.strip())
            if match is None:
                raise RuntimeError(f"{wheel.name} has an unexpected requirement: {line}")
            table[match[3]] = (normalize(match[1]), match[2])
        if sorted(table) != sorted(fields.get_all("Provides-Extra") or []):
            raise RuntimeError(f"{wheel.name} declares extras without requirements")
        tables.add(json.dumps(table, sort_keys=True))
    if len(tables) != 1:
        raise RuntimeError("the staged gukhanmun wheels disagree about their extras")
    return {extra: tuple(pin) for extra, pin in json.loads(tables.pop()).items()}


def audit_flags(arguments, installed):
    """Pass the audit record of the installed core wheel to the checks."""
    if getattr(arguments, "audit_record", None) is None:
        return []
    return ["--audit-record", arguments.audit_record, "--wheel", installed["gukhanmun"]["filename"],
            "--wheel-sha256", installed["gukhanmun"]["sha256"]]


def smoke(executable, venv, work, arguments, *, label):
    """Run the smoke checks from a directory that contains no package source."""
    directory = Path(tempfile.mkdtemp(prefix=f"smoke-{label}-", dir=work))
    script = directory / "smoke.py"
    shutil.copyfile(SMOKE, script)
    result = run([executable, "-I", script, *arguments], cwd=directory,
                 env=runtime_environment(venv), capture=True)
    print(result.stdout, end="", flush=True)
    lines = [line for line in result.stdout.splitlines() if line.startswith("SMOKE-RESULT ")]
    if len(lines) != 1:
        raise RuntimeError("the smoke checks did not report a result")
    return json.loads(lines[0].removeprefix("SMOKE-RESULT "))


def expectations(arguments):
    flags = []
    for name in ("implementation", "python", "machine"):
        value = getattr(arguments, f"expect_{name}")
        if value:
            flags += [f"--expect-{name}", value]
    return flags


def smoke_run(arguments):
    # Commands run from temporary directories; keep the venv symlink itself.
    arguments.python = Path(os.path.abspath(arguments.python))
    if getattr(arguments, "audit_record", None):
        arguments.audit_record = Path(os.path.abspath(arguments.audit_record))
    artifacts = collect(arguments.artifacts)
    work = Path(tempfile.mkdtemp(prefix="gukhanmun-wheel-smoke-"))
    try:
        index, staged = build_index(artifacts, work / "index")
        version = arguments.version
        table = extras(staged, version)
        evidence = {
            "schema": SCHEMA, "label": arguments.label, "version": version,
            "staged": {name: entry["sha256"] for name, entry in sorted(staged.items())},
            "host": {"system": platform.system(), "machine": platform.machine()},
        }
        flags = expectations(arguments)

        base = work / "base"
        executable, evidence["pip"] = create_venv(arguments.python, base, work)
        report = pip_install(executable, base, index, f"gukhanmun=={version}", work / "base.json", work)
        evidence["base"] = {"installed": check_report(report, staged, index, {"gukhanmun": version})}
        evidence["base"]["result"] = smoke(executable, base, work,
                                           ["--mode", "base", *flags,
                                            *audit_flags(arguments, evidence["base"]["installed"])],
                                           label="base")

        resolve = work / "resolve"
        executable, _ = create_venv(arguments.python, resolve, work)
        resolved = {}
        for extra, (data, pin) in sorted(table.items()):
            if pin != version:
                raise RuntimeError(f"extra {extra} pins {data}=={pin}, not {version}")
            report = pip_install(executable, resolve, index, f"gukhanmun[{extra}]=={version}",
                                 work / f"resolve-{extra}.json", work, dry_run=True)
            resolved[extra] = check_report(report, staged, index, {"gukhanmun": version, data: version})
        evidence["extras"] = resolved

        if arguments.official:
            official = work / "official"
            executable, _ = create_venv(arguments.python, official, work)
            requirement = f"gukhanmun[{','.join(sorted(table))}]=={version}"
            report = pip_install(executable, official, index, requirement, work / "official.json", work)
            expected = {"gukhanmun": version, **{data: version for data, _ in table.values()}}
            evidence["official"] = {"installed": check_report(report, staged, index, expected)}
            corpus = work / "official-corpus.json"
            shutil.copyfile(arguments.official, corpus)
            evidence["official"]["result"] = smoke(
                executable, official, work,
                ["--mode", "official", "--corpus", corpus, *flags,
                 *audit_flags(arguments, evidence["official"]["installed"])],
                label="official")
        arguments.evidence.parent.mkdir(parents=True, exist_ok=True)
        arguments.evidence.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"Smoke evidence: {arguments.evidence}")
    finally:
        shutil.rmtree(work, ignore_errors=True)


def local(arguments):
    """Smoke-test this checkout's host wheel and data wheels with this Python."""
    import tomllib

    manifest = tomllib.loads((ROOT / "python/gukhanmun/pyproject.toml").read_text(encoding="utf-8"))
    output = ROOT / "target" / "python"
    data = sorted(path for path in (output / "dictionaries").glob("gukhanmun-*") if path.is_dir())
    if not list((output / "wheels").glob("*.whl")) or not data:
        raise RuntimeError("run py-build and py-dicts-build first")
    version = sys.version_info
    machine = MACHINES.get(platform.machine().lower())
    arguments.artifacts = [output / "wheels", *data]
    arguments.python = Path(sys.executable)
    arguments.version = manifest["project"]["version"]
    arguments.label = f"host-{sys.implementation.name}{version.major}{version.minor}"
    arguments.evidence = output / "smoke" / f"{arguments.label}.json"
    arguments.official = ROOT / "tests" / "fixtures" / "dictionaries" / "official.json"
    arguments.expect_implementation = sys.implementation.name
    arguments.expect_python = f"{version.major}.{version.minor}"
    arguments.expect_machine = machine
    arguments.audit_record = None
    smoke_run(arguments)


MACHINES = {"x86_64": "x86_64", "amd64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64"}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    index = commands.add_parser("index", help="write a PEP 503 index of the artifacts")
    index.add_argument("artifacts", nargs="+", type=Path)
    index.add_argument("--output", type=Path, required=True)
    smoke_parser = commands.add_parser("run", help="install the artifacts and run the smoke checks")
    smoke_parser.add_argument("--artifacts", nargs="+", type=Path, required=True,
                              help="directories holding the wheels and sdists to stage")
    smoke_parser.add_argument("--python", type=Path, required=True, help="interpreter under test")
    smoke_parser.add_argument("--version", required=True, help="the artifact version to install")
    smoke_parser.add_argument("--label", required=True, help="identifies the target and interpreter")
    smoke_parser.add_argument("--evidence", type=Path, required=True, help="where to write the record")
    smoke_parser.add_argument("--official", type=Path, help="official dictionary corpus JSON")
    smoke_parser.add_argument("--expect-implementation", choices=("cpython", "pypy"))
    smoke_parser.add_argument("--expect-python", help="major.minor of the interpreter under test")
    smoke_parser.add_argument("--expect-machine", choices=("x86_64", "aarch64"))
    smoke_parser.add_argument("--audit-record", type=Path,
                              help="audit record of the wheels; on Windows it drives the import check")
    commands.add_parser("local", help="test this checkout's host and data wheels with this interpreter")
    arguments = parser.parse_args()
    if arguments.command == "local":
        local(arguments)
    elif arguments.command == "index":
        _, staged = build_index(collect(arguments.artifacts), arguments.output)
        print(json.dumps({name: entry["sha256"] for name, entry in staged.items()}, indent=2))
    else:
        smoke_run(arguments)


if __name__ == "__main__":
    sys.exit(main())
