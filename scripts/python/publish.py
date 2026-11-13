# Gukhanmun: Validates and publishes the original tested Python artifact set.
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

"""Offline validation and fail-closed, resumable production PyPI publication.

The manifest, not a receipt, is authoritative. Each invocation reconciles all
seven projects before submitting anything. Receipts describe partial progress;
a rerun recovers only by comparing the original files with the registry.
"""

import argparse
from dataclasses import asdict, dataclass
import email.parser
import hashlib
import html
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import time
import tomllib
from types import SimpleNamespace
import urllib.error
import urllib.parse
import urllib.request
import zipfile

from packaging.utils import canonicalize_name, parse_sdist_filename, parse_wheel_filename
from packaging.version import Version

ROOT = Path(os.environ.get("GUKHANMUN_REPO_ROOT", Path(__file__).resolve().parents[2]))
UPLOAD_URL = "https://upload.pypi.org/legacy/"
JSON_URL = "https://pypi.org/pypi"
SIMPLE_URL = "https://pypi.org/simple"
HTTP_TIMEOUT = 20
MAX_RESPONSE_BYTES = 16 * 1024 * 1024
RESPONSE_CHUNK_BYTES = 64 * 1024
UPLOAD_TIMEOUT = 180
VISIBILITY_WINDOW = 90
TOTAL_BUDGET = 25 * 60
SHA256 = re.compile(r"[0-9a-f]{64}")
COMMIT = re.compile(r"[0-9a-f]{40}")
POSITIVE = re.compile(r"[1-9][0-9]*")


def load(name):
    spec = importlib.util.spec_from_file_location(f"_gukhanmun_publish_{name}",
                                                 Path(__file__).with_name(f"{name}.py"))
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


artifacts = load("artifacts")


class PublicationError(RuntimeError):
    """An identity, integrity, visibility or upload failure stops publication."""


class RegistryUnavailable(PublicationError):
    """A transient registry read failure may be retried within visibility time."""


def require(condition, message):
    if not condition:
        raise PublicationError(message)


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def object_pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"), object_pairs_hook=object_pairs)


def safe_name(name):
    return (isinstance(name, str) and bool(name) and name not in {".", ".."}
            and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", name) is not None
            and "/" not in name and "\\" not in name)


def positive(value):
    return isinstance(value, str) and POSITIVE.fullmatch(value) is not None


def canonical_version(version):
    require(isinstance(version, str), "version must be a string")
    parsed = Version(version)
    require(str(parsed) == version and parsed.local is None and not parsed.epoch
            and parsed.post is None, "version must be canonical PEP 440 without a local label, epoch or post-release")
    return parsed


def source_identity(source, env):
    require(isinstance(source, dict), "manifest source must be an object")
    require(isinstance(source.get("commit"), str) and COMMIT.fullmatch(source["commit"]), "invalid source commit")
    require(isinstance(source.get("ref"), str) and source["ref"].startswith("refs/"), "invalid source ref")
    require(positive(source.get("run_id")) and positive(source.get("run_attempt")), "invalid source run identity")
    # Validation in Actions always binds to that run, including credential-free
    # feature/PR runs. Retained artifacts may come from an earlier attempt.
    if env.get("GITHUB_ACTIONS") == "true" or any(env.get(key) for key in
                                                ("GITHUB_RUN_ID", "GITHUB_SHA", "GITHUB_REF")):
        for field, key in (("commit", "GITHUB_SHA"), ("ref", "GITHUB_REF"), ("run_id", "GITHUB_RUN_ID")):
            require(bool(env.get(key)) and source[field] == env[key], f"manifest {field} differs from {key}")
        require(positive(env.get("GITHUB_RUN_ATTEMPT")) and
                int(source["run_attempt"]) <= int(env["GITHUB_RUN_ATTEMPT"]), "manifest run attempt is from the future")


def fields_for(text, name, version, label, root):
    fields = email.parser.Parser().parsestr(text)
    for key, expected in (("Name", name), ("Version", version)):
        values = fields.get_all(key) or []
        require(len(values) == 1, f"{label} requires exactly one {key}")
        if key == "Name":
            require(canonicalize_name(values[0]) == canonicalize_name(expected), f"{label} has the wrong Name")
        else:
            require(values[0] == expected, f"{label} has the wrong Version")
    requirements = fields.get_all("Requires-Dist") or []
    require(len(requirements) == len(set(requirements)), f"{label} has duplicate dependencies")
    if name == "gukhanmun":
        try:
            artifacts.check_extras(text, version, root, label)
            artifacts.load("checks").check_readme(
                text, (root / "python/gukhanmun/README.md").read_text(encoding="utf-8"), label=label)
        except RuntimeError as error:
            raise PublicationError(str(error)) from error
        # check_extras historically overwrites duplicate extra keys. Require
        # exactly one dependency and Provides-Extra for each intended extra.
        count = len(artifacts.dictionary_table(root).EXTRAS)
        require(len(requirements) == count, f"{label} has duplicate or missing extra dependencies")
    else:
        require(not requirements and not fields.get_all("Provides-Extra"), f"{label} data must have no dependencies/extras")
    return fields


def archive_names(names, label):
    require(len(names) == len(set(names)), f"{label} has duplicate archive members")
    for name in names:
        path = PurePosixPath(name)
        require(not path.is_absolute() and ".." not in path.parts and "\\" not in name,
                f"{label} has unsafe archive member {name!r}")


def metadata(path, description, version, root):
    name = description["distribution"]
    if description["kind"] == "wheel":
        with zipfile.ZipFile(path) as archive:
            names = archive.namelist()
            archive_names(names, path.name)
            intended = f"{name.replace('-', '_')}-{version}.dist-info/METADATA"
            require([member for member in names if member.endswith(".dist-info/METADATA")] == [intended],
                    f"{path.name} must have exactly its intended METADATA")
            fields_for(archive.read(intended).decode("utf-8"), name, version, path.name, root)
        return
    with tarfile.open(path, "r:gz") as archive:
        members = archive.getmembers()
        names = [member.name for member in members]
        archive_names(names, path.name)
        prefix = path.name.removesuffix(".tar.gz")
        info, project_path = f"{prefix}/PKG-INFO", f"{prefix}/pyproject.toml"
        require([member for member in names if member.endswith("/PKG-INFO")] == [info],
                f"{path.name} must have exactly its intended PKG-INFO")
        require(project_path in names, f"{path.name} lacks pyproject.toml")
        def read(member):
            entry = archive.getmember(member)
            require(entry.isfile(), f"{path.name}: {member} is not a regular member")
            return archive.extractfile(entry).read().decode("utf-8")
        fields_for(read(info), name, version, path.name, root)
        project = tomllib.loads(read(project_path)).get("project")
        require(isinstance(project, dict), f"{path.name} requires project metadata")
        require(project.get("name") == name and project.get("version") == version,
                f"{path.name} pyproject identity differs from packaged metadata")
        require(project.get("dependencies", []) == [], f"{path.name} has unexpected dependencies")
        expected = ({extra: [f"{distribution}=={version}"]
                     for extra, distribution in artifacts.dictionary_table(root).EXTRAS.items()}
                    if name == "gukhanmun" else {})
        require(project.get("optional-dependencies", {}) == expected,
                f"{path.name} pyproject extras differ from packaged metadata")
        require(not set(project.get("dynamic", [])) & {"name", "version", "dependencies", "optional-dependencies"},
                f"{path.name} has dynamic package identity/dependencies")


def validate(directory, version, *, env=None, manifest_sha256=None, root=ROOT):
    """Validate the complete flat artifact set without network or credentials."""
    env = os.environ if env is None else env
    directory = Path(directory)
    canonical_version(version)
    require(directory.is_dir() and not directory.is_symlink(), "artifact directory must be a real directory")
    for path in directory.iterdir():
        require(safe_name(path.name) and path.is_file() and not path.is_symlink(),
                f"unsafe or nonregular artifact entry {path.name!r}")
    manifest = directory / "manifest.json"
    require(manifest.is_file(), "missing manifest.json")
    if manifest_sha256 is not None:
        require(isinstance(manifest_sha256, str) and SHA256.fullmatch(manifest_sha256), "invalid pinned manifest digest")
        require(digest(manifest) == manifest_sha256, "pinned manifest digest differs")
    data = read_json(manifest)
    require(isinstance(data, dict) and type(data.get("schema")) is int and data["schema"] == artifacts.SCHEMA,
            "unsupported manifest schema")
    require(data.get("version") == version, "manifest version differs from expected version")
    require(isinstance(data.get("toolchain"), dict), "manifest toolchain must be an object")
    source_identity(data.get("source"), env)
    expected = artifacts.expected_files(version, root)
    require(len(expected) == 31, "release definition must contain exactly 31 files")
    seen, listed = set(), {}
    for key in ("artifacts", "evidence"):
        require(isinstance(data.get(key), list), f"manifest {key} must be a list")
        for entry in data[key]:
            require(isinstance(entry, dict), f"manifest {key} entry must be an object")
            name = entry.get("filename")
            require(safe_name(name) and name not in seen and name not in {"manifest.json", "SHA256SUMS"},
                    f"unsafe or duplicate manifest filename {name!r}")
            seen.add(name)
            require(type(entry.get("size")) is int and entry["size"] > 0, f"{name} size must be positive")
            require(isinstance(entry.get("sha256"), str) and SHA256.fullmatch(entry["sha256"]), f"{name} has invalid SHA-256")
            require((directory / name).is_file() and (directory / name).stat().st_size == entry["size"]
                    and digest(directory / name) == entry["sha256"], f"{name} content differs from manifest")
            if key == "artifacts":
                require(name in expected, f"unexpected distribution file {name}")
                require(all(entry.get(field) == value for field, value in expected[name].items()),
                        f"{name} has incorrect classification")
                if name.endswith(".whl"):
                    _, _, python, abi, platform = artifacts.wheel_tag(name)
                    require(entry.get("tags") == {"python": python, "abi": abi, "platform": platform},
                            f"{name} has incorrect tags")
                listed[name] = entry
            else:
                require(name.endswith(".json"), f"unexpected evidence filename {name}")
                require(isinstance(read_json(directory / name), dict), f"{name} evidence must be a JSON object")
    require(set(listed) == set(expected), "manifest must list exactly the expected 31 files")
    require({p.name for p in directory.iterdir()} == seen | {"manifest.json", "SHA256SUMS"},
            "artifact directory inventory differs from manifest")
    require({name for name in seen if name.startswith("build-info-")} ==
            {f"build-info-{item.id}.json" for item in artifacts.TARGETS},
            "release must contain exactly eight expected build-info records")
    for item in artifacts.TARGETS:
        filename = f"build-info-{item.id}.json"
        require(filename in seen, f"missing {filename}")
        record = read_json(directory / filename)
        require(isinstance(record, dict) and type(record.get("schema")) is int and record["schema"] == artifacts.SCHEMA,
                f"invalid build record {filename}")
        target_record = record.get("target")
        require(isinstance(target_record, dict) and target_record == asdict(item)
                and all(type(target_record.get(key)) is type(value) for key, value in asdict(item).items())
                and record.get("commit") == data["source"]["commit"],
                f"{filename} target or commit differs")
        wanted = {name: entry["sha256"] for name, entry in listed.items() if entry.get("target") == item.id}
        require(record.get("wheels") == wanted, f"{filename} does not identify all target wheel hashes")
    try:
        artifacts.verify(SimpleNamespace(dir=directory, complete=True))
    except SystemExit as error:
        raise PublicationError(str(error)) from error
    for name, description in expected.items():
        metadata(directory / name, description, version, root)
    return data


def upload_environment(env):
    """Keep only process plumbing, OIDC, and explicit TLS/proxy inputs."""
    allowed = {"PATH", "HOME", "USERPROFILE", "SYSTEMROOT", "SystemRoot", "WINDIR", "COMSPEC", "PATHEXT",
               "TEMP", "TMP", "TMPDIR", "LANG", "LC_ALL", "SSL_CERT_FILE", "SSL_CERT_DIR", "REQUESTS_CA_BUNDLE",
               "CURL_CA_BUNDLE", "UV_SYSTEM_CERTS", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY",
               "http_proxy", "https_proxy", "all_proxy", "no_proxy", "GITHUB_ACTIONS",
               "ACTIONS_ID_TOKEN_REQUEST_URL", "ACTIONS_ID_TOKEN_REQUEST_TOKEN"}
    return {key: value for key, value in env.items() if key in allowed}


def redact(message, env=None):
    message = str(message)
    for key, value in (env or {}).items():
        if value and len(value) >= 8 and ("TOKEN" in key.upper() or "PASSWORD" in key.upper()):
            message = message.replace(value, "[REDACTED]")
    message = re.sub(r"(?i)(?:pypi-|gh[pousr]_)[A-Za-z0-9_-]+", "[REDACTED]", message)
    message = re.sub(r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b", "[REDACTED]", message)
    message = re.sub(r"(?i)\b(bearer\s+)[^\s,;]+", r"\1[REDACTED]", message)
    return message[:4000]


class Upload:
    """Invoke the mise-pinned uv with Trusted Publishing and no fallback."""

    def __init__(self, env, executable="uv"):
        self.env = upload_environment(env)
        self.executable = executable

    def __call__(self, path, timeout):
        command = [self.executable, "--no-config", "publish", "--trusted-publishing", "always",
                   "--publish-url", UPLOAD_URL, "--keyring-provider", "disabled", str(path)]
        # Capture through temporary files so a broken child cannot fill memory.
        import tempfile
        with tempfile.TemporaryFile() as output:
            try:
                result = subprocess.run(command, env=self.env, stdout=output, stderr=output,
                                        timeout=timeout, check=False)
            except subprocess.TimeoutExpired as error:
                raise PublicationError("uv upload timed out") from error
            if result.returncode:
                size = output.tell()
                output.seek(max(0, size - 12000))
                message = output.read(12000).decode("utf-8", errors="replace")
                raise PublicationError(f"uv upload failed ({result.returncode}): {redact(message, self.env)}")


@dataclass
class Budget:
    """One monotonic deadline, shared by HTTP, visibility and upload calls."""

    end: float
    clock: object = time.monotonic
    sleep: object = time.sleep

    def remaining(self):
        return self.end - self.clock()

    def timeout(self, maximum):
        require(self.remaining() > 0, "publication deadline exceeded")
        return min(maximum, self.remaining())

    def before_upload(self):
        require(self.remaining() >= UPLOAD_TIMEOUT + VISIBILITY_WINDOW,
                "publication deadline leaves insufficient upload and reconciliation time")


class Registry:
    """Production JSON and PEP 691 clients; no credentials are sent here."""

    def __init__(self, budget):
        self.budget = budget

    def get(self, url, *, missing=False):
        """Read bounded chunks and check the overall deadline between reads.

        urllib's timeout bounds socket operations. Response headers and an
        already-blocking read can overrun the monotonic deadline until control
        returns; the Actions job timeout remains the final external bound.
        read1 avoids read(n)'s loop filling an entire large body at once.
        """
        request = urllib.request.Request(url, headers={"Accept": "application/vnd.pypi.simple.v1+json" if
                                                      "/simple/" in url else "application/json",
                                                      "Cache-Control": "no-cache"})
        try:
            with urllib.request.urlopen(request, timeout=self.budget.timeout(HTTP_TIMEOUT)) as response:
                raw = bytearray()
                while True:
                    require(self.budget.remaining() > 0, "publication deadline exceeded while reading registry body")
                    chunk = response.read1(min(RESPONSE_CHUNK_BYTES, MAX_RESPONSE_BYTES + 1 - len(raw)))
                    require(self.budget.remaining() > 0, "publication deadline exceeded while reading registry body")
                    raw.extend(chunk)
                    require(len(raw) <= MAX_RESPONSE_BYTES, "registry response is too large")
                    if not chunk:
                        return json.loads(raw, object_pairs_hook=object_pairs)
        except urllib.error.HTTPError as error:
            if missing and error.code == 404:
                return None
            error_class = RegistryUnavailable if error.code == 429 or error.code >= 500 else PublicationError
            raise error_class(f"registry HTTP {error.code}") from error
        except OSError as error:
            raise RegistryUnavailable(f"registry request failed: {redact(error)}") from error
        except ValueError as error:
            raise PublicationError(f"malformed registry JSON: {redact(error)}") from error

    def project(self, name):
        data = self.get(f"{JSON_URL}/{name}/json", missing=True)
        require(data is not None, f"PyPI project {name} is absent; seed all seven projects and register their publishers first")
        require(isinstance(data, dict) and isinstance(data.get("info"), dict)
                and canonicalize_name(data["info"].get("name", "")) == canonicalize_name(name),
                f"wrong registry project identity for {name}")

    def release(self, name, version):
        data = self.get(f"{JSON_URL}/{name}/{version}/json", missing=True)
        if data is None:
            return []
        require(isinstance(data, dict) and isinstance(data.get("info"), dict)
                and canonicalize_name(data["info"].get("name", "")) == canonicalize_name(name)
                and data["info"].get("version") == version and isinstance(data.get("urls"), list),
                f"wrong registry release identity for {name}")
        return data["urls"]

    def simple(self, name):
        data = self.get(f"{SIMPLE_URL}/{name}/")
        require(isinstance(data, dict) and data.get("name") == canonicalize_name(name)
                and isinstance(data.get("files"), list), f"wrong Simple identity for {name}")
        return data["files"]


def inventory(files, expected, *, simple=False, project=None, version=None):
    """Validate one release's exact inventory, filtering Simple by parsed version."""
    name = None
    def check(condition, message):
        if not condition:
            error = PublicationError(message)
            error.filename = name
            raise error
    check(isinstance(files, list), "registry files must be a list")
    found = {}
    for entry in files:
        check(isinstance(entry, dict), "registry file must be an object")
        name = entry.get("filename")
        check(safe_name(name), "registry filename is unsafe")
        if simple:
            try:
                parsed_name, parsed_version, *_ = (parse_wheel_filename(name) if name.endswith(".whl")
                                                  else parse_sdist_filename(name))
            except ValueError:
                check(name not in expected, f"cannot parse intended filename {name}")
                continue
            if parsed_version != Version(version) or canonicalize_name(parsed_name) != canonicalize_name(project):
                continue
        check(name not in found, f"registry duplicates {name}")
        check(name in expected, f"unexpected registry file at this version: {name}")
        digest_map = entry.get("hashes" if simple else "digests")
        check(isinstance(digest_map, dict) and isinstance(digest_map.get("sha256"), str)
                and SHA256.fullmatch(digest_map["sha256"]), f"invalid registry hash for {name}")
        check(type(entry.get("yanked")) is bool or (simple and isinstance(entry.get("yanked"), str)),
                f"invalid registry yanked state for {name}")
        check(entry["yanked"] is False, f"registry file is yanked: {name}")
        check(digest_map["sha256"] == expected[name]["sha256"], f"registry hash conflict for {name}")
        if not simple:
            check(type(entry.get("size")) is int and entry["size"] > 0
                    and entry["size"] == expected[name]["size"], f"registry size conflict for {name}")
        found[name] = entry
    return set(found)


def verify_upload_identity(data, version, artifact_id, manifest_sha256, env, context, head):
    require(positive(artifact_id), "artifact ID must be a nonempty positive integer")
    require(isinstance(manifest_sha256, str) and SHA256.fullmatch(manifest_sha256), "manifest digest is required")
    require(env.get("GITHUB_ACTIONS") == "true", "publication requires GitHub Actions")
    for key in ("GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_RUN_NUMBER"):
        require(positive(env.get(key)), f"publication requires positive {key}")
    require(isinstance(context, dict) and context.get("eligible") is True,
            f"current event is not eligible: {context.get('reason', 'missing context') if isinstance(context, dict) else 'missing context'}")
    require(context.get("version") == version == data["version"], "context output version differs from artifact version")
    source_identity(data["source"], env)
    require(head == data["source"]["commit"] == env.get("GITHUB_SHA"), "checkout HEAD differs from artifact SHA")
    for key in ("ACTIONS_ID_TOKEN_REQUEST_URL", "ACTIONS_ID_TOKEN_REQUEST_TOKEN"):
        require(isinstance(env.get(key), str) and bool(env[key].strip()), f"publication requires {key}")


def write_receipt(path, receipt):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(temporary, path)


def receipt_location(directory, path):
    """Receipts must never replace or add to the immutable input bundle."""
    require(not Path(path).resolve().is_relative_to(Path(directory).resolve()),
            "receipt must be outside the artifact directory")


def publish(directory, version, artifact_id, manifest_sha256, receipt_path, *, env=None,
            context=None, head=None, registry=None, uploader=None, budget=None, root=ROOT):
    """Reconcile then upload data, core wheels, core sdist, retaining a receipt.

    Tests inject all external boundaries. Production resolves eligibility again
    here; the CLI/job gate and a previous offline validation confer no authority.
    """
    env = os.environ if env is None else env
    budget = budget or Budget(time.monotonic() + TOTAL_BUDGET)
    receipt_location(directory, receipt_path)
    receipt = {"schema": 1, "artifact_id": artifact_id, "manifest_sha256": manifest_sha256,
               "version": version, "run_id": env.get("GITHUB_RUN_ID"), "ref": env.get("GITHUB_REF"),
               "commit": env.get("GITHUB_SHA"), "current_attempt": env.get("GITHUB_RUN_ATTEMPT"),
               "manifest_attempt": None, "submitted": [], "completed": [], "missing": [],
               "conflicting": [], "last_error": None, "status": "validating"}
    write_receipt(receipt_path, receipt)
    try:
        data = validate(directory, version, env=env, manifest_sha256=manifest_sha256, root=root)
        receipt["manifest_attempt"] = data["source"]["run_attempt"]
        if context is None:
            context = load("release_context").resolve(env, root=root)
        if head is None:
            head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, check=True, capture_output=True,
                                  text=True, timeout=HTTP_TIMEOUT).stdout.strip()
        verify_upload_identity(data, version, artifact_id, manifest_sha256, env, context, head)
        registry = registry or Registry(budget)
        uploader = uploader or Upload(env)
        entries = {entry["filename"]: entry for entry in data["artifacts"]}
        receipt["missing"] = sorted(entries)
        write_receipt(receipt_path, receipt)
        projects = sorted({entry["distribution"] for entry in entries.values()})
        grouped = {project: {name: entry for name, entry in entries.items() if entry["distribution"] == project}
                   for project in projects}
        def snapshot(simple=False):
            visible = set()
            for project in projects:
                files = registry.simple(project) if simple else registry.release(project, version)
                try:
                    visible |= inventory(files, grouped[project], simple=simple, project=project, version=version)
                except PublicationError as error:
                    name = getattr(error, "filename", None)
                    receipt["conflicting"] = [name] if name else []
                    raise
            return visible
        # Distinguish missing projects from absent versions before any upload.
        for project in projects:
            registry.project(project)
        present = snapshot()
        simple_present = snapshot(simple=True)
        known = present | simple_present
        receipt["completed"] = sorted(present & simple_present)
        receipt["missing"] = sorted(set(entries) - set(receipt["completed"]))
        receipt["status"] = "publishing"
        write_receipt(receipt_path, receipt)
        def confirm(wanted):
            original_end = budget.end
            budget.end = min(original_end, budget.clock() + VISIBILITY_WINDOW)
            pause = 1
            try:
                while True:
                    require(budget.remaining() > 0, "registry visibility deadline exceeded")
                    try:
                        visible = snapshot() & snapshot(simple=True)
                    except RegistryUnavailable as error:
                        receipt["last_error"] = redact(error, env)
                    else:
                        receipt["completed"] = sorted(visible)
                        receipt["missing"] = sorted(set(entries) - visible)
                        write_receipt(receipt_path, receipt)
                        if wanted <= visible:
                            return
                    write_receipt(receipt_path, receipt)
                    require(budget.remaining() > 0, "registry visibility deadline exceeded")
                    budget.sleep(min(pause, budget.remaining()))
                    pause = min(pause * 2, 10)
            finally:
                budget.end = original_end
        data_files = {name for name, entry in entries.items() if entry["distribution"] != "gukhanmun"}
        wheels = {name for name, entry in entries.items() if entry["distribution"] == "gukhanmun" and entry["kind"] == "wheel"}
        source = {name for name, entry in entries.items() if entry["distribution"] == "gukhanmun" and entry["kind"] == "sdist"}
        for group in (data_files, wheels, source):
            for name in sorted(group):
                # Existing JSON content may only be stale in Simple. Wait for
                # both views; never submit an identical known file again.
                if name in known:
                    continue
                budget.before_upload()
                receipt["submitted"].append(name)
                write_receipt(receipt_path, receipt)
                upload_error = None
                try:
                    uploader(Path(directory) / name, budget.timeout(UPLOAD_TIMEOUT))
                except Exception as error:
                    upload_error = redact(error, env)
                    receipt["last_error"] = upload_error
                    write_receipt(receipt_path, receipt)
                try:
                    confirm({name})
                except Exception as error:
                    if upload_error:
                        raise PublicationError(f"{upload_error}; remote content is unconfirmed: {redact(error, env)}. "
                                               "Stop and recover the original files; a deleted/reused filename requires a new version") from error
                    raise
                known.add(name)
                receipt["last_error"] = None
                write_receipt(receipt_path, receipt)
            # The first barrier confirms all 12 data files before core upload.
            confirm(group)
        confirm(set(entries))
        receipt["status"] = "complete"
        receipt["last_error"] = None
        write_receipt(receipt_path, receipt)
        return receipt
    except Exception as error:
        receipt["status"] = "failed"
        receipt["last_error"] = redact(error, env)
        write_receipt(receipt_path, receipt)
        raise PublicationError(receipt["last_error"]) from error


def summarize(receipt_path, env):
    """Expose partial progress in logs and the Actions summary, even on failure."""
    if receipt_path is None or not Path(receipt_path).is_file():
        return
    receipt = read_json(receipt_path)
    if "completed" not in receipt:
        return
    message = (f"Python publication {receipt['status']}: {len(receipt['completed'])} completed, "
               f"{len(receipt['missing'])} missing, {len(receipt['conflicting'])} conflicting.")
    print(message)
    last_error = redact(receipt.get("last_error"), env) if receipt.get("last_error") else None
    if last_error:
        print(f"Last error: {last_error}")
    if summary := env.get("GITHUB_STEP_SUMMARY"):
        with open(summary, "a", encoding="utf-8") as stream:
            stream.write(message + "\n\n")
            if last_error:
                stream.write(f"Last error:\n\n<pre>{html.escape(last_error)}</pre>\n\n")
            for key in ("submitted", "missing", "conflicting"):
                if receipt[key]:
                    stream.write(f"{key.capitalize()}:\n\n")
                    stream.writelines(f"- `{name}`\n" for name in receipt[key] if safe_name(name))
                    stream.write("\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("validate", "publish"))
    parser.add_argument("--dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--artifact-id")
    parser.add_argument("--manifest-sha256")
    parser.add_argument("--receipt", type=Path)
    arguments = parser.parse_args()
    try:
        if arguments.receipt is not None:
            receipt_location(arguments.dir, arguments.receipt)
        if arguments.command == "validate":
            data = validate(arguments.dir, arguments.version, manifest_sha256=arguments.manifest_sha256)
            if arguments.receipt:
                require(positive(arguments.artifact_id), "validation receipt requires artifact ID")
                require(arguments.manifest_sha256 is not None, "validation receipt requires manifest digest")
                write_receipt(arguments.receipt, {"schema": 1, "status": "validated", "artifact_id": arguments.artifact_id,
                                                 "manifest_sha256": arguments.manifest_sha256, "version": arguments.version,
                                                 "source": data["source"], "current_attempt": os.environ.get("GITHUB_RUN_ATTEMPT")})
            total_bytes = sum(entry["size"] for entry in data["artifacts"])
            print(f"Validated 31 Python artifacts at {arguments.version}, {total_bytes} artifact bytes")
        else:
            require(arguments.receipt is not None, "publication requires --receipt")
            publish(arguments.dir, arguments.version, arguments.artifact_id, arguments.manifest_sha256,
                    arguments.receipt)
            print(f"Published and confirmed all 31 Python artifacts at {arguments.version}")
        summarize(arguments.receipt, os.environ)
        return 0
    except (PublicationError, OSError, ValueError, KeyError, TypeError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(f"Python {arguments.command} failed: {redact(error, os.environ)}", file=sys.stderr)
        summarize(arguments.receipt, os.environ)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
