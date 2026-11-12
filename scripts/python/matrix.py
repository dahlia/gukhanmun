# Gukhanmun: Selects pinned interpreters and verifies installed Python wheels.
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

"""Run the semantic matrix with mise-owned interpreters and locked tox tools."""

import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import sysconfig
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]
TOOLING_TESTS = ("test_checks.py", "test_version.py", "test_dictionary_packages.py", "test_wheel_smoke.py",
                 "test_dists.py", "test_artifacts.py")


def pins(root=ROOT):
    """Read the optional tool profile without activating all its interpreters."""
    versions = tomllib.loads((root / "mise.python.toml").read_text(encoding="utf-8"))["tools"]["python"]
    default = tomllib.loads((root / "mise.toml").read_text(encoding="utf-8"))["tools"]["python"]
    if not versions or versions[0] != default:
        raise RuntimeError("the matrix's first Python pin must match the development interpreter")
    result = {}
    for version in versions:
        match = re.fullmatch(r"3\.(11|12|13|14|15)\.\d+", version)
        if match:
            name = "py3" + match[1]
        else:
            match = re.fullmatch(r"pypy3\.(11|12)-\d+\.\d+\.\d+", version)
            if not match:
                raise RuntimeError(f"unsupported Python pin: {version}")
            name = "pypy3" + match[1]
        if name in result:
            raise RuntimeError(f"duplicate interpreter: {name}")
        result[name] = version
    expected = {"py311", "py312", "py313", "py314", "py315", "pypy311", "pypy312"}
    if result.keys() != expected:
        raise RuntimeError(f"matrix must contain {sorted(expected)}")
    return result


def runtime():
    return {
        "implementation": sys.implementation.name,
        "version": list(sys.version_info[:3]),
        "implementation_version": list(sys.implementation.version[:3]),
        "executable": sys.executable,
        "base_prefix": str(Path(sys.base_prefix).resolve()),
        "platform": sys.platform,
        "architecture": platform.machine(),
        "abi": sysconfig.get_config_var("SOABI"),
        "free_threaded": bool(sysconfig.get_config_var("Py_GIL_DISABLED")),
    }


def verify_runtime(actual, pin, prefix):
    pypy = pin.startswith("pypy")
    language, implementation = (pin.removeprefix("pypy").split("-") if pypy else (pin, None))
    expected_version = [int(part) for part in language.split(".")]
    if (actual["implementation"] != ("pypy" if pypy else "cpython")
            or actual["version"][:len(expected_version)] != expected_version
            or Path(actual["base_prefix"]).resolve() != Path(prefix).resolve()
            or actual["free_threaded"]
            or (pypy and actual["implementation_version"] != [int(p) for p in implementation.split(".")])):
        raise RuntimeError(f"expected mise Python {pin} at {prefix}, got {actual}")


def interpreter(pin):
    found = subprocess.run(["mise", "where", "python@" + pin], cwd=ROOT,
                           capture_output=True, text=True, encoding="utf-8")
    if found.returncode:
        raise RuntimeError(f"mise Python {pin} is unavailable; run `mise --env python install`\n{found.stderr}")
    prefix = Path(found.stdout.strip()).resolve()
    candidates = ([prefix / "python.exe", prefix / "pypy3.exe", prefix / "pypy.exe"]
                  if os.name == "nt" else [prefix / "bin/python"])
    executable = next((path for path in candidates if path.is_file()), None)
    if executable is None:
        raise RuntimeError(f"mise Python {pin} has no executable at {prefix}; run `mise --env python install`")
    probe = subprocess.run([executable, "-I", Path(__file__).resolve(), "--probe"],
                           cwd=ROOT, capture_output=True, text=True, encoding="utf-8", check=True)
    actual = json.loads(probe.stdout)
    verify_runtime(actual, pin, prefix)
    print("Selected mise interpreter: " + json.dumps(actual), flush=True)
    return executable, prefix


def selected_names(selection, available):
    names = list(available) if selection is None else selection.split(",")
    if not names or any(name not in available for name in names):
        raise RuntimeError(f"choose environments from {', '.join(available)}")
    return list(dict.fromkeys(names))


def matrix(selection=None):
    available = pins()
    # One invocation per env keeps its exact base_python unambiguous and avoids
    # concurrent Cargo builds for different Python implementations.
    for name in selected_names(selection, available):
        pin = available[name]
        executable, prefix = interpreter(pin)
        environment = {key: value for key, value in os.environ.items()
                       if not key.startswith("TOX_") and key not in
                       ("TOXENV", "PYTHONPATH", "PYTHONHOME", "MYPYPATH", "VIRTUAL_ENV")}
        environment.update(
            GUKHANMUN_PYTHON=str(executable),
            GUKHANMUN_EXPECTED_PREFIX=str(prefix),
            GUKHANMUN_EXPECTED_PIN=pin,
            GUKHANMUN_REPO_ROOT=str(ROOT),
            GUKHANMUN_FIXTURE_DIR=str(ROOT / "target/python/fixtures"),
            UV_PYTHON_DOWNLOADS="never", UV_PYTHON_PREFERENCE="only-system", UV_FROZEN="false",
        )
        subprocess.run([sys.executable, "-m", "tox", "run", "-c", ROOT / "tox.ini", "-e", name],
                       cwd=ROOT, env=environment, check=True)


def semantic():
    # These must come from the launcher even when a tox env already exists.
    prefix = os.environ.get("GUKHANMUN_EXPECTED_PREFIX")
    pin = os.environ.get("GUKHANMUN_EXPECTED_PIN")
    if not prefix or not pin or not os.environ.get("GUKHANMUN_PYTHON"):
        raise RuntimeError("run semantic tests through `mise run py-test-all`")
    if pin not in pins().values():
        raise RuntimeError(f"unsupported expected Python pin: {pin}")
    actual = runtime()
    verify_runtime(actual, pin, prefix)
    import gukhanmun
    from gukhanmun import _native
    from packaging.tags import parse_tag, sys_tags

    distribution = importlib.metadata.distribution("gukhanmun")
    expected_version = tomllib.loads((ROOT / "python/gukhanmun/pyproject.toml").read_text(encoding="utf-8"))["project"]["version"]
    if distribution.version != expected_version:
        raise RuntimeError(f"expected distribution {expected_version}, got {distribution.version}")
    installed = Path(sys.prefix).resolve()
    source = (ROOT / "python/gukhanmun/src").resolve()
    for module in (gukhanmun, _native):
        path = Path(module.__file__).resolve()
        if not path.is_relative_to(installed) or path.is_relative_to(source):
            raise RuntimeError(f"expected an installed wheel module, got {path}")
    direct = json.loads(distribution.read_text("direct_url.json") or "{}")
    if direct.get("dir_info", {}).get("editable"):
        raise RuntimeError("semantic tests require a noneditable wheel")
    wheel = distribution.read_text("WHEEL") or ""
    tags = {tag for line in wheel.splitlines() if line.startswith("Tag: ")
            for tag in parse_tag(line.removeprefix("Tag: "))}
    if not tags.intersection(sys_tags()):
        raise RuntimeError(f"wheel tags do not support this interpreter: {tags}")
    package = Path(gukhanmun.__file__).parent
    if not all((package / filename).is_file() for filename in ("py.typed", "_native.pyi")):
        raise RuntimeError("installed wheel is missing its typing contract")
    native = Path(_native.__file__)
    actual.update(distribution_version=distribution.version, package=str(package), native=str(native),
                  native_sha256=hashlib.sha256(native.read_bytes()).hexdigest(),
                  wheel_tags=sorted(map(str, tags)))
    print("Installed-wheel diagnostics: " + json.dumps(actual), flush=True)
    environment = {key: value for key, value in os.environ.items()
                   if key not in ("PYTHONPATH", "MYPYPATH", "PYTHONHOME")}
    with tempfile.TemporaryDirectory(prefix="gukhanmun-semantic-") as temporary:
        directory = Path(temporary)
        tests = directory / "tests"
        shutil.copytree(ROOT / "tests/python", tests)
        for name in TOOLING_TESTS:
            (tests / name).unlink(missing_ok=True)
        subprocess.run([sys.executable, "-I", "-m", "pytest", "-q", tests],
                       cwd=directory, env=environment, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    choice = parser.add_mutually_exclusive_group()
    choice.add_argument("-e", "--environment", help="comma-separated tox environments (default: all)")
    choice.add_argument("--pin", choices=list(pins()), help="print the mise pin for CI tool installation")
    choice.add_argument("--probe", action="store_true", help=argparse.SUPPRESS)
    choice.add_argument("--semantic", action="store_true", help=argparse.SUPPRESS)
    arguments = parser.parse_args()
    if arguments.pin:
        print(pins()[arguments.pin])
    elif arguments.probe:
        print(json.dumps(runtime()))
    elif arguments.semantic:
        semantic()
    else:
        matrix(arguments.environment)


if __name__ == "__main__":
    main()
