# Gukhanmun: Tests completion bundle provenance, host builds and failure paths.
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

import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import generate as g


REVISION = "1234567890abcdef" * 2 + "12345678"
OTHER_REVISION = "f" * 40
FILENAMES = {"gukhanmun.bash", "_gukhanmun", "gukhanmun.fish", "_gukhanmun.ps1", "gukhanmun.nu"}


class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.4.0"\n', encoding="utf-8")
        self.directory = self.root / "bundle"
        self.binary = self.root / "host binary.exe"
        self.binary.touch()
        self.calls = []
        self.dirty = b" M Cargo.toml\n"
        self.fail_shell = None
        self.cli_version = b"gukhanmun 0.4.0\n"

    def fake_run(self, command, **kwargs):
        self.calls.append(command)
        self.assertEqual(kwargs.get("root"), self.root)
        if command == ["git", "rev-parse", "HEAD"]:
            return (REVISION + "\n").encode()
        if command == ["git", "status", "--porcelain=v1", "-z", "--untracked-files=all"]:
            return self.dirty
        if command == [str(self.binary), "--version"]:
            return self.cli_version
        if command[:2] == [str(self.binary), "--generate-completion"]:
            shell = command[2]
            if shell == self.fail_shell:
                raise RuntimeError("completion generation failed")
            return f"# {shell} for gukhanmun\r\n# 한글\r\n".encode()
        self.fail(f"unexpected command: {command}")

    def generate(self, **kwargs):
        with patch.object(g, "run", side_effect=self.fake_run):
            return g.generate(self.directory, binary=self.binary, root=self.root, **kwargs)

    def manifest(self, change):
        path = self.directory / "manifest.json"
        value = json.loads(path.read_bytes())
        change(value)
        path.write_text(json.dumps(value), encoding="utf-8")

    def snapshot(self):
        return {path.name: path.read_bytes() for path in self.directory.iterdir() if path.is_file()}

    def test_generation_is_deterministic_and_records_actual_identity(self):
        manifest = self.generate(revision=REVISION, version="0.4.0")
        self.assertEqual(manifest["source_revision"], REVISION)
        self.assertEqual(manifest["version"], "0.4.0")
        self.assertTrue(manifest["worktree_dirty"])
        self.assertEqual(set(manifest["files"]), FILENAMES)
        self.assertEqual(set(self.snapshot()), FILENAMES | {"manifest.json"})
        for name in FILENAMES:
            data = (self.directory / name).read_bytes()
            self.assertNotIn(b"\r", data)
            self.assertEqual(hashlib.sha256(data).hexdigest(), manifest["files"][name])
        self.assertEqual((self.directory / "manifest.json").read_bytes(),
                         (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode())
        before = self.snapshot()
        self.generate(revision=REVISION, version="0.4.0")
        self.assertEqual(before, self.snapshot())
        self.assertEqual(g.verify(self.directory, REVISION, "0.4.0"), manifest)

    def test_builds_once_and_does_not_use_an_installed_cli(self):
        with patch.object(g, "run", side_effect=self.fake_run), \
                patch.object(g, "build_host_binary", return_value=self.binary) as build:
            g.generate(self.directory, root=self.root)
        build.assert_called_once_with(self.root)
        self.assertEqual([call[2] for call in self.calls if call[1:2] == ["--generate-completion"]],
                         ["bash", "zsh", "fish", "powershell", "nushell"])

    def test_dirtiness_before_or_after_build_is_preserved(self):
        for initial, final in ((b" M Cargo.toml\0", b""), (b"", b" M Cargo.toml\0")):
            self.dirty = initial

            def build(root):
                self.dirty = final
                return self.binary

            with self.subTest(initial=initial, final=final), \
                    patch.object(g, "run", side_effect=self.fake_run), \
                    patch.object(g, "build_host_binary", side_effect=build):
                manifest = g.generate(self.directory, root=self.root)
            self.assertTrue(manifest["worktree_dirty"])
            with self.assertRaisesRegex(RuntimeError, "dirty worktree"):
                g.verify(self.directory, require_clean=True)

    def test_untracked_outputs_do_not_dirty_real_checkout(self):
        original_run = g.run
        git_env = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}

        def git(*arguments):
            return original_run(["git", *arguments], root=self.root, env=git_env)

        git("init", "--quiet")
        source = self.root / "source.rs"
        source.write_text("original source\n", encoding="utf-8")
        git("add", "Cargo.toml", self.binary.name, source.name)
        git("-c", "user.name=Test", "-c", "user.email=test@example.com",
            "-c", "commit.gpgsign=false", "-c", f"core.hooksPath={self.root / '.git' / 'no-hooks'}",
            "commit", "--quiet", "-m", "Initial fixture")
        self.directory = self.root / "completion [files] 한글"

        def execute(command, **kwargs):
            if command[0] == "git":
                return original_run(command, **kwargs, env=git_env)
            return self.fake_run(command, **kwargs)

        with patch.object(g, "run", side_effect=execute):
            first = g.generate(self.directory, binary=self.binary, root=self.root)
            before = self.snapshot()
            second = g.generate(self.directory, binary=self.binary, root=self.root)
            self.assertFalse(first["worktree_dirty"])
            self.assertEqual(first, second)
            self.assertEqual(before, self.snapshot())
            g.verify(self.directory, require_clean=True)
            notes = self.directory / "notes.txt"
            notes.write_text("unrelated user file\n", encoding="utf-8")
            self.assertTrue(g.generate(self.directory, binary=self.binary, root=self.root)["worktree_dirty"])
            notes.unlink()
            source.write_text("changed source\n", encoding="utf-8")
            self.assertTrue(g.generate(self.directory, binary=self.binary, root=self.root)["worktree_dirty"])
            git("restore", source.name)
            git("add", str(self.directory.relative_to(self.root)))
            git("-c", "user.name=Test", "-c", "user.email=test@example.com",
                "-c", "commit.gpgsign=false", "-c", f"core.hooksPath={self.root / '.git' / 'no-hooks'}",
                "commit", "--quiet", "-m", "Track bundle fixture")
            (self.directory / "gukhanmun.bash").write_bytes(b"changed tracked output\n")
            self.assertTrue(g.generate(self.directory, binary=self.binary, root=self.root)["worktree_dirty"])

    def test_checkout_change_during_generation_leaves_existing_bundle_untouched(self):
        self.generate()
        (self.directory / "notes.txt").write_bytes(b"keep this user file")
        before = self.snapshot()
        self.dirty = b""
        for phase in ("build", "last shell"):
            checkout_revision = REVISION

            def build(root):
                nonlocal checkout_revision
                self.assertEqual(root, self.root)
                if phase == "build":
                    checkout_revision = OTHER_REVISION
                return self.binary

            def execute(command, **kwargs):
                nonlocal checkout_revision
                result = self.fake_run(command, **kwargs)
                if phase == "last shell" and command == [str(self.binary), "--generate-completion", "nushell"]:
                    checkout_revision = OTHER_REVISION
                if command == ["git", "rev-parse", "HEAD"]:
                    return (checkout_revision + "\n").encode()
                return result

            with self.subTest(phase=phase), \
                    patch.object(g, "run", side_effect=execute), \
                    patch.object(g, "build_host_binary", side_effect=build), \
                    patch.object(g, "publish") as publish, \
                    self.assertRaisesRegex(RuntimeError, "checkout revision changed"):
                g.generate(self.directory, root=self.root, revision=REVISION, version="0.4.0")
            publish.assert_not_called()
            self.assertEqual(before, self.snapshot())

    def test_wrong_expected_version_and_revision_fail_before_build_or_publish(self):
        for arguments, message in (({"version": "9.9.9"}, "workspace version"),
                                   ({"revision": OTHER_REVISION}, "checkout revision"),
                                   ({"revision": "HEAD"}, "full lowercase Git object ID")):
            with self.subTest(arguments=arguments), \
                    patch.object(g, "build_host_binary") as build, \
                    patch.object(g, "run", side_effect=self.fake_run), \
                    self.assertRaisesRegex(RuntimeError, message):
                g.generate(self.directory, root=self.root, **arguments)
            build.assert_not_called()
            self.assertFalse(self.directory.exists())

    def test_wrong_binary_version_never_generates_scripts(self):
        self.cli_version = b"gukhanmun 0.3.0\n"
        with self.assertRaisesRegex(RuntimeError, "CLI version"):
            self.generate()
        self.assertFalse(self.directory.exists())
        self.assertFalse(any("--generate-completion" in call for call in self.calls))

    def test_last_shell_failure_leaves_existing_bundle_and_user_file_untouched(self):
        self.generate()
        (self.directory / "notes.txt").write_bytes(b"keep this user file")
        before = self.snapshot()
        self.fail_shell = "nushell"
        with patch.object(g.os, "replace") as replace, \
                self.assertRaisesRegex(RuntimeError, "completion generation failed"):
            self.generate()
        replace.assert_not_called()
        self.assertEqual(before, self.snapshot())
        self.assertFalse(any(path.is_dir() for path in self.directory.iterdir()))

    def test_staging_write_failure_does_not_publish_any_file(self):
        self.generate()
        before = self.snapshot()
        original = Path.write_bytes

        def write(path, data):
            if path.parent.name.startswith(".gukhanmun-completions-") and path.name == "manifest.json":
                raise OSError("disk full")
            return original(path, data)

        with patch.object(Path, "write_bytes", write), patch.object(g.os, "replace") as replace, \
                self.assertRaisesRegex(OSError, "disk full"):
            self.generate()
        replace.assert_not_called()
        self.assertEqual(before, self.snapshot())
        self.assertFalse(any(path.is_dir() for path in self.directory.iterdir()))

    def test_directory_destination_is_rejected_before_replacing_other_files(self):
        self.generate()
        (self.directory / "gukhanmun.nu").unlink()
        (self.directory / "gukhanmun.nu").mkdir()
        before = self.snapshot()
        with patch.object(g.os, "replace") as replace, self.assertRaisesRegex(RuntimeError, "destination is a directory"):
            self.generate()
        replace.assert_not_called()
        self.assertEqual(before, self.snapshot())

    def test_atomic_replaces_publish_manifest_last_and_preserve_user_files(self):
        self.generate()
        (self.directory / "notes.txt").write_bytes(b"keep")
        replacements = []
        replace = os.replace

        def record(source, target):
            self.assertEqual(source.parent.parent, self.directory)
            self.assertEqual(target.parent, self.directory)
            replacements.append(target.name)
            return replace(source, target)

        with patch.object(g.os, "replace", side_effect=record):
            self.generate()
        self.assertEqual(replacements[-1], "manifest.json")
        self.assertEqual(set(replacements[:-1]), FILENAMES)
        self.assertEqual((self.directory / "notes.txt").read_bytes(), b"keep")
        g.verify(self.directory)

    def test_verify_needs_neither_subprocesses_nor_checkout(self):
        self.dirty = b""
        manifest = self.generate()
        (self.root / "Cargo.toml").unlink()
        self.binary.unlink()
        with patch.object(g.subprocess, "run", side_effect=AssertionError("verification invoked a tool")):
            self.assertEqual(g.verify(self.directory, REVISION, "0.4.0", require_clean=True), manifest)

    def test_verify_cli_runs_with_no_rust_or_git_on_path(self):
        self.generate()
        result = subprocess.run(
            [sys.executable, "-B", str(Path(g.__file__).resolve()), "verify", "--dir", str(self.directory),
             "--version", "0.4.0", "--revision", REVISION],
            cwd=self.temporary.name, env={**os.environ, "PATH": ""}, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())

    def test_verifier_rejects_wrong_identity_and_dirty_release(self):
        self.generate()
        for arguments, message in (({"version": "0.3.0"}, "version does not match"),
                                   ({"revision": OTHER_REVISION}, "revision does not match"),
                                   ({"require_clean": True}, "dirty worktree")):
            with self.subTest(arguments=arguments), self.assertRaisesRegex(RuntimeError, message):
                g.verify(self.directory, **arguments)

    def test_verifier_rejects_corrupt_or_missing_script(self):
        self.generate()
        (self.directory / "_gukhanmun.ps1").write_bytes(b"corrupt\n")
        with self.assertRaisesRegex(RuntimeError, "hash mismatch: _gukhanmun.ps1"):
            g.verify(self.directory)
        self.generate()
        (self.directory / "_gukhanmun.ps1").unlink()
        with self.assertRaises(FileNotFoundError):
            g.verify(self.directory)

    def test_verifier_rejects_unexpected_manifest_shape_and_unsafe_filenames(self):
        changes = [
            lambda m: m.update(extra=True),
            lambda m: m.update(schema_version=True),
            lambda m: m.update(schema_version=2),
            lambda m: m.update(command="other"),
            lambda m: m.update(version=4),
            lambda m: m.update(source_revision="1234567"),
            lambda m: m.update(worktree_dirty="false"),
            lambda m: m["files"].pop("gukhanmun.nu"),
            lambda m: m["files"].update({"../outside": "0" * 64}),
            lambda m: m["files"].update({"gukhanmun.nu": "invalid digest"}),
        ]
        for change in changes:
            with self.subTest(change=change):
                self.generate()
                self.manifest(change)
                with self.assertRaises(RuntimeError):
                    g.verify(self.directory)

    def test_verifier_rejects_duplicate_fields_and_invalid_json(self):
        self.generate()
        path = self.directory / "manifest.json"
        for data, message in ((b'{"schema_version":1,"schema_version":1}', "duplicate manifest field"),
                              (b"{", "invalid completion manifest JSON")):
            with self.subTest(data=data):
                path.write_bytes(data)
                with self.assertRaisesRegex(RuntimeError, message):
                    g.verify(self.directory)

    def test_main_reports_read_errors_without_a_traceback(self):
        error = io.StringIO()
        with contextlib.redirect_stderr(error):
            self.assertEqual(g.main(["verify", "--dir", str(self.directory)]), 1)
        self.assertIn("completion bundle:", error.getvalue())
        self.assertNotIn("Traceback", error.getvalue())


class HostBuildTests(unittest.TestCase):
    def test_explicit_host_ignores_cross_defaults_preserving_wrappers_and_target_options(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            executable = root / "custom target dir/host/debug/gukhanmun.exe"
            executable.parent.mkdir(parents=True)
            executable.touch()
            calls = []
            artifact = {
                "reason": "compiler-artifact", "manifest_path": str(root / "crates/gukhanmun-cli/Cargo.toml"),
                "target": {"name": "gukhanmun", "kind": ["bin"]}, "executable": str(executable),
            }

            def execute(command, **kwargs):
                calls.append((command, kwargs))
                if command[0] == "rustc":
                    output = b"rustc 1.96.0\nhost: x86_64-pc-windows-msvc\nrelease: 1.96.0\n"
                else:
                    dependency = {**artifact, "target": {"name": "gukhanmun", "kind": ["lib"]},
                                  "executable": str(root / "wrong")}
                    output = (json.dumps(dependency) + "\n" + json.dumps(artifact) + "\n").encode()
                return subprocess.CompletedProcess(command, 0, output, b"")

            environment = {
                "RUSTFLAGS": "cross-crt-flags", "CARGO_ENCODED_RUSTFLAGS": "encoded-cross-flags",
                "CARGO_BUILD_RUSTFLAGS": "build-cross-flags", "CARGO_BUILD_TARGET": "aarch64-unknown-linux-gnu",
                "RUSTC_WRAPPER": "sccache", "RUSTC_WORKSPACE_WRAPPER": "workspace-wrapper",
                "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS": "host-options",
                "CARGO_TARGET_DIR": str(root / "custom target dir"),
            }
            with patch.dict(g.os.environ, environment, clear=True), patch.object(g.subprocess, "run", side_effect=execute):
                self.assertEqual(g.build_host_binary(root), executable)
            self.assertEqual(len(calls), 2)
            command, kwargs = calls[1]
            self.assertEqual(command[:2], ["cargo", "build"])
            self.assertIn("--locked", command)
            self.assertEqual(command[command.index("--target") + 1], "x86_64-pc-windows-msvc")
            self.assertEqual(command[command.index("--package") + 1], "gukhanmun-cli")
            self.assertEqual(command[command.index("--bin") + 1], "gukhanmun")
            self.assertIn("--message-format=json-render-diagnostics", command)
            self.assertEqual(kwargs["cwd"], root)
            for _, options in calls:
                for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_TARGET"):
                    self.assertNotIn(name, options["env"])
                for name in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_TARGET_DIR",
                             "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS"):
                    self.assertEqual(options["env"][name], environment[name])

    def test_missing_host_and_failed_cargo_do_not_invent_a_binary_path(self):
        with patch.object(g, "run", return_value=b"rustc 1.96.0\n"), \
                self.assertRaisesRegex(RuntimeError, "host triple"):
            g.build_host_binary()
        with patch.object(g, "run", side_effect=[b"host: x86_64-unknown-linux-gnu\n", b'{}\n']), \
                self.assertRaisesRegex(RuntimeError, "exactly one"):
            g.build_host_binary()
        result = subprocess.CompletedProcess(["cargo"], 101, b"", b"dependency build failed")
        with patch.object(g.subprocess, "run", return_value=result), \
                self.assertRaisesRegex(RuntimeError, "status 101: dependency build failed"):
            g.run(["cargo", "build"])


if __name__ == "__main__":
    unittest.main()
