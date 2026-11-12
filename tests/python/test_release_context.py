# Gukhanmun: Tests publication event identity and version derivation.
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

import importlib.util
import os
from pathlib import Path

import pytest
from packaging.version import Version

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SPEC = importlib.util.spec_from_file_location("release_context_under_test", ROOT / "scripts/python/release_context.py")
context = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(context)


def event(**overrides):
    return {"GITHUB_REPOSITORY": "dahlia/gukhanmun", "GITHUB_EVENT_NAME": "push",
            "GITHUB_REF_TYPE": "branch", "GITHUB_REF": "refs/heads/main",
            "GITHUB_RUN_NUMBER": "12", **overrides}


def workspace(tmp_path, version="0.4.0"):
    (tmp_path / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{version}"\n')
    return tmp_path


@pytest.mark.parametrize("ref", ["refs/heads/main", "refs/heads/0.4-maintenance", "refs/heads/12.34-maintenance"])
def test_eligible_branch_uses_run_number(tmp_path, ref):
    result = context.resolve(event(GITHUB_REF=ref), workspace(tmp_path))
    assert result["eligible"] is True
    assert result["version"] == "0.4.0.dev12"
    assert "+" not in result["version"]
    assert context.resolve(event(GITHUB_REF=ref, GITHUB_RUN_ATTEMPT="2"), tmp_path) == result


@pytest.mark.parametrize("overrides", [
    {"GITHUB_REPOSITORY": "fork/gukhanmun"},
    {"GITHUB_EVENT_NAME": "pull_request"},
    {"GITHUB_EVENT_NAME": "pull_request_target"},
    {"GITHUB_REF": "refs/pull/52/merge"},
    {"GITHUB_REF": "refs/heads/ci/pypi-publish"},
    {"GITHUB_REF": "refs/heads/x-maintenance"},
    {"GITHUB_REF": "refs/heads/main/extra"},
    {"GITHUB_REF": "refs/heads/0.4.0"},
    {"GITHUB_REF": "refs/tags/main"},
    {"GITHUB_REF_TYPE": "tag", "GITHUB_REF": "refs/heads/main"},
])
def test_nonpublishing_events_keep_dev0(tmp_path, overrides):
    result = context.resolve(event(**overrides), workspace(tmp_path))
    assert result["eligible"] is False
    assert result["reason"]
    assert result["version"] == ("0.4.0" if overrides.get("GITHUB_REF_TYPE") == "tag" else "0.4.0.dev0")


@pytest.mark.parametrize("number", ["", "0", "-1", "01", "1.0", "one", "12\ninjected=yes"])
def test_invalid_run_identity_is_not_publishable(tmp_path, number):
    result = context.resolve(event(GITHUB_RUN_NUMBER=number), workspace(tmp_path))
    assert not result["eligible"]
    assert "positive" in result["reason"]
    assert result["version"] == "0.4.0.dev0"


@pytest.mark.parametrize("overrides,eligible", [
    ({}, True),
    ({"GITHUB_REF": "refs/tags/0.4.1"}, False),
    ({"GITHUB_REF": "refs/tags/v0.4.0"}, False),
    ({"GITHUB_REF": "refs/tags/00.4.0"}, False),
    ({"GITHUB_REF": "refs/tags/0.4.0/extra"}, False),
    ({"GITHUB_REPOSITORY": "fork/gukhanmun"}, False),
    ({"GITHUB_EVENT_NAME": "pull_request"}, False),
])
def test_tags_require_exact_workspace_version_and_repository(tmp_path, overrides, eligible):
    environment = event(GITHUB_REF_TYPE="tag", GITHUB_REF="refs/tags/0.4.0")
    environment.update(overrides)
    result = context.resolve(environment, workspace(tmp_path))
    assert result["eligible"] is eligible
    assert result["version"] == "0.4.0"


@pytest.mark.parametrize("cargo,expected", [("1.2.0-rc.2", "1.2.0rc2.dev12"), ("1.2.0-beta.3", "1.2.0b3.dev12")])
def test_prerelease_bases_use_pep440(tmp_path, cargo, expected):
    result = context.resolve(event(), workspace(tmp_path, cargo))
    assert result["eligible"]
    assert result["version"] == expected


def test_already_development_base_builds_but_cannot_publish(tmp_path):
    result = context.resolve(event(), workspace(tmp_path, "0.4.0-dev.7"))
    assert result["version"] == "0.4.0.dev7"
    assert not result["eligible"]
    assert "another .devN" in result["reason"]


def test_local_labels_cannot_become_artifact_versions(tmp_path):
    with pytest.raises(ValueError, match="local label"):
        context.resolve(event(), workspace(tmp_path, "0.4.0+sha"))


def test_version_ordering_matches_the_release_cycle():
    assert Version("0.3.1") < Version("0.4.0.dev0") < Version("0.4.0.dev12") < Version("0.4.0rc1") < Version("0.4.0")
