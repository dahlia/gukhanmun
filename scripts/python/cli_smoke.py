# Gukhanmun: Executes installed standalone wheels and isolated tool lifecycles.
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

"""Native-only smoke tests. Missing interpreters, tools or consoles fail."""

import argparse
import ctypes
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import tomllib
import zipfile

_spec = importlib.util.spec_from_file_location("_cli_smoke_wheel", Path(__file__).with_name("cli_wheel.py"))
wheel = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = wheel
_spec.loader.exec_module(wheel)
artifacts = wheel.load("cli_artifacts")

ROOT = wheel.ROOT
KEEP = {"SYSTEMROOT", "SYSTEMDRIVE", "WINDIR", "COMSPEC", "PATHEXT",
        "TEMP", "TMP", "TMPDIR", "PROCESSOR_ARCHITECTURE", "NUMBER_OF_PROCESSORS"}


def run(command, *, cwd, env, input=None, check=True, timeout=120):
    result = subprocess.run(list(map(str, command)), cwd=cwd, env=env, input=input,
                            capture_output=True, timeout=timeout)
    if check:
        wheel.require(result.returncode == 0,
                      f"command failed ({result.returncode}): {command!r}\n"
                      + result.stdout.decode(errors="replace") + result.stderr.decode(errors="replace"))
    return result


def environment(home, paths=()):
    """No inherited installer/config/python/loader settings or Rust tools."""
    env = {k: v for k, v in os.environ.items() if k.upper() in KEEP}
    env.update(
        HOME=str(home), USERPROFILE=str(home), APPDATA=str(home / "appdata"),
        LOCALAPPDATA=str(home / "local"), XDG_CONFIG_HOME=str(home / "config"),
        XDG_CACHE_HOME=str(home / "cache"), PATH=os.pathsep.join(map(str, paths)),
        PYTHONUTF8="1", PYTHONDONTWRITEBYTECODE="1", PIP_CONFIG_FILE=os.devnull,
        PIP_NO_INDEX="1", PIP_DISABLE_PIP_VERSION_CHECK="1", PIP_NO_INPUT="1",
        PIP_ONLY_BINARY=":all:", PIP_NO_CACHE_DIR="1",
        UV_NO_CONFIG="1", UV_NO_INDEX="1", UV_OFFLINE="1", UV_PYTHON_DOWNLOADS="never",
        UV_CACHE_DIR=str(home / "uv-cache"), UV_TOOL_DIR=str(home / "uv-tools"),
        UV_TOOL_BIN_DIR=str(home / "uv-bin"), UV_PYTHON_INSTALL_DIR=str(home / "uv-python"),
        PIPX_HOME=str(home / "pipx"), PIPX_BIN_DIR=str(home / "pipx-bin"),
        PIPX_MAN_DIR=str(home / "pipx-man"), PIPX_COMPLETION_DIR=str(home / "pipx-completions"),
        PIPX_DEFAULT_BACKEND="pip", PIPX_FETCH_PYTHON="never",
        PIPX_DISABLE_SHARED_LIBS_AUTO_UPGRADE="1",
        NO_COLOR="1", TERM="xterm", LC_ALL="C.UTF-8",
    )
    for tool in ("cargo", "rustc", "rustup", "maturin"):
        wheel.require(shutil.which(tool, path=env["PATH"]) is None, f"{tool} is reachable")
    return env


def wheelhouse_check(directory, *, root=ROOT):
    """Every installer wheel must match the committed uv.lock, not a new solve."""
    lock = tomllib.loads((root / "uv.lock").read_text(encoding="utf-8"))
    hashes = {entry["hash"].removeprefix("sha256:")
              for package in lock["package"] for entry in package.get("wheels", [])}
    found = {}
    for path in directory.iterdir():
        wheel.require(path.is_file() and not path.is_symlink() and path.suffix == ".whl",
                      f"wheelhouse contains a non-wheel: {path.name}")
        wheel.require(wheel.digest(path) in hashes, f"wheelhouse wheel not pinned in uv.lock: {path.name}")
        found[path.name.split("-", 1)[0].replace("_", "-")] = path
    wheel.require("pip" in found and found["pip"].name.startswith("pip-26.0-"),
                  "wheelhouse requires locked pip 26.0")
    return found


def libc_identity(environment_name):
    if not sys.platform.startswith("linux"):
        return None
    libc = ctypes.CDLL(None)
    if hasattr(libc, "gnu_get_libc_version"):
        libc.gnu_get_libc_version.restype = ctypes.c_char_p
        identity = {"family": "glibc", "version": libc.gnu_get_libc_version().decode("ascii")}
    else:
        loaders = list(Path("/lib").glob("ld-musl-*.so.1"))
        wheel.require(len(loaders) == 1, "cannot identify running Linux libc")
        result = subprocess.run([str(loaders[0])], capture_output=True, timeout=10)
        import re
        match = re.search(rb"Version (\d+\.\d+\.\d+)", result.stderr + result.stdout)
        wheel.require(match is not None, "musl loader did not identify its version")
        identity = {"family": "musl", "version": match[1].decode("ascii")}
    if environment_name != "host":
        family, prefix = {"manylinux_2_28": ("glibc", "2.28"),
                          "musllinux_1_2": ("musl", "1.2")}[environment_name]
        wheel.require(identity["family"] == family and
                      (identity["version"] == prefix or identity["version"].startswith(prefix + ".")),
                      "libc floor image does not run the claimed libc")
    return identity


def resolver_checks(python, work, env):
    """Resolver-only synthetic wheels: never install or execute fake versions."""
    indexes = {}
    for name, versions in (("mixed", ("0.4.0", "0.5.0.dev1")), ("dev-only", ("0.5.0.dev1",))):
        directory = work / f"resolver-{name}"
        directory.mkdir()
        indexes[name] = directory
        for version in versions:
            info = f"gukhanmun_cli-{version}.dist-info"
            with zipfile.ZipFile(directory / f"gukhanmun_cli-{version}-py3-none-any.whl", "w") as archive:
                archive.writestr(f"{info}/METADATA", f"Metadata-Version: 2.4\nName: gukhanmun-cli\nVersion: {version}\nRequires-Python: >=3.11\n")
                archive.writestr(f"{info}/WHEEL", "Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n")
                archive.writestr(f"{info}/RECORD", "")
    for index, requirement, extra, expected in (
        ("mixed", "gukhanmun-cli", [], "0.4.0"),
        ("dev-only", "gukhanmun-cli", [], "0.5.0.dev1"),
        ("mixed", "gukhanmun-cli==0.5.0.dev1", [], "0.5.0.dev1"),
        ("mixed", "gukhanmun-cli", ["--pre"], "0.5.0.dev1"),
    ):
        report = work / "resolver-report.json"
        run([python, "-I", "-m", "pip", "--isolated", "install", "--dry-run",
             "--ignore-installed", "--no-index", "--find-links", indexes[index],
             "--only-binary=:all:", "--report", report, *extra, requirement], cwd=work, env=env)
        entries = wheel.read_json(report)["install"]
        wheel.require(len(entries) == 1 and entries[0]["metadata"]["version"] == expected,
                      "local prerelease resolution policy differs")


def uv_resolver_checks(uv, python, work, env):
    for index, requirement, policy, expected in (
        ("mixed", "gukhanmun-cli", "if-necessary", "0.4.0"),
        ("dev-only", "gukhanmun-cli", "if-necessary", "0.5.0.dev1"),
        ("mixed", "gukhanmun-cli==0.5.0.dev1", "if-necessary", "0.5.0.dev1"),
        ("mixed", "gukhanmun-cli", "allow", "0.5.0.dev1"),
    ):
        requirement_file = work / "resolver-input.txt"
        requirement_file.write_text(requirement + "\n", encoding="utf-8")
        result = run([uv, "--no-config", "pip", "compile", "--offline", "--no-index",
                      "--find-links", work / f"resolver-{index}", "--python", python,
                      "--prerelease", policy, "--no-header", "--no-annotate", requirement_file],
                     cwd=work, env=env)
        lines = [line.strip() for line in result.stdout.decode().splitlines() if line.strip() and not line.startswith("#")]
        wheel.require(lines == [f"gukhanmun-cli=={expected}"], "uv prerelease resolver policy differs")


def scripts(venv):
    return venv / ("Scripts" if os.name == "nt" else "bin")


def python_in(venv):
    return scripts(venv) / ("python.exe" if os.name == "nt" else "python")


def command_in(directory, name="gukhanmun"):
    return directory / (name + (".exe" if os.name == "nt" else ""))


def install(python, venv, wheel_path, wheelhouse, work, env):
    smoke = wheel.load("wheel_smoke")
    run(smoke.venv_command(python, venv), cwd=work, env=env)
    interpreter = python_in(venv)
    local_env = env | {"PATH": str(scripts(venv))}
    common = [interpreter, "-I", "-m", "pip", "--isolated", "install",
              "--no-index", "--find-links", wheelhouse, "--only-binary=:all:", "--no-cache-dir"]
    run([*common, "pip==26.0"], cwd=work, env=local_env)
    report = work / "pip-report.json"
    run([*common, "--no-deps", "--report", report, wheel_path], cwd=work, env=local_env)
    installed = wheel.read_json(report)["install"]
    wheel.require(len(installed) == 1 and installed[0]["metadata"]["name"] == "gukhanmun-cli"
                  and installed[0]["download_info"]["archive_info"]["hashes"]["sha256"] == wheel.digest(wheel_path),
                  "pip report does not identify the original CLI wheel")
    return interpreter, local_env


def installed_info(python, work, env):
    code = """
import importlib.metadata as m, importlib.resources as r, json, platform, sys
names = {d.metadata['Name'].lower().replace('_','-') for d in m.distributions()}
assert not any(n == 'gukhanmun' or n.startswith('gukhanmun-stdict') or n.startswith('gukhanmun-opendict') or n.startswith('gukhanmun-loanwords') for n in names), names
root = r.files('gukhanmun_cli')
payload = json.loads(root.joinpath('payload.json').read_bytes())
assert m.version('gukhanmun-cli') == payload['version']
print(json.dumps({'root': str(root), 'payload': payload,
 'interpreter': {'implementation': platform.python_implementation().lower(),
                 'version': list(sys.version_info[:3]), 'executable': sys.executable}}))
"""
    return json.loads(run([python, "-I", "-c", code], cwd=work, env=env).stdout)


def signals(command, native, work, env):
    """Compare direct and installed dispositions/statuses on the same host."""
    if os.name == "nt":
        return {"direct": windows_console(native, work, env),
                "pip": windows_console(command, work, env)}
    results = {}
    for number in (signal.SIGINT, signal.SIGTERM):
        statuses = []
        for executable in (native, command):
            with subprocess.Popen([str(executable)], cwd=work, env=env, stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
                try:
                    time.sleep(1)
                    wheel.require(child.poll() is None, "signal fixture exited before delivery")
                    child.send_signal(number)
                    child.communicate(timeout=20)
                    statuses.append(child.returncode)
                except BaseException:
                    child.kill()
                    child.communicate()
                    raise
        wheel.require(statuses[0] == statuses[1] and statuses[0] != 0, f"signal parity differs: {number}: {statuses}")
        results[str(number)] = statuses
    statuses = []
    for executable in (native, command):
        read_fd, write_fd = os.pipe()
        os.close(read_fd)
        try:
            result = subprocess.run([str(executable)], input=b"hello\n" * 10000,
                                    stdout=write_fd, stderr=subprocess.PIPE, cwd=work, env=env, timeout=60)
            statuses.append(result.returncode)
        finally:
            os.close(write_fd)
    wheel.require(statuses[0] == statuses[1], f"broken-pipe parity differs: {statuses}")
    results["broken_pipe"] = statuses
    return results


def windows_console(executable, work, env):
    helper = ROOT / "tests/python-cli/windows_console.py"
    result = run([sys.executable, "-I", helper, str(executable)], cwd=work, env=env, timeout=90)
    record = json.loads(result.stdout)
    wheel.require(record == {"ctrl_c": 0xC000013A, "ctrl_break": 0xC000013A},
                  f"Windows console status differs: {record}")
    return record


def mkdict_runtime(command, info, work, env):
    """Build both dictionary backends and consume them with the installed CLI."""
    builder = command_in(command.parent, "gukhanmun-mkdict")
    native = command_in(Path(info["root"]) / "_bin", "gukhanmun-mkdict")
    wheel.require(builder.is_file(), "installer did not expose gukhanmun-mkdict")
    wheel.require(wheel.digest(native) == info["payload"]["mkdict_sha256"], "installed mkdict hash differs")
    wheel.require(run([builder, "--version"], cwd=work, env=env).stdout.strip()
                  == f"gukhanmun-mkdict {info['payload']['native_version']}".encode(), "mkdict version differs")
    wheel.require(b"--format" in run([builder, "--help"], cwd=work, env=env).stdout, "mkdict help absent")
    source = work / "사용자 space dictionary.tsv"
    source.write_text("hanja\thangul\n學校\t배움터\n", encoding="utf-8")
    for backend in ("fst", "cdb"):
        output = work / f"사용자 space dictionary.guk{backend}"
        direct = work / f"direct.guk{backend}"
        for executable, destination in ((native, direct), (builder, output)):
            run([executable, "--format", backend, "--validate", "--output", destination, source],
                cwd=work, env=env)
        wheel.require(output.read_bytes() == direct.read_bytes(), "mkdict native/launcher output differs")
        converted = run([command, "--no-bundled-dictionaries", "--dictionary", output,
                         "--rendering", "hangul-only", "--disambiguation", "off"],
                        cwd=work, env=env, input="學校\n".encode()).stdout
        wheel.require(converted.decode().strip() == "배움터", f"installed mkdict {backend} round trip failed")
    malformed = work / "invalid.tsv"
    malformed.write_text("not a dictionary\n", encoding="utf-8")
    for arguments in (["--not-a-real-option"], ["--output", work / "invalid.gukfst", malformed],
                      ["--output", work / "missing.gukfst", work / "absent.tsv"]):
        direct = run([native, *arguments], cwd=work, env=env, check=False)
        wrapped = run([builder, *arguments], cwd=work, env=env, check=False)
        wheel.require(direct.returncode == wrapped.returncode and wrapped.returncode != 0,
                      "mkdict failure status differs")


def runtime(command, native, info, work, env):
    payload = info["payload"]
    wheel.require(run([command, "--version"], cwd=work, env=env).stdout.strip()
                  == f"gukhanmun {payload['native_version']}".encode(), "installed native version differs")
    wheel.require(b"--preset" in run([command, "--help"], cwd=work, env=env).stdout, "help absent")
    # This inflected compound needs the bundled dictionary's word-level
    # reading, rather than a character-by-character fallback.
    text = "汽車길\n".encode()
    options = ["--rendering", "hangul-only", "--disambiguation", "off"]
    direct = run([native, *options], cwd=work, env=env, input=text).stdout
    converted = run([command, *options], cwd=work, env=env, input=text).stdout
    wheel.require(converted == direct and converted.decode().strip() == "기찻길", "bundled dictionary conversion failed")
    source, destination = work / "한글 space input.txt", work / "한글 space output.txt"
    source.write_bytes(text)
    run([command, source, "--output", destination, *options], cwd=work, env=env)
    wheel.require(destination.read_bytes() == converted, "file/pipe or spaced/non-ASCII path parity differs")
    for arguments in (["--not-a-real-option"], [str(work / "missing input.txt")]):
        a = run([native, *arguments], cwd=work, env=env, check=False)
        b = run([command, *arguments], cwd=work, env=env, check=False)
        wheel.require(a.returncode == b.returncode and b.returncode != 0, "failure status differs")
    root = Path(info["root"])
    wheel.require(wheel.digest(native) == payload["executable_sha256"], "installed native hash differs")
    for shell, name in wheel.SHELL_FILES.items():
        emitted = run([command, "--generate-completion", shell], cwd=work, env=env).stdout.replace(b"\r\n", b"\n").replace(b"\r", b"\n")
        if not emitted.endswith(b"\n"):
            emitted += b"\n"
        wheel.require(emitted == (root / "completions" / name).read_bytes(), f"installed completion differs: {shell}")
    for name, checksum in payload["resources"].items():
        relative = name.removeprefix("gukhanmun_cli/")
        wheel.require(wheel.digest(root / relative) == checksum, f"installed resource differs: {name}")
    mkdict_runtime(command, info, work, env)
    console = signals(command, native, work, env)
    return console


def shell_probes(selected, info, command, work, env, item, base, output_dir):
    if not selected:
        return
    generate = wheel.load("completion_generate", ROOT / "scripts/completions/generate.py")
    previous = sys.modules.get("generate")
    sys.modules["generate"] = generate
    try:
        probes = wheel.load("completion_shells", ROOT / "scripts/completions/shells.py")
    finally:
        if previous is None:
            sys.modules.pop("generate", None)
        else:
            sys.modules["generate"] = previous
    for shell in selected:
        binary = probes.executable(shell)
        with tempfile.TemporaryDirectory(prefix=f"cli-installed-{shell}-") as temporary:
            directory = Path(temporary)
            for name in probes.FIXTURES:
                (directory / name).touch()
            # Shell helper utilities remain available, but no Rust directory
            # may enter PATH. The installed command comes first.
            safe_dirs = [p for p in os.get_exec_path()
                         if not any(shutil.which(tool, path=p) for tool in ("cargo", "rustc", "rustup", "maturin"))]
            shell_env = environment(directory, [command.parent, Path(binary).parent, *safe_dirs])
            source = Path(info["root"]) / "completions" / wheel.SHELL_FILES[shell]
            getattr(probes, shell)(binary, source, directory, shell_env)
            version = run([binary, "--version"], cwd=directory, env=shell_env).stdout.decode(errors="replace").strip()
        record = base | {"host": artifacts.native_host(item), "shell": shell, "shell_version": version,
                         "checks": ["activation", "option", "value"]}
        (output_dir / f"cli-shell-{item.id}-{shell}.json").write_bytes(wheel.json_bytes(record))


def tool_lifecycles(args, interpreter, work, wheel_path, native, expected, item):
    tools, console = [], {}
    if expected["implementation"] != "cpython" or expected["version"][:2] != [3, 15]:
        return tools, console
    names = ["pipx", "uv"] if args.environment == "host" else (["pipx"] if args.environment == "musllinux_1_2" else [])
    for name in names:
        home = work / f"isolated {name}"
        home.mkdir()
        env = environment(home, [interpreter.parent])
        env["PIP_FIND_LINKS"] = args.wheelhouse.as_uri()
        env["PIPX_DEFAULT_PYTHON"] = str(interpreter)
        if name == "pipx":
            wheel.require(args.pipx is not None and args.pipx.is_absolute(), "absolute pipx required")
            version = run([args.pipx, "--version"], cwd=home, env=env).stdout.strip()
            wheel.require(version == b"1.18.16", "pipx must match locked 1.18.16")
            # Preseed pipx's shared pip from the locked local wheelhouse. Its
            # bootstrap must not silently fetch the latest pip from PyPI.
            shared_venv = home / "pipx" / "shared"
            create = wheel.load("wheel_smoke").venv_command(interpreter, shared_venv)
            run(create, cwd=home, env=env)
            run([python_in(shared_venv), "-I", "-m", "pip", "--isolated", "install",
                 "--no-index", "--find-links", args.wheelhouse, "--only-binary=:all:", "pip==26.0"],
                cwd=home, env=env)
            pip_args = (f"--no-index --find-links {args.wheelhouse.as_uri()} "
                        f"--find-links {args.dir.resolve().as_uri()} --only-binary=:all:"
                        + (" --pre" if ".dev" in args.version or "rc" in args.version else ""))
            run([args.pipx, "install", "--backend", "pip", "--fetch-python", "never",
                 "--skip-maintenance", "--python", interpreter, "--pip-args", pip_args,
                 f"gukhanmun-cli=={args.version}"], cwd=home, env=env)
            command = command_in(home / "pipx-bin")
            venv = home / "pipx" / "venvs" / "gukhanmun-cli"
            python = python_in(venv)
        else:
            wheel.require(args.uv is not None and args.uv.is_absolute(), "absolute uv required")
            version = run([args.uv, "--version"], cwd=home, env=env).stdout.strip()
            wheel.require(version.startswith(b"uv 0.13.7"), "uv must match pinned 0.13.7")
            uv_resolver_checks(args.uv, interpreter, work, env)
            run([args.uv, "--no-config", "tool", "install", "--offline", "--no-index",
                 "--find-links", args.wheelhouse, "--find-links", args.dir.resolve(),
                 "--prerelease", "allow", "--python", interpreter,
                 f"gukhanmun-cli=={args.version}"], cwd=home, env=env)
            command = command_in(home / "uv-bin")
            venv = home / "uv-tools" / "gukhanmun-cli"
            python = python_in(venv)
        wheel.require(command.is_file(), f"{name} did not expose installed command")
        info = installed_info(python, home, env)
        wheel.require(info["payload"] == expected["payload"], f"{name} installed another payload")
        installed_native = Path(info["root"]) / "_bin" / native.name
        tool_env = env | {"PATH": str(command.parent)}
        control = runtime(command, installed_native, info, home, tool_env)
        if os.name == "nt":
            console[name] = control["pip"]
        if name == "pipx":
            run([args.pipx, "uninstall", "gukhanmun-cli"], cwd=home, env=env)
        else:
            run([args.uv, "--no-config", "tool", "uninstall", "gukhanmun-cli"], cwd=home, env=env)
        wheel.require(not command.exists() and not command_in(command.parent, "gukhanmun-mkdict").exists()
                      and not venv.exists(), f"{name} uninstall left installed command/environment")
        tools.append(name)
    return tools, console


def smoke(args):
    args.dir = args.dir.resolve()
    args.wheelhouse = args.wheelhouse.resolve(strict=True)
    item = artifacts.target(args.target)
    host = artifacts.native_host(item)
    libc = libc_identity(args.environment)
    wheel.require(args.python.is_absolute() and args.python.is_file(), "absolute interpreter required")
    wheelhouse_check(args.wheelhouse)
    if args.environment == "host":
        wheel.require(args.image is None, "host evidence must not claim a libc image")
    else:
        expected_image = wheel.load("artifacts").IMAGES.get(f"{args.environment}_{item.machine}")
        wheel.require(item.family == "linux" and args.image == expected_image, "exact pinned libc image required")
    wheel_path = (args.dir / wheel.filename(item, args.version)).resolve()
    payload, base = artifacts.record_base(wheel_path, item, args.version)
    with tempfile.TemporaryDirectory(prefix="gukhanmun cli 한글 ") as temporary:
        work = Path(temporary)
        env = environment(work)
        interpreter, env = install(args.python, work / "venv space", wheel_path, args.wheelhouse, work, env)
        info = installed_info(interpreter, work, env)
        wheel.require(info["payload"] == payload, "installed payload identity differs")
        command = command_in(scripts(work / "venv space"))
        native = Path(info["root"]) / "_bin" / ("gukhanmun.exe" if item.family == "windows" else "gukhanmun")
        console = runtime(command, native, info, work, env)
        resolver = info["interpreter"]["implementation"] == "cpython" and info["interpreter"]["version"][:2] == [3, 15] and args.environment == "host"
        if resolver:
            resolver_checks(interpreter, work, env)
        shell_probes(args.shell, info, command, work, env, item, base, args.dir)
        expected = info["interpreter"] | {"payload": info["payload"]}
        tools, extra_console = tool_lifecycles(args, args.python, work, wheel_path, native, expected, item)
        console.update(extra_console)
        run([interpreter, "-I", "-m", "pip", "--isolated", "uninstall", "-y", "gukhanmun-cli"], cwd=work, env=env)
        wheel.require(not command.exists() and not command_in(command.parent, "gukhanmun-mkdict").exists(),
                      "pip uninstall left console command")
        absent = run([interpreter, "-I", "-c", "import gukhanmun_cli"], cwd=work, env=env, check=False)
        wheel.require(absent.returncode != 0, "pip uninstall left importable package")
    implementation, version = info["interpreter"]["implementation"], info["interpreter"]["version"]
    suffix = f"{implementation}{version[0]}{version[1]}"
    record = base | {"host": host, "libc": libc, "interpreter": info["interpreter"], "environment": args.environment,
                     "image": args.image, "tools": tools, "console": console,
                     "mkdict_checks": ["version", "help", "fst", "cdb", "error"],
                     "checks": ["version", "help", "dictionary", "pipe", "file", "error",
                                "completions", "signals", "uninstall"]}
    if resolver:
        record["checks"].append("resolver")
    (args.dir / f"cli-execution-{item.id}-{args.environment}-{suffix}.json").write_bytes(wheel.json_bytes(record))
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("target", "version"):
        parser.add_argument(f"--{name}", required=True)
    for name in ("dir", "python", "wheelhouse"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--environment", choices=("host", "manylinux_2_28", "musllinux_1_2"), required=True)
    parser.add_argument("--image")
    parser.add_argument("--pipx", type=Path)
    parser.add_argument("--uv", type=Path)
    parser.add_argument("--shell", choices=wheel.SHELL_FILES, action="append", default=[])
    smoke(parser.parse_args())


if __name__ == "__main__":
    main()
