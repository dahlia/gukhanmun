# Gukhanmun: Tests npm CLI archive boundaries and immutable candidate validation.
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
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import artifacts


def tar(path, files, modes=None):
    with tarfile.open(path, "w:gz" if path.suffix == ".tgz" else "w:bz2") as archive:
        for name, data in files.items():
            member = tarfile.TarInfo(name)
            member.size = len(data)
            member.mode = (modes or {}).get(name, 0o644)
            archive.addfile(member, io.BytesIO(data))


class Candidate:
    """Synthetic binaries deliberately do not count as native execution."""

    def __init__(self, directory):
        self.directory = Path(directory)
        self.revision = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=artifacts.ROOT, text=True).strip()
        self.version = "0.4.0-dev.10+" + self.revision[:8]
        self.source = {"commit": self.revision, "ref": "", "run_id": "", "run_attempt": ""}
        self.completions = {name: f"# {shell}\n".encode() for shell, name in artifacts.SHELL_FILES.items()}
        self.completion_manifest = {
            "schema_version": 1, "command": "gukhanmun", "version": self.version,
            "source_revision": self.revision, "worktree_dirty": True,
            "files": {name: artifacts.sha256(data) for name, data in self.completions.items()},
        }
        self.completions["manifest.json"] = json.dumps(self.completion_manifest).encode()
        self.manifest = {
            "schema_version": 1, "version": self.version, "npm_version": artifacts.npm_version(self.version),
            "source": self.source, "completions": self.completion_manifest,
            "archives": {}, "packages": [],
        }
        self.files, self.modes = {}, {}
        for target, spec in artifacts.TARGETS.items():
            suffix = ".zip" if spec["os"] == "win32" else ".tar.bz2"
            self.manifest["archives"][target] = {
                "filename": f"gukhanmun-{self.version}-{target}{suffix}", "sha256": "a" * 64,
            }
            metadata = {"name": f"{artifacts.PARENT}-{target}", "version": self.version,
                        "os": [spec["os"]], "cpu": [spec["cpu"]], "license": "SEE LICENSE IN LICENSE"}
            files = {spec["binary"]: b"synthetic native payload\n",
                     "LICENSE": (artifacts.ROOT / "LICENSE").read_bytes(),
                     "NOTICE.md": (artifacts.ROOT / "NOTICE.md").read_bytes()}
            for dictionary in ("loanwords", "opendict"):
                files[f"ATTRIBUTION.{dictionary}.md"] = (
                    artifacts.ROOT / f"crates/gukhanmun-{dictionary}/ATTRIBUTION.md").read_bytes()
            binary_hash = artifacts.sha256(files[spec["binary"]])
            files["provenance.json"] = json.dumps({
                "schema_version": 1, "source": self.source, "version": self.version,
                "target": target, "archive": self.manifest["archives"][target],
                "binary_sha256": binary_hash,
            }).encode()
            self.add(target, metadata, files, {spec["binary"]: 0o755}, binary_hash)
        metadata = {"name": artifacts.PARENT, "version": self.version, "type": "commonjs",
                    "bin": {"gukhanmun": "bin/gukhanmun.cjs"}, "engines": {"node": ">=22"},
                    "optionalDependencies": {f"{artifacts.PARENT}-{key}": self.version for key in artifacts.TARGETS}}
        files = {"LICENSE": (artifacts.ROOT / "LICENSE").read_bytes(),
                 "bin/gukhanmun.cjs": (artifacts.ROOT / "packages/cli/bin/gukhanmun.cjs").read_bytes(),
                 "lib/launcher.cjs": (artifacts.ROOT / "packages/cli/lib/launcher.cjs").read_bytes(),
                 **{f"completions/{key}": value for key, value in self.completions.items()}}
        self.add(None, metadata, files, {"bin/gukhanmun.cjs": 0o755})
        self.save()

    def add(self, target, metadata, files, modes, binary_hash=None):
        record = {"name": metadata["name"], "version": self.version,
                  "filename": f"gukhanmun-cli{('-' + target) if target else ''}-{self.version}.tgz"}
        if target:
            record.update(target=target, binary_sha256=binary_hash)
        files["package.json"] = json.dumps(metadata).encode()
        self.files[target] = files
        self.modes[target] = modes
        self.manifest["packages"].append(record)
        self.repack(target)

    def repack(self, target):
        record = next(item for item in self.manifest["packages"] if item.get("target") == target)
        files = {"package/" + key: value for key, value in self.files[target].items()}
        modes = {"package/" + key: value for key, value in self.modes[target].items()}
        path = self.directory / record["filename"]
        tar(path, files, modes)
        data = path.read_bytes()
        record.update(sha256=artifacts.sha256(data), integrity=artifacts.integrity(data))

    def save(self):
        artifacts.write_json(self.directory / "manifest.json", self.manifest)


class ArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="npm-cli-test-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.environment = patch.dict(os.environ, {
            "GITHUB_SHA": "", "GITHUB_REF": "", "GITHUB_RUN_ID": "", "GITHUB_RUN_ATTEMPT": "",
        })
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.candidate = Candidate(self.directory)

    def validate(self):
        self.candidate.save()
        digest = artifacts.sha256((self.directory / "manifest.json").read_bytes())
        return artifacts.validate_bundle(self.directory, digest, "12345")

    def test_complete_candidate(self):
        self.assertEqual(self.validate()["version"], self.candidate.version)

    def test_integrity_and_manifest_digest(self):
        with self.assertRaisesRegex(RuntimeError, "manifest SHA"):
            artifacts.validate_bundle(self.directory, "0" * 64)
        record = self.candidate.manifest["packages"][0]
        with (self.directory / record["filename"]).open("ab") as stream:
            stream.write(b"changed")
        with self.assertRaisesRegex(RuntimeError, "tarball hash"):
            self.validate()

    def test_normalized_identity(self):
        self.assertEqual(artifacts.npm_version("1.2.3-dev.10+abc123"), "1.2.3-dev.10")
        for value in ("1.2", "01.2.3", "../x", "1.2.3-dev.01+a", "1.2.3\n", "1.2.3+xyz"):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                artifacts.npm_version(value)
        for version in ("0.4.0-dev.10", "0.4.0-dev.10+00000000", "0.4.0+abc"):
            with self.subTest(version=version), self.assertRaises(RuntimeError):
                artifacts.validate_version_source(version, self.candidate.revision)

    def test_inventory_is_exact(self):
        (self.directory / "surprise").write_text("extra")
        with self.assertRaisesRegex(RuntimeError, "unexpected files"):
            self.validate()

    def test_source_bound_to_context_and_head(self):
        with patch.dict(os.environ, {"GITHUB_RUN_ID": "1234"}):
            with self.assertRaisesRegex(RuntimeError, "run_id"):
                self.validate()
        with patch.dict(os.environ, {"GITHUB_RUN_ATTEMPT": "9"}):
            self.validate()
        self.candidate.source["commit"] = "0" * 40
        with self.assertRaisesRegex(RuntimeError, "HEAD"):
            self.validate()

    def test_allowlist_rejects_rehashed_extra_member(self):
        self.candidate.files[None]["node_modules/evil.js"] = b"bad"
        self.candidate.repack(None)
        with self.assertRaisesRegex(RuntimeError, "allowlist"):
            self.validate()

    def test_parent_metadata_and_exact_full_pins(self):
        original = self.candidate.files[None]["package.json"]
        mutations = (
            ("scripts", {"postinstall": "curl something"}),
            ("optionalDependencies", {f"{artifacts.PARENT}-{key}": artifacts.npm_version(self.candidate.version)
                                      for key in artifacts.TARGETS}),
            ("dependencies", {"unapproved": "1.0.0"}),
            ("engines", {"node": ">=26"}),
        )
        for key, value in mutations:
            with self.subTest(key=key):
                metadata = json.loads(original)
                metadata[key] = value
                self.candidate.files[None]["package.json"] = json.dumps(metadata).encode()
                self.candidate.repack(None)
                with self.assertRaises(RuntimeError):
                    self.validate()

    def test_platform_constraints_not_libc(self):
        target = next(iter(artifacts.TARGETS))
        metadata = json.loads(self.candidate.files[target]["package.json"])
        metadata["libc"] = ["musl"]
        self.candidate.files[target]["package.json"] = json.dumps(metadata).encode()
        self.candidate.repack(target)
        with self.assertRaisesRegex(RuntimeError, "constraints"):
            self.validate()

    def test_modes_notices_binary_provenance(self):
        target = next(iter(artifacts.TARGETS))
        original = copy.deepcopy(self.candidate.files[target])
        binary = artifacts.TARGETS[target]["binary"]
        for name, value in ((binary, b"modified"), ("LICENSE", b"changed"),
                            ("provenance.json", b"{}")):
            with self.subTest(name=name):
                self.candidate.files[target] = dict(original, **{name: value})
                self.candidate.repack(target)
                with self.assertRaises(RuntimeError):
                    self.validate()
        self.candidate.files[target] = original
        self.candidate.modes[target][binary] = 0o644
        self.candidate.repack(target)
        with self.assertRaisesRegex(RuntimeError, "0755"):
            self.validate()

    def test_completion_hash_and_revision(self):
        self.candidate.files[None]["completions/gukhanmun.bash"] = b"modified"
        self.candidate.repack(None)
        with self.assertRaisesRegex(RuntimeError, "hash mismatch"):
            self.validate()

    def test_tar_member_traversal_and_symlink(self):
        path = self.directory / "bad.tar.bz2"
        for name in ("../outside", "/absolute", "package/../escape", "package\\escape"):
            tar(path, {name: b"bad"})
            with self.subTest(name=name), self.assertRaisesRegex(RuntimeError, "unsafe"):
                artifacts.archive_files(path)
        with tarfile.open(path, "w:bz2") as archive:
            link = tarfile.TarInfo("link")
            link.type = tarfile.SYMTYPE
            link.linkname = "/outside"
            archive.addfile(link)
        with self.assertRaisesRegex(RuntimeError, "nonregular"):
            artifacts.archive_files(path)

    def test_duplicate_archive_member(self):
        path = self.directory / "duplicate.tar.bz2"
        with tarfile.open(path, "w:bz2") as archive:
            for _ in range(2):
                entry = tarfile.TarInfo("same")
                archive.addfile(entry, io.BytesIO(b""))
        with self.assertRaisesRegex(RuntimeError, "duplicate"):
            artifacts.archive_files(path)

    def test_duplicate_json_fields(self):
        with self.assertRaisesRegex(RuntimeError, "duplicate"):
            artifacts.read_json(b'{"version":"a","version":"b"}')

    def test_config_isolation(self):
        with patch.dict(os.environ, {"npm_config_@gukhanmun:registry": "https://bad.invalid",
                                     "NODE_OPTIONS": "--require=bad", "NODE_AUTH_TOKEN": "secret"}):
            env = artifacts.isolated_env(self.directory / "env")
        self.assertNotIn("NODE_OPTIONS", env)
        self.assertNotIn("NODE_AUTH_TOKEN", env)
        self.assertNotIn("npm_config_@gukhanmun:registry", env)
        self.assertEqual(env["NPM_CONFIG_IGNORE_SCRIPTS"], "true")

    @unittest.skipUnless(shutil.which("node") and shutil.which("npm"), "Node.js/npm unavailable")
    @unittest.skipIf(os.name == "nt", "The producer packs once on POSIX; Windows consumes those tarballs")
    def test_actual_npm_pack_all_seven_and_archive_completion_binding(self):
        archives = self.directory / "archives"
        completions = self.directory / "completions"
        output = self.directory / "packed"
        archives.mkdir()
        completions.mkdir()
        for name, data in self.candidate.completions.items():
            (completions / name).write_bytes(data)
        for target, spec in artifacts.TARGETS.items():
            record = self.candidate.manifest["archives"][target]
            files = {spec["binary"]: self.candidate.files[target][spec["binary"]],
                     "LICENSE": (artifacts.ROOT / "LICENSE").read_bytes(),
                     "NOTICE.md": (artifacts.ROOT / "NOTICE.md").read_bytes(),
                     **{f"completions/{name}": data for name, data in self.candidate.completions.items()}}
            path = archives / record["filename"]
            if spec["os"] == "win32":
                with zipfile.ZipFile(path, "w") as archive:
                    for name, data in files.items():
                        archive.writestr(name, data)
            else:
                tar(path, files)
        result = artifacts.pack(archives, completions, output, self.candidate.version, self.candidate.revision)
        self.assertEqual(len(result["packages"]), 7)
        digest = artifacts.sha256((output / "manifest.json").read_bytes())
        self.assertEqual(artifacts.validate_bundle(output, digest)["version"], self.candidate.version)
        self.assertTrue(all(self.candidate.version in item["filename"] for item in result["packages"]))
        with self.assertRaisesRegex(RuntimeError, "empty"):
            artifacts.pack(archives, completions, output, self.candidate.version, self.candidate.revision)


if __name__ == "__main__":
    unittest.main()
