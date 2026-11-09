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


def project(root: Path, version: str) -> tuple[Path, str]:
    (root / "Cargo.toml").write_text('[workspace.package]\nversion = "' + version + '"\n')
    manifest = root / "python" / "gukhanmun" / "pyproject.toml"
    manifest.parent.mkdir(parents=True)
    source = '# Keep this comment\n[project]\nname = "gukhanmun"\nversion = "0.4.0" # release version\n\n[tool.example]\nversion = "unchanged"\n'
    manifest.write_text(source)
    return manifest, source


def invoke(root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run([sys.executable, "-I", SCRIPT, root], text=True, capture_output=True)


@pytest.mark.parametrize("cargo,python", [
    ("0.4.1", "0.4.1"), ("1.2.0-rc.2", "1.2.0rc2"),
    ("1.2.0-beta.3", "1.2.0b3"),
])
def test_sync_preserves_other_manifest_text(tmp_path: Path, cargo: str, python: str) -> None:
    manifest, original = project(tmp_path, cargo)
    result = invoke(tmp_path)
    assert result.returncode == 0, result.stderr
    assert manifest.read_text() == original.replace('version = "0.4.0"', 'version = "' + python + '"')
    assert tomllib.loads(manifest.read_text())["project"]["version"] == python
    again = invoke(tmp_path)
    assert again.returncode == 0
    assert manifest.read_text() == original.replace('version = "0.4.0"', 'version = "' + python + '"')


@pytest.mark.parametrize("version", ["1.2.0-preview-special", "1.2.0-1", "1.2.0-post.1"])
def test_unrepresentable_workspace_version_does_not_edit_manifest(tmp_path: Path, version: str) -> None:
    manifest, original = project(tmp_path, version)
    result = invoke(tmp_path)
    assert result.returncode != 0
    assert manifest.read_text() == original


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
    assert "python/gukhanmun/pyproject.toml" in amend.split()
    assert "uv.lock" in amend.split()
    assert any("uv lock" in line and "--no-build" in line for line in tasks["py-sync-version"]["run"])
