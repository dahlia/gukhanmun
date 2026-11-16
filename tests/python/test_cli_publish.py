# Gukhanmun: Tests the CLI adapter's independent validation and shared uploader.
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
import urllib.error

import pytest

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SPEC = importlib.util.spec_from_file_location("cli_publish_test_loader", ROOT / "scripts/python/cli_wheel.py")
wheel = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(wheel)
adapter = wheel.load("cli_publish")


def test_adapter_uses_narrow_hooks_without_core_classification(monkeypatch, tmp_path):
    calls = []
    monkeypatch.setattr(adapter.shared, "publish", lambda *a, **kw: calls.append((a, kw)) or "receipt")
    assert adapter.publish(tmp_path, "0.4.0", "123", "a" * 64, tmp_path.parent / "receipt") == "receipt"
    _, kwargs = calls[0]
    assert kwargs["validator"] is adapter.validate
    assert kwargs["upload_groups"]({"one.whl": {}, "two.whl": {}}) == [{"one.whl", "two.whl"}]
    assert kwargs["allow_missing_projects"] == ("gukhanmun-cli",)


def test_validator_reports_shared_publication_error(monkeypatch, tmp_path):
    def fail(*args, **kwargs):
        raise RuntimeError("CLI audit missing")
    monkeypatch.setattr(adapter.cli_artifacts, "validate", fail)
    with pytest.raises(adapter.shared.PublicationError, match="CLI audit missing"):
        adapter.validate(tmp_path, "0.4.0")


def test_invalid_cli_bundle_never_reaches_registry(tmp_path):
    class Registry:
        def project(self, *args):
            pytest.fail("validation must precede registry access")
    receipt = tmp_path.parent / f"{tmp_path.name}-receipt.json"
    with pytest.raises(adapter.shared.PublicationError):
        adapter.publish(tmp_path, "0.4.0", "123", "a" * 64, receipt, registry=Registry(), env={})
    assert wheel.read_json(receipt)["status"] == "failed"


@pytest.mark.parametrize("repository", ["someone/gukhanmun", "dahlia/gukhanmun"])
def test_buildable_dev_zero_never_authorizes_publication(tmp_path, monkeypatch, repository):
    # Isolate authorization from archive validation, as in the transport fixture.
    root, directory = tmp_path / "source", tmp_path / "bundle"
    root.mkdir()
    directory.mkdir()
    (root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.4.0"\n')
    source = {"commit": "a" * 40, "ref": "refs/heads/main", "run_id": "123", "run_attempt": "1"}
    data = {"version": "0.4.0.dev0", "source": source, "artifacts": []}
    monkeypatch.setattr(adapter, "validate", lambda *a, **kw: data)
    environment = {
        "GITHUB_ACTIONS": "true", "GITHUB_REPOSITORY": repository, "GITHUB_EVENT_NAME": "push",
        "GITHUB_REF_TYPE": "branch", "GITHUB_REF": source["ref"], "GITHUB_SHA": source["commit"],
        "GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "1", "GITHUB_RUN_NUMBER": "42",
        "ACTIONS_ID_TOKEN_REQUEST_URL": "https://example.invalid/oidc",
        "ACTIONS_ID_TOKEN_REQUEST_TOKEN": "synthetic-test-token",
    }
    class Registry:
        def project(self, *args):
            pytest.fail("dev0 authorization must fail before network access")
    with pytest.raises(adapter.shared.PublicationError, match="not eligible|version differs"):
        adapter.publish(directory, data["version"], "456", "b" * 64, tmp_path / "receipt.json",
                        root=root, env=environment, head=source["commit"], registry=Registry())


@pytest.fixture
def transport(tmp_path, monkeypatch):
    """Inject offline validation to isolate the already-shared registry loop."""
    directory = tmp_path / "bundle"
    directory.mkdir()
    entries = {}
    for name, description in adapter.cli_artifacts.expected_files("0.4.0.dev12").items():
        path = directory / name
        path.write_bytes(b"synthetic-transport-fixture")
        entries[name] = adapter.cli_artifacts.inventory(path) | description
    source = {"commit": "a" * 40, "ref": "refs/heads/main", "run_id": "123", "run_attempt": "1"}
    data = {"version": "0.4.0.dev12", "source": source, "artifacts": list(entries.values())}
    monkeypatch.setattr(adapter, "validate", lambda *a, **kw: data)
    env = {"GITHUB_ACTIONS": "true", "GITHUB_SHA": source["commit"], "GITHUB_REF": source["ref"],
           "GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "2", "GITHUB_RUN_NUMBER": "12",
           "ACTIONS_ID_TOKEN_REQUEST_URL": "https://example.invalid/oidc",
           "ACTIONS_ID_TOKEN_REQUEST_TOKEN": "synthetic-test-token"}
    class Registry:
        def __init__(self):
            self.json = set()
            self.index = set()
            self.conflicts = {}
        def project(self, name):
            assert name == "gukhanmun-cli"
        def release(self, name, version):
            return [{"filename": filename, "digests": {"sha256": self.conflicts.get(filename, entries[filename]["sha256"])},
                     "size": entries[filename]["size"], "yanked": False} for filename in sorted(self.json)]
        def simple(self, name):
            return [{"filename": filename, "hashes": {"sha256": self.conflicts.get(filename, entries[filename]["sha256"])},
                     "yanked": False} for filename in sorted(self.index)]
    registry = Registry()
    uploads = []
    def upload(path, timeout):
        uploads.append(path.name)
        registry.json.add(path.name)
        registry.index.add(path.name)
    def publish(uploader=upload, *, budget=None):
        return adapter.publish(
            directory, data["version"], "456", "b" * 64, tmp_path / "receipt.json",
            env=env, context={"eligible": True, "version": data["version"]}, head=source["commit"],
            registry=registry, uploader=uploader,
            budget=budget or adapter.shared.Budget(10000, clock=lambda: 0, sleep=lambda _: None),
        )
    return entries, registry, uploads, upload, publish


def test_cli_partial_upload_recovery_skips_exact_remote_files(transport):
    entries, registry, uploads, _, publish = transport
    existing = next(iter(entries))
    registry.json.add(existing)
    registry.index.add(existing)
    receipt = publish()
    assert existing not in uploads and len(uploads) == 5
    assert receipt["status"] == "complete" and receipt["manifest_attempt"] == "1"
    assert receipt["current_attempt"] == "2"
    publish()
    assert len(uploads) == 5


def test_cli_pending_publisher_creates_project_and_confirms_both_indexes(transport, monkeypatch):
    entries, registry, uploads, _, publish = transport
    client = adapter.shared.Registry(
        adapter.shared.Budget(10000), allow_missing_projects=("gukhanmun-cli",),
    )
    release, simple = registry.release, registry.simple
    absent = []
    delayed = set()

    def get(url, *, missing=False):
        assert missing
        if not uploads:
            absent.append(url)
            return None
        # Each inventory can still be absent just after the first upload.
        if url not in delayed:
            delayed.add(url)
            return None
        if "/simple/" in url:
            return {"name": "gukhanmun-cli", "files": simple("gukhanmun-cli")}
        return {"info": {"name": "gukhanmun-cli", "version": "0.4.0.dev12"},
                "urls": release("gukhanmun-cli", "0.4.0.dev12")}

    monkeypatch.setattr(client, "get", get)
    for method in ("project", "release", "simple"):
        monkeypatch.setattr(registry, method, getattr(client, method))
    receipt = publish()
    assert len(absent) == 3  # Project, release JSON and Simple preflight.
    assert len(uploads) == len(entries) == 6
    assert receipt["status"] == "complete"
    assert set(receipt["completed"]) == set(entries)
    assert not receipt["missing"]


@pytest.mark.parametrize("status", [403, 429, 500])
def test_pending_publisher_does_not_hide_registry_errors(monkeypatch, status):
    client = adapter.shared.Registry(
        adapter.shared.Budget(10000, clock=lambda: 0), allow_missing_projects=("gukhanmun-cli",),
    )
    def fail(request, **kwargs):
        raise urllib.error.HTTPError(request.full_url, status, "synthetic", {}, None)
    monkeypatch.setattr(adapter.shared.urllib.request, "urlopen", fail)
    for method, args in ((client.project, ("gukhanmun-cli",)),
                         (client.release, ("gukhanmun-cli", "0.4.0.dev12")),
                         (client.simple, ("gukhanmun-cli",))):
        with pytest.raises(adapter.shared.PublicationError, match=f"registry HTTP {status}"):
            method(*args)


@pytest.mark.parametrize("allowed", [False, True])
def test_pending_publisher_http_not_found_is_opt_in(monkeypatch, allowed):
    client = adapter.shared.Registry(
        adapter.shared.Budget(10000, clock=lambda: 0),
        allow_missing_projects=("gukhanmun-cli",) if allowed else (),
    )
    def absent(request, **kwargs):
        raise urllib.error.HTTPError(request.full_url, 404, "synthetic", {}, None)
    monkeypatch.setattr(adapter.shared.urllib.request, "urlopen", absent)
    if allowed:
        client.project("gukhanmun-cli")
        assert client.simple("gukhanmun-cli") == []
    else:
        with pytest.raises(adapter.shared.PublicationError, match="absent"):
            client.project("gukhanmun-cli")
        with pytest.raises(adapter.shared.PublicationError, match="registry HTTP 404"):
            client.simple("gukhanmun-cli")
    assert client.release("gukhanmun-cli", "0.4.0.dev12") == []


def test_pending_publisher_missing_project_permission_is_scoped(monkeypatch):
    client = adapter.shared.Registry(
        adapter.shared.Budget(10000), allow_missing_projects=("gukhanmun-cli",),
    )
    monkeypatch.setattr(client, "get", lambda *args, **kwargs: None)
    client.project("gukhanmun-cli")
    assert client.simple("gukhanmun-cli") == []
    with pytest.raises(adapter.shared.PublicationError, match="all seven projects"):
        client.project("gukhanmun")
    with pytest.raises(adapter.shared.PublicationError, match="wrong Simple identity"):
        client.simple("gukhanmun")


def test_pending_publisher_still_checks_existing_project_identity(monkeypatch):
    client = adapter.shared.Registry(
        adapter.shared.Budget(10000), allow_missing_projects=("gukhanmun-cli",),
    )
    monkeypatch.setattr(client, "get", lambda *args, **kwargs: {"info": {"name": "other"}, "files": []})
    with pytest.raises(adapter.shared.PublicationError, match="wrong registry project identity"):
        client.project("gukhanmun-cli")
    with pytest.raises(adapter.shared.PublicationError, match="wrong Simple identity"):
        client.simple("gukhanmun-cli")


def test_pending_publisher_missing_indexes_never_confirm_failed_upload(transport, monkeypatch, tmp_path):
    _, registry, uploads, _, publish = transport
    clock = [0]
    def sleep(seconds):
        clock[0] += seconds
    budget = adapter.shared.Budget(10000, clock=lambda: clock[0], sleep=sleep)
    client = adapter.shared.Registry(budget, allow_missing_projects=("gukhanmun-cli",))
    monkeypatch.setattr(client, "get", lambda *args, **kwargs: None)
    for method in ("project", "release", "simple"):
        monkeypatch.setattr(registry, method, getattr(client, method))
    def denied(path, timeout):
        uploads.append(path.name)
        raise adapter.shared.PublicationError("OIDC denied")
    with pytest.raises(adapter.shared.PublicationError, match="OIDC denied.*unconfirmed"):
        publish(denied, budget=budget)
    receipt = wheel.read_json(tmp_path / "receipt.json")
    assert len(uploads) == 1
    assert receipt["status"] == "failed" and receipt["completed"] == []
    assert len(receipt["missing"]) == 6


def test_cli_lost_upload_response_requires_remote_confirmation(transport):
    entries, _, uploads, upload, publish = transport
    def lost(path, timeout):
        upload(path, timeout)
        raise TimeoutError("synthetic lost upload response")
    receipt = publish(lost)
    assert len(uploads) == len(entries) == 6
    assert receipt["status"] == "complete"


def test_cli_conflicting_file_preflight_prevents_all_uploads(transport):
    entries, registry, uploads, _, publish = transport
    existing = next(iter(entries))
    registry.json.add(existing)
    registry.conflicts[existing] = "f" * 64
    with pytest.raises(adapter.shared.PublicationError, match="conflict"):
        publish()
    assert not uploads


def test_cli_simple_known_file_is_not_reuploaded_while_json_catches_up(transport, monkeypatch):
    entries, registry, uploads, _, publish = transport
    known = next(iter(entries))
    registry.index.add(known)
    original = registry.release
    calls = 0
    def release(name, version):
        nonlocal calls
        calls += 1
        if calls > 1:
            registry.json.add(known)
        return original(name, version)
    monkeypatch.setattr(registry, "release", release)
    assert publish()["status"] == "complete"
    assert known not in uploads and len(uploads) == 5
