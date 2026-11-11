# Gukhanmun: Verifies the Python dictionary data package layout and staging.
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

import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
import zlib

import pytest

REPOSITORY = Path(os.environ["GUKHANMUN_REPO_ROOT"])


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


tool = load("dictionary_packages_under_test", REPOSITORY / "scripts/python/dictionaries.py")
DISTRIBUTIONS = tool.load_table()


def test_table_describes_six_distinct_distributions() -> None:
    names = [distribution.name for distribution in DISTRIBUTIONS]
    assert names == [f"gukhanmun-{source}-{kind}" for source in ("stdict", "opendict", "loanwords")
                     for kind in ("fst", "cdb")]
    for distribution in DISTRIBUTIONS:
        assert distribution.package == distribution.name.replace("-", "_")
        assert distribution.role == ("loanword" if distribution.dictionary == "loanwords" else "reference")
        for resource in distribution.resources:
            assert resource.gzip == resource.name.endswith(".gz")
    categories = [r.category for d in DISTRIBUTIONS if d.dictionary == "opendict" for r in d.resources]
    assert categories == ["general", "north-korean", "dialect", "archaic"] * 2


@pytest.mark.parametrize("distribution", DISTRIBUTIONS, ids=lambda d: d.name)
def test_manifest_matches_the_table(distribution) -> None:
    project_dir = REPOSITORY / "python" / distribution.name
    manifest = tomllib.loads((project_dir / "pyproject.toml").read_text(encoding="utf-8"))
    project = manifest["project"]
    core = tomllib.loads((REPOSITORY / "python/gukhanmun/pyproject.toml").read_text(encoding="utf-8"))
    assert project["name"] == distribution.name
    assert project["version"] == core["project"]["version"]
    assert project["dependencies"] == []
    assert project["license"] == distribution.license
    assert project["license-files"] == list(tool.STAGED_TEXT)
    assert not any(c.startswith("License ::") for c in project.get("classifiers", []))
    assert manifest["build-system"] == {"requires": ["hatchling==1.33.0"], "build-backend": "hatchling.build"}
    targets = manifest["tool"]["hatch"]["build"]["targets"]
    assert targets["wheel"]["packages"] == [f"src/{distribution.package}"]
    package = f"src/{distribution.package}"
    staged = [f"{package}/{r.name}" for r in distribution.resources] + [f"{package}/provenance.json"]
    assert targets["wheel"]["force-include"] == {
        path: distribution.package + path.removeprefix(package) for path in staged}
    assert targets["sdist"]["force-include"] == {path: path for path in tool.staged_files(distribution)}
    initializer = (project_dir / package / "__init__.py").read_text(encoding="utf-8")
    assert "import" not in initializer.split('"""')[-1]


def test_build_group_pins_the_data_build_backend() -> None:
    root = tomllib.loads((REPOSITORY / "pyproject.toml").read_text(encoding="utf-8"))
    groups = root["dependency-groups"]
    assert groups["build"] == ["hatchling==1.33.0"]
    assert {"include-group": "build"} in groups["dev"]


def test_core_extras_pin_each_data_distribution_exactly() -> None:
    table = load("dictionary_table_under_test", REPOSITORY / "python/gukhanmun/src/gukhanmun/_dictionary_data.py")
    assert set(table.EXTRAS.values()) == {d.name for d in DISTRIBUTIONS}
    for distribution in DISTRIBUTIONS:
        assert table.EXTRAS[distribution.extra] == distribution.name
        assert table.EXTRAS[f"{distribution.dictionary}-{distribution.format}"] == distribution.name
        if distribution.format == "fst":
            assert table.EXTRAS[distribution.dictionary] == distribution.name
    core = tomllib.loads((REPOSITORY / "python/gukhanmun/pyproject.toml").read_text(encoding="utf-8"))
    version = core["project"]["version"]
    assert core["project"]["optional-dependencies"] == {
        extra: [f"{name}=={version}"] for extra, name in table.EXTRAS.items()}
    root = tomllib.loads((REPOSITORY / "pyproject.toml").read_text(encoding="utf-8"))
    assert root["tool"]["uv"]["sources"] == {d.name: {"workspace": True} for d in DISTRIBUTIONS}
    lock = tomllib.loads((REPOSITORY / "uv.lock").read_text(encoding="utf-8"))
    locked = next(p for p in lock["package"] if p["name"] == "gukhanmun")
    assert {extra: [entry["name"] for entry in entries]
            for extra, entries in locked["optional-dependencies"].items()} == {
        extra: [name] for extra, name in table.EXTRAS.items()}
    # The lock records workspace members as editable sources, not specifiers.
    assert sorted((entry["marker"], entry["name"]) for entry in locked["metadata"]["requires-dist"]) == sorted(
        (f"extra == '{extra}'", name) for extra, name in table.EXTRAS.items())
    assert locked["metadata"]["provides-extras"] == list(table.EXTRAS)


def test_staged_files_are_ignored_and_untracked() -> None:
    staged = [f"python/{d.name}/{name}" for d in DISTRIBUTIONS for name in tool.staged_files(d)]
    ignored = subprocess.run(["git", "check-ignore", "--no-index", "--stdin"], cwd=REPOSITORY,
                             input="\n".join(staged), capture_output=True, text=True)
    assert sorted(ignored.stdout.split()) == sorted(staged)
    kept = ["python/gukhanmun/LICENSE", "python/gukhanmun/pyproject.toml",
            *(f"python/{d.name}/{name}" for d in DISTRIBUTIONS
              for name in ("pyproject.toml", "README.md", ".gitignore", ".hgignore",
                           f"src/{d.package}/__init__.py"))]
    unignored = subprocess.run(["git", "check-ignore", "--no-index", "--stdin"], cwd=REPOSITORY,
                               input="\n".join(kept), capture_output=True, text=True)
    assert unignored.stdout == ""
    tracked = subprocess.run(["git", "ls-files", "--", *staged], cwd=REPOSITORY,
                             capture_output=True, text=True, check=True)
    assert tracked.stdout == ""


@pytest.mark.parametrize("distribution", DISTRIBUTIONS, ids=lambda d: d.name)
def test_local_vcs_ignore_files_stop_hatchling_search(distribution) -> None:
    # Hatchling copies the nearest .gitignore and .hgignore into the sdist,
    # searching parents up to the file system root for the latter.
    project = REPOSITORY / "python" / distribution.name
    gitignore = (project / ".gitignore").read_text(encoding="utf-8").splitlines()
    hgignore = (project / ".hgignore").read_text(encoding="utf-8").splitlines()
    patterns = [line for line in gitignore if line and not line.startswith("#")]
    assert "syntax: glob" in hgignore
    assert [line for line in hgignore[hgignore.index("syntax: glob") + 1:]] == [p.lstrip("/") for p in patterns]
    # Only the nearest ignore file applies, so it must also exclude bytecode
    # that running code from the source tree leaves behind.
    assert {"__pycache__/", "*.py[cod]"} <= set(patterns)


def test_bump_amends_every_python_manifest() -> None:
    tasks = tomllib.loads((REPOSITORY / "mise.toml").read_text(encoding="utf-8"))["tasks"]
    lines = tasks["bump-execute"]["run"].splitlines()
    add = next(line for line in lines if line.startswith("git add"))
    amend = next(line for line in lines if "git commit -S --amend" in line)
    assert "python/*/pyproject.toml" in add.split()
    assert "python/*/pyproject.toml" in amend.split()


def fake_repository(root: Path) -> None:
    """Copy the canonical sources staging reads, with tiny synthetic artifacts."""
    for name in ("LICENSE", "NOTICE.md"):
        shutil.copy2(REPOSITORY / name, root / name)
    for distribution in DISTRIBUTIONS:
        project = root / "python" / distribution.name
        project.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPOSITORY / "python" / distribution.name / "pyproject.toml", project)
        data = root / distribution.crate / "data"
        data.mkdir(parents=True, exist_ok=True)
        for name in ("metadata.json", "source.json"):
            if (REPOSITORY / distribution.crate / "data" / name).is_file():
                shutil.copy2(REPOSITORY / distribution.crate / "data" / name, data / name)
        if distribution.dictionary == "loanwords":
            for name in tool.STAGED_TEXT:
                shutil.copy2(REPOSITORY / distribution.crate / name, root / distribution.crate / name)
        if distribution.dictionary == "opendict":
            source = root / "packages" / f"opendict-{distribution.format}"
            source.mkdir(parents=True, exist_ok=True)
            for name in tool.STAGED_TEXT:
                shutil.copy2(REPOSITORY / "packages" / f"opendict-{distribution.format}" / name, source / name)
        for resource in distribution.resources:
            artifact = root / resource.artifact
            artifact.parent.mkdir(parents=True, exist_ok=True)
            payload = f"{resource.key}:{distribution.format}".encode()
            artifact.write_bytes(gzip.compress(payload, mtime=0) if resource.gzip else payload)


def test_stage_generates_resources_licenses_and_provenance(tmp_path: Path) -> None:
    fake_repository(tmp_path)
    tool.stage(tmp_path)
    for distribution in DISTRIBUTIONS:
        project = tmp_path / "python" / distribution.name
        package = project / "src" / distribution.package
        record = json.loads((package / "provenance.json").read_text(encoding="utf-8"))
        assert set(record) >= {"schema", "distribution", "version", "dictionary", "format", "source", "resources"}
        assert record["schema"] == 1
        assert record["distribution"] == distribution.name
        assert record["format"] == distribution.format
        assert ("snapshot" in record) == (distribution.dictionary != "loanwords")
        assert set(record["resources"]) == {r.name for r in distribution.resources}
        for resource in distribution.resources:
            data = (package / resource.name).read_bytes()
            assert data == (tmp_path / resource.artifact).read_bytes()
            entry = record["resources"][resource.name]
            plain = gzip.decompress(data) if resource.gzip else data
            assert entry["sha256"] == hashlib.sha256(data).hexdigest()
            assert entry["decoded_sha256"] == hashlib.sha256(plain).hexdigest()
            assert entry["encoding"] == ("gzip" if resource.gzip else "identity")
            assert entry.get("category") == resource.category
        attribution = (project / "ATTRIBUTION.md").read_text(encoding="utf-8")
        if distribution.dictionary == "stdict":
            assert (project / "LICENSE").read_bytes() == (REPOSITORY / "LICENSE").read_bytes()
            assert "GPL-3.0-only" in attribution
            assert "CC BY-SA 2.0 KR" in attribution
            assert "Snapshot:" in attribution
            assert "copyrightPolicy.do" in attribution
            assert "Open Korean Dictionary" not in attribution
        elif distribution.dictionary == "opendict":
            source = REPOSITORY / "packages" / f"opendict-{distribution.format}"
            assert attribution == (source / "ATTRIBUTION.md").read_text(encoding="utf-8")
        else:
            assert (project / "LICENSE").read_bytes() == (REPOSITORY / distribution.crate / "LICENSE").read_bytes()
            assert "NIKL" in attribution
    # Staging the same inputs again is byte-for-byte stable.
    first = {p: p.read_bytes() for p in (tmp_path / "python").rglob("*") if p.is_file()}
    tool.stage(tmp_path)
    assert {p: p.read_bytes() for p in (tmp_path / "python").rglob("*") if p.is_file()} == first


def test_stage_rejects_missing_artifacts_and_corrupt_gzip(tmp_path: Path) -> None:
    fake_repository(tmp_path)
    compressed = next(r for d in DISTRIBUTIONS for r in d.resources if r.gzip)
    (tmp_path / compressed.artifact).write_bytes(b"\x1f\x8b not gzip")
    with pytest.raises(RuntimeError, match="not valid gzip"):
        tool.stage(tmp_path)
    # A valid header with a corrupt deflate body raises zlib.error, not OSError.
    body = bytearray(gzip.compress(b"payload" * 64, mtime=0))
    body[10] = 0xff  # the first deflate block header: an invalid block type
    with pytest.raises(zlib.error):
        gzip.decompress(bytes(body))
    (tmp_path / compressed.artifact).write_bytes(bytes(body))
    with pytest.raises(RuntimeError, match="not valid gzip"):
        tool.stage(tmp_path)
    # Nothing is staged when any artifact is invalid.
    assert not any((tmp_path / "python").rglob("provenance.json"))
    fake_repository(tmp_path)
    (tmp_path / DISTRIBUTIONS[0].resources[0].artifact).unlink()
    with pytest.raises(RuntimeError, match="missing generated artifact"):
        tool.stage(tmp_path)


def test_notice_section_requires_its_snapshot_and_links(tmp_path: Path) -> None:
    fake_repository(tmp_path)
    notice = tmp_path / "NOTICE.md"
    text = notice.read_text(encoding="utf-8")
    section = tool.notice_section(tmp_path)
    assert section.startswith("The bundled dictionary data")
    assert "Open Korean Dictionary data" not in section
    notice.write_text(text.replace("[Creative Commons Attribution-ShareAlike 2.0 Korea]: ", "[unused]: ", 1),
                      encoding="utf-8")
    with pytest.raises(RuntimeError, match="link definitions"):
        tool.notice_section(tmp_path)


EXTRAS = load("dictionary_extras_under_test",
              REPOSITORY / "python/gukhanmun/src/gukhanmun/_dictionary_data.py").EXTRAS
CORE_METADATA = "Metadata-Version: 2.4\nName: gukhanmun\nVersion: 1.2.0.dev3\n" + "".join(
    f"Provides-Extra: {extra}\n" for extra in EXTRAS)


def requirement_lines(extras: dict[str, str], version: str) -> str:
    return "".join(f"Requires-Dist: {name}=={version} ; extra == '{extra}'\n" for extra, name in extras.items())


def test_core_wheel_extras_check_accepts_exact_pins_and_rejects_drift() -> None:
    checks = load("python_checks_under_test", REPOSITORY / "scripts/python/checks.py")
    extras = EXTRAS
    checks.check_extras(CORE_METADATA + requirement_lines(extras, "1.2.0.dev3"))
    for broken in [
        CORE_METADATA + requirement_lines(extras, "1.2.0"),
        CORE_METADATA + requirement_lines({**extras, "stdict": "gukhanmun-stdict-cdb"}, "1.2.0.dev3"),
        CORE_METADATA + requirement_lines(extras, "1.2.0.dev3") + "Requires-Dist: requests\n",
        CORE_METADATA + requirement_lines(extras, "1.2.0.dev3").replace(
            "gukhanmun-loanwords-cdb==1.2.0.dev3", "gukhanmun-loanwords-cdb>=1.2.0.dev3"),
        CORE_METADATA.replace("Provides-Extra: loanwords-cdb\n", "")
        + requirement_lines(extras, "1.2.0.dev3"),
    ]:
        with pytest.raises(RuntimeError):
            checks.check_extras(broken)
