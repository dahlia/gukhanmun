# Gukhanmun: Builds, audits, tests and inventories the Python release artifacts.
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

"""The Python release artifact set: one table, and the commands around it.

`TARGETS` lists every native wheel target.  With the six data wheels and the
seven sdists it defines the 31 files of a complete release, and the CI matrix
is generated from it (`matrix`).

 -  `build` compiles the CPython `abi3` wheel and, where supported, the PyPy
    wheels of one target.  Linux targets build inside a digest-pinned PyPA
    image with its interpreters; macOS and Windows build on the host with the
    mise-pinned interpreters.  Each build writes a provenance record.
 -  `audit` checks the wheels of one target: structure and metadata, the
    binary's architecture and dependencies, and the platform floor (with the
    image's `auditwheel` on Linux, `lipo`/`otool` on macOS and `pefile` on
    Windows).
 -  `test` installs the wheels of one target with each interpreter it must
    support, through *wheel_smoke.py*: mise interpreters on the host, and the
    image interpreters for musl and for the glibc floor.
 -  `manifest` inventories a directory of artifacts and evidence; `verify`
    recomputes it and, with `--complete`, requires the whole release with an
    audit, execution evidence and an sdist rebuild for every file.

The module needs only the standard library (plus `pefile` for Windows audits),
so it also runs on the bare interpreters of the build images.
"""

import argparse
from dataclasses import asdict, dataclass
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import platform
import posixpath
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "target" / "python" / "release"
SCHEMA = 1
MACOS_DEPLOYMENT_TARGET = "11.0"
GLIBC_FLOOR = (2, 28)
MUSL_FLOOR = (1, 2)
# CPython runs the abi3 wheel from this version on; it also builds it.
CPYTHON_MINIMUM = "3.11"
IMAGES = {
    "manylinux_2_28_x86_64": "quay.io/pypa/manylinux_2_28_x86_64@sha256:"
                             "9555ba6c479fa69a2aa80ad1dc57154a60d562b19df159361d9379a02beb1e36",
    "manylinux_2_28_aarch64": "quay.io/pypa/manylinux_2_28_aarch64@sha256:"
                              "cb37f0d66c8a75abb93cf6085cad1f1427cd7208f0c0fc48359bb1eea9924f21",
    "musllinux_1_2_x86_64": "quay.io/pypa/musllinux_1_2_x86_64@sha256:"
                            "cf82db9d8dc8c5c2cb27f422208baf266ebc8ad70be4ab1bc79bcc8807a6432d",
    "musllinux_1_2_aarch64": "quay.io/pypa/musllinux_1_2_aarch64@sha256:"
                             "6cdf1dca55fa6fb1e689e7516e465b51eed1f822167386d61102116c49235d6f",
}
# Where the checkout and the artifacts appear inside a container.
MOUNT_SOURCE = "/gukhanmun/source"
MOUNT_WORK = "/gukhanmun/work"


@dataclass(frozen=True)
class Target:
    """One native wheel target and the runner that builds and executes it."""

    id: str
    # GitHub-hosted runner label; each one executes natively on its target.
    runner: str
    rust: str
    # Wheel platform tag, which is also the support floor.
    platform: str
    # "manylinux", "musllinux", "macos" or "windows".
    family: str
    # Normalized machine: "x86_64" or "aarch64".
    machine: str
    # Whether PyPy wheels are built; upstream PyPy has no musl or Windows ARM64.
    pypy: bool

    @property
    def image(self):
        return IMAGES.get(self.platform)


TARGETS = (
    Target("linux-x86_64", "ubuntu-latest", "x86_64-unknown-linux-gnu", "manylinux_2_28_x86_64",
           "manylinux", "x86_64", True),
    Target("linux-aarch64", "ubuntu-24.04-arm", "aarch64-unknown-linux-gnu", "manylinux_2_28_aarch64",
           "manylinux", "aarch64", True),
    Target("musl-x86_64", "ubuntu-latest", "x86_64-unknown-linux-musl", "musllinux_1_2_x86_64",
           "musllinux", "x86_64", False),
    Target("musl-aarch64", "ubuntu-24.04-arm", "aarch64-unknown-linux-musl", "musllinux_1_2_aarch64",
           "musllinux", "aarch64", False),
    Target("macos-x86_64", "macos-15-intel", "x86_64-apple-darwin", "macosx_11_0_x86_64",
           "macos", "x86_64", True),
    Target("macos-arm64", "macos-latest", "aarch64-apple-darwin", "macosx_11_0_arm64",
           "macos", "aarch64", True),
    Target("windows-amd64", "windows-latest", "x86_64-pc-windows-msvc", "win_amd64",
           "windows", "x86_64", True),
    Target("windows-arm64", "windows-11-arm", "aarch64-pc-windows-msvc", "win_arm64",
           "windows", "aarch64", False),
)
MACHINES = {"x86_64": "x86_64", "amd64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64"}


def load(name, path=None):
    """Load a sibling script or the private dictionary table by path."""
    path = path or Path(__file__).with_name(f"{name}.py")
    spec = importlib.util.spec_from_file_location(f"_gukhanmun_{name}", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def dictionary_table(root=ROOT):
    return load("dictionary_data", root / "python/gukhanmun/src/gukhanmun/_dictionary_data.py")


def target(name):
    for item in TARGETS:
        if item.id == name:
            return item
    raise SystemExit(f"unknown target {name!r}; choose from {', '.join(t.id for t in TARGETS)}")


def python_pins(root=ROOT):
    """The mise interpreter pins, keyed by tox-style environment name."""
    return load("matrix").pins(root)


def pypy_abis(root=ROOT):
    """Wheel tags of every pinned PyPy, e.g. ("3.11", "pp311", "pypy311_pp80")."""
    result = []
    for name, pin in python_pins(root).items():
        if not name.startswith("pypy"):
            continue
        match = re.fullmatch(r"pypy(3)\.(\d+)-(\d+)\.(\d+)\.\d+", pin)
        major, minor, pypy_major, pypy_minor = match.groups()
        result.append({"python": f"{major}.{minor}", "tag": f"pp{major}{minor}",
                       "abi": f"pypy{major}{minor}_pp{pypy_major}{pypy_minor}", "pin": pin})
    return result


def core_version(root=ROOT):
    manifest = tomllib.loads((root / "python/gukhanmun/pyproject.toml").read_text(encoding="utf-8"))
    return manifest["project"]["version"]


def expected_files(version, root=ROOT):
    """Every file of a complete release, with what each one is."""
    files = {}
    for item in TARGETS:
        files[f"gukhanmun-{version}-cp311-abi3-{item.platform}.whl"] = {
            "kind": "wheel", "distribution": "gukhanmun", "target": item.id,
            "implementation": "cpython", "python": CPYTHON_MINIMUM}
        if item.pypy:
            for abi in pypy_abis(root):
                files[f"gukhanmun-{version}-{abi['tag']}-{abi['abi']}-{item.platform}.whl"] = {
                    "kind": "wheel", "distribution": "gukhanmun", "target": item.id,
                    "implementation": "pypy", "python": abi["python"]}
    files[f"gukhanmun-{version}.tar.gz"] = {"kind": "sdist", "distribution": "gukhanmun"}
    for distribution in dictionary_table(root).DISTRIBUTIONS:
        files[f"{distribution.package}-{version}-py3-none-any.whl"] = {
            "kind": "wheel", "distribution": distribution.name}
        files[f"{distribution.package}-{version}.tar.gz"] = {"kind": "sdist", "distribution": distribution.name}
    return files


def test_interpreters(item, root=ROOT):
    """The interpreters a target's wheels must be executed with.

    The abi3 wheel runs on the oldest and the newest supported CPython; each
    PyPy wheel on its own PyPy.  musl targets use the image interpreters,
    since mise provides none for musl, and glibc targets also run once inside
    the manylinux image, on the glibc floor itself.
    """
    pins = python_pins(root)
    cpython = sorted((pin for name, pin in pins.items() if name.startswith("py3")),
                     key=lambda pin: tuple(map(int, pin.split("."))))
    oldest, newest = cpython[0], cpython[-1]
    result = []
    if item.family == "musllinux":
        for pin in (oldest, newest):
            minor = pin.split(".")[1]
            result.append({"label": f"cp3{minor}-image", "implementation": "cpython", "libc": ["musl"],
                           "python": f"3.{minor}", "image": f"/opt/python/cp3{minor}-cp3{minor}/bin/python"})
        return result
    for pin in (oldest, newest):
        minor = pin.split(".")[1]
        result.append({"label": f"cp3{minor}", "implementation": "cpython", "python": f"3.{minor}", "pin": pin})
    if item.pypy:
        for abi in pypy_abis(root):
            result.append({"label": abi["tag"], "implementation": "pypy", "python": abi["python"],
                           "pin": abi["pin"]})
    if item.family == "manylinux":
        minor = oldest.split(".")[1]
        result.append({"label": f"cp3{minor}-floor", "implementation": "cpython", "python": f"3.{minor}",
                       "libc": ["glibc", ".".join(map(str, GLIBC_FLOOR))],
                       "image": f"/opt/python/cp3{minor}-cp3{minor}/bin/python"})
    return result


def build_interpreters(item, root=ROOT):
    """The interpreters maturin builds a target's wheels for."""
    minor = CPYTHON_MINIMUM.split(".")[1]
    result = [{"implementation": "cpython", "python": CPYTHON_MINIMUM, "abi": "abi3",
               "pin": next(pin for pin in python_pins(root).values() if pin.startswith(CPYTHON_MINIMUM + ".")),
               "image": f"/opt/python/cp3{minor}-cp3{minor}/bin/python"}]
    if item.pypy:
        for abi in pypy_abis(root):
            implementation_abi = abi["abi"].removeprefix(f"pypy{abi['tag'][2:]}_")
            result.append({"implementation": "pypy", "python": abi["python"], "abi": abi["abi"], "pin": abi["pin"],
                           "image": f"/opt/python/{abi['tag']}-{abi['tag'].replace('pp', 'pypy')}_"
                                    f"{implementation_abi}/bin/python"})
    return result


def matrix(root=ROOT):
    rows = [{"target": item.id, "runner": item.runner, "family": item.family} for item in TARGETS]
    return {"include": rows}


# ---------------------------------------------------------------------------
# Running commands, interpreters and containers


def run(command, *, cwd=ROOT, env=None, capture=False):
    print("+", " ".join(map(str, command)), flush=True)
    result = subprocess.run(list(map(str, command)), cwd=cwd, env=env, text=True, encoding="utf-8",
                            errors="replace", capture_output=capture)
    if result.returncode != 0:
        output = (result.stdout or "") + (result.stderr or "")
        raise RuntimeError(f"{command[0]} failed with exit code {result.returncode}\n{output}")
    return result


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


PROBE = ("import json, platform, struct, sys, sysconfig; print(json.dumps({"
         "'implementation': sys.implementation.name, "
         "'python': '%d.%d.%d' % sys.version_info[:3], "
         "'implementation_version': '%d.%d.%d' % tuple(sys.implementation.version[:3]), "
         "'soabi': sysconfig.get_config_var('SOABI'), 'machine': platform.machine(), "
         "'pointer_bits': struct.calcsize('P') * 8, "
         "'free_threaded': bool(sysconfig.get_config_var('Py_GIL_DISABLED')), "
         "'libc': platform.libc_ver(), 'executable': sys.executable}))")
# platform.libc_ver() cannot name musl before Python 3.13; the probe is
# informative only, while execution evidence normalizes it in smoke.py.


def probe(executable):
    """Describe an interpreter, failing clearly when it is missing."""
    if not Path(executable).is_file():
        raise RuntimeError(f"required interpreter {executable} does not exist")
    return json.loads(run([executable, "-I", "-c", PROBE], capture=True).stdout)


def check_interpreter(found, expected, item):
    """Refuse a build or test interpreter that is not the one the table names."""
    problems = []
    if found["implementation"] != expected["implementation"]:
        problems.append(f"implementation {found['implementation']}")
    if not found["python"].startswith(expected["python"] + "."):
        problems.append(f"Python {found['python']}")
    if MACHINES.get(found["machine"].lower()) != item.machine or found["pointer_bits"] != 64:
        problems.append(f"machine {found['machine']} ({found['pointer_bits']}-bit)")
    if found["free_threaded"]:
        problems.append("a free-threaded build")
    abi = expected.get("abi", "")
    if expected["implementation"] == "pypy" and abi:
        # SOABI "pypy311-pp80" corresponds to the wheel ABI tag "pypy311_pp80".
        if not (found["soabi"] or "").replace("-", "_").startswith(abi):
            problems.append(f"SOABI {found['soabi']}")
    if problems:
        raise RuntimeError(f"{found['executable']} is not the expected {expected}: " + ", ".join(problems))
    return found


def mise_interpreter(pin):
    """The executable of a mise-installed interpreter, verified by matrix.py."""
    executable, _ = load("matrix").interpreter(pin)
    return Path(executable)


def host_machine():
    return MACHINES.get(platform.machine().lower())


def host_family():
    return {"linux": "linux", "darwin": "macos", "win32": "windows"}.get(sys.platform, sys.platform)


def require_host(item):
    family = "linux" if item.family in ("manylinux", "musllinux") else item.family
    if host_family() != family or host_machine() != item.machine:
        raise RuntimeError(f"{item.id} must run on {family}/{item.machine}, "
                           f"not {host_family()}/{platform.machine()}; cross-compiled or emulated "
                           "artifacts do not count as executed on their target")


def container_engine():
    return os.environ.get("GUKHANMUN_CONTAINER_ENGINE") or "docker"


def volume(source, destination, *, read_only=False):
    """A bind mount; GUKHANMUN_CONTAINER_VOLUME_OPTIONS adds e.g. SELinux labels."""
    options = [option for option in (["ro"] if read_only else [])
               + os.environ.get("GUKHANMUN_CONTAINER_VOLUME_OPTIONS", "").split(",") if option]
    return ["--volume", f"{source}:{destination}" + (f":{','.join(options)}" if options else "")]


def container_command(image, work):
    return [container_engine(), "run", "--rm", *volume(ROOT, MOUNT_SOURCE, read_only=True),
            *volume(work, MOUNT_WORK), "--workdir", MOUNT_WORK]


def in_container(image, work, arguments, *, env=None):
    """Run this script inside the image with the checkout mounted read-only."""
    command = container_command(image, work)
    for key, value in (env or {}).items():
        command += ["--env", f"{key}={value}"]
    script = f"{MOUNT_SOURCE}/scripts/python/artifacts.py"
    run([*command, image, "/opt/python/cp311-cp311/bin/python", "-I", script, *arguments])


def image_digest(image):
    return image.split("@", 1)[1] if image and "@" in image else None


# ---------------------------------------------------------------------------
# Building


def rust_version(root=ROOT):
    return tomllib.loads((root / "mise.toml").read_text(encoding="utf-8"))["tools"]["rust"]["version"]


def ensure_rust(item, root):
    """Select the pinned Rust toolchain, installing it in a fresh container."""
    version = rust_version(root)
    environment = dict(os.environ, RUSTUP_TOOLCHAIN=version)
    cargo_bin = Path(environment.get("CARGO_HOME", Path.home() / ".cargo")) / "bin"
    if shutil.which("rustup", path=environment.get("PATH")) is None and (cargo_bin / "rustup").is_file():
        environment["PATH"] = f"{cargo_bin}{os.pathsep}{environment.get('PATH', '')}"
    if shutil.which("rustup", path=environment.get("PATH")) is None:
        if not os.environ.get("GUKHANMUN_IN_CONTAINER"):
            raise RuntimeError("rustup is required to build wheels")
        installer = Path(tempfile.mkdtemp()) / "rustup-init.sh"
        run(["curl", "--proto", "=https", "--tlsv1.2", "-sSf", "-o", installer, "https://sh.rustup.rs"])
        run(["sh", installer, "-y", "--no-modify-path", "--profile", "minimal", "--default-toolchain", version])
        environment["PATH"] = f"{cargo_bin}{os.pathsep}{environment.get('PATH', '')}"
    run(["rustup", "toolchain", "install", version, "--profile", "minimal", "--target", item.rust,
         "--no-self-update"], env=environment)
    return environment


def build_venv(interpreter, constraints, work, environment):
    """A venv with the locked, hash-checked build backend."""
    venv = Path(work) / "build-venv"
    run([interpreter, "-I", "-m", "venv", venv], env=environment)
    python = venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    run([python, "-I", "-m", "pip", "install", "--disable-pip-version-check", "--no-input",
         "--require-hashes", "--only-binary=:all:", "-r", constraints], env=environment)
    return python


def build_inside(item, work, root, commit):
    """Compile the target's wheels in this environment (image or host)."""
    require_host(item)
    work = Path(work)
    wheels = work / "wheels"
    shutil.rmtree(wheels, ignore_errors=True)
    wheels.mkdir(parents=True)
    interpreters = []
    for expected in build_interpreters(item, root):
        executable = Path(expected["image"]) if item.image else mise_interpreter(expected["pin"])
        interpreters.append(check_interpreter(probe(executable), expected, item))
    environment = ensure_rust(item, root)
    for key in ("CARGO_BUILD_TARGET", "CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS", "RUSTC_WRAPPER",
                "PYO3_PYTHON", "PYO3_CROSS", "PYO3_CROSS_LIB_DIR", "MATURIN_PEP517_ARGS"):
        environment.pop(key, None)
    environment["CARGO_TARGET_DIR"] = str(work / "cargo")
    if item.family == "macos":
        environment["MACOSX_DEPLOYMENT_TARGET"] = MACOS_DEPLOYMENT_TARGET
    maturin = build_venv(interpreters[0]["executable"], work / "build-constraints.txt", work, environment)
    command = [maturin, "-I", "-m", "maturin", "build", "--release", "--locked", "--target", item.rust,
               "--out", wheels]
    if item.family in ("manylinux", "musllinux"):
        command += ["--compatibility", item.platform]
    for interpreter in interpreters:
        command += ["--interpreter", interpreter["executable"]]
    run(command, cwd=root / "python/gukhanmun", env=environment)
    built = sorted(wheels.glob("*.whl"))
    expected = sorted(name for name, data in expected_files(core_version(root), root).items()
                      if data.get("target") == item.id)
    if [path.name for path in built] != expected:
        raise RuntimeError(f"{item.id} built {[path.name for path in built]}, expected {expected}")
    tool = lambda *command: run(list(command), env=environment, capture=True).stdout.strip()  # noqa: E731
    record = {
        "schema": SCHEMA, "target": asdict(item), "image": item.image, "image_digest": image_digest(item.image),
        "commit": commit, "rustc": tool("rustc", "-Vv"), "cargo": tool("cargo", "-V"),
        "maturin": tool(str(maturin), "-I", "-m", "maturin", "--version"),
        "interpreters": interpreters, "macos_deployment_target": environment.get("MACOSX_DEPLOYMENT_TARGET"),
        "runner": {key: os.environ.get(key) for key in ("RUNNER_OS", "RUNNER_ARCH", "ImageOS", "ImageVersion")},
        "wheels": {path.name: sha256(path) for path in built},
    }
    (work / f"build-info-{item.id}.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n",
                                                     encoding="utf-8")
    print(f"Built {', '.join(record['wheels'])}")


def export_constraints(work, root):
    run(["uv", "export", "--quiet", "--locked", "--only-group", "build", "--no-emit-workspace",
         "--output-file", Path(work) / "build-constraints.txt"], cwd=root)


def source_commit(root=ROOT):
    if os.environ.get("GITHUB_SHA"):
        return os.environ["GITHUB_SHA"]
    result = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True)
    return result.stdout.strip() or None


def build(arguments):
    item = target(arguments.target)
    work = Path(arguments.out).resolve()
    work.mkdir(parents=True, exist_ok=True)
    if arguments.inside:
        build_inside(item, work, Path(MOUNT_SOURCE), os.environ.get("GUKHANMUN_COMMIT"))
        return
    export_constraints(work, ROOT)
    if item.image:
        require_host(item)
        in_container(item.image, work, ["build", "--target", item.id, "--out", MOUNT_WORK, "--inside"],
                     env={"GUKHANMUN_IN_CONTAINER": "1", "GUKHANMUN_COMMIT": source_commit() or ""})
    else:
        build_inside(item, work, ROOT, source_commit())


# ---------------------------------------------------------------------------
# Auditing


def wheel_tag(filename):
    """Split a wheel filename into (distribution, version, python, abi, platform)."""
    parts = filename.removesuffix(".whl").split("-")
    if len(parts) != 5:
        raise RuntimeError(f"unexpected wheel filename {filename}")
    return tuple(parts)


def read_fields(text):
    import email.parser

    return email.parser.Parser().parsestr(text)


def check_structure(path, item, version, root):
    """Structure and metadata of one native wheel."""
    _, wheel_version, python, abi, platform_tag = wheel_tag(path.name)
    expected = expected_files(version, root).get(path.name)
    if expected is None or expected.get("target") != item.id or wheel_version != version:
        raise RuntimeError(f"{path.name} is not a {item.id} wheel of version {version}")
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        info = f"gukhanmun-{version}.dist-info"
        wheel = read_fields(archive.read(f"{info}/WHEEL").decode("utf-8"))
        metadata = archive.read(f"{info}/METADATA").decode("utf-8")
        extensions = [name for name in names if name.startswith("gukhanmun/_native.")
                      and not name.endswith(".pyi")]
        if wheel.get_all("Tag") != [f"{python}-{abi}-{platform_tag}"] or wheel["Root-Is-Purelib"] != "false":
            raise RuntimeError(f"{path.name} WHEEL metadata does not match its filename")
        if len(extensions) != 1:
            raise RuntimeError(f"{path.name} has extensions {extensions}")
        extension = PurePosixPath(extensions[0]).name
        suffix = ".pyd" if item.family == "windows" else ".so"
        if expected["implementation"] == "cpython":
            valid = extension == ("_native.pyd" if item.family == "windows" else "_native.abi3.so")
        else:
            soabi = abi.replace("_", "-")
            valid = extension.startswith(f"_native.{soabi}") and extension.endswith(suffix)
        if not valid:
            raise RuntimeError(f"{path.name} has the extension {extension}, which does not match {abi}")
        required = {"gukhanmun/__init__.py", "gukhanmun/py.typed", "gukhanmun/_native.pyi",
                    f"{info}/licenses/LICENSE", f"{info}/RECORD"}
        if not required <= set(names):
            raise RuntimeError(f"{path.name} lacks {sorted(required - set(names))}")
        cached = [name for name in names if "__pycache__/" in name or name.endswith((".pyc", ".pyo"))]
        if cached:
            raise RuntimeError(f"{path.name} contains bytecode caches: {cached}")
        check_extras(metadata, version, root, path.name)
        data = archive.read(extensions[0])
    return extensions[0], data


def check_extras(metadata, version, root, name):
    """Every extra pins exactly its data distribution at this version."""
    fields = read_fields(metadata)
    expected = {extra: (distribution, version) for extra, distribution in dictionary_table(root).EXTRAS.items()}
    actual = {}
    for line in fields.get_all("Requires-Dist") or []:
        match = re.fullmatch(r"""([A-Za-z0-9._-]+)\s*==\s*([^\s;]+)\s*;\s*extra\s*==\s*["']([^"']+)["']""",
                             line.strip())
        if match is None:
            raise RuntimeError(f"{name} has an unexpected requirement: {line}")
        actual[match[3]] = (match[1], match[2])
    if actual != expected or sorted(fields.get_all("Provides-Extra") or []) != sorted(expected):
        raise RuntimeError(f"{name} extras do not pin their data exactly: {actual}")


def parse_auditwheel(output):
    """Extract the computed platform tag from `auditwheel show` output."""
    text = " ".join(output.split())
    match = re.search(r'is consistent with the following platform tag: "([^"]+)"', text)
    if match is None:
        raise RuntimeError(f"unrecognized auditwheel output:\n{output}")
    return match[1]


def check_policy(computed, item):
    """The computed tag must be the target's floor or an older one."""
    pattern = r"(manylinux|musllinux)_(\d+)_(\d+)_(x86_64|aarch64)"
    found, wanted = re.fullmatch(pattern, computed), re.fullmatch(pattern, item.platform)
    if (found is None or found[1] != wanted[1] or found[4] != wanted[4]
            or (int(found[2]), int(found[3])) > (int(wanted[2]), int(wanted[3]))):
        raise RuntimeError(f"auditwheel computed {computed}, which does not satisfy {item.platform}")
    return computed


ELF_MACHINES = {"x86_64": "Advanced Micro Devices X86-64", "aarch64": "AArch64"}


def elf_machine(path, item):
    header = run(["readelf", "-h", path], capture=True).stdout
    machine = re.search(r"Machine:\s+(.+)", header)
    if machine is None or machine[1].strip() != ELF_MACHINES[item.machine]:
        raise RuntimeError(f"{Path(path).name} is built for {machine and machine[1].strip()}")
    return machine[1].strip()


def check_search_path(entry, member):
    """Each search path must be $ORIGIN-relative and stay inside the wheel."""
    for part in entry.split(":"):
        if part != "$ORIGIN" and not part.startswith("$ORIGIN/"):
            raise RuntimeError(f"{member} has a search path outside the wheel: {entry}")
        relative = part.removeprefix("$ORIGIN").lstrip("/")
        resolved = posixpath.normpath(posixpath.join(posixpath.dirname(member), relative))
        if resolved == ".." or resolved.startswith("../") or posixpath.isabs(resolved):
            raise RuntimeError(f"{member} has a search path outside the wheel: {entry}")


def check_elf(extension, item, member, dynamic=None):
    """Architecture, linked libraries and search paths of a Linux extension."""
    machine = elf_machine(extension, item)
    if dynamic is None:
        dynamic = run(["readelf", "-d", extension], capture=True).stdout
    needed = re.findall(r"\(NEEDED\)\s+Shared library: \[([^\]]+)\]", dynamic)
    paths = re.findall(r"\((?:RPATH|RUNPATH)\)\s+Library (?:rpath|runpath): \[([^\]]+)\]", dynamic)
    if any(name.startswith("libpython") for name in needed):
        raise RuntimeError(f"{member} links libpython: {needed}")
    for entry in paths:
        check_search_path(entry, member)
    return {"machine": machine, "needed": needed, "search_paths": paths}


MACHO_ARCHS = {"x86_64": "x86_64", "aarch64": "arm64"}


def check_macho(extension, item):
    """Architecture, linked libraries and deployment target of a macOS extension."""
    archs = run(["lipo", "-archs", extension], capture=True).stdout.split()
    if archs != [MACHO_ARCHS[item.machine]]:
        raise RuntimeError(f"{extension.name} contains {archs}")
    identity = run(["otool", "-D", extension], capture=True).stdout
    libraries = parse_otool_libraries(run(["otool", "-L", extension], capture=True).stdout, identity)
    foreign = [name for name in libraries if not name.startswith(("/usr/lib/", "/System/"))]
    if foreign:
        raise RuntimeError(f"{extension.name} links libraries outside the system: {foreign}")
    commands = run(["otool", "-l", extension], capture=True).stdout
    minimum = parse_macos_minimum(commands)
    if minimum > (*map(int, MACOS_DEPLOYMENT_TARGET.split(".")), 0):
        raise RuntimeError(f"{extension.name} requires macOS {'.'.join(map(str, minimum))}")
    return {"archs": archs, "libraries": libraries, "minos": ".".join(map(str, minimum))}


def parse_otool_libraries(listing, identity):
    """The libraries in `otool -L` output, without the image's own install name.

    A bundle or dylib lists its `LC_ID_DYLIB` install name first, such as
    `@rpath/gukhanmun._native.abi3.so`; `otool -D` prints that name.
    """
    own = {line.strip() for line in identity.splitlines()[1:] if line.strip()}
    libraries = [line.split(" (")[0].strip() for line in listing.splitlines()[1:] if line.strip()]
    return [name for name in libraries if name not in own]


def parse_macos_minimum(output):
    """Read the minimum macOS version from `otool -l` output."""
    for block in re.split(r"\n(?=Load command \d+)", output):
        if "cmd LC_BUILD_VERSION" in block:
            match = re.search(r"\n\s*minos\s+(\d+)\.(\d+)(?:\.(\d+))?\s", block + "\n")
        elif "cmd LC_VERSION_MIN_MACOSX" in block:
            match = re.search(r"\n\s*version\s+(\d+)\.(\d+)(?:\.(\d+))?\s", block + "\n")
        else:
            continue
        if match is None:
            raise RuntimeError(f"unrecognized deployment load command:\n{block}")
        return int(match[1]), int(match[2]), int(match[3] or 0)
    raise RuntimeError("the extension has no macOS deployment load command")


PE_MACHINES = {"x86_64": 0x8664, "aarch64": 0xAA64}
# The extension may import only system libraries and the libraries that the
# interpreter itself ships; Windows resolves api-ms-win-* API sets itself.
WINDOWS_SYSTEM = {"kernel32.dll", "kernelbase.dll", "ntdll.dll", "advapi32.dll", "bcrypt.dll",
                  "bcryptprimitives.dll", "ws2_32.dll", "userenv.dll", "secur32.dll", "crypt32.dll",
                  "ole32.dll", "oleaut32.dll", "shell32.dll", "user32.dll"}
WINDOWS_RUNTIME = {"vcruntime140.dll", "vcruntime140_1.dll"}


def check_pe(extension, item, implementation, python):
    """Architecture and imports of a Windows extension, including delay imports."""
    import pefile

    image = pefile.PE(str(extension), fast_load=True)
    try:
        image.parse_data_directories(directories=[
            pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_IMPORT"],
            pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT"]])
        if image.FILE_HEADER.Machine != PE_MACHINES[item.machine]:
            raise RuntimeError(f"{extension.name} is built for machine {image.FILE_HEADER.Machine:#x}")
        imports = sorted(entry.dll.decode("ascii").lower() for entry in getattr(image, "DIRECTORY_ENTRY_IMPORT", []))
        delayed = sorted(entry.dll.decode("ascii").lower()
                         for entry in getattr(image, "DIRECTORY_ENTRY_DELAY_IMPORT", []))
    finally:
        image.close()
    interpreter = {"python3.dll"} if implementation == "cpython" else {f"libpypy{python}-c.dll"}
    allowed = WINDOWS_SYSTEM | WINDOWS_RUNTIME | interpreter
    unexpected = [name for name in imports + delayed
                  if name not in allowed and not name.startswith(("api-ms-win-crt-", "api-ms-win-core-"))]
    if unexpected:
        raise RuntimeError(f"{extension.name} imports unexpected libraries: {unexpected}")
    if implementation == "cpython" and "python3.dll" not in imports:
        raise RuntimeError(f"{extension.name} does not use the stable ABI library python3.dll: {imports}")
    return {"machine": f"{PE_MACHINES[item.machine]:#x}", "imports": imports, "delay_imports": delayed}


def audit_inside(item, work, root):
    work = Path(work)
    version = core_version(root)
    wheels = sorted((work / "wheels").glob("*.whl"))
    expected = sorted(name for name, data in expected_files(version, root).items() if data.get("target") == item.id)
    if [path.name for path in wheels] != expected:
        raise RuntimeError(f"{item.id} has {[path.name for path in wheels]}, expected {expected}")
    records = []
    for wheel in wheels:
        description = expected_files(version, root)[wheel.name]
        member, data = check_structure(wheel, item, version, root)
        record = {"wheel": wheel.name, "sha256": sha256(wheel), "target": item.id, "extension": member}
        with tempfile.TemporaryDirectory(prefix="gukhanmun-audit-") as temporary:
            extension = Path(temporary) / PurePosixPath(member).name
            extension.write_bytes(data)
            if item.family in ("manylinux", "musllinux"):
                output = run(["auditwheel", "show", wheel], capture=True)
                record["auditwheel"] = {"version": run(["auditwheel", "--version"], capture=True).stdout.split()[1],
                                        "output": output.stdout + output.stderr,
                                        "policy": check_policy(parse_auditwheel(output.stdout), item)}
                record["elf"] = check_elf(extension, item, member)
                # Libraries that the repair bundled must match the target too.
                with zipfile.ZipFile(wheel) as archive:
                    bundled = [name for name in archive.namelist() if name.startswith("gukhanmun.libs/")]
                    record["bundled"] = {}
                    for name in bundled:
                        library = Path(temporary) / PurePosixPath(name).name
                        library.write_bytes(archive.read(name))
                        record["bundled"][name] = elf_machine(library, item)
            elif item.family == "macos":
                record["macho"] = check_macho(extension, item)
            else:
                record["pe"] = check_pe(extension, item, description["implementation"], description["python"])
        records.append(record)
    summary = {"schema": SCHEMA, "target": item.id, "image": item.image, "records": records}
    (work / f"audit-{item.id}.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n",
                                                encoding="utf-8")
    print(f"Audited {len(records)} {item.id} wheels")


def audit(arguments):
    item = target(arguments.target)
    work = Path(arguments.out).resolve()
    if arguments.inside:
        audit_inside(item, work, Path(MOUNT_SOURCE))
    elif item.image:
        require_host(item)
        in_container(item.image, work, ["audit", "--target", item.id, "--out", MOUNT_WORK, "--inside"])
    else:
        audit_inside(item, work, ROOT)


# ---------------------------------------------------------------------------
# Executing


def test(arguments):
    """Install and smoke-test one target's wheels with every required interpreter."""
    item = target(arguments.target)
    require_host(item)
    work = Path(arguments.out).resolve()
    data = [Path(path).resolve() for path in arguments.data] if arguments.data else sorted(
        path for path in (ROOT / "target/python/dictionaries").glob("gukhanmun-*") if path.is_dir())
    data_wheels = [wheel for directory in data for wheel in directory.glob("*.whl")]
    if len(data_wheels) != len(dictionary_table().DISTRIBUTIONS):
        raise RuntimeError(f"expected the six data wheels, found {[wheel.name for wheel in data_wheels]}")
    version = core_version()
    audit_record = work / f"audit-{item.id}.json"
    if item.family == "windows" and not audit_record.is_file():
        raise RuntimeError(f"run audit first: Windows tests check the imports listed in {audit_record.name}")
    for interpreter in test_interpreters(item):
        evidence = f"smoke-{item.id}-{interpreter['label']}.json"
        smoke_arguments = ["run", "--artifacts", "wheels", "data", "--version", version,
                           "--label", f"{item.id}-{interpreter['label']}", "--evidence", evidence,
                           "--official", "official.json", "--expect-implementation", interpreter["implementation"],
                           "--expect-python", interpreter["python"], "--expect-machine", item.machine]
        if audit_record.is_file():
            smoke_arguments += ["--audit-record", audit_record.name]
        staging = Path(tempfile.mkdtemp(prefix="gukhanmun-test-"))
        try:
            shutil.copytree(work / "wheels", staging / "wheels")
            (staging / "data").mkdir()
            for wheel in data_wheels:
                shutil.copyfile(wheel, staging / "data" / wheel.name)
            shutil.copyfile(ROOT / "tests/fixtures/dictionaries/official.json", staging / "official.json")
            if audit_record.is_file():
                shutil.copyfile(audit_record, staging / audit_record.name)
            if "image" in interpreter:
                command = [*container_command(item.image, staging), item.image, interpreter["image"], "-I", f"{MOUNT_SOURCE}/scripts/python/wheel_smoke.py",
                           *smoke_arguments, "--python", interpreter["image"]]
                run(command)
            else:
                executable = mise_interpreter(interpreter["pin"])
                run([executable, "-I", ROOT / "scripts/python/wheel_smoke.py", *smoke_arguments,
                     "--python", executable], cwd=staging)
            shutil.copyfile(staging / evidence, work / evidence)
        finally:
            shutil.rmtree(staging, ignore_errors=True)
    print(f"Executed {item.id} wheels with {len(test_interpreters(item))} interpreters")


# ---------------------------------------------------------------------------
# Manifest and verification


def describe(path):
    return {"filename": path.name, "size": path.stat().st_size, "sha256": sha256(path)}


def manifest(arguments):
    """Inventory a directory of release files and their evidence records."""
    directory = Path(arguments.dir).resolve()
    version = arguments.version or core_version()
    expected = expected_files(version)
    artifacts = []
    for path in sorted(directory.iterdir()):
        if path.name.endswith((".whl", ".tar.gz")):
            entry = describe(path)
            entry.update(expected.get(path.name, {"kind": "unexpected"}))
            if path.name.endswith(".whl"):
                _, _, python, abi, platform_tag = wheel_tag(path.name)
                entry["tags"] = {"python": python, "abi": abi, "platform": platform_tag}
            artifacts.append(entry)
    evidence = [describe(path) for path in sorted(directory.glob("*.json")) if path.name != "manifest.json"]
    result = {
        "schema": SCHEMA, "version": version,
        "source": {"commit": source_commit(), "ref": os.environ.get("GITHUB_REF"),
                   "run_id": os.environ.get("GITHUB_RUN_ID"), "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT")},
        "toolchain": {"rust": rust_version(), "python_pins": python_pins(),
                      "build_backends": build_backends()},
        "artifacts": artifacts, "evidence": evidence,
    }
    (directory / "manifest.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (directory / "SHA256SUMS").write_text("".join(f"{entry['sha256']}  {entry['filename']}\n"
                                                  for entry in artifacts), encoding="utf-8")
    print(f"Wrote manifest.json and SHA256SUMS for {len(artifacts)} artifacts")


def build_backends(root=ROOT):
    groups = tomllib.loads((root / "pyproject.toml").read_text(encoding="utf-8"))["dependency-groups"]
    return sorted(groups["build"])


def load_json(directory, name):
    path = directory / name
    if not path.is_file():
        raise RuntimeError(f"missing evidence {name}")
    return json.loads(path.read_text(encoding="utf-8"))


def verify(arguments):
    directory = Path(arguments.dir).resolve()
    data = load_json(directory, "manifest.json")
    version = data["version"]
    problems = []
    listed = {entry["filename"]: entry for entry in data["artifacts"]}
    present = {path.name for path in directory.iterdir() if path.name.endswith((".whl", ".tar.gz"))}
    if present != set(listed):
        problems.append(f"files differ from the manifest: {sorted(present ^ set(listed))}")
    for name, entry in listed.items():
        path = directory / name
        if path.is_symlink() or not path.is_file():
            problems.append(f"{name} is not a regular file")
        elif path.stat().st_size != entry["size"] or sha256(path) != entry["sha256"]:
            problems.append(f"{name} does not match its size or hash")
        if not name.startswith(tuple(f"{prefix}-{version}" for prefix in ("gukhanmun", "gukhanmun_"))) \
                and f"-{version}" not in name:
            problems.append(f"{name} is not version {version}")
    for entry in data["evidence"]:
        path = directory / entry["filename"]
        if path.is_symlink() or not path.is_file() or sha256(path) != entry["sha256"]:
            problems.append(f"evidence {entry['filename']} is missing or changed")
    problems += check_sums(directory, listed)
    # Completeness reads the evidence, so only once it is known to be intact.
    if arguments.complete and not problems:
        try:
            problems += completeness(directory, data, listed)
        except (KeyError, TypeError, ValueError, AttributeError, RuntimeError) as error:
            problems.append(f"malformed evidence: {error!r}")
    if problems:
        raise SystemExit("verification failed:\n - " + "\n - ".join(problems))
    print(f"Verified {len(listed)} artifacts" + (" as a complete release" if arguments.complete else ""))


def check_sums(directory, listed):
    """SHA256SUMS must list exactly the manifest's artifacts and hashes."""
    path = directory / "SHA256SUMS"
    if not path.is_file():
        return ["SHA256SUMS is missing"]
    sums = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (\S+)", line)
        if match is None or match[2] in sums:
            return [f"SHA256SUMS has a malformed or repeated line: {line!r}"]
        sums[match[2]] = match[1]
    if sums != {name: entry["sha256"] for name, entry in listed.items()}:
        return ["SHA256SUMS does not match the manifest"]
    return []


def completeness(directory, data, listed):
    """Everything a release needs, with evidence for every file."""
    version = data["version"]
    expected = expected_files(version)
    problems = []
    if set(listed) != set(expected):
        problems.append(f"missing {sorted(set(expected) - set(listed))}, "
                        f"unexpected {sorted(set(listed) - set(expected))}")
    hashes = {name: entry["sha256"] for name, entry in listed.items()}
    audited, executed = {}, []
    for entry in data["evidence"]:
        record = load_json(directory, entry["filename"])
        if entry["filename"].startswith("audit-"):
            for item in record["records"]:
                audited[item["wheel"]] = item["sha256"]
        elif entry["filename"].startswith("smoke-"):
            executed.append(record)
    for name, description in expected.items():
        if description.get("target") is None:
            continue
        if audited.get(name) != hashes.get(name):
            problems.append(f"no audit of {name}")
    native = {name: description for name, description in expected.items() if description.get("target")}
    for name, description in native.items():
        item = target(description["target"])
        for interpreter in test_interpreters(item):
            if description["implementation"] != interpreter["implementation"]:
                continue
            if description["implementation"] == "pypy" and description["python"] != interpreter["python"]:
                continue
            label = f"{item.id}-{interpreter['label']}"
            if not any(executed_with(record, label, name, hashes.get(name), interpreter,
                                     windows=item.family == "windows") for record in executed):
                problems.append(f"no base and official execution of {name} with {interpreter['label']}")
    for name, description in expected.items():
        if description["kind"] == "wheel" and description.get("target") is None:
            if not any(record.get("official", {}).get("installed", {})
                       .get(description["distribution"], {}).get("sha256") == hashes.get(name)
                       and passed(record, "official") for record in executed):
                problems.append(f"no official execution of {name}")
    problems += rebuild_completeness(directory, data, expected, hashes)
    return problems


# The check each smoke mode must finish with: the base mode proves the core
# package works without data, the official mode runs the dictionary corpus.
FINAL_CHECKS = {"base": "missing-data", "official": "official-corpus"}


def passed(record, mode):
    result = record.get(mode, {}).get("result", {})
    return result.get("mode") == mode and FINAL_CHECKS[mode] in result.get("checks", [])


def executed_with(record, label, name, digest, interpreter, windows=False):
    if record.get("label") != label:
        return False
    for mode in ("base", "official"):
        result = record.get(mode, {})
        if not passed(record, mode):
            return False
        if windows and "windows-import-closure" not in result.get("result", {}).get("checks", []):
            return False
        libc = interpreter.get("libc")
        if libc and list(result.get("result", {}).get("interpreter", {}).get("libc", []))[:len(libc)] != libc:
            return False
        installed = result.get("installed", {}).get("gukhanmun", {})
        info = result.get("result", {}).get("interpreter", {})
        if (installed.get("filename") != name or installed.get("sha256") != digest
                or info.get("implementation") != interpreter["implementation"]
                or not str(info.get("python", "")).startswith(interpreter["python"] + ".")):
            return False
    return True


def rebuild_completeness(directory, data, expected, hashes):
    """Every sdist was rebuilt outside the checkout and its wheel executed."""
    names = {entry["filename"] for entry in data["evidence"]}
    if "rebuilds.json" not in names or "smoke-sdist-rebuild.json" not in names:
        return ["no sdist rebuild records"]
    rebuilds = load_json(directory, "rebuilds.json")
    smoke = load_json(directory, "smoke-sdist-rebuild.json")
    problems = []
    if rebuilds.get("evidence_sha256") != sha256(directory / "smoke-sdist-rebuild.json"):
        problems.append("the sdist rebuild records name a different smoke run")
    sdists = {name for name, description in expected.items() if description["kind"] == "sdist"}
    recorded = {record["sdist"]: record for record in rebuilds.get("rebuilds", [])}
    for name in sorted(sdists):
        record = recorded.get(name)
        if record is None or record.get("sdist_sha256") != hashes.get(name):
            problems.append(f"{name} was not rebuilt from the released file")
        elif not rebuilt_executed(smoke, record, expected[name]["distribution"]):
            problems.append(f"the wheel rebuilt from {name} was not executed")
    for mode in ("base", "official"):
        if not passed(smoke, mode):
            problems.append(f"the sdist rebuild lacks its {mode} execution")
    return problems


def rebuilt_executed(smoke, record, distribution):
    """The rebuilt wheel was installed, not just staged, by the passing runs."""
    modes = ("base", "official") if distribution == "gukhanmun" else ("official",)
    for mode in modes:
        installed = smoke.get(mode, {}).get("installed", {}).get(distribution, {})
        if installed.get("filename") != record["wheel"] or installed.get("sha256") != record["wheel_sha256"]:
            return False
    return True


# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("matrix", help="print the CI matrix generated from the target table")
    commands.add_parser("expected", help="print every file of a complete release")
    for name, help_text in (("build", "build one target's wheels"), ("audit", "audit one target's wheels"),
                            ("test", "execute one target's wheels")):
        command = commands.add_parser(name, help=help_text)
        command.add_argument("--target", required=True, choices=[item.id for item in TARGETS])
        command.add_argument("--out", type=Path, default=None)
        if name == "test":
            command.add_argument("--data", type=Path, nargs="*",
                                 help="directories of the six data wheels (default: this checkout's builds)")
        else:
            command.add_argument("--inside", action="store_true", help=argparse.SUPPRESS)
    for name in ("manifest", "verify"):
        command = commands.add_parser(name)
        command.add_argument("--dir", type=Path, required=True)
        if name == "manifest":
            command.add_argument("--version")
        else:
            command.add_argument("--complete", action="store_true")
    arguments = parser.parse_args()
    if getattr(arguments, "out", None) is None and hasattr(arguments, "target"):
        arguments.out = OUTPUT / arguments.target
    if arguments.command == "matrix":
        print(json.dumps(matrix()))
    elif arguments.command == "expected":
        print(json.dumps(expected_files(core_version()), indent=2, sort_keys=True))
    else:
        {"build": build, "audit": audit, "test": test, "manifest": manifest, "verify": verify}[arguments.command](
            arguments)


if __name__ == "__main__":
    main()
