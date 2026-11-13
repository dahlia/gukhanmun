# Gukhanmun: Builds the native Python sdist and rebuilds every sdist standalone.
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

"""Python distribution artifacts; invoked only through the mise tasks.

`sdist` builds the native `gukhanmun` source distribution with maturin and
checks that the archive stands alone: the binding crate, every workspace path
crate it needs, the Cargo manifests and lockfile, the Python sources, stubs
and licenses, and none of the unused dictionary source data of the optional
crates.  `sdist-test` unpacks that archive outside the checkout, builds a
wheel from it with an isolated PEP 517 frontend, and installs the result
together with the six data wheels that `py-dicts-build` rebuilt from their
own sdists.  The rebuilt set then passes the same Rust-free smoke checks as
the release wheels, and a record links every sdist to the wheel built from it.
"""

import argparse
import email.parser
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "target" / "python"
SDIST = OUTPUT / "sdist"
REBUILT = OUTPUT / "sdist-rebuilt"
DICTIONARIES = OUTPUT / "dictionaries"
# Cargo output of the standalone rebuild stays out of the unpacked archive,
# and under target/ so CI caches can reuse compiled registry dependencies.
CARGO_TARGET = OUTPUT / "sdist-target"
# PyPI's default per-file upload limit.
SIZE_LIMIT = 100 * 1024 * 1024
DICTIONARY_CRATES = ("gukhanmun-stdict", "gukhanmun-opendict", "gukhanmun-loanwords")
PYTHON_SOURCES = ("__init__.py", "_api.py", "_types.py", "_dictionary_data.py", "dictionaries.py",
                  "_native.pyi", "py.typed")
# Separately licensed crates whose license material must stay in the archive.
CRATE_LICENSES = {"gukhanmun-opendict": ("LICENSE", "ATTRIBUTION.md"),
                  "gukhanmun-loanwords": ("LICENSE", "ATTRIBUTION.md")}
# Inherited settings that could redirect or alter the standalone Cargo build.
CARGO_OVERRIDES = ("CARGO_TARGET_DIR", "CARGO_BUILD_TARGET", "CARGO_BUILD_TARGET_DIR", "CARGO_ENCODED_RUSTFLAGS",
                   "RUSTFLAGS", "RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "MATURIN_PEP517_ARGS")


def load(name):
    """Load a sibling script by path; the scripts are not a package."""
    spec = importlib.util.spec_from_file_location(f"_gukhanmun_{name}", Path(__file__).with_name(f"{name}.py"))
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def run(command, *, cwd=ROOT, env=None):
    print("+", " ".join(map(str, command)), flush=True)
    subprocess.run(list(map(str, command)), cwd=cwd, env=env, check=True)


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def core_version(root=ROOT):
    manifest = root / "python" / "gukhanmun" / "pyproject.toml"
    return tomllib.loads(manifest.read_text(encoding="utf-8"))["project"]["version"]


def hashed_constraints(directory):
    """Export the locked build backends, as the data packages already do."""
    constraints = Path(directory) / "build-constraints.txt"
    run(["uv", "export", "--quiet", "--locked", "--only-group", "build", "--no-emit-workspace",
         "--output-file", constraints])
    return ["--build-constraints", constraints, "--require-build-hashes",
            "--preview-features", "build-dependency-hashes"]


def check_sdist(path, version):
    """Require a standalone archive without unrelated or unsafe members."""
    prefix = f"gukhanmun-{version}"
    if path.name != f"{prefix}.tar.gz":
        raise RuntimeError(f"unexpected sdist name {path.name}")
    if path.stat().st_size >= SIZE_LIMIT:
        raise RuntimeError(f"{path.name} is {path.stat().st_size} bytes, over the PyPI file limit")
    with tarfile.open(path) as archive:
        members = archive.getmembers()
        names = set()
        for member in members:
            name = PurePosixPath(member.name)
            if name.is_absolute() or ".." in name.parts or name.parts[0] != prefix:
                raise RuntimeError(f"{path.name} has an unsafe member {member.name}")
            if not (member.isfile() or member.isdir()):
                raise RuntimeError(f"{path.name} has a link or special member {member.name}")
            relative = PurePosixPath(*name.parts[1:]) if len(name.parts) > 1 else None
            if relative is None or member.isdir():
                continue
            text = relative.as_posix()
            if text in names:
                raise RuntimeError(f"{path.name} has a duplicate member {text}")
            names.add(text)
            if "__pycache__" in relative.parts or relative.suffix in (".pyc", ".pyo") or ".git" in relative.parts:
                raise RuntimeError(f"{path.name} contains {text}")
            if len(relative.parts) > 2 and relative.parts[0] == "crates" \
                    and relative.parts[1] in DICTIONARY_CRATES and relative.parts[2] == "data":
                raise RuntimeError(f"{path.name} contains unused dictionary data {text}")
        read = lambda name: archive.extractfile(f"{prefix}/{name}").read().decode("utf-8")  # noqa: E731
        required = {"pyproject.toml", "PKG-INFO", "README.md", "LICENSE", "Cargo.toml", "Cargo.lock",
                    "crates/gukhanmun-python/Cargo.toml", "crates/gukhanmun-python/build.rs",
                    "crates/gukhanmun-python/src/lib.rs",
                    *(f"src/gukhanmun/{name}" for name in PYTHON_SOURCES),
                    *(f"crates/{crate}/{name}" for crate, files in CRATE_LICENSES.items() for name in files)}
        missing = required - names
        if missing:
            raise RuntimeError(f"{path.name} lacks {sorted(missing)}")
        workspace = tomllib.loads(read("Cargo.toml"))["workspace"]
        members = workspace.get("members", [])
        absent = [member for member in members if f"{member}/Cargo.toml" not in names]
        if "crates/gukhanmun-python" not in members or absent:
            raise RuntimeError(f"{path.name} workspace members are incomplete: {absent or members}")
        fields = email.parser.Parser().parsestr(read("PKG-INFO"))
        if (fields["Name"] != "gukhanmun" or fields["Version"] != version
                or fields["License-Expression"] != "GPL-3.0-only"):
            raise RuntimeError(f"{path.name} has unexpected PKG-INFO metadata")
        checks = load("checks")
        checks.check_extras(read("PKG-INFO"))
        checks.check_readme(read("PKG-INFO"), read("README.md"), label=path.name)
    return sorted(names)


def check_sources(names, root=ROOT):
    """Require every source file of each packaged workspace crate.

    Cargo needs each member's whole *src/* tree and build script to compile
    it; a missing module would only surface when someone builds the archive.
    """
    present = set(names)
    members = {PurePosixPath(name).parent.as_posix() for name in present
               if name.startswith("crates/") and name.endswith("/Cargo.toml")}
    missing = []
    for member in sorted(members):
        crate = root / member
        expected = [crate / "build.rs"] if (crate / "build.rs").is_file() else []
        expected += [path for path in (crate / "src").rglob("*") if path.is_file()
                     and "__pycache__" not in path.parts] if (crate / "src").is_dir() else []
        missing += [path.relative_to(root).as_posix() for path in expected
                    if path.relative_to(root).as_posix() not in present]
    if missing:
        raise RuntimeError(f"the sdist lacks crate sources: {missing}")
    return sorted(members)


def sdist(root=ROOT):
    version = core_version(root)
    shutil.rmtree(SDIST, ignore_errors=True)
    SDIST.mkdir(parents=True)
    run([sys.executable, "-m", "maturin", "sdist", "--out", SDIST], cwd=root / "python" / "gukhanmun")
    archives = list(SDIST.glob("*.tar.gz"))
    if len(archives) != 1:
        raise RuntimeError(f"expected one native sdist, got {archives}")
    names = check_sdist(archives[0], version)
    crates = check_sources(names, root)
    print(f"Verified {archives[0].name}: {len(names)} files of {len(crates)} crates, "
          f"{archives[0].stat().st_size} bytes")


def lock_packages(path):
    """Map each locked package to its checksum and resolved dependency edges.

    Cargo writes a dependency as `name` when only one version is locked and as
    `name version` (with a source when needed) otherwise, so pruning can
    shorten the notation; edges are compared as resolved (name, version) pairs.
    """
    packages = tomllib.loads(path.read_text(encoding="utf-8")).get("package", [])
    versions = {}
    for package in packages:
        versions.setdefault(package["name"], []).append(package["version"])
    result = {}
    for package in packages:
        edges = set()
        for dependency in package.get("dependencies", []):
            name, *rest = dependency.split(" ")
            if rest:
                edges.add((name, rest[0]))
            elif len(versions.get(name, [])) == 1:
                edges.add((name, versions[name][0]))
            else:
                raise RuntimeError(f"{path.name}: ambiguous dependency {dependency!r} of {package['name']}")
        result[(package["name"], package["version"], package.get("source"))] = (package.get("checksum"),
                                                                                frozenset(edges))
    return result


def check_lock_subset(shipped, rebuilt):
    """The rebuild may prune the shipped lock, never add to or change it.

    Every remaining package keeps its checksum, and its dependency edges can
    only shrink: Cargo also drops edges that only the pruned members enable
    through features, but a new or re-pointed edge would mean a different
    resolution.

    maturin keeps the whole workspace lockfile while listing only the crates
    the binding needs, so `cargo --locked` would refuse the archive; an
    unlocked build may drop the unused entries and nothing else.
    """
    original, actual = lock_packages(shipped), lock_packages(rebuilt)
    added = sorted(key for key in actual if key not in original)
    changed = sorted(key for key in actual if key in original
                     and (actual[key][0] != original[key][0] or not actual[key][1] <= original[key][1]))
    if added or changed:
        raise RuntimeError(f"the standalone build changed Cargo.lock: added {added}, changed {changed}")
    if not any(name == "gukhanmun-python" for name, _, _ in actual):
        raise RuntimeError("the rebuilt Cargo.lock does not contain gukhanmun-python")
    return {"shipped_packages": len(original), "built_packages": len(actual)}


def outside(path, root=ROOT):
    resolved = Path(os.path.realpath(path))
    if resolved.is_relative_to(Path(os.path.realpath(root))):
        raise RuntimeError(f"{resolved} must be outside the checkout")
    return resolved


def rebuild_native(sdist_path, constraints_dir):
    """Build the native wheel from the unpacked sdist alone, outside the checkout."""
    directory = outside(tempfile.mkdtemp(prefix="gukhanmun-native-sdist-"))
    try:
        with tarfile.open(sdist_path) as archive:
            archive.extractall(directory, filter="data")
        source = next(directory.iterdir())
        shipped = directory / "Cargo.lock.shipped"
        shutil.copyfile(source / "Cargo.lock", shipped)
        environment = {key: value for key, value in os.environ.items()
                       if key not in CARGO_OVERRIDES and not key.startswith("PYO3_")
                       and key not in ("PYTHONPATH", "VIRTUAL_ENV", "GIT_DIR", "GIT_WORK_TREE")}
        environment["CARGO_TARGET_DIR"] = str(CARGO_TARGET)
        out = directory / "out"
        run(["uv", "build", "--wheel", *hashed_constraints(constraints_dir), "--out-dir", out, source],
            cwd=directory, env=environment)
        wheels = list(out.glob("*.whl"))
        if len(wheels) != 1:
            raise RuntimeError(f"the native sdist rebuilt into {wheels}")
        lock = check_lock_subset(shipped, source / "Cargo.lock")
        target = REBUILT / "gukhanmun"
        target.mkdir(parents=True)
        shutil.copy2(wheels[0], target / wheels[0].name)
        return target / wheels[0].name, lock
    finally:
        shutil.rmtree(directory, ignore_errors=True)


def check_data_record(record, version, directory=DICTIONARIES):
    """A data rebuild record must still describe the files on disk.

    Both the data sdist and the wheel rebuilt from it are compared with the
    hashes that py-dicts-build recorded, so a replaced or missing file cannot
    pass as a tested rebuild.
    """
    if record["version"] != version:
        raise RuntimeError(f"{record['sdist']} is not version {version}")
    for path, digest in ((directory / record["distribution"] / record["sdist"], record["sdist_sha256"]),
                         (directory / "rebuilt" / record["distribution"] / record["wheel"],
                          record["wheel_sha256"])):
        if not path.is_file() or sha256(path) != digest:
            raise RuntimeError(f"{path} is missing or changed after py-dicts-build recorded it")


def sdist_test(arguments, root=ROOT):
    version = core_version(root)
    archives = list(SDIST.glob("*.tar.gz"))
    if len(archives) != 1:
        raise RuntimeError("run py-sdist before py-sdist-test")
    native = archives[0]
    check_sdist(native, version)
    records_path = DICTIONARIES / "rebuilt" / "records.json"
    if not records_path.is_file():
        raise RuntimeError("run py-dicts-build before py-sdist-test")
    data_records = json.loads(records_path.read_text(encoding="utf-8"))
    shutil.rmtree(REBUILT, ignore_errors=True)
    REBUILT.mkdir(parents=True)
    wheel, lock = rebuild_native(native, REBUILT)
    records = [{"distribution": "gukhanmun", "sdist": native.name, "sdist_sha256": sha256(native),
                "wheel": wheel.name, "wheel_sha256": sha256(wheel), "cargo_lock": lock}, *data_records]
    for record in data_records:
        check_data_record(record, version)
    evidence = OUTPUT / "smoke" / "sdist-rebuild.json"
    smoke = Path(__file__).with_name("wheel_smoke.py")
    run([arguments.python, "-I", smoke, "run", "--artifacts", wheel.parent,
         *(DICTIONARIES / "rebuilt" / record["distribution"] for record in data_records),
         "--python", arguments.python, "--version", version, "--label", "sdist-rebuild",
         "--evidence", evidence, "--official", root / "tests/fixtures/dictionaries/official.json"])
    tested = json.loads(evidence.read_text(encoding="utf-8"))
    for record in records:
        if tested["staged"].get(record["wheel"]) != record["wheel_sha256"]:
            raise RuntimeError(f"the smoke run did not test the rebuilt {record['wheel']}")
    summary = {"schema": 1, "version": version, "evidence": evidence.name,
               "evidence_sha256": sha256(evidence), "rebuilds": records}
    (REBUILT / "rebuilds.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Rebuilt and tested {len(records)} sdists; records in {REBUILT / 'rebuilds.json'}")


def base_interpreter():
    """The interpreter the development venv was created from."""
    return Path(getattr(sys, "_base_executable", sys.executable))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("sdist", help="build and check the native sdist")
    test = commands.add_parser("sdist-test", help="rebuild every sdist outside the checkout and test it")
    test.add_argument("--python", type=Path, default=base_interpreter(), help="interpreter for the smoke run")
    arguments = parser.parse_args()
    if arguments.command == "sdist":
        sdist()
    else:
        sdist_test(arguments)


if __name__ == "__main__":
    main()
