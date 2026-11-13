# Gukhanmun: Tests offline publication validation and resumable upload ordering.
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
import subprocess
import sys
import tarfile
from types import SimpleNamespace
import zipfile

import pytest

REPOSITORY = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SPEC = importlib.util.spec_from_file_location("publish_under_test", REPOSITORY / "scripts/python/publish.py")
publish = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = publish
SPEC.loader.exec_module(publish)
a = publish.artifacts
VERSION = "0.4.0.dev7"
README = (REPOSITORY / "python/gukhanmun/README.md").read_text(encoding="utf-8")
SHA = "a" * 40
ENV = {"GITHUB_ACTIONS": "true", "GITHUB_REPOSITORY": "dahlia/gukhanmun", "GITHUB_EVENT_NAME": "push",
       "GITHUB_REF": "refs/heads/main", "GITHUB_REF_TYPE": "branch", "GITHUB_SHA": SHA,
       "GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "2", "GITHUB_RUN_NUMBER": "7",
       "ACTIONS_ID_TOKEN_REQUEST_URL": "https://oidc.invalid/token",
       "ACTIONS_ID_TOKEN_REQUEST_TOKEN": "secret-test-token"}
CONTEXT = {"version": VERSION, "eligible": True, "reason": "test"}


def write_json(path, value):
    path.write_text(json.dumps(value), encoding="utf-8")


def package_metadata(name, version=None):
    version = VERSION if version is None else version
    text = f"Metadata-Version: 2.4\nName: {name}\nVersion: {version}\n"
    if name == "gukhanmun":
        text += "Description-Content-Type: text/markdown\n"
        for extra, distribution in a.dictionary_table(REPOSITORY).EXTRAS.items():
            text += f'Provides-Extra: {extra}\nRequires-Dist: {distribution}=={version}; extra == "{extra}"\n'
    return text + "\n" + (README if name == "gukhanmun" else "")


def write_package(path, description, text=None, project=None, duplicate=False):
    name = description["distribution"]
    text = text or package_metadata(name)
    if path.name.endswith(".whl"):
        with zipfile.ZipFile(path, "w") as archive:
            member = f"{name.replace('-', '_')}-{VERSION}.dist-info/METADATA"
            archive.writestr(member, text)
            if duplicate:
                archive.writestr(member, text)
    else:
        if project is None:
            project = f'[project]\nname = "{name}"\nversion = "{VERSION}"\ndependencies = []\n'
            if name == "gukhanmun":
                project += "[project.optional-dependencies]\n"
                project += "".join(f'{extra} = ["{distribution}=={VERSION}"]\n'
                                   for extra, distribution in a.dictionary_table(REPOSITORY).EXTRAS.items())
        prefix = path.name.removesuffix(".tar.gz")
        with tarfile.open(path, "w:gz") as archive:
            for filename, content in (("PKG-INFO", text), ("pyproject.toml", project)):
                data = content.encode()
                info = tarfile.TarInfo(f"{prefix}/{filename}")
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))


def refresh(directory, *, source=None):
    """Build actual verifier evidence around tiny archives, without compiling."""
    expected = a.expected_files(VERSION, REPOSITORY)
    hashes = {name: publish.digest(directory / name) for name in expected}
    data_installed = {d["distribution"]: {"sha256": hashes[name]} for name, d in expected.items()
                      if d["kind"] == "wheel" and not d.get("target")}
    for target in a.TARGETS:
        wheels = {name: d for name, d in expected.items() if d.get("target") == target.id}
        write_json(directory / f"audit-{target.id}.json", {"records": [{"wheel": name, "sha256": hashes[name]} for name in wheels]})
        write_json(directory / f"build-info-{target.id}.json", {"schema": 1, "target": a.asdict(target),
                                                              "commit": SHA, "wheels": {name: hashes[name] for name in wheels}})
        for interpreter in a.test_interpreters(target, REPOSITORY):
            name = next(name for name, d in wheels.items() if d["implementation"] == interpreter["implementation"]
                        and (d["implementation"] == "cpython" or d["python"] == interpreter["python"]))
            record = {"label": f"{target.id}-{interpreter['label']}"}
            for mode, final in a.FINAL_CHECKS.items():
                record[mode] = {"installed": {"gukhanmun": {"filename": name, "sha256": hashes[name]},
                                               **(data_installed if mode == "official" else {})},
                                "result": {"mode": mode, "checks": [final, "windows-import-closure"],
                                           "interpreter": {"implementation": interpreter["implementation"],
                                                           "python": interpreter["python"] + ".1",
                                                           "libc": interpreter.get("libc", ["glibc", "2.43"]),
                                                           "distribution": VERSION, "public_version": VERSION}}}
            write_json(directory / f"smoke-{target.id}-{interpreter['label']}.json", record)
    smoke = {mode: {"installed": {}, "result": {"mode": mode, "checks": [final],
                                              "interpreter": {"distribution": VERSION, "public_version": VERSION}}}
             for mode, final in a.FINAL_CHECKS.items()}
    rebuilds = []
    for name, d in expected.items():
        if d["kind"] != "sdist":
            continue
        wheel = f"rebuilt-{name}.whl"
        installed = {"filename": wheel, "sha256": "f" * 64}
        smoke["official"]["installed"][d["distribution"]] = installed
        if d["distribution"] == "gukhanmun":
            smoke["base"]["installed"]["gukhanmun"] = installed
        rebuilds.append({"sdist": name, "sdist_sha256": hashes[name], "wheel": wheel, "wheel_sha256": "f" * 64})
    write_json(directory / "smoke-sdist-rebuild.json", smoke)
    write_json(directory / "rebuilds.json", {"evidence_sha256": publish.digest(directory / "smoke-sdist-rebuild.json"),
                                            "rebuilds": rebuilds})
    entries = []
    for name, d in expected.items():
        entry = {**a.describe(directory / name), **d}
        if name.endswith(".whl"):
            _, _, python, abi, platform = a.wheel_tag(name)
            entry["tags"] = {"python": python, "abi": abi, "platform": platform}
        entries.append(entry)
    source = source or {"commit": SHA, "ref": ENV["GITHUB_REF"], "run_id": "123", "run_attempt": "1"}
    write_json(directory / "manifest.json", {"schema": 1, "version": VERSION, "source": source, "toolchain": {},
                                            "artifacts": entries, "evidence": [a.describe(p) for p in sorted(directory.glob("*.json"))
                                                                                if p.name != "manifest.json"]})
    (directory / "SHA256SUMS").write_text("".join(f"{entry['sha256']}  {entry['filename']}\n" for entry in entries))


@pytest.fixture
def release(tmp_path):
    directory = tmp_path / "release"
    directory.mkdir()
    for name, description in a.expected_files(VERSION, REPOSITORY).items():
        write_package(directory / name, description)
    refresh(directory)
    return directory


def change_manifest(directory, mutation):
    path = directory / "manifest.json"
    data = json.loads(path.read_text())
    mutation(data)
    write_json(path, data)


def test_validate_is_completely_offline(release, monkeypatch):
    monkeypatch.setattr(publish.urllib.request, "urlopen", lambda *args, **kwargs: pytest.fail("network"))
    monkeypatch.setattr(publish.subprocess, "run", lambda *args, **kwargs: pytest.fail("subprocess"))
    result = publish.validate(release, VERSION, env=ENV)
    assert len(result["artifacts"]) == 31
    assert result["source"]["run_attempt"] == "1"


def test_receipts_cannot_overwrite_original_artifacts(release, monkeypatch):
    manifest = release / "manifest.json"
    original = manifest.read_bytes()
    with pytest.raises(publish.PublicationError, match="outside"):
        publish.publish(release, VERSION, "987", publish.digest(manifest), manifest, env=ENV)
    monkeypatch.setattr(sys, "argv", ["publish.py", "validate", "--dir", str(release), "--version", VERSION,
                                      "--receipt", str(manifest), "--artifact-id", "987",
                                      "--manifest-sha256", publish.digest(manifest)])
    assert publish.main() == 1
    assert manifest.read_bytes() == original


def test_duplicate_evidence_keys_are_rejected(release):
    path = next(release.glob("audit-*.json"))
    value = path.read_text()
    path.write_text('{"records": [],' + value[1:])
    change_manifest(release, lambda data: next(entry for entry in data["evidence"]
                    if entry["filename"] == path.name).update(sha256=publish.digest(path), size=path.stat().st_size))
    with pytest.raises(publish.PublicationError, match="duplicate JSON key"):
        publish.validate(release, VERSION, env=ENV)


@pytest.mark.parametrize("mutation,message", [
    (lambda d: d.update(schema=True), "schema"),
    (lambda d: d.update(artifacts={}), "list"),
    (lambda d: d["artifacts"][0].update(size=True), "size"),
    (lambda d: d["artifacts"][0].update(sha256="A" * 64), "SHA-256"),
    (lambda d: d["artifacts"][0].update(filename="../escape.whl"), "filename"),
    (lambda d: d["artifacts"].append(d["artifacts"][0]), "duplicate"),
    (lambda d: d["artifacts"][0].update(kind="sdist"), "classification"),
    (lambda d: d["source"].update(run_attempt="3"), "future"),
    (lambda d: d["source"].update(commit="b" * 40), "GITHUB_SHA"),
    (lambda d: d["source"].update(run_id="124"), "GITHUB_RUN_ID"),
    (lambda d: d["source"].update(ref="refs/heads/other"), "GITHUB_REF"),
])
def test_rejects_bad_manifest_before_verifier_or_network(release, monkeypatch, mutation, message):
    change_manifest(release, mutation)
    monkeypatch.setattr(a, "verify", lambda *args: pytest.fail("verifier reached"))
    with pytest.raises(publish.PublicationError, match=message):
        publish.validate(release, VERSION, env=ENV)


@pytest.mark.parametrize("name", ["manifest.json", "SHA256SUMS", "rebuilds.json"])
def test_rejects_symlink_control_files(release, tmp_path, name):
    target = tmp_path / "original"
    (release / name).rename(target)
    (release / name).symlink_to(target)
    with pytest.raises(publish.PublicationError, match="nonregular"):
        publish.validate(release, VERSION, env=ENV)


def test_unexpected_file_is_not_ignored(release):
    (release / "note.txt").write_text("surprise")
    with pytest.raises(publish.PublicationError, match="inventory"):
        publish.validate(release, VERSION, env=ENV)


def test_build_provenance_must_cover_every_wheel(release):
    path = release / "build-info-linux-x86_64.json"
    record = json.loads(path.read_text())
    record["wheels"].pop(next(iter(record["wheels"])))
    write_json(path, record)
    change_manifest(release, lambda d: d["evidence"].__setitem__(next(i for i, e in enumerate(d["evidence"]) if e["filename"] == path.name), a.describe(path)))
    with pytest.raises(publish.PublicationError, match="all target wheel hashes"):
        publish.validate(release, VERSION, env=ENV)


@pytest.mark.parametrize("change,message", [
    (lambda text: text.replace("Name: gukhanmun\n", "Name: wrong\n"), "Name"),
    (lambda text: text.replace(f"Version: {VERSION}\n", "Version: 0.4.0\n"), "Version"),
    (lambda text: text.replace("Name: gukhanmun\n", "Name: gukhanmun\nName: gukhanmun\n"), "one Name"),
    (lambda text: text.replace("\n\n", f'\nRequires-Dist: gukhanmun-stdict-fst=={VERSION}; extra == "stdict"\n\n', 1), "duplicate"),
    (lambda text: text.replace(f"gukhanmun-stdict-fst=={VERSION}", "gukhanmun-stdict-fst==0.3.0"), "pin"),
    (lambda text: text.replace("Description-Content-Type: text/markdown\n", ""), "README"),
    (lambda text: text.replace("text/markdown", "text/plain"), "README"),
    (lambda text: text.split("\n\n", 1)[0] + "\n\n", "README"),
    (lambda text: text.split("\n\n", 1)[0] + "\n\nAnother README", "README"),
])
def test_native_metadata_is_inspected(release, change, message):
    name, description = next((name, d) for name, d in a.expected_files(VERSION).items() if d.get("target"))
    write_package(release / name, description, text=change(package_metadata("gukhanmun").rstrip() + "\n"))
    refresh(release)
    with pytest.raises(publish.PublicationError, match=message):
        publish.validate(release, VERSION, env=ENV)


def test_data_metadata_must_have_no_dependencies(release):
    name, d = next((name, d) for name, d in a.expected_files(VERSION).items() if d["kind"] == "wheel" and not d.get("target"))
    write_package(release / name, d, text=package_metadata(d["distribution"]).rstrip() + "\nRequires-Dist: surprise\n")
    refresh(release)
    with pytest.raises(publish.PublicationError, match="no dependencies"):
        publish.validate(release, VERSION, env=ENV)


def test_sdist_project_must_match_metadata(release):
    name = f"gukhanmun-{VERSION}.tar.gz"
    write_package(release / name, {"distribution": "gukhanmun"}, project=f'[project]\nname="gukhanmun"\nversion="{VERSION}"\n')
    refresh(release)
    with pytest.raises(publish.PublicationError, match="pyproject extras"):
        publish.validate(release, VERSION, env=ENV)


def test_duplicate_archive_metadata_is_rejected(release):
    name, d = next(iter(a.expected_files(VERSION).items()))
    with pytest.warns(UserWarning, match="Duplicate"):
        write_package(release / name, d, duplicate=True)
    refresh(release)
    with pytest.raises(publish.PublicationError, match="duplicate archive"):
        publish.validate(release, VERSION, env=ENV)


class FakeClock:
    def __init__(self):
        self.now = 0

    def __call__(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


class FakeRegistry:
    def __init__(self, entries):
        self.expected = entries
        self.remote = set()
        self.simple_remote = set()
        self.extra = []
        self.uploads = []
        self.calls = []
        self.stale = 0
        self.failed = None
        self.lost_response = False
        self.bootstrap_missing = False

    def project(self, name):
        self.calls.append(("project", name))
        if self.bootstrap_missing:
            raise publish.PublicationError("seed all seven projects")

    def file(self, name, simple=False):
        entry = self.expected[name]
        result = {"filename": name, "yanked": False, "hashes" if simple else "digests": {"sha256": entry["sha256"]}}
        if not simple:
            result["size"] = entry["size"]
        return result

    def release(self, project, version):
        self.calls.append(("json", project))
        return [self.file(name) for name in self.remote if self.expected[name]["distribution"] == project] + self.extra

    def simple(self, project):
        self.calls.append(("simple", project))
        if self.stale:
            self.stale -= 1
        else:
            self.simple_remote |= self.remote
        return [self.file(name, True) for name in self.simple_remote if self.expected[name]["distribution"] == project]

    def upload(self, path, timeout):
        assert timeout <= publish.UPLOAD_TIMEOUT
        name = path.name
        if self.expected[name]["distribution"] == "gukhanmun":
            assert len([n for n in self.simple_remote if self.expected[n]["distribution"] != "gukhanmun"]) == 12
        self.uploads.append(name)
        if self.failed == name:
            if self.lost_response:
                self.remote.add(name)
            raise publish.PublicationError("ambiguous upload")
        self.remote.add(name)


@pytest.fixture
def harness(release, tmp_path):
    entries = {e["filename"]: e for e in json.loads((release / "manifest.json").read_text())["artifacts"]}
    registry = FakeRegistry(entries)
    clock = FakeClock()
    receipt = tmp_path / "receipt.json"
    def run(**options):
        arguments = dict(env=ENV, context=CONTEXT, head=SHA, registry=registry, uploader=registry.upload,
                         budget=publish.Budget(1500, clock, clock.sleep))
        arguments.update(options)
        return publish.publish(release, VERSION, "987", publish.digest(release / "manifest.json"), receipt, **arguments)
    return SimpleNamespace(run=run, registry=registry, clock=clock, receipt=receipt, release=release)


def test_data_precedes_core_and_core_sdist_is_last(harness):
    result = harness.run()
    uploads = harness.registry.uploads
    assert len(uploads) == 31
    assert all(harness.registry.expected[n]["distribution"] != "gukhanmun" for n in uploads[:12])
    assert uploads[-1] == f"gukhanmun-{VERSION}.tar.gz"
    assert result["status"] == "complete" and len(result["completed"]) == 31
    assert result["missing"] == [] and result["manifest_attempt"] == "1" and result["current_attempt"] == "2"
    first_upload = uploads[0]
    assert first_upload in result["submitted"]


@pytest.mark.parametrize("options,message", [
    ({"context": {"version": VERSION, "eligible": False, "reason": "feature"}}, "not eligible"),
    ({"context": {"version": "0.4.0.dev8", "eligible": True}}, "version differs"),
    ({"head": "b" * 40}, "HEAD"),
    ({"env": {**ENV, "GITHUB_ACTIONS": "false"}}, "GitHub Actions"),
    ({"env": {**ENV, "ACTIONS_ID_TOKEN_REQUEST_TOKEN": ""}}, "REQUEST_TOKEN"),
])
def test_identity_failure_occurs_before_registry(harness, options, message):
    with pytest.raises(publish.PublicationError, match=message):
        harness.run(**options)
    assert not harness.registry.calls and not harness.registry.uploads
    assert json.loads(harness.receipt.read_text())["status"] == "failed"


def test_actual_event_resolver_is_used(harness, tmp_path, monkeypatch):
    context_root = tmp_path / "version-context"
    context_root.mkdir()
    (context_root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.4.0"\n')
    original_load = publish.load
    resolver = original_load("release_context")
    def load(name):
        if name == "release_context":
            return SimpleNamespace(resolve=lambda env, root: resolver.resolve(env, context_root))
        return original_load(name)
    monkeypatch.setattr(publish, "load", load)
    result = harness.run(context=None)
    assert result["status"] == "complete"


def test_feature_validation_does_not_authorize_upload(harness):
    env = {**ENV, "GITHUB_REF": "refs/heads/feature"}
    change_manifest(harness.release, lambda d: d["source"].update(ref=env["GITHUB_REF"]))
    # A feature .dev0 set is offline-valid; this set's version is separately
    # rejected by the current resolver, irrespective of previous validation.
    publish.validate(harness.release, VERSION, env=env)
    with pytest.raises(publish.PublicationError, match="not eligible"):
        harness.run(env=env, context=None)
    assert not harness.registry.calls


def test_preflight_conflict_prevents_all_uploads(harness):
    name = next(iter(harness.registry.expected))
    conflict = harness.registry.file(name)
    conflict["digests"]["sha256"] = "0" * 64
    harness.registry.extra = [conflict]
    with pytest.raises(publish.PublicationError, match="conflict"):
        harness.run()
    assert not harness.registry.uploads
    assert json.loads(harness.receipt.read_text())["conflicting"]


def test_absent_project_is_actionable_before_upload(harness):
    harness.registry.bootstrap_missing = True
    with pytest.raises(publish.PublicationError, match="seed all seven"):
        harness.run()
    assert not harness.registry.uploads


def test_identical_existing_files_are_skipped_and_simple_visibility_waits(harness):
    names = set(harness.registry.expected)
    harness.registry.remote = names
    harness.registry.stale = 16
    result = harness.run()
    assert result["status"] == "complete" and not harness.registry.uploads
    assert harness.clock.now > 0


def test_lost_success_upload_response_is_reconciled(harness):
    first = sorted(name for name, d in harness.registry.expected.items() if d["distribution"] != "gukhanmun")[0]
    harness.registry.failed, harness.registry.lost_response = first, True
    result = harness.run()
    assert len(harness.registry.uploads) == 31 and result["last_error"] is None


def test_failed_upload_stops_then_rerun_recovers_identical_progress(harness):
    data = sorted(name for name, d in harness.registry.expected.items() if d["distribution"] != "gukhanmun")
    harness.registry.failed = data[2]
    with pytest.raises(publish.PublicationError, match="new version"):
        harness.run()
    assert harness.registry.uploads == data[:3]
    receipt = json.loads(harness.receipt.read_text())
    assert len(receipt["completed"]) == 2 and len(receipt["missing"]) == 29
    harness.registry.failed = None
    harness.registry.uploads.clear()
    result = harness.run()
    assert len(harness.registry.uploads) == 29
    assert result["status"] == "complete"


def test_deadline_reserves_time_for_reconciliation(harness):
    budget = publish.Budget(269, harness.clock, harness.clock.sleep)
    with pytest.raises(publish.PublicationError, match="insufficient"):
        harness.run(budget=budget)
    assert not harness.registry.uploads


@pytest.mark.parametrize("entry_update,message", [
    ({"size": True}, "size"), ({"yanked": 0}, "yanked"), ({"yanked": True}, "yanked"),
    ({"filename": "unexpected-0.4.0.dev7.tar.gz"}, "unexpected"),
])
def test_registry_inventory_is_strict(harness, entry_update, message):
    name = next(iter(harness.registry.expected))
    entry = harness.registry.file(name)
    entry.update(entry_update)
    with pytest.raises(publish.PublicationError, match=message):
        publish.inventory([entry], harness.registry.expected)


def test_simple_is_version_scoped_without_prefix_confusion(harness):
    project = "gukhanmun"
    expected = {n: e for n, e in harness.registry.expected.items() if e["distribution"] == project}
    entries = [{"filename": f"gukhanmun-{version}.tar.gz"} for version in ("0.4.0.dev70", "0.4.0", "0.3.9")]
    entries.append({"filename": "old-unparseable.tar.gz"})
    assert publish.inventory(entries, expected, simple=True, project=project, version=VERSION) == set()
    entries.append({"filename": f"gukhanmun-{VERSION}-py3-none-any.whl"})
    with pytest.raises(publish.PublicationError, match="unexpected"):
        publish.inventory(entries, expected, simple=True, project=project, version=VERSION)


def test_uv_environment_and_command_have_no_auth_fallback(tmp_path, monkeypatch):
    env = {**ENV, "PATH": "/tools", "UV_PUBLISH_TOKEN": "pypi-secret-value", "UV_PUBLISH_URL": "https://evil.invalid",
           "PIP_INDEX_URL": "https://index.invalid", "UV_CONFIG_FILE": "/evil", "HTTP_PROXY": "http://proxy.invalid"}
    commands = []
    def run(command, **kwargs):
        commands.append((command, kwargs))
        assert "UV_PUBLISH_TOKEN" not in kwargs["env"]
        assert "UV_CONFIG_FILE" not in kwargs["env"] and "UV_PUBLISH_URL" not in kwargs["env"]
        assert kwargs["env"]["GITHUB_ACTIONS"] == "true"
        assert kwargs["env"]["ACTIONS_ID_TOKEN_REQUEST_TOKEN"] == ENV["ACTIONS_ID_TOKEN_REQUEST_TOKEN"]
        assert kwargs["env"]["HTTP_PROXY"] == env["HTTP_PROXY"]
        return SimpleNamespace(returncode=0)
    monkeypatch.setattr(publish.subprocess, "run", run)
    publish.Upload(env)(tmp_path / "a.whl", 42)
    command, options = commands[0]
    assert command[:5] == ["uv", "--no-config", "publish", "--trusted-publishing", "always"]
    assert "--check-url" not in command and "--verbose" not in command
    assert command[command.index("--publish-url") + 1] == publish.UPLOAD_URL
    assert command[command.index("--keyring-provider") + 1] == "disabled"
    assert options["timeout"] == 42


def test_upload_output_is_capped_and_redacted(tmp_path, monkeypatch):
    def run(command, **options):
        options["stderr"].write(("x" * 20000 + " pypi-secretvalue eyJabc.def.ghi " + ENV["ACTIONS_ID_TOKEN_REQUEST_TOKEN"]).encode())
        return SimpleNamespace(returncode=1)
    monkeypatch.setattr(publish.subprocess, "run", run)
    with pytest.raises(publish.PublicationError) as error:
        publish.Upload(ENV)(tmp_path / "a.whl", 42)
    text = str(error.value)
    assert len(text) < 4100
    assert "pypi-secretvalue" not in text and "eyJabc.def.ghi" not in text
    assert ENV["ACTIONS_ID_TOKEN_REQUEST_TOKEN"] not in text


def test_upload_timeout_is_finite(tmp_path, monkeypatch):
    def run(*args, **kwargs):
        raise subprocess.TimeoutExpired(args[0], kwargs["timeout"])
    monkeypatch.setattr(publish.subprocess, "run", run)
    with pytest.raises(publish.PublicationError, match="timed out"):
        publish.Upload(ENV)(tmp_path / "a.whl", 1)


def test_real_registry_http_is_bounded_and_missing_project_differs_from_release(monkeypatch):
    from urllib.error import HTTPError
    timeouts = []
    def urlopen(request, timeout):
        timeouts.append(timeout)
        raise HTTPError(request.full_url, 404, "not found", {}, None)
    monkeypatch.setattr(publish.urllib.request, "urlopen", urlopen)
    clock = FakeClock()
    registry = publish.Registry(publish.Budget(1500, clock, clock.sleep))
    assert registry.release("gukhanmun", VERSION) == []
    with pytest.raises(publish.PublicationError, match="seed all seven"):
        registry.project("gukhanmun")
    assert timeouts == [publish.HTTP_TIMEOUT, publish.HTTP_TIMEOUT]


@pytest.mark.parametrize("version", ["0.4.0.dev7+abc", "0.4.0-dev.7", "1!0.4.0", "0.4.0.post1"])
def test_nonpublication_version_spellings_fail_offline(release, version):
    with pytest.raises(publish.PublicationError, match="canonical"):
        publish.validate(release, version, env=ENV)


def test_feature_dev_zero_is_buildable_and_offline_valid(tmp_path, monkeypatch):
    monkeypatch.setattr(sys.modules[__name__], "VERSION", "0.4.0.dev0")
    directory = tmp_path / "dev-zero"
    directory.mkdir()
    env = {**ENV, "GITHUB_REF": "refs/heads/feature"}
    for name, description in a.expected_files(VERSION, REPOSITORY).items():
        write_package(directory / name, description)
    refresh(directory, source={"commit": SHA, "ref": env["GITHUB_REF"], "run_id": "123", "run_attempt": "1"})
    assert publish.validate(directory, VERSION, env=env)["version"] == "0.4.0.dev0"
    assert publish.load("release_context").resolve(env, root=REPOSITORY)["eligible"] is False


def test_manifest_pin_and_artifact_identity_are_required_before_network(harness):
    for artifact_id, pinned, message in (("", publish.digest(harness.release / "manifest.json"), "artifact ID"),
                                         ("0", publish.digest(harness.release / "manifest.json"), "artifact ID"),
                                         ("123", "0" * 64, "digest differs"), ("123", None, "digest is required")):
        with pytest.raises(publish.PublicationError, match=message):
            publish.publish(harness.release, VERSION, artifact_id, pinned, harness.receipt,
                            env=ENV, context=CONTEXT, head=SHA, registry=harness.registry,
                            uploader=harness.registry.upload)
    assert not harness.registry.calls


def test_unexpected_build_record_is_rejected(release):
    write_json(release / "build-info-imposter.json", {})
    change_manifest(release, lambda d: d["evidence"].append(a.describe(release / "build-info-imposter.json")))
    with pytest.raises(publish.PublicationError, match="eight"):
        publish.validate(release, VERSION, env=ENV)


def test_registry_visibility_transient_failure_is_bounded_and_retried(harness):
    original = harness.registry.simple
    count = 0
    def temporary(project):
        nonlocal count
        count += 1
        # Initial preflight reads all seven projects; transient visibility
        # failures after submission are retryable without resubmitting.
        if count == 8:
            raise publish.RegistryUnavailable("temporary registry outage")
        return original(project)
    harness.registry.simple = temporary
    assert harness.run()["status"] == "complete"
    assert len(harness.registry.uploads) == 31 and harness.clock.now > 0


def test_completed_resume_clears_a_transient_barrier_error(harness):
    harness.registry.remote = set(harness.registry.expected)
    harness.registry.simple_remote = set(harness.registry.expected)
    original = harness.registry.simple
    count = 0
    def temporary(project):
        nonlocal count
        count += 1
        if count == 8:
            raise publish.RegistryUnavailable("temporary registry outage")
        return original(project)
    harness.registry.simple = temporary
    result = harness.run()
    assert result["status"] == "complete" and result["last_error"] is None
    assert not harness.registry.uploads and harness.clock.now > 0


def test_simple_known_file_is_not_uploaded_while_json_catches_up(harness):
    names = set(harness.registry.expected)
    harness.registry.simple_remote = names
    original = harness.registry.release
    count = 0
    def delayed(project, version):
        nonlocal count
        count += 1
        if count > 7:
            harness.registry.remote = names
        return original(project, version)
    harness.registry.release = delayed
    assert harness.run()["status"] == "complete"
    assert not harness.registry.uploads


def test_registry_conflict_names_the_actual_conflicting_file(harness):
    project = sorted({e["distribution"] for e in harness.registry.expected.values()})[0]
    name = next(n for n, e in harness.registry.expected.items() if e["distribution"] == project)
    entry = harness.registry.file(name)
    entry["digests"]["sha256"] = "0" * 64
    harness.registry.extra = [entry]
    with pytest.raises(publish.PublicationError, match="conflict"):
        harness.run()
    assert json.loads(harness.receipt.read_text())["conflicting"] == [name]


def test_receipt_error_redacts_secrets_after_partial_failure(harness):
    def fail(path, timeout):
        raise RuntimeError("failed " + ENV["ACTIONS_ID_TOKEN_REQUEST_TOKEN"] + " pypi-sensitive eyJabc.def.ghi")
    with pytest.raises(publish.PublicationError):
        harness.run(uploader=fail)
    text = harness.receipt.read_text()
    assert ENV["ACTIONS_ID_TOKEN_REQUEST_TOKEN"] not in text
    assert "pypi-sensitive" not in text and "eyJabc.def.ghi" not in text
    assert "[REDACTED]" in text


def test_json_duplicate_keys_and_nonpositive_sizes_fail(release):
    path = release / "manifest.json"
    text = path.read_text()
    path.write_text(text.replace('"schema": 1', '"schema": 1, "schema": 1'))
    with pytest.raises(publish.PublicationError, match="duplicate JSON"):
        publish.validate(release, VERSION, env=ENV)


def test_simple_wrong_version_and_yanked_content_cannot_confirm(harness):
    expected = harness.registry.expected
    name = next(iter(expected))
    entry = harness.registry.file(name, simple=True)
    entry["yanked"] = "withdrawn"
    with pytest.raises(publish.PublicationError, match="yanked"):
        publish.inventory([entry], expected, simple=True, project="gukhanmun", version=VERSION)


def test_registry_response_identity_is_checked(monkeypatch):
    clock = FakeClock()
    registry = publish.Registry(publish.Budget(1500, clock, clock.sleep))
    monkeypatch.setattr(registry, "get", lambda *args, **kwargs: {"info": {"name": "imposter", "version": VERSION}, "urls": []})
    with pytest.raises(publish.PublicationError, match="project identity"):
        registry.project("gukhanmun")
    with pytest.raises(publish.PublicationError, match="release identity"):
        registry.release("gukhanmun", VERSION)
    monkeypatch.setattr(registry, "get", lambda *args, **kwargs: {"name": "imposter", "files": []})
    with pytest.raises(publish.PublicationError, match="Simple identity"):
        registry.simple("gukhanmun")


def test_partial_summary_reports_counts_and_missing_filenames(harness, tmp_path, capsys):
    harness.registry.failed = sorted(n for n, d in harness.registry.expected.items() if d["distribution"] != "gukhanmun")[1]
    with pytest.raises(publish.PublicationError):
        harness.run()
    summary = tmp_path / "summary.md"
    publish.summarize(harness.receipt, {"GITHUB_STEP_SUMMARY": str(summary)})
    assert "1 completed, 30 missing" in capsys.readouterr().out
    assert "30 missing" in summary.read_text() and harness.registry.failed in summary.read_text()


def test_redaction_covers_tokens_jwts_and_bearer_values():
    text = publish.redact("pypi-secret ghp_secret eyJabc.def.ghi Bearer privatevalue " + ENV["ACTIONS_ID_TOKEN_REQUEST_TOKEN"], ENV)
    assert text.count("[REDACTED]") == 5
    assert "privatevalue" not in text


class FakeResponse:
    def __init__(self, chunks, clock, seconds=0):
        self.chunks = iter(chunks)
        self.clock = clock
        self.seconds = seconds
        self.read_sizes = []
        self.closed = False

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.closed = True

    def read1(self, size):
        self.read_sizes.append(size)
        self.clock.now += self.seconds
        chunk = next(self.chunks, b"")
        assert len(chunk) <= size
        return chunk


def test_registry_reads_chunks_with_overall_deadline_checks(monkeypatch):
    clock = FakeClock()
    response = FakeResponse([b'{"name":', b'"example"}', b""], clock, seconds=1)
    timeouts = []
    def urlopen(request, timeout):
        timeouts.append(timeout)
        return response
    monkeypatch.setattr(publish.urllib.request, "urlopen", urlopen)
    registry = publish.Registry(publish.Budget(10, clock, clock.sleep))
    assert registry.get("https://registry.invalid/json") == {"name": "example"}
    assert timeouts == [10] and len(response.read_sizes) == 3 and response.closed
    assert all(size <= publish.RESPONSE_CHUNK_BYTES for size in response.read_sizes)


def test_registry_body_deadline_stops_before_reading_remaining_chunks(monkeypatch):
    clock = FakeClock()
    response = FakeResponse([b'{"name":', b'"example"}', b""], clock, seconds=3)
    monkeypatch.setattr(publish.urllib.request, "urlopen", lambda *args, **kwargs: response)
    registry = publish.Registry(publish.Budget(5, clock, clock.sleep))
    with pytest.raises(publish.PublicationError, match="deadline exceeded.*body"):
        registry.get("https://registry.invalid/json")
    assert len(response.read_sizes) == 2 and response.closed


def test_registry_expired_header_budget_prevents_first_body_read(monkeypatch):
    clock = FakeClock()
    response = FakeResponse([b"{}"], clock)
    def urlopen(*args, **kwargs):
        clock.now = 6
        return response
    monkeypatch.setattr(publish.urllib.request, "urlopen", urlopen)
    registry = publish.Registry(publish.Budget(5, clock, clock.sleep))
    with pytest.raises(publish.PublicationError, match="deadline exceeded.*body"):
        registry.get("https://registry.invalid/json")
    assert not response.read_sizes and response.closed


def test_registry_response_size_cap_is_checked_between_chunks(monkeypatch):
    clock = FakeClock()
    monkeypatch.setattr(publish, "MAX_RESPONSE_BYTES", 10)
    monkeypatch.setattr(publish, "RESPONSE_CHUNK_BYTES", 4)
    response = FakeResponse([b"1234", b"5678", b"901"], clock)
    monkeypatch.setattr(publish.urllib.request, "urlopen", lambda *args, **kwargs: response)
    registry = publish.Registry(publish.Budget(10, clock, clock.sleep))
    with pytest.raises(publish.PublicationError, match="too large"):
        registry.get("https://registry.invalid/json")
    assert response.read_sizes == [4, 4, 3] and response.closed


def test_summary_error_is_redacted_capped_and_html_escaped(tmp_path, capsys):
    receipt = tmp_path / "receipt.json"
    summary = tmp_path / "summary.md"
    write_json(receipt, {"status": "failed", "submitted": [], "completed": [], "missing": [], "conflicting": [],
                         "last_error": "<error> " + ENV["ACTIONS_ID_TOKEN_REQUEST_TOKEN"] + " pypi-secret " + "x" * 9000})
    publish.summarize(receipt, {**ENV, "GITHUB_STEP_SUMMARY": str(summary)})
    output = capsys.readouterr().out
    markdown = summary.read_text()
    assert "Last error:" in output and "Last error:" in markdown
    assert "[REDACTED]" in output and "[REDACTED]" in markdown
    assert "pypi-secret" not in output and ENV["ACTIONS_ID_TOKEN_REQUEST_TOKEN"] not in markdown
    assert "&lt;error&gt;" in markdown and "<error>" not in markdown
    assert len(output) < 4200 and len(markdown) < 4300


def test_validation_cli_reports_artifact_bytes(release, monkeypatch, capsys):
    monkeypatch.setattr(sys, "argv", ["publish.py", "validate", "--dir", str(release), "--version", VERSION])
    for key in ("GITHUB_ACTIONS", "GITHUB_RUN_ID", "GITHUB_REF", "GITHUB_SHA"):
        monkeypatch.delenv(key, raising=False)
    expected = sum(entry["size"] for entry in json.loads((release / "manifest.json").read_text())["artifacts"])
    assert publish.main() == 0
    assert f"{expected} artifact bytes" in capsys.readouterr().out
