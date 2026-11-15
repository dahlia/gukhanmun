# Gukhanmun: Test npm CLI publication policy with an in-memory registry.
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
"""No unit test contacts a registry, executes npm, or mutates remote state."""

import base64
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

import publish


def candidate(version="0.4.0-dev.10"):
    return {
        "version": version + "+abcdef" if "-dev." in version else version,
        "npm_version": version,
        "source": {"commit": "abcdef" * 6 + "abcd", "ref": "refs/heads/main",
                   "run_id": "42", "run_attempt": "1"},
        "packages": [
            {"name": name, "filename": name.split("/")[-1] + ".tgz",
             "integrity": "sha512-" + base64.b64encode(b"x" * 64).decode()}
            for name in [publish.PARENT, *[f"@gukhanmun/cli-platform-{n}" for n in range(6)]]
        ],
    }


def provenance(package, manifest):
    source = manifest["source"]
    return {
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{"name": "pkg:npm/" + package["name"].replace("@", "%40", 1)
                     + "@" + manifest["npm_version"], "digest": {"sha512": (b"x" * 64).hex()}}],
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://slsa-framework.github.io/github-actions-buildtypes/workflow/v1",
                "externalParameters": {"workflow": {"repository": publish.REPOSITORY,
                                                   "path": publish.WORKFLOW, "ref": source["ref"]}},
                "internalParameters": {"github": {"event_name": "push", "repository_id": "123",
                                                  "repository_owner_id": "456"}},
                "resolvedDependencies": [{"uri": f"git+{publish.REPOSITORY}@{source['ref']}",
                                          "digest": {"gitCommit": source["commit"]}}],
            },
            "runDetails": {
                "builder": {"id": "https://github.com/actions/runner/github-hosted"},
                "metadata": {"invocationId": f"{publish.REPOSITORY}/actions/runs/42/attempts/2"},
            },
        },
    }


class FakeRegistry:
    def __init__(self, manifest, previous="0.4.0-dev.9"):
        self.manifest = manifest
        self.docs = {p["name"]: {"versions": {previous: {}}, "dist-tags": {"dev": previous}}
                     for p in manifest["packages"]}
        self.reads = 0
        self.change = None

    def package(self, name):
        self.reads += 1
        if self.change:
            self.change(self)
        return copy.deepcopy(self.docs[name])

    def statement(self, metadata):
        return metadata["statement"]

    def add(self, package, channel="dev"):
        self.docs[package["name"]]["versions"][self.manifest["npm_version"]] = {
            "dist": {"integrity": package["integrity"], "attestations": {"url": "fake"}},
            "statement": provenance(package, self.manifest),
        }
        self.docs[package["name"]]["dist-tags"][channel] = self.manifest["npm_version"]


class FakeNpm:
    def __init__(self, registry):
        self.registry, self.uploads, self.audits = registry, [], []
        self.interrupt = None

    def audit(self, name, version):
        self.audits.append((name, version))

    def default_absent(self, name):
        pass

    def publish(self, tarball, channel):
        package = next(p for p in self.registry.manifest["packages"] if p["filename"] == tarball.name)
        self.uploads.append(package["name"])
        self.registry.add(package, channel)
        if self.interrupt == len(self.uploads):
            raise ValueError("Response lost after successful upload")


class BundledNpmTests(unittest.TestCase):
    def bundled_npm(self):
        node = shutil.which("node")
        if not node:
            self.skipTest("Node.js is not installed")
        bindir = Path(node).resolve().parent
        candidates = [bindir / "node_modules/npm", bindir.parent / "lib/node_modules/npm"]
        npm = shutil.which("npm")
        if npm:
            candidates.append(Path(npm).resolve().parent.parent)
        for root in candidates:
            if (root / "node_modules/npm-pick-manifest/lib/index.js").is_file():
                return node, root
        self.skipTest("Bundled npm implementation is unavailable")

    def test_actual_pick_manifest_default_errors_are_narrowly_recognized(self):
        node, root = self.bundled_npm()
        script = r"""
const pick = require(process.argv[1] + '/node_modules/npm-pick-manifest');
const name = '@gukhanmun/cli';
const dev = {name, version:'0.4.0-dev.10'};
const inventories = [
  {name, versions:{'0.4.0-dev.10':dev}, 'dist-tags':{dev:dev.version}},
  {name, versions:{}, 'dist-tags':{}},
  {name, versions:{'0.4.0-dev.10':dev}, 'dist-tags':{dev:dev.version, latest:dev.version}},
];
console.log(JSON.stringify(inventories.map(packument => {
  try { pick(packument, '*'); return {returncode:0, stdout:'{}'}; }
  catch (error) {
    return {returncode:1, stdout:JSON.stringify({error:{code:error.code, summary:error.message}})};
  }
})));
"""
        results = json.loads(subprocess.check_output([node, "-e", script, str(root)], text=True))
        self.assertEqual([json.loads(r["stdout"]).get("error", {}).get("code") for r in results],
                         ["ETARGET", "ENOVERSIONS", None])
        for result, expected in zip(results, (True, True, False)):
            self.assertEqual(publish.default_resolution_absent(
                subprocess.CompletedProcess([], **result), publish.PARENT), expected)

    def test_actual_npm1120_provenance_generator_matches_fixture(self):
        node, root = self.bundled_npm()
        if json.loads((root / "package.json").read_text())["version"] != "11.20.0":
            self.skipTest("Exact provenance fixture test requires publisher npm11.20.0")
        # Only signing is mocked. Execute npm's unmodified primary-source
        # generator and npm-package-arg's actual PURL construction, offline.
        script = r"""
const root = process.argv[1];
const directory = root + '/node_modules/libnpmpublish/lib';
const sigstore = require.resolve('sigstore', {paths:[directory]});
require.cache[sigstore] = {id:sigstore, filename:sigstore, loaded:true,
  exports:{attest:async data => JSON.parse(data.toString())}};
const ci = require.resolve('ci-info', {paths:[directory]});
require.cache[ci] = {id:ci, filename:ci, loaded:true,
  exports:{GITHUB_ACTIONS:true, GITLAB:false}};
const npa = require(root + '/node_modules/npm-package-arg');
const subject = {name:npa.toPurl(npa('@gukhanmun/cli@0.4.0-dev.10')),
  digest:{sha512:Buffer.alloc(64, 'x').toString('hex')}};
require(directory + '/provenance.js').generateProvenance([subject], {})
  .then(statement => console.log(JSON.stringify(statement)));
"""
        manifest = candidate()
        env = {
            "PATH": os.environ["PATH"], "GITHUB_SERVER_URL": "https://github.com",
            "GITHUB_REPOSITORY": "dahlia/gukhanmun",
            "GITHUB_WORKFLOW_REF": "dahlia/gukhanmun/.github/workflows/main.yaml@refs/heads/main",
            "GITHUB_EVENT_NAME": "push", "GITHUB_REF": manifest["source"]["ref"],
            "GITHUB_SHA": manifest["source"]["commit"], "GITHUB_RUN_ID": "42",
            "GITHUB_RUN_ATTEMPT": "2", "GITHUB_REPOSITORY_ID": "123",
            "GITHUB_REPOSITORY_OWNER_ID": "456", "RUNNER_ENVIRONMENT": "github-hosted",
        }
        statement = json.loads(subprocess.check_output(
            [node, "-e", script, str(root)], text=True, env=env))
        self.assertEqual(statement, provenance(manifest["packages"][0], manifest))
        publish.verify_statement(statement, manifest["packages"][0], manifest)


class PolicyTests(unittest.TestCase):
    def test_default_gate_rejects_unrelated_errors_and_malformed_output(self):
        for code, summary in (
            ("E401", "Unable to authenticate"),
            ("ENOTFOUND", "No versions available for @gukhanmun/cli"),
            ("ETARGET", "No matching version found for @gukhanmun/cli@dev."),
            ("ETARGET", "No matching version found for @other/package@*."),
            ("ENOVERSIONS", "No versions available for @other/package"),
        ):
            result = subprocess.CompletedProcess([], 1, stdout=json.dumps(
                {"error": {"code": code, "summary": summary}}), stderr="ENOVERSIONS")
            self.assertFalse(publish.default_resolution_absent(result, publish.PARENT))
        for stdout in ("not json", "null", "[]", '{"error":null}'):
            result = subprocess.CompletedProcess([], 1, stdout=stdout)
            self.assertFalse(publish.default_resolution_absent(result, publish.PARENT))

    def test_default_gate_does_not_copy_auth_failures_to_exception(self):
        with tempfile.TemporaryDirectory() as directory:
            npm = publish.Npm(Path(directory), {"PATH": os.environ["PATH"]})
            result = subprocess.CompletedProcess([], 1, stdout='{"error":{"code":"E401"}}',
                                                 stderr="token-secret")
            with mock.patch.object(npm, "run", return_value=result), self.assertRaises(ValueError) as error:
                npm.default_absent(publish.PARENT)
            self.assertNotIn("token-secret", str(error.exception))

    def test_publish_does_not_copy_credentialed_output_to_exception(self):
        with tempfile.TemporaryDirectory() as directory:
            npm = publish.Npm(Path(directory), {"PATH": os.environ["PATH"]})
            failure = subprocess.CalledProcessError(1, ["npm", "publish"],
                                                    output="secret-out", stderr="secret-token")
            with mock.patch.object(npm, "run", side_effect=failure), self.assertRaises(ValueError) as error:
                npm.publish(Path("candidate.tgz"), "dev")
            self.assertNotIn("secret", str(error.exception))
            self.assertTrue(error.exception.__suppress_context__)

    def documents(self, manifest, previous="0.4.0-dev.9"):
        return FakeRegistry(manifest, previous).docs

    def test_dev_numeric_order_and_cross_base(self):
        manifest = candidate()
        publish.reconcile(self.documents(manifest), manifest, "dev")
        publish.reconcile(self.documents(manifest, "0.3.9-dev.9"), manifest, "dev")
        for previous in ("0.3.9-dev.11", "0.3.9-dev.10", "0.4.0-dev.100"):
            with self.subTest(previous=previous), self.assertRaises(ValueError):
                publish.reconcile(self.documents(manifest, previous), manifest, "dev")

    def test_any_of_seven_stale_inventories_blocks(self):
        manifest = candidate()
        docs = self.documents(manifest)
        docs[manifest["packages"][-1]["name"]]["versions"]["0.1.0-dev.11"] = {}
        with self.assertRaisesRegex(ValueError, "Stale"):
            publish.reconcile(docs, manifest, "dev")

    def test_maintenance_latest_can_move_across_lines_not_back_within_line(self):
        manifest = candidate("0.3.6")
        docs = self.documents(manifest, "0.4.0")
        for doc in docs.values():
            doc["dist-tags"] = {"latest": "0.4.0"}
        publish.reconcile(docs, manifest, "latest")
        manifest = candidate("0.4.1")
        for doc in docs.values():
            doc["versions"]["0.4.2"] = {}
            doc["versions"]["0.3.6"] = {}
            doc["dist-tags"] = {"latest": "0.3.6"}
        with self.assertRaisesRegex(ValueError, "Newer stable"):
            publish.reconcile(docs, manifest, "latest")

    def test_invalid_dangling_and_wrong_channel_tags(self):
        manifest = candidate()
        for tags in ({"dev": "garbage"}, {"latest": "0.4.0-dev.9"}, {"dev": "0.4.0-dev.8"}):
            docs = self.documents(manifest)
            next(iter(docs.values()))["dist-tags"] = tags
            with self.subTest(tags=tags), self.assertRaises(ValueError):
                publish.reconcile(docs, manifest, "dev")

    def test_existing_candidate_requires_selected_tag_no_automatic_repair(self):
        manifest = candidate()
        registry = FakeRegistry(manifest)
        registry.add(manifest["packages"][0])
        registry.docs[publish.PARENT]["dist-tags"]["dev"] = "0.4.0-dev.9"
        with self.assertRaisesRegex(ValueError, "operator repair"):
            publish.reconcile(registry.docs, manifest, "dev")

    def test_event_ref_source_and_dev_zero_guards(self):
        manifest = candidate()
        env = {"GITHUB_EVENT_NAME": "push", "GITHUB_REPOSITORY": "dahlia/gukhanmun",
               "RUNNER_ENVIRONMENT": "github-hosted", "GITHUB_SHA": manifest["source"]["commit"],
               "GITHUB_REF": manifest["source"]["ref"], "GITHUB_RUN_ID": "42",
               "GITHUB_RUN_NUMBER": "10"}
        self.assertEqual(publish.publication_channel(manifest, env), "dev")
        for key, value in (("GITHUB_EVENT_NAME", "pull_request"),
                           ("GITHUB_REPOSITORY", "fork/gukhanmun"),
                           ("RUNNER_ENVIRONMENT", "self-hosted"),
                           ("GITHUB_SHA", "bad"), ("GITHUB_RUN_ID", "43"),
                           ("GITHUB_RUN_NUMBER", "9")):
            with self.subTest(key=key), self.assertRaises(ValueError):
                publish.publication_channel(manifest, dict(env, **{key: value}))
        with self.assertRaises(ValueError):
            publish.publication_channel(candidate("0.4.0-dev.0"), env)
        stable = candidate("0.4.0")
        stable["source"]["ref"] = "refs/tags/0.4.0"
        self.assertEqual(publish.publication_channel(stable, dict(env, GITHUB_REF="refs/tags/0.4.0")), "latest")

    def test_auth_and_configuration_are_allowlisted_not_inherited(self):
        with tempfile.TemporaryDirectory() as directory:
            env = publish.sanitized_env(Path(directory), {
                "PATH": "/tools", "HOME": "/unsafe", "NPM_TOKEN": "secret",
                "NODE_AUTH_TOKEN": "secret", "GITHUB_TOKEN": "secret",
                "npm_config_registry": "https://evil", "NODE_OPTIONS": "--require evil",
                "ACTIONS_ID_TOKEN_REQUEST_TOKEN": "oidc", "GITHUB_SHA": "commit"})
            self.assertEqual(env["ACTIONS_ID_TOKEN_REQUEST_TOKEN"], "oidc")
            self.assertEqual(env["GITHUB_SHA"], "commit")
            for key in ("NPM_TOKEN", "NODE_AUTH_TOKEN", "GITHUB_TOKEN", "npm_config_registry", "NODE_OPTIONS"):
                self.assertNotIn(key, env)
            self.assertNotEqual(env["HOME"], "/unsafe")

    def test_provenance_subject_source_workflow_run_and_host(self):
        manifest = candidate()
        package = manifest["packages"][0]
        statement = provenance(package, manifest)
        publish.verify_statement(statement, package, manifest)
        mutations = [
            lambda s: s.update(_type="https://in-toto.io/Statement/v0.1"),
            lambda s: s["subject"][0]["digest"].update(sha512="bad"),
            lambda s: s["subject"][0].update(name="pkg:npm/other@0.4.0"),
            lambda s: s["predicate"]["buildDefinition"]["externalParameters"]["workflow"].update(path="other.yaml"),
            lambda s: s["predicate"]["buildDefinition"]["externalParameters"]["workflow"].update(repository="https://github.com/evil/repo"),
            lambda s: s["predicate"]["buildDefinition"]["resolvedDependencies"][0]["digest"].update(gitCommit="bad"),
            lambda s: s["predicate"]["buildDefinition"]["internalParameters"]["github"].update(event_name="pull_request"),
            lambda s: s["predicate"]["runDetails"]["metadata"].update(invocationId=f"{publish.REPOSITORY}/actions/runs/43/attempts/1"),
            lambda s: s["predicate"]["runDetails"]["builder"].update(id="self-hosted"),
        ]
        for mutation in mutations:
            changed = copy.deepcopy(statement)
            mutation(changed)
            with self.assertRaises(ValueError):
                publish.verify_statement(changed, package, manifest)


class PublisherTests(unittest.TestCase):
    def setup_publisher(self):
        manifest = candidate()
        registry = FakeRegistry(manifest)
        npm = FakeNpm(registry)
        publisher = publish.Publisher(Path("/candidate"), manifest, "dev", registry, npm, sleep=lambda _: None)
        return manifest, registry, npm, publisher

    def test_payloads_first_parent_last_and_audited(self):
        manifest, registry, npm, publisher = self.setup_publisher()
        publisher.publish()
        self.assertEqual(npm.uploads[-1], publish.PARENT)
        self.assertEqual(len(npm.uploads), 7)
        self.assertEqual(len(npm.audits), 7)

    def test_all_conflicts_preflight_before_any_upload(self):
        manifest, registry, npm, publisher = self.setup_publisher()
        package = manifest["packages"][-1]
        registry.add(package)
        registry.docs[package["name"]]["versions"][manifest["npm_version"]]["dist"]["integrity"] = "bad"
        with self.assertRaisesRegex(ValueError, "Conflicting"):
            publisher.publish()
        self.assertEqual(npm.uploads, [])

    def test_bootstrap_default_gate_precedes_every_upload(self):
        manifest, registry, npm, publisher = self.setup_publisher()
        def default_available(name):
            raise ValueError("Registry exposes dev through unqualified install")
        npm.default_absent = default_available
        with self.assertRaisesRegex(ValueError, "unqualified"):
            publisher.publish()
        self.assertEqual(npm.uploads, [])

    def test_partial_response_loss_recovers_only_identical_provenance(self):
        manifest, registry, npm, publisher = self.setup_publisher()
        npm.interrupt = 3
        with self.assertRaisesRegex(ValueError, "Response lost"):
            publisher.publish()
        npm.interrupt = None
        publisher.publish()
        self.assertEqual(len(npm.uploads), 7)
        self.assertEqual(npm.uploads[-1], publish.PARENT)

    def test_partial_missing_or_wrong_provenance_is_not_skipped(self):
        for bad in ("missing", "wrong"):
            manifest, registry, npm, publisher = self.setup_publisher()
            package = manifest["packages"][-1]
            registry.add(package)
            meta = registry.docs[package["name"]]["versions"][manifest["npm_version"]]
            if bad == "missing":
                del meta["statement"]
            else:
                meta["statement"]["subject"][0]["digest"]["sha512"] = "bad"
            with self.assertRaises((KeyError, ValueError)):
                publisher.publish()
            self.assertEqual(npm.uploads, [])

    def test_external_nonselected_tag_change_blocks_next_write(self):
        manifest, registry, npm, publisher = self.setup_publisher()
        def mutate(r):
            if npm.uploads:
                r.docs[publish.PARENT]["versions"]["0.3.0"] = {}
                r.docs[publish.PARENT]["dist-tags"]["latest"] = "0.3.0"
        registry.change = mutate
        with self.assertRaisesRegex(ValueError, "changed externally"):
            publisher.publish()
        self.assertEqual(len(npm.uploads), 1)

    def test_visibility_is_bounded_and_parent_never_published_on_timeout(self):
        manifest, registry, npm, publisher = self.setup_publisher()
        npm.publish = lambda path, channel: npm.uploads.append(path.name)
        with self.assertRaisesRegex(ValueError, "visibility timed out"):
            publisher.publish()
        self.assertEqual(len(npm.uploads), 1)
        self.assertNotEqual(npm.uploads[0], "cli.tgz")


if __name__ == "__main__":
    unittest.main()
