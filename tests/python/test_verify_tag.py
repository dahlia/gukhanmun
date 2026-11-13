# Gukhanmun: Verifies release-tag binding using mocked Git and API responses.
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
import io
import json
import os
from pathlib import Path
import sys

import pytest

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SPEC = importlib.util.spec_from_file_location("tag_binding", ROOT / "scripts/python/verify_tag.py")
tag = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(tag)


def inputs():
    return {"ref_object": {"type": "tag", "sha": "a" * 40},
            "tag_object": {"sha": "a" * 40, "tag": "0.4.0", "object": {"type": "commit", "sha": "b" * 40},
                           "verification": {"verified": True}},
            "local_object": "a" * 40, "local_type": "tag", "local_target": "b" * 40,
            "head": "b" * 40, "source_sha": "b" * 40, "tag_name": "0.4.0"}


def test_same_verified_tag_and_built_commit_pass():
    tag.validate(**inputs())


@pytest.mark.parametrize("signed_name", ["v0.4.0", "0.4.1", None])
def test_signed_tag_name_must_match_requested_release(signed_name):
    arguments = inputs()
    arguments["tag_object"]["tag"] = signed_name
    with pytest.raises(ValueError, match="signed tag name"):
        tag.validate(**arguments)


@pytest.mark.parametrize("field,value,message", [
    ("local_type", "commit", "annotated"),
    ("local_object", "c" * 40, "tag object"),
    ("local_target", "c" * 40, "target"),
    ("head", "c" * 40, "target"),
    ("source_sha", "c" * 40, "target"),
    ("source_sha", None, "source commit"),
])
def test_local_or_event_identity_mismatch_is_rejected(field, value, message):
    arguments = inputs()
    arguments[field] = value
    with pytest.raises(ValueError, match=message):
        tag.validate(**arguments)


@pytest.mark.parametrize("change,message", [
    (lambda value: value["ref_object"].update(type="commit"), "annotated"),
    (lambda value: value["ref_object"].update(sha="c" * 40), "tag object"),
    (lambda value: value["tag_object"].update(sha="c" * 40), "tag object"),
    (lambda value: value["tag_object"]["verification"].update(verified=False), "signature"),
    (lambda value: value["tag_object"]["verification"].update(verified="true"), "signature"),
    (lambda value: value["tag_object"]["object"].update(type="tag"), "directly"),
    (lambda value: value["tag_object"]["object"].update(sha="c" * 40), "target"),
])
def test_moved_nested_or_unverified_remote_tag_is_rejected(change, message):
    arguments = inputs()
    change(arguments)
    with pytest.raises(ValueError, match=message):
        tag.validate(**arguments)


def test_cli_captures_git_and_api_without_creating_tags(monkeypatch):
    arguments = inputs()
    monkeypatch.setattr(sys, "argv", ["verify_tag.py", "--tag", "0.4.0", "--ref-object", json.dumps(arguments["ref_object"])])
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(arguments["tag_object"])))
    monkeypatch.setenv("GITHUB_SHA", arguments["source_sha"])
    calls = []
    def command(argv, **kwargs):
        calls.append(argv)
        if argv[1] == "cat-file":
            return "tag\n"
        return ("a" if argv[-1] == "refs/tags/0.4.0" else "b") * 40 + "\n"
    monkeypatch.setattr(tag.subprocess, "check_output", command)
    assert tag.main() == 0
    assert calls == [["git", "rev-parse", "--verify", "refs/tags/0.4.0"],
                     ["git", "cat-file", "-t", "refs/tags/0.4.0"],
                     ["git", "rev-parse", "--verify", "refs/tags/0.4.0^{commit}"],
                     ["git", "rev-parse", "HEAD"]]
