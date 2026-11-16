# Gukhanmun: Publish the original, natively tested npm CLI candidate through OIDC.
# Copyright (C) 2026  Hong Minhee
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program. If not, see <https://www.gnu.org/licenses/>.
"""No packing, token fallback, bootstrap, dist-tag repair, or arbitrary-run recovery."""

import argparse
import base64
import copy
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

REGISTRY = "https://registry.npmjs.org/"
REPOSITORY = "https://github.com/dahlia/gukhanmun"
WORKFLOW = ".github/workflows/main.yaml"
PARENT = "@gukhanmun/cli"
VERSION = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-dev\.(0|[1-9]\d*))?$")
SETUP = (
    "Complete the documented seven-package bootstrap and direct-publish trusted "
    "publisher setup first; verify dev never exposes latest. No token fallback "
    "or automated tag repair is supported."
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def version_parts(version):
    match = VERSION.fullmatch(version)
    require(match is not None, f"Unrecognized registry version/tag: {version!r}")
    return tuple(int(v) if v is not None else None for v in match.groups())


def publication_channel(manifest, env):
    source = manifest["source"]
    require(env.get("GITHUB_EVENT_NAME") == "push", "Only push events may publish")
    require(env.get("GITHUB_REPOSITORY") == "dahlia/gukhanmun", "Not the upstream repository")
    require(env.get("RUNNER_ENVIRONMENT") == "github-hosted", "OIDC requires a GitHub-hosted runner")
    for field, variable in (("commit", "GITHUB_SHA"), ("ref", "GITHUB_REF"), ("run_id", "GITHUB_RUN_ID")):
        require(str(source[field]) == env.get(variable), f"Candidate source mismatch: {field}")
    ref = source["ref"]
    version = manifest["npm_version"]
    parts = version_parts(version)
    if ref.startswith("refs/tags/"):
        require(ref == f"refs/tags/{version}" and parts[3] is None, "Stable version/tag mismatch")
        require(manifest["version"] == version, "Stable tarball must have the exact tag version")
        return "latest"
    require(ref == "refs/heads/main" or re.fullmatch(r"refs/heads/\d+\.\d+-maintenance", ref),
            "Ineligible branch")
    require(parts[3] is not None and parts[3] > 0, "Refuse nonpublishing dev.0 candidates")
    require(parts[3] == int(env["GITHUB_RUN_NUMBER"]), "Development run number mismatch")
    require(manifest["version"].startswith(version + "+")
            and source["commit"].startswith(manifest["version"].split("+", 1)[1]),
            "Development build metadata must identify the source commit")
    return "dev"


def sanitized_env(root, original):
    # Allowlist, rather than trying to enumerate token/config spelling variants.
    allowed = {"PATH", "SystemRoot", "SYSTEMROOT", "WINDIR", "COMSPEC", "PATHEXT",
               "TEMP", "TMP", "TMPDIR", "CI", "RUNNER_ENVIRONMENT"}
    env = {key: value for key, value in original.items()
           if key in allowed or (key.startswith("GITHUB_") and "TOKEN" not in key and "AUTH" not in key)
           or key in {"ACTIONS_ID_TOKEN_REQUEST_URL", "ACTIONS_ID_TOKEN_REQUEST_TOKEN"}}
    home = root / "home"
    home.mkdir(exist_ok=True)
    for name in ("user.npmrc", "global.npmrc"):
        (root / name).write_text("", encoding="utf-8")
    env.update(HOME=str(home), USERPROFILE=str(home),
               NPM_CONFIG_USERCONFIG=str(root / "user.npmrc"),
               NPM_CONFIG_GLOBALCONFIG=str(root / "global.npmrc"),
               NPM_CONFIG_CACHE=str(root / "cache"),
               NPM_CONFIG_REGISTRY=REGISTRY)
    return env


class Npm:
    def __init__(self, root, original):
        self.root = root
        self.env = sanitized_env(root, original)
        self.executable = shutil.which("npm")
        require(self.executable is not None, "npm is required")

    def run(self, *args, cwd=None, input=None, check=True):
        return subprocess.run([self.executable, *args, "--registry", REGISTRY],
                              cwd=cwd or self.root, env=self.env, text=True,
                              input=input, capture_output=True, check=check)

    def check_tools(self):
        node = subprocess.check_output(["node", "--version"], env=self.env, text=True).strip()
        npm = self.run("--version").stdout.strip()
        require(node == "v26.13.1" and npm == "11.20.0",
                f"Publisher requires pinned Node26.13.1/npm11.20.0, got {node}/{npm}")
        require(all(self.env.get(key) for key in
                    ("ACTIONS_ID_TOKEN_REQUEST_URL", "ACTIONS_ID_TOKEN_REQUEST_TOKEN")),
                "GitHub OIDC request variables are missing")

    def audit(self, name, version):
        # npm performs signature and Sigstore cryptographic verification. The
        # separate statement inspection below checks this candidate's identity.
        with tempfile.TemporaryDirectory(dir=self.root) as directory:
            cwd = Path(directory)
            (cwd / "package.json").write_text('{"name":"cli-provenance-check","private":true}')
            self.run("install", "--ignore-scripts", "--force", "--no-audit", "--no-fund",
                     f"{name}@{version}", cwd=cwd)
            self.run("audit", "signatures", cwd=cwd)

    def publish(self, tarball, channel):
        try:
            self.run("publish", str(tarball.resolve()), "--ignore-scripts", "--provenance",
                     "--access", "public", "--tag", channel)
        except subprocess.CalledProcessError:
            # Do not copy credentialed npm stdout/stderr to logs or receipts.
            raise ValueError(f"OIDC publication failed. {SETUP}") from None


class Registry:
    def get(self, url):
        require(url.startswith(REGISTRY), "Refuse registry redirect outside registry.npmjs.org")
        request = urllib.request.Request(url, headers={"Accept": "application/json",
                                                       "Cache-Control": "no-cache"})
        with urllib.request.urlopen(request, timeout=30) as response:
            require(response.url.startswith(REGISTRY), "Unexpected registry redirect")
            return json.load(response)

    def package(self, name):
        try:
            return self.get(REGISTRY + urllib.parse.quote(name, safe=""))
        except urllib.error.HTTPError as error:
            if error.code == 404:
                raise ValueError(f"Package {name} is missing. {SETUP}") from error
            raise

    def statement(self, metadata):
        url = metadata["dist"].get("attestations", {}).get("url")
        require(url is not None, "Existing version has no provenance; refuse recovery")
        response = self.get(url)
        statements = []
        for entry in response.get("attestations", []):
            if entry.get("predicateType") == "https://slsa.dev/provenance/v1":
                payload = entry["bundle"]["dsseEnvelope"]["payload"]
                statements.append(json.loads(base64.b64decode(payload, validate=True)))
        require(len(statements) == 1, "Expected exactly one npm SLSA provenance statement")
        return statements[0]


def verify_statement(statement, package, manifest):
    digest = base64.b64decode(package["integrity"].removeprefix("sha512-"), validate=True).hex()
    subject = statement.get("subject", [])
    purl = "pkg:npm/" + package["name"].replace("@", "%40", 1) + "@" + manifest["npm_version"]
    require(len(subject) == 1 and subject[0].get("name") == purl
            and subject[0].get("digest", {}).get("sha512") == digest,
            "Provenance subject does not match the original tarball")
    require(statement.get("predicateType") == "https://slsa.dev/provenance/v1",
            "Unsupported provenance predicate")
    require(statement.get("_type") == "https://in-toto.io/Statement/v1",
            "Unsupported provenance statement type")
    predicate = statement["predicate"]
    definition = predicate["buildDefinition"]
    source = manifest["source"]
    require(definition["buildType"] ==
            "https://slsa-framework.github.io/github-actions-buildtypes/workflow/v1",
            "Unexpected provenance build type")
    require(definition["internalParameters"]["github"]["event_name"] == "push",
            "Provenance publication event is not push")
    require(definition["externalParameters"]["workflow"] ==
            {"repository": REPOSITORY, "path": WORKFLOW, "ref": source["ref"]},
            "Provenance workflow/repository/ref mismatch")
    require({"uri": f"git+{REPOSITORY}@{source['ref']}",
             "digest": {"gitCommit": source["commit"]}} in definition["resolvedDependencies"],
            "Provenance source commit mismatch")
    details = predicate["runDetails"]
    require(details["builder"]["id"] == "https://github.com/actions/runner/github-hosted",
            "Provenance is not from a hosted runner")
    invocation = details["metadata"]["invocationId"]
    require(re.fullmatch(re.escape(f"{REPOSITORY}/actions/runs/{source['run_id']}/attempts/")
                         + r"[1-9]\d*", invocation), "Provenance publication run mismatch")


def reconcile(documents, manifest, channel):
    """Validate the whole seven-package inventory before any registry write."""
    candidate = manifest["npm_version"]
    major, minor, patch, dev = version_parts(candidate)
    snapshots = {}
    for package in manifest["packages"]:
        name = package["name"]
        doc = documents[name]
        versions = doc.get("versions", {})
        tags = doc.get("dist-tags", {})
        require(isinstance(tags, dict) and isinstance(versions, dict), "Malformed npm inventory")
        for tag in ("latest", "dev"):
            if tag in tags:
                parts = version_parts(tags[tag])
                # npm can assign latest to the first development publication.
                # Later dev uploads must preserve it, not require it to be stable.
                require(tag == "latest" or parts[3] is not None, f"Invalid {name} {tag} tag")
                require(tags[tag] in versions, f"Dangling {name} {tag} tag")
        for published in versions:
            if (published == "0.0.0-stage" and isinstance(versions[published], dict)
                    and versions[published].get("description") ==
                    "Temporary package placeholder for staged publishing"):
                continue
            # This distribution has only stable and dev.N versions.
            pmajor, pminor, ppatch, pdev = version_parts(published)
            if channel == "dev" and pdev is not None:
                require(pdev <= dev, f"Stale development candidate for {name}: {published}")
                require(pdev != dev or published == candidate,
                        f"Run number already used with another base for {name}")
            if channel == "latest" and pdev is None and (pmajor, pminor) == (major, minor):
                require(ppatch <= patch, f"Newer stable version exists for {name}: {published}")
        if candidate in versions:
            require(tags.get(channel) == candidate,
                    f"Existing {name}@{candidate} has an anomalous tag; operator repair required")
        snapshots[name] = copy.deepcopy(tags)
    return snapshots


class Publisher:
    def __init__(self, directory, manifest, channel, registry, npm, sleep=time.sleep):
        self.directory, self.manifest, self.channel = directory, manifest, channel
        self.registry, self.npm, self.sleep = registry, npm, sleep
        self.snapshots = {}
        self.completed = []

    def documents(self):
        return {p["name"]: self.registry.package(p["name"]) for p in self.manifest["packages"]}

    def validate_existing(self, package, document):
        metadata = document.get("versions", {}).get(self.manifest["npm_version"])
        if metadata is None:
            return False
        require(metadata.get("dist", {}).get("integrity") == package["integrity"],
                f"Conflicting original bytes for {package['name']}")
        verify_statement(self.registry.statement(metadata), package, self.manifest)
        self.npm.audit(package["name"], self.manifest["npm_version"])
        return True

    def unchanged(self, documents):
        reconcile(documents, self.manifest, self.channel)
        for name, tags in self.snapshots.items():
            require(documents[name].get("dist-tags", {}) == tags,
                    f"Registry tags changed externally for {name}; refuse next write")

    def visible(self, package):
        # Only absence/incomplete visibility is retried. Conflicting content,
        # unexpected tags, and invalid provenance are never silently repaired.
        for attempt in range(8):
            doc = self.registry.package(package["name"])
            metadata = doc.get("versions", {}).get(self.manifest["npm_version"])
            tags = doc.get("dist-tags", {})
            original = self.snapshots[package["name"]]
            expected = dict(original)
            expected[self.channel] = self.manifest["npm_version"]
            require(tags == original or tags == expected,
                    f"Unexpected tags after publishing {package['name']}")
            if metadata:
                require(metadata.get("dist", {}).get("integrity") == package["integrity"],
                        f"Conflicting original bytes for {package['name']}")
            if metadata and metadata.get("dist", {}).get("attestations") and tags == expected:
                self.validate_existing(package, doc)
                self.snapshots[package["name"]] = expected
                return
            if attempt < 7:
                self.sleep(min(2 ** attempt, 15))
        raise ValueError(f"Registry visibility timed out for {package['name']}; rerun only this job")

    def publish(self):
        documents = self.documents()
        self.snapshots = reconcile(documents, self.manifest, self.channel)
        # Every conflict/provenance check must finish before the first upload.
        existing = {p["name"]: self.validate_existing(p, documents[p["name"]])
                    for p in self.manifest["packages"]}
        ordered = sorted(self.manifest["packages"], key=lambda p: p["name"] == PARENT)
        for package in ordered:
            self.unchanged(self.documents())
            if not existing[package["name"]]:
                self.npm.publish(self.directory / package["filename"], self.channel)
                self.visible(package)
            self.completed.append(package["name"])
        self.unchanged(self.documents())


def real_installs(npm, manifest, channel):
    """Exercise public channel lookup and exact identity in isolated installations."""
    version = manifest["version"]
    specs = [f"{PARENT}@{channel}", f"{PARENT}@{manifest['npm_version']}"]
    if channel == "latest":
        specs.append(PARENT)
    for index, spec in enumerate(specs):
        cwd = npm.root / f"install-{index}"
        cwd.mkdir()
        prefix = cwd / "prefix"
        npm.run("install", "--global", "--ignore-scripts", "--no-audit", "--no-fund",
                "--prefix", str(prefix), "--cache", str(cwd / "global-cache"), spec, cwd=cwd)
        command = prefix / "bin" / "gukhanmun"
        env = dict(npm.env)
        for executable in ([str(command)],
                           [npm.executable, "exec", "--yes", "--ignore-scripts",
                            "--cache", str(cwd / "npx-cache"),
                            "--registry", REGISTRY, "--package", spec, "--", "gukhanmun"]):
            def run(*args, input=None):
                return subprocess.run([*executable, *args], cwd=cwd, env=env,
                                      input=input, text=True, capture_output=True)
            result = run("--version")
            require(result.returncode == 0 and version in result.stdout,
                    f"Installed full/native version mismatch for {spec}")
            for extra, expected in (([], "기찻길 색깔론 도쿄"),
                                    (["--no-bundled-dictionaries"], "기거길 색깔론 동경")):
                result = run("--disambiguation", "off", *extra, input="汽車길 色깔論 東京")
                require(result.returncode == 0 and result.stdout.strip() == expected,
                        f"Real registry dictionary witness failed for {spec}")
            require(run("--definitely-invalid-option").returncode != 0,
                    "Launcher lost native failure status")
        installed = json.loads((prefix / "lib/node_modules/@gukhanmun/cli/package.json").read_text())
        require(installed["version"] == version, "Installed parent manifest version mismatch")
    npm.audit(PARENT, manifest["npm_version"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", type=Path, required=True)
    parser.add_argument("--artifact-id", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    receipt = {"artifact_id": args.artifact_id, "manifest_sha256": args.manifest_sha256,
               "run_id": os.environ.get("GITHUB_RUN_ID"),
               "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT"), "completed": []}
    publisher = None
    try:
        # Same-directory imports are intentional; all helpers are stdlib only.
        from artifacts import validate_bundle
        manifest = validate_bundle(args.dir, args.manifest_sha256, args.artifact_id)
        channel = publication_channel(manifest, os.environ)
        head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
        require(head == manifest["source"]["commit"], "Checkout HEAD differs from candidate")
        receipt.update(source=manifest["source"], version=manifest["version"], channel=channel)
        with tempfile.TemporaryDirectory(prefix="npm-cli-publish-") as directory:
            npm = Npm(Path(directory), os.environ)
            npm.check_tools()
            publisher = Publisher(args.dir, manifest, channel, Registry(), npm)
            publisher.publish()
            real_installs(npm, manifest, channel)
            publisher.unchanged(publisher.documents())
        receipt["status"] = "verified"
    except Exception as error:
        receipt.update(status="failed", error=str(error))
        raise
    finally:
        if publisher is not None:
            receipt["completed"] = publisher.completed
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        if os.environ.get("GITHUB_STEP_SUMMARY"):
            with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as summary:
                summary.write(
                    f"\n### npm CLI publication: {receipt['status']}\n\n"
                    f"Original artifact: `{args.artifact_id}`; manifest: `{args.manifest_sha256}`.\n\n"
                    f"Confirmed packages: {len(receipt['completed'])}/7. "
                    "On failure rerun only this publisher with the original candidate, "
                    "never rebuild or rerun all jobs after an upload.\n"
                )


if __name__ == "__main__":
    main()
