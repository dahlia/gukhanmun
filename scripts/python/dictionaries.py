# Gukhanmun: Stages, builds and inspects the Python dictionary data packages.
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

"""Python dictionary data packages; invoked only through the mise tasks.

`stage` copies the artifacts that the existing `*-data` tasks generate under
*packages/* into each resource package, together with generated license,
attribution and provenance files.  `build` turns the six staged packages into
wheels and sdists and checks them, including a rebuild of every sdist outside
the checkout; it keeps each rebuilt wheel under *rebuilt/* with a record of
its sdist.  `test` resolves and installs the core wheel through its extras
from those local artifacts alone and runs *tests/python-dicts/* against the
installed official data.  No step runs npm or downloads
dictionary sources.
"""

import argparse
import email.parser
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile
import zlib

ROOT = Path(__file__).resolve().parents[2]
TABLE = ROOT / "python" / "gukhanmun" / "src" / "gukhanmun" / "_dictionary_data.py"
OUTPUT = ROOT / "target" / "python" / "dictionaries"
# Wheels rebuilt from the sdists, kept apart from the deliverables.
REBUILT = OUTPUT / "rebuilt"
# PyPI's default per-file upload limit.
SIZE_LIMIT = 100 * 1024 * 1024
STAGED_TEXT = ("LICENSE", "ATTRIBUTION.md")
VCS_IGNORE = (".gitignore", ".hgignore")
NOTICE_SECTION = "Standard Korean Language Dictionary data"


def load_module(path=TABLE):
    """Load the private table by path, without importing gukhanmun._native."""
    spec = importlib.util.spec_from_file_location("_gukhanmun_dictionary_data", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def load_table(path=TABLE):
    return load_module(path).DISTRIBUTIONS


def project_dir(root, distribution):
    return root / "python" / distribution.name


def package_dir(root, distribution):
    return project_dir(root, distribution) / "src" / distribution.package


def manifest_version(root, distribution):
    manifest = project_dir(root, distribution) / "pyproject.toml"
    return tomllib.loads(manifest.read_text(encoding="utf-8"))["project"]["version"]


def write_atomic(path, data):
    """Replace a file only after its complete contents are written."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = tempfile.NamedTemporaryFile(dir=path.parent, prefix=f".{path.name}.", delete=False)
    pending = Path(temporary.name)
    try:
        with temporary:
            temporary.write(data)
        os.replace(pending, path)
    finally:
        # A failed write, close or replace must not leave a partial file in
        # the package source, where the next build would ship it.
        pending.unlink(missing_ok=True)


def text_bytes(text):
    # Pin encoding and newlines so Windows and POSIX stage identical bytes.
    return text.encode("utf-8")


def json_bytes(value):
    return text_bytes(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n")


def decoded(resource, data):
    """Return the bytes the core loader receives for a shipped resource."""
    if not resource.gzip:
        return data
    if not data:
        # gzip.decompress() accepts empty input, but the runtime loader
        # rejects an empty compressed resource as damaged.
        raise EOFError("the compressed resource is empty")
    return gzip.decompress(data)


def notice_section(root):
    """Extract the stdict data section of NOTICE.md with its link definitions."""
    text = (root / "NOTICE.md").read_text(encoding="utf-8").replace("\r\n", "\n")
    heading = f"\n{NOTICE_SECTION}\n" + "-" * len(NOTICE_SECTION) + "\n"
    start = text.find(heading)
    if start < 0:
        raise RuntimeError(f"NOTICE.md has no {NOTICE_SECTION!r} section")
    body = text[start + len(heading):]
    following = re.search(r"\n\n\n[^\n]+\n-+\n", body)
    if following is not None:
        body = body[:following.start()]
    body = body.strip("\n")
    if "Snapshot:" not in body:
        raise RuntimeError("the NOTICE.md stdict section has no Snapshot: line")
    references = set(re.findall(r"\[([^\]]+)\](?!\()", body))
    defined = set(re.findall(r"(?m)^\[([^\]]+)\]: \S+", body))
    if not defined or not references <= defined:
        raise RuntimeError("the NOTICE.md stdict section lacks link definitions: "
                           + ", ".join(sorted(references - defined)))
    return body


def stdict_attribution(root, distribution):
    kind = distribution.format.upper()
    title = f"Standard Korean Language Dictionary {kind} package license and attribution"
    return (
        f"{title}\n{'=' * len(title)}\n\n"
        "This package contains Gukhanmun package code and bundled *Standard Korean\n"
        f"Language Dictionary* (標準國語大辭典) {kind} data.\n\n\n"
        "Code license\n------------\n\n"
        "The Gukhanmun package code is licensed under GPL-3.0-only. See *LICENSE* for\n"
        "the full license text.\n\n\n"
        "Data attribution\n----------------\n\n"
        f"{notice_section(root)}\n"
    )


def license_files(root, distribution):
    """Generate license material from its canonical repository sources."""
    if distribution.dictionary == "stdict":
        return {"LICENSE": (root / "LICENSE").read_bytes(),
                "ATTRIBUTION.md": text_bytes(stdict_attribution(root, distribution))}
    if distribution.dictionary == "opendict":
        source = root / "packages" / f"opendict-{distribution.format}"
    else:
        source = root / distribution.crate
    return {name: (source / name).read_bytes() for name in STAGED_TEXT}


def provenance(root, distribution, payloads):
    crate = root / distribution.crate / "data"
    record = {
        "schema": 1,
        "distribution": distribution.name,
        "version": manifest_version(root, distribution),
        "dictionary": distribution.dictionary,
        "format": distribution.format,
        "source": json.loads((crate / "metadata.json").read_text(encoding="utf-8")),
        "resources": {},
    }
    snapshot = crate / "source.json"
    if snapshot.is_file():
        source = json.loads(snapshot.read_text(encoding="utf-8"))
        record["snapshot"] = {key: source[key] for key in
                              ("archive_filename", "archive_date", "archive_sha256", "inputs_sha256")
                              if key in source}
    for resource in distribution.resources:
        data = payloads[resource.name]
        plain = decoded(resource, data)
        entry = {"size": len(data), "sha256": hashlib.sha256(data).hexdigest(),
                 "encoding": "gzip" if resource.gzip else "identity",
                 "decoded_size": len(plain), "decoded_sha256": hashlib.sha256(plain).hexdigest()}
        if resource.category is not None:
            entry["category"] = resource.category
        record["resources"][resource.name] = entry
    return record


def staged_files(distribution):
    """Map each staged path, relative to the project, to its role."""
    package = f"src/{distribution.package}"
    files = [f"{package}/{resource.name}" for resource in distribution.resources]
    return [*files, f"{package}/provenance.json", *STAGED_TEXT]


def stage(root=ROOT):
    distributions = load_table()
    # Read and validate every artifact before writing anything.
    staged = []
    for distribution in distributions:
        payloads = {}
        for resource in distribution.resources:
            artifact = root / resource.artifact
            if not artifact.is_file():
                raise RuntimeError(f"missing generated artifact {resource.artifact}; run its *-data task")
            payloads[resource.name] = artifact.read_bytes()
            try:
                decoded(resource, payloads[resource.name])
            except (OSError, EOFError, zlib.error) as error:
                raise RuntimeError(f"{resource.artifact} is not valid gzip data: {error}") from error
        staged.append((distribution, payloads, provenance(root, distribution, payloads),
                       license_files(root, distribution)))
    for distribution, payloads, record, licenses in staged:
        target = package_dir(root, distribution)
        for name, data in payloads.items():
            write_atomic(target / name, data)
        write_atomic(target / "provenance.json", json_bytes(record))
        for name, data in licenses.items():
            write_atomic(project_dir(root, distribution) / name, data)
        print(f"Staged {distribution.name}: {', '.join(payloads)}")


def run(command, *, cwd=ROOT, env=None):
    print("+", " ".join(map(str, command)), flush=True)
    subprocess.run(list(map(str, command)), cwd=cwd, env=env, check=True)


def remove_tree(path):
    # Windows can briefly hold handles after a child process exits.
    shutil.rmtree(path, ignore_errors=True)


def metadata(text):
    return email.parser.Parser().parsestr(text)


def wheel_contents(path):
    with zipfile.ZipFile(path) as wheel:
        names = wheel.namelist()
        if len(names) != len(set(names)):
            raise RuntimeError(f"{path.name} has duplicate members")
        return {name: wheel.read(name) for name in names}


def check_wheel(root, distribution, wheel, version):
    contents = wheel_contents(wheel)
    info = f"{distribution.package}-{version}.dist-info"
    package = distribution.package
    expected = {f"{package}/__init__.py", f"{package}/provenance.json",
                *(f"{package}/{resource.name}" for resource in distribution.resources),
                f"{info}/METADATA", f"{info}/WHEEL", f"{info}/RECORD",
                f"{info}/licenses/LICENSE", f"{info}/licenses/ATTRIBUTION.md"}
    if set(contents) != expected:
        raise RuntimeError(f"{wheel.name} members differ: extra {sorted(set(contents) - expected)}, "
                           f"missing {sorted(expected - set(contents))}")
    tags = metadata(contents[f"{info}/WHEEL"].decode("utf-8"))
    if tags["Root-Is-Purelib"] != "true" or tags.get_all("Tag") != ["py3-none-any"]:
        raise RuntimeError(f"{wheel.name} is not a pure py3-none-any wheel")
    fields = metadata(contents[f"{info}/METADATA"].decode("utf-8"))
    if (fields["Name"] != distribution.name or fields["Version"] != version
            or fields["License-Expression"] != distribution.license
            or fields.get_all("Requires-Dist") is not None
            or sorted(fields.get_all("License-File") or []) != sorted(STAGED_TEXT)):
        raise RuntimeError(f"{wheel.name} has unexpected metadata")
    staged = package_dir(root, distribution)
    for resource in distribution.resources:
        if contents[f"{package}/{resource.name}"] != (staged / resource.name).read_bytes():
            raise RuntimeError(f"{wheel.name} {resource.name} differs from the staged file")
    record = json.loads(contents[f"{package}/provenance.json"])
    if record["version"] != version:
        raise RuntimeError(f"{wheel.name} provenance version {record['version']} != {version}")
    for resource in distribution.resources:
        data = contents[f"{package}/{resource.name}"]
        entry = record["resources"][resource.name]
        plain = decoded(resource, data)
        if (entry["sha256"] != hashlib.sha256(data).hexdigest()
                or entry["decoded_sha256"] != hashlib.sha256(plain).hexdigest()
                or entry["size"] != len(data) or entry["decoded_size"] != len(plain)):
            raise RuntimeError(f"{wheel.name} provenance does not match {resource.name}")
    return contents


def check_sdist(distribution, sdist, version):
    prefix = f"{distribution.package}-{version}"
    with tarfile.open(sdist) as archive:
        names = [member.name for member in archive.getmembers() if member.isfile()]
    if len(names) != len(set(names)):
        raise RuntimeError(f"{sdist.name} has duplicate members")
    # Hatchling adds the nearest VCS ignore files to an sdist; each project
    # keeps its own so nothing from parent directories leaks into it.
    expected = {f"{prefix}/{name}" for name in ("pyproject.toml", "PKG-INFO", "README.md", "hatch_build.py", *VCS_IGNORE,
                                                f"src/{distribution.package}/__init__.py",
                                                *staged_files(distribution))}
    if set(names) != expected:
        raise RuntimeError(f"{sdist.name} members differ: extra {sorted(set(names) - expected)}, "
                           f"missing {sorted(expected - set(names))}")


def build(root=ROOT):
    distributions = load_table()
    versions = {distribution.name: manifest_version(root, distribution) for distribution in distributions}
    core = tomllib.loads((root / "python/gukhanmun/pyproject.toml").read_text(encoding="utf-8"))
    if set(versions.values()) != {core["project"]["version"]}:
        raise RuntimeError(f"Python manifest versions disagree: {versions}")
    for distribution in distributions:
        for name in staged_files(distribution):
            if not (project_dir(root, distribution) / name).is_file():
                raise RuntimeError(f"{distribution.name} is not staged ({name}); run py-dicts-stage")
    remove_tree(OUTPUT)
    OUTPUT.mkdir(parents=True)
    constraints = OUTPUT / "build-constraints.txt"
    run(["uv", "export", "--quiet", "--locked", "--only-group", "build", "--no-emit-workspace",
         "--output-file", constraints])
    hashed = ["--build-constraints", constraints, "--require-build-hashes",
              "--preview-features", "build-dependency-hashes"]
    wheels = {}
    records = []
    for distribution in distributions:
        version = versions[distribution.name]
        out = OUTPUT / distribution.name
        # Without --sdist/--wheel, uv builds the sdist and then the wheel from it.
        run(["uv", "build", *hashed, "--out-dir", out, project_dir(root, distribution)])
        sdists, built = list(out.glob("*.tar.gz")), list(out.glob("*.whl"))
        stem = f"{distribution.package}-{version}"
        if [p.name for p in sdists] != [f"{stem}.tar.gz"] or [p.name for p in built] != [f"{stem}-py3-none-any.whl"]:
            raise RuntimeError(f"unexpected artifacts for {distribution.name}: {sorted(p.name for p in out.iterdir())}")
        contents = check_wheel(root, distribution, built[0], version)
        check_sdist(distribution, sdists[0], version)
        for artifact in (sdists[0], built[0]):
            size = artifact.stat().st_size
            if size >= SIZE_LIMIT:
                raise RuntimeError(f"{artifact.name} is {size} bytes, over the PyPI file limit")
            print(f"{artifact.name}: {size} bytes")
        records.append(rebuild(distribution, sdists[0], contents, hashed, version))
        wheels[distribution.name] = built[0]
    resource_only(distributions, wheels)
    # py-sdist-test installs these rebuilt wheels; the records tie each one to
    # the sdist it came from.
    write_atomic(REBUILT / "records.json", json_bytes(records))
    print(f"Verified {len(wheels)} data wheels and sdists in {OUTPUT}")


def sha256_file(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rebuild(distribution, sdist, original, hashed, version):
    """Build the wheel again from the sdist alone, outside the checkout.

    The validated wheel is kept under *rebuilt/* and described by a record
    linking it to the hash of the sdist it was built from.
    """
    directory = Path(tempfile.mkdtemp(prefix="gukhanmun-sdist-"))
    try:
        copy = directory / sdist.name
        shutil.copy2(sdist, copy)
        run(["uv", "build", "--wheel", *hashed, "--out-dir", directory / "out", copy], cwd=directory)
        rebuilt = list((directory / "out").glob("*.whl"))
        if len(rebuilt) != 1:
            raise RuntimeError(f"rebuilding {sdist.name} produced {rebuilt}")
        contents = wheel_contents(rebuilt[0])
        info = f"{distribution.package}-{version}.dist-info"
        for name in ("RECORD", "METADATA"):
            if contents[f"{info}/{name}"] != original[f"{info}/{name}"]:
                raise RuntimeError(f"the wheel rebuilt from {sdist.name} has a different {name}")
        target = REBUILT / distribution.name / rebuilt[0].name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(rebuilt[0], target)
        return {"distribution": distribution.name, "version": version, "sdist": sdist.name,
                "sdist_sha256": sha256_file(sdist), "wheel": target.name, "wheel_sha256": sha256_file(target)}
    finally:
        remove_tree(directory)


PROBE = """
import hashlib, importlib.resources, importlib.util, json, sys
assert importlib.util.find_spec("gukhanmun") is None, "the native package must not be installed"
for package, names in json.loads(sys.argv[1]).items():
    files = importlib.resources.files(package)
    record = json.loads(files.joinpath("provenance.json").read_bytes())
    for name in names:
        data = files.joinpath(name).read_bytes()
        assert hashlib.sha256(data).hexdigest() == record["resources"][name]["sha256"], name
print("resource-only import verified")
"""


def resource_only(distributions, wheels):
    """Install only the data wheels and read every resource through importlib."""
    directory = Path(tempfile.mkdtemp(prefix="gukhanmun-data-only-"))
    try:
        run(["uv", "venv", "--quiet", "--python", sys.executable, directory / "venv"])
        python = directory / "venv" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        run(["uv", "pip", "install", "--quiet", "--python", python, "--no-deps", "--no-index", *wheels.values()])
        names = {d.package: [resource.name for resource in d.resources] for d in distributions}
        environment = {k: v for k, v in os.environ.items() if k not in ("PYTHONPATH", "PYTHONHOME")}
        run([python, "-I", "-c", PROBE, json.dumps(names)], cwd=directory, env=environment)
    finally:
        remove_tree(directory)


WHEELS = ROOT / "target" / "python" / "wheels"

EXTRA_PROBE = """
import importlib.util, sys
import gukhanmun
from gukhanmun import Converter, ErrorCode, GukhanmunError
from gukhanmun.dictionaries import stdict
for package in sys.argv[1:]:
    assert importlib.util.find_spec(package) is None, package
assert Converter().convert("漢字") == "한자"
try:
    stdict()
except GukhanmunError as error:
    assert error.code is ErrorCode.DICTIONARY_LOAD
    assert "pip install 'gukhanmun[stdict]'" in error.message
else:
    raise AssertionError("stdict() loaded data that is not installed")
print("base install verified")
"""


def venv(directory):
    run(["uv", "venv", "--quiet", "--python", sys.executable, directory])
    return directory / ("Scripts/python.exe" if os.name == "nt" else "bin/python")


def resolve_extras(version, links):
    """Resolve each extra from the local artifacts alone, as an index would."""
    from packaging.requirements import Requirement

    cases = {**{extra: {"gukhanmun", name} for extra, name in load_module().EXTRAS.items()}, None: {"gukhanmun"}}
    for extra, expected in cases.items():
        requirement = f"gukhanmun[{extra}]=={version}" if extra else f"gukhanmun=={version}"
        result = subprocess.run(
            ["uv", "pip", "compile", "--quiet", "--no-header", "--no-annotate", "--no-index",
             *(argument for link in links for argument in ("--find-links", link)), "-"],
            input=requirement + "\n", text=True, encoding="utf-8", capture_output=True, check=False)
        if result.returncode != 0:
            raise RuntimeError(f"could not resolve {requirement}:\n{result.stderr}")
        pins = [Requirement(line) for line in result.stdout.splitlines() if line and not line.startswith("#")]
        if ({pin.name for pin in pins} != expected
                or any(str(pin.specifier) != f"=={version}" for pin in pins)):
            raise RuntimeError(f"{requirement} resolved to {result.stdout.strip()!r}")
        print(f"{requirement} -> {', '.join(sorted(str(pin) for pin in pins))}")


def test(root=ROOT):
    """Install the core wheel through its extras and test the official data."""
    distributions = load_table()
    version = manifest_version(root, distributions[0])
    links = [WHEELS, *(OUTPUT / distribution.name for distribution in distributions)]
    if not list(WHEELS.glob("*.whl")):
        raise RuntimeError("run py-build before py-dicts-test")
    for link in links[1:]:
        if not list(link.glob("*.whl")):
            raise RuntimeError("run py-dicts-build before py-dicts-test")
    resolve_extras(version, links)
    find_links = [argument for link in links for argument in ("--find-links", link)]
    environment = {k: v for k, v in os.environ.items() if k not in ("PYTHONPATH", "MYPYPATH", "PYTHONHOME")}
    directory = Path(tempfile.mkdtemp(prefix="gukhanmun-official-"))
    try:
        # The core package alone: no data, and the helpers explain how to add it.
        python = venv(directory / "base")
        run(["uv", "pip", "install", "--quiet", "--python", python, "--no-index", *find_links,
             f"gukhanmun=={version}"])
        run([python, "-I", "-c", EXTRA_PROBE, *(d.package for d in distributions)],
            cwd=directory, env=environment)
        # Every extra together, with the locked test dependencies.
        python = venv(directory / "official")
        requirements = directory / "test-requirements.txt"
        run(["uv", "export", "--quiet", "--locked", "--only-group", "test", "--no-emit-workspace",
             "--output-file", requirements])
        run(["uv", "pip", "install", "--quiet", "--python", python, "--require-hashes", "-r", requirements])
        extras = ",".join(sorted({distribution.extra for distribution in distributions}))
        run(["uv", "pip", "install", "--quiet", "--python", python, "--no-index", *find_links,
             f"gukhanmun[{extras}]=={version}"])
        tests = directory / "tests"
        shutil.copytree(root / "tests" / "python-dicts", tests)
        environment["GUKHANMUN_REPO_ROOT"] = str(root)
        run([python, "-I", "-m", "pytest", "-q", tests], cwd=directory, env=environment)
    finally:
        remove_tree(directory)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("stage", "build", "test"))
    arguments = parser.parse_args()
    {"stage": stage, "build": build, "test": test}[arguments.command]()


if __name__ == "__main__":
    main()
