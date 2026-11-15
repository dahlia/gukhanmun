# Gukhanmun: Tests isolated CLI identities and semantic native stamp policy.
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

import copy
import importlib.util
import os
from pathlib import Path
import subprocess
import sys

import pytest

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SPEC = importlib.util.spec_from_file_location("cli_context_test", ROOT / "scripts/python/cli_context.py")
context = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(context)


def event(**overrides):
    return {"GITHUB_REPOSITORY": "dahlia/gukhanmun", "GITHUB_EVENT_NAME": "push",
            "GITHUB_REF_TYPE": "branch", "GITHUB_REF": "refs/heads/main",
            "GITHUB_RUN_NUMBER": "42", "GITHUB_SHA": "a" * 40, **overrides}


@pytest.mark.parametrize("base", ["0.4.0", "0.4.0-rc.2", "0.4.0-dev.7"])
@pytest.mark.parametrize("ref", ["refs/heads/main", "refs/heads/0.4-maintenance"])
def test_branch_native_policy_is_unchanged(base, ref):
    result = context.native(event(GITHUB_REF=ref), base, "abc1234")
    assert result["version"] == base + "-dev.42+abc1234"
    assert result["stamp-native-version"] == result["should-publish"] == "true"


@pytest.mark.parametrize("overrides", [
    {"GITHUB_EVENT_NAME": "pull_request", "GITHUB_REF": "refs/pull/72/merge"},
    {"GITHUB_EVENT_NAME": "pull_request_target"},
    {"GITHUB_REF": "refs/heads/feature/cli"},
    {"GITHUB_REF": "refs/heads/x-maintenance"},
])
def test_nonpublishing_native_builds_remain_unstamped(overrides):
    result = context.native(event(**overrides), "0.4.0-dev.7", "abc1234")
    assert result["version"] == "0.4.0-dev.7"
    assert result["stamp-native-version"] == result["should-publish"] == "false"


def test_release_tag_has_no_native_stamp():
    result = context.native(event(GITHUB_REF_TYPE="tag", GITHUB_REF="refs/tags/0.4.0"), "0.4.0", "abc1234")
    assert result["version"] == "0.4.0" and result["stamp-native-version"] == "false"
    assert result["should-publish"] == "true"


@pytest.mark.parametrize("tag", ["v0.4.0", "0.4.1", "0.4.0-rc.2", "0.4.0/extra"])
def test_invalid_release_tag_is_rejected(tag):
    with pytest.raises(ValueError):
        context.native(event(GITHUB_REF_TYPE="tag", GITHUB_REF="refs/tags/" + tag), "0.4.0", "abc1234")


@pytest.mark.parametrize("cargo,python", [("0.4.0", "0.4.0.dev42"), ("0.4.0-rc.2", "0.4.0rc2.dev42"),
                                        ("0.4.0-dev.7", "0.4.0.dev7")])
def test_python_policy_is_independent(tmp_path, cargo, python):
    (tmp_path / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{cargo}"\n')
    result = context.load("release_context").resolve(event(), tmp_path)
    assert result["version"] == python
    assert result["eligible"] == ("dev.7" not in cargo)
    if not result["eligible"]:
        assert "another .devN" in result["reason"]
    pr = context.load("release_context").resolve(event(GITHUB_EVENT_NAME="pull_request"), tmp_path)
    assert not pr["eligible"]
    assert pr["version"] == ("0.4.0.dev7" if "dev.7" in cargo else python.removesuffix("42") + "0")


@pytest.mark.parametrize("cargo,python", [
    ("0.4.0", "0.4.0.dev0"), ("0.4.0-rc.2", "0.4.0rc2.dev0"),
    ("0.4.0-dev.7", "0.4.0.dev7"),
])
@pytest.mark.parametrize("ref", ["refs/heads/main", "refs/heads/0.4-maintenance"])
def test_fork_push_native_and_python_contexts_form_a_buildable_nonpublishing_pair(tmp_path, cargo, python, ref):
    (tmp_path / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{cargo}"\n')
    environment = event(GITHUB_REPOSITORY="someone/gukhanmun", GITHUB_REF=ref)
    native = context.native(environment, cargo, "aaaaaaa")
    pypi = context.load("release_context").resolve(environment, tmp_path)
    assert native["version"] == cargo + "-dev.42+aaaaaaa"
    assert pypi["version"] == python and not pypi["eligible"]
    context.load("cli_wheel").version_pair_check(native["version"], pypi["version"], environment["GITHUB_SHA"])


def test_python_import_failure_preserves_already_emitted_native_context(tmp_path):
    output = tmp_path / "output"
    environment = {**os.environ, **event(), "GITHUB_OUTPUT": str(output)}
    environment["GITHUB_SHA"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    script = ROOT / "scripts/python/cli_context.py"
    # -S removes packaging as well as any user/site dependency injection.
    native = subprocess.run([sys.executable, "-I", "-S", script, "native"], cwd=ROOT,
                            env=environment, capture_output=True, text=True)
    assert native.returncode == 0, native.stderr
    original = output.read_text()
    python = subprocess.run([sys.executable, "-I", "-S", script, "python"], cwd=ROOT,
                            env=environment, capture_output=True, text=True)
    assert python.returncode != 0 and "packaging" in python.stderr
    assert output.read_text() == original
    assert "should-publish=true\n" in original and "stamp-native-version=true\n" in original


def stamp_fixture():
    before = {"workspace": {"package": {"version": "0.4.0", "license": "GPL-3.0-only"},
                            "dependencies": {"ours": {"path": "crates/ours", "version": "0.4.0"},
                                             "external": {"version": "1"}}}}
    after = copy.deepcopy(before)
    after["workspace"]["package"]["version"] = "0.4.0-dev.42+abc1234"
    after["workspace"]["dependencies"]["ours"]["version"] = "0.4.0-dev.42+abc1234"
    lock = {"version": 4, "package": [
        {"name": "ours", "version": "0.4.0", "dependencies": ["external 1"]},
        {"name": "consumer", "version": "2", "source": "registry+url",
         "checksum": "hash", "dependencies": ["ours 0.4.0"]},
    ]}
    stamped = copy.deepcopy(lock)
    stamped["package"][0]["version"] = "0.4.0-dev.42+abc1234"
    stamped["package"][1]["dependencies"] = ["ours 0.4.0-dev.42+abc1234"]
    return before, after, lock, stamped


def test_native_stamp_accepts_only_workspace_version_semantics():
    context.check_stamp(*stamp_fixture(), {"ours"}, "0.4.0-dev.42+abc1234")


@pytest.mark.parametrize("change", ["license", "dependency-feature", "external-version", "checksum", "new-package"])
def test_native_stamp_rejects_unrelated_semantic_changes(change):
    before, after, lock, stamped = stamp_fixture()
    if change == "license":
        after["workspace"]["package"]["license"] = "MIT"
    elif change == "dependency-feature":
        after["workspace"]["dependencies"]["ours"]["features"] = ["unexpected"]
    elif change == "external-version":
        stamped["package"][1]["version"] = "3"
    elif change == "checksum":
        stamped["package"][1]["checksum"] = "changed"
    else:
        stamped["package"].append({"name": "extra", "version": "1"})
    with pytest.raises(ValueError, match="allowlist|nonworkspace"):
        context.check_stamp(before, after, lock, stamped, {"ours"}, "0.4.0-dev.42+abc1234")
