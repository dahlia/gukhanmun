# Gukhanmun: Verifies wheel isolation, Python syntax and native stub signatures.
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

import os
from pathlib import Path
import subprocess
import sys
import tomllib

import pytest

REPOSITORY = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SCRIPT = REPOSITORY / "scripts" / "python" / "sync_version.py"


DATA = [f"gukhanmun-{source}-{kind}" for source in ("stdict", "opendict", "loanwords") for kind in ("fst", "cdb")]
DATA_SOURCE = '[project]\nname = "{name}"\nversion = "0.4.0"\n'
EXTRAS = {"stdict": "stdict-fst", "stdict-fst": "stdict-fst", "stdict-cdb": "stdict-cdb",
          "opendict": "opendict-fst", "opendict-fst": "opendict-fst", "opendict-cdb": "opendict-cdb",
          "loanwords": "loanwords-fst", "loanwords-fst": "loanwords-fst", "loanwords-cdb": "loanwords-cdb"}
EXTRAS_SOURCE = "\n[project.optional-dependencies]\n" + "".join(
    f'{extra} = ["gukhanmun-{data}==0.4.0"]  # pinned\n' for extra, data in EXTRAS.items())


def project(root: Path, version: str) -> tuple[Path, str]:
    (root / "Cargo.toml").write_text('[workspace.package]\nversion = "' + version + '"\n')
    manifest = root / "python" / "gukhanmun" / "pyproject.toml"
    manifest.parent.mkdir(parents=True)
    source = ('# Keep this comment\n[project]\nname = "gukhanmun"\nversion = "0.4.0" # release version\n'
              + EXTRAS_SOURCE + '\n[tool.example]\nversion = "unchanged"\npin = "gukhanmun-stdict-fst==0.4.0"\n')
    manifest.write_text(source)
    for name in DATA:
        data = root / "python" / name / "pyproject.toml"
        data.parent.mkdir(parents=True)
        data.write_text(DATA_SOURCE.format(name=name))
    return manifest, source


def data_versions(root: Path) -> set[str]:
    return {tomllib.loads((root / "python" / name / "pyproject.toml").read_text())["project"]["version"]
            for name in DATA}


def invoke(root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run([sys.executable, "-I", SCRIPT, root], text=True, capture_output=True)


@pytest.mark.parametrize("cargo,python", [
    ("0.4.1", "0.4.1"), ("1.2.0-rc.2", "1.2.0rc2"),
    ("1.2.0-beta.3", "1.2.0b3"), ("1.2.0-dev.7", "1.2.0.dev7"),
])
def test_sync_preserves_other_manifest_text(tmp_path: Path, cargo: str, python: str) -> None:
    manifest, original = project(tmp_path, cargo)
    result = invoke(tmp_path)
    assert result.returncode == 0, result.stderr
    expected = original.replace('version = "0.4.0" #', 'version = "' + python + '" #')
    expected = expected.replace('==0.4.0"]', '==' + python + '"]')
    assert manifest.read_text() == expected
    assert tomllib.loads(manifest.read_text())["project"]["version"] == python
    assert tomllib.loads(manifest.read_text())["project"]["optional-dependencies"] == {
        extra: [f"gukhanmun-{data}=={python}"] for extra, data in EXTRAS.items()}
    assert data_versions(tmp_path) == {python}
    again = invoke(tmp_path)
    assert again.returncode == 0
    assert manifest.read_text() == expected


@pytest.mark.parametrize("version", ["1.2.0-preview-special", "1.2.0-1", "1.2.0-post.1"])
def test_unrepresentable_workspace_version_does_not_edit_manifest(tmp_path: Path, version: str) -> None:
    manifest, original = project(tmp_path, version)
    result = invoke(tmp_path)
    assert result.returncode != 0
    assert manifest.read_text() == original
    assert data_versions(tmp_path) == {"0.4.0"}


def test_invalid_data_manifest_leaves_every_manifest_unchanged(tmp_path: Path) -> None:
    manifest, original = project(tmp_path, "0.4.1")
    broken = tmp_path / "python" / DATA[-1] / "pyproject.toml"
    broken.write_text('[project]\nname = "x"\ndynamic = ["version"]\n')
    result = invoke(tmp_path)
    assert result.returncode != 0
    assert DATA[-1] in result.stderr
    assert manifest.read_text() == original
    assert {tomllib.loads((tmp_path / "python" / name / "pyproject.toml").read_text())["project"]["version"]
            for name in DATA[:-1]} == {"0.4.0"}


@pytest.mark.parametrize("change", [
    lambda text: text.replace('loanwords-cdb = ["gukhanmun-loanwords-cdb==0.4.0"]  # pinned\n', ""),
    lambda text: text.replace('"gukhanmun-stdict-cdb==0.4.0"', '"gukhanmun-stdict-cdb>=0.4.0"'),
    lambda text: text.replace('opendict = ["gukhanmun-opendict-fst==0.4.0"]', 'opendict = ["gukhanmun-opendict-cdb==0.4.0"]'),
    lambda text: text.replace("[tool.example]", 'extra = ["requests"]\n\n[tool.example]'),
    lambda text: text.replace(EXTRAS_SOURCE, ""),
], ids=["missing", "range", "wrong-data", "unexpected", "no-table"])
def test_malformed_extras_leave_every_manifest_unchanged(tmp_path: Path, change) -> None:
    manifest, original = project(tmp_path, "0.4.1")
    broken = change(original)
    assert broken != original
    manifest.write_text(broken)
    result = invoke(tmp_path)
    assert result.returncode != 0
    assert manifest.read_text() == broken
    assert data_versions(tmp_path) == {"0.4.0"}


def test_crlf_manifests_are_synchronized(tmp_path: Path) -> None:
    manifest, original = project(tmp_path, "0.4.1")
    for path in [manifest, *(tmp_path / "python" / name / "pyproject.toml" for name in DATA)]:
        path.write_bytes(path.read_bytes().replace(b"\r\n", b"\n").replace(b"\n", b"\r\n"))
    result = invoke(tmp_path)
    assert result.returncode == 0, result.stderr
    assert tomllib.loads(manifest.read_text())["project"]["version"] == "0.4.1"
    assert data_versions(tmp_path) == {"0.4.1"}


def test_dynamic_python_version_is_rejected(tmp_path: Path) -> None:
    manifest, original = project(tmp_path, "0.4.1")
    source = original.replace('version = "0.4.0" # release version', 'dynamic = ["version"]')
    manifest.write_text(source)
    result = invoke(tmp_path)
    assert result.returncode != 0
    assert "static project.version" in result.stderr
    assert manifest.read_text() == source


def test_bump_task_refreshes_and_amends_the_python_manifest_and_lock() -> None:
    tasks = tomllib.loads((REPOSITORY / "mise.toml").read_text())["tasks"]
    assert "mise run py-sync-version" in tasks["bump-execute"]["run"]
    amend = next(line for line in tasks["bump-execute"]["run"].splitlines() if "git commit -S --amend" in line)
    assert "python/*/pyproject.toml" in amend.split()
    assert "uv.lock" in amend.split()
    assert any("uv lock" in line and "--no-build" in line for line in tasks["py-sync-version"]["run"])
