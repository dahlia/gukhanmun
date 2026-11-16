# Gukhanmun: Runs reproducible CLI installer and native-host CI coverage.
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

"""Run through mise, with pinned interpreters installed before host coverage."""

import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts/python"


def run(*args, **kwargs):
    return subprocess.run([str(arg) for arg in args], cwd=ROOT, check=True, **kwargs)


def load(name):
    spec = importlib.util.spec_from_file_location("_cli_ci_" + name, SCRIPTS / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def target(name):
    result = run(sys.executable, "-I", SCRIPTS / "cli_artifacts.py", "matrix",
                 capture_output=True, text=True)
    return next(item for item in json.loads(result.stdout)["include"] if item["id"] == name)


def names(item):
    return ["py311", "py315"] + (["pypy311", "pypy312"] if item["pypy"] else [])


def executables(directory):
    scripts = directory / ("Scripts" if os.name == "nt" else "bin")
    return scripts / ("python.exe" if os.name == "nt" else "python"), scripts / (
        "pipx.exe" if os.name == "nt" else "pipx")


def prepare(directory):
    """Download only hashed locked wheels, evaluating markers on Python 3.11."""
    directory.mkdir(parents=True, exist_ok=True)
    requirements = directory / "installer-requirements.txt"
    run("uv", "export", "--locked", "--only-group", "installer", "--no-emit-workspace",
        "--output-file", requirements)
    minimum, _ = load("matrix").interpreter(load("matrix").pins()["py311"])
    tools = directory / "installer"
    run("uv", "venv", "--python", minimum, tools)
    python, pipx = executables(tools)
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith(("PIP_", "UV_")) and key not in ("PYTHONPATH", "PYTHONHOME", "VIRTUAL_ENV")}
    environment.update(UV_PYTHON_DOWNLOADS="never", PIP_CONFIG_FILE=os.devnull)
    run("uv", "pip", "sync", "--python", python, "--require-hashes", "--only-binary", ":all:",
        requirements, env=environment)
    wheelhouse = directory / "wheelhouse"
    wheelhouse.mkdir(exist_ok=True)
    run(python, "-I", "-m", "pip", "--isolated", "download", "--require-hashes",
        "--only-binary=:all:", "--no-deps", "--dest", wheelhouse, "-r", requirements, env=environment)
    if not pipx.is_file():
        raise RuntimeError("locked installer group did not provide pipx")
    return python, pipx, wheelhouse


def smoke(item, directory, version, python, environment="host", image=None, pipx=None, uv=None, shells=()):
    command = [sys.executable, "-I", SCRIPTS / "cli_smoke.py", "--target", item["id"],
               "--dir", directory, "--version", version, "--python", python,
               "--environment", environment, "--wheelhouse", directory / "wheelhouse"]
    if image:
        command += ["--image", image]
    if pipx:
        command += ["--pipx", pipx]
    if uv:
        command += ["--uv", uv]
    for shell in shells:
        command += ["--shell", shell]
    run(*command)


def host(item, directory, version, shells=()):
    """Run each pinned host interpreter, never substituting a system Python."""
    python, pipx = executables(directory / "installer")
    if not python.is_file() or not pipx.is_file():
        raise RuntimeError("run cli_ci.py prepare first")
    uv = shutil.which("uv")
    if not uv:
        raise RuntimeError("mise-pinned uv is required")
    run(sys.executable, "-I", SCRIPTS / "cli_artifacts.py", "audit", "--target", item["id"],
        "--dir", directory, "--version", version)
    matrix = load("matrix")
    pins = matrix.pins()
    for name in names(item):
        interpreter, _ = matrix.interpreter(pins[name])
        smoke(item, directory, version, interpreter, pipx=pipx, uv=Path(uv).resolve(),
              shells=shells if name == "py315" else ())


def container_run(item, directory, version, environment, image):
    """Inside the pinned native-CPU image, with no build tools used by smoke."""
    tools = directory / ("installer-" + environment)
    python = Path("/opt/python/cp315-cp315/bin/python")
    run(python, "-I", "-m", "venv", tools)
    tool_python, pipx = executables(tools)
    run(tool_python, "-I", "-m", "pip", "--isolated", "install", "--no-index",
        "--find-links", directory / "wheelhouse", "--require-hashes",
        "-r", directory / "installer-requirements.txt")
    # Both floor interpreters install the same wheel, not a rebuilt payload.
    for executable in (Path("/opt/python/cp311-cp311/bin/python"), python):
        smoke(item, directory, version, executable, environment=environment, image=image, pipx=pipx)


def containers(item, directory, version):
    """Run both libc floors on matching actual CPU, reusing existing image pins."""
    if item["family"] != "linux":
        raise ValueError("libc images apply only to Linux")
    artifacts = load("artifacts")
    for environment in ("manylinux_2_28", "musllinux_1_2"):
        image = artifacts.IMAGES[f'{environment}_{item["machine"]}']
        run(*artifacts.container_command(image, directory), image,
            "/opt/python/cp315-cp315/bin/python", "-I", "/gukhanmun/source/scripts/python/cli_ci.py",
            "container-run", "--target", item["id"], "--dir", "/gukhanmun/work",
            "--version", version, "--environment", environment, "--image", image)


def build_native(rust_target):
    """Use the same static-CRT policy for task-driven and local wheel builds."""
    environment = os.environ.copy()
    if rust_target.endswith("-windows-msvc"):
        environment["RUSTFLAGS"] = (environment.get("RUSTFLAGS", "") + " -C target-feature=+crt-static").strip()
    run("cargo", "build", "--locked", "--release", "--target", rust_target,
        "-p", "gukhanmun-cli", "-p", "gukhanmun-mkdict", env=environment)


def local(item, directory, version, shells=()):
    """Build without stamping, preserving accurate local completion dirty state."""
    native = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
    commit = run("git", "rev-parse", "HEAD", capture_output=True, text=True).stdout.strip()
    build_native(item["rust"])
    binary = ROOT / "target" / item["rust"] / "release" / (
        "gukhanmun.exe" if item["family"] == "windows" else "gukhanmun")
    mkdict_binary = binary.with_name("gukhanmun-mkdict" + (".exe" if item["family"] == "windows" else ""))
    run("mise", "run", "completions", "--", "--binary", binary, "--revision", commit, "--version", native)
    directory.mkdir(parents=True, exist_ok=True)
    compiler = directory.parent / ("rustc-" + item["id"] + ".txt")
    compiler.write_text(run("rustc", "-vV", capture_output=True, text=True).stdout, encoding="utf-8")
    run(sys.executable, "-I", SCRIPTS / "cli_wheel.py", "--target", item["id"], "--binary", binary,
        "--mkdict-binary", mkdict_binary,
        "--completions", ROOT / "target/completions", "--native-version", native,
        "--version", version, "--commit", commit, "--compiler-file", compiler, "--output", directory)
    prepare(directory)
    host(item, directory, version, shells)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("native", "pins", "prepare", "host", "containers", "container-run", "local"))
    parser.add_argument("--target", required=True)
    parser.add_argument("--dir", type=Path, default=ROOT / "target/python/cli-local")
    parser.add_argument("--version")
    parser.add_argument("--shell", action="append", default=[])
    parser.add_argument("--environment", choices=("manylinux_2_28", "musllinux_1_2"))
    parser.add_argument("--image")
    args = parser.parse_args()
    if args.command == "native":
        build_native(args.target)
        return 0
    item = target(args.target)
    directory = args.dir.resolve()
    if args.command == "pins":
        pins = load("matrix").pins()
        print(" ".join("python@" + pins[name] for name in names(item)))
        return 0
    if args.command == "prepare":
        prepare(directory)
        return 0
    version = args.version or tomllib.loads((ROOT / "python/gukhanmun-cli/pyproject.toml").read_text(
        encoding="utf-8"))["project"]["version"]
    if args.command == "container-run":
        expected = load("artifacts").IMAGES.get(f'{args.environment}_{item["machine"]}')
        if not args.environment or args.image != expected:
            parser.error("container-run requires the exact pinned image")
        container_run(item, directory, version, args.environment, args.image)
    elif args.command == "local":
        local(item, directory, version, args.shell)
    elif args.command == "host":
        host(item, directory, version, args.shell)
    else:
        containers(item, directory, version)
    return 0


if __name__ == "__main__":
    sys.exit(main())
