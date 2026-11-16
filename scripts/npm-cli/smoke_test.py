# Gukhanmun: Tests the isolated registry and native npm CLI smoke boundaries.
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

import json
import os
from pathlib import Path
import platform
import shutil
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request

import artifacts
from artifacts_test import Candidate
import smoke


def elf(machine=62, kind=1, dynamic_tag=0):
    data = bytearray(256)
    data[:6] = b"\x7fELF\x02\x01"
    struct.pack_into("<H", data, 18, machine)
    struct.pack_into("<Q", data, 32, 64)
    struct.pack_into("<HH", data, 54, 56, 1)
    struct.pack_into("<IIQQQQQQ", data, 64, kind, 0, 128, 0, 0, 16, 16, 1)
    struct.pack_into("<qQ", data, 128, dynamic_tag, 0)
    return bytes(data)


class SmokeTests(unittest.TestCase):
    def test_windows_shim_command_avoids_c_runtime_requoting(self):
        path = r"C:\temp\gukhanmun npm 한글\prefix\gukhanmun.cmd"
        with patch.object(os, "name", "nt"), \
                patch.dict(os.environ, {"COMSPEC": r"C:\Windows\system32\cmd.exe"}):
            command = smoke.shim_command(path, ["--version"])
        self.assertEqual(
            command,
            f'C:\\Windows\\system32\\cmd.exe /d /s /c ""{path}" --version"',
        )
        self.assertNotIn(r'\"', command)

    @unittest.skipUnless(os.name == "nt", "cmd.exe execution requires Windows")
    def test_actual_windows_shim_in_unicode_spaced_directory(self):
        with tempfile.TemporaryDirectory(prefix="npm shim 한글 ") as temporary:
            directory = Path(temporary)
            shim = directory / "gukhanmun.cmd"
            shim.write_text("@echo off\r\necho shim-%1\r\n", encoding="ascii")
            result = smoke.run(smoke.shim_command(shim, ["--version"]),
                               directory=directory, env=os.environ.copy())
            self.assertEqual(result.stdout.strip(), "shim---version")

    def test_static_elf_and_target_architecture(self):
        self.assertEqual(smoke.audit_elf(elf(), "x64")["elf_machine"], 62)
        self.assertEqual(smoke.audit_elf(elf(machine=183), "arm64")["elf_machine"], 183)
        with self.assertRaisesRegex(RuntimeError, "architecture"):
            smoke.audit_elf(elf(), "arm64")

    def test_dynamic_elf_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "PT_INTERP"):
            smoke.audit_elf(elf(kind=3), "x64")
        with self.assertRaisesRegex(RuntimeError, "DT_NEEDED"):
            smoke.audit_elf(elf(kind=2, dynamic_tag=1), "x64")
        with self.assertRaisesRegex(RuntimeError, "unterminated"):
            smoke.audit_elf(elf(kind=2, dynamic_tag=5), "x64")
        smoke.audit_elf(elf(kind=2), "x64")

    def test_corrupt_elf_rejected(self):
        for data in (b"", b"\x7fELF", elf()[:100]):
            with self.subTest(size=len(data)), self.assertRaises(RuntimeError):
                smoke.audit_elf(data, "x64")
        data = bytearray(elf())
        struct.pack_into("<Q", data, 64 + 32, 100000)
        with self.assertRaisesRegex(RuntimeError, "segment"):
            smoke.audit_elf(data, "x64")

    def test_nonnative_host_rejected_before_execution(self):
        with patch.object(platform, "system", return_value="Darwin"), \
                patch.object(platform, "machine", return_value="arm64"):
            with self.assertRaisesRegex(RuntimeError, "nonnative"):
                smoke.native_target("x86_64-unknown-linux-musl")

    def test_node_arch_and_user_floor(self):
        with patch.object(platform, "system", return_value="Linux"), \
                patch.object(platform, "machine", return_value="x86_64"), \
                patch.object(shutil, "which", return_value="/node"), \
                patch("subprocess.check_output") as output:
            for node in ({"os": "linux", "cpu": "arm64", "version": "22.0.0"},
                         {"os": "linux", "cpu": "x64", "version": "20.0.0"}):
                output.return_value = json.dumps(node)
                with self.subTest(node=node), self.assertRaisesRegex(RuntimeError, "native and >=22"):
                    smoke.native_target("x86_64-unknown-linux-musl")
            output.return_value = json.dumps({"os": "linux", "cpu": "x64", "version": "22.0.0"})
            self.assertEqual(smoke.native_target("x86_64-unknown-linux-musl")["version"], "22.0.0")

    def test_registry_normalizes_only_metadata_and_serves_original_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            candidate = Candidate(temporary)
            with smoke.local_registry(temporary, candidate.manifest) as registry:
                with urllib.request.urlopen(registry + "@gukhanmun%2fcli") as response:
                    packument = json.load(response)
                identity = artifacts.npm_version(candidate.version)
                self.assertEqual(packument["dist-tags"], {"dev": identity})
                self.assertEqual(list(packument["versions"]), [identity])
                metadata = packument["versions"][identity]
                self.assertTrue(all(value == candidate.version for value in metadata["optionalDependencies"].values()))
                with urllib.request.urlopen(metadata["dist"]["tarball"]) as response:
                    data = response.read()
                parent = next(item for item in candidate.manifest["packages"] if item["name"] == artifacts.PARENT)
                self.assertEqual(data, (Path(temporary) / parent["filename"]).read_bytes())
                self.assertEqual(artifacts.integrity(data), metadata["dist"]["integrity"])
                with self.assertRaises(urllib.error.HTTPError) as error:
                    urllib.request.urlopen(registry + "unlisted-dependency")
                self.assertEqual(error.exception.code, 404)
                error.exception.close()

    @unittest.skipUnless(shutil.which("node") and shutil.which("npm"), "Node.js/npm unavailable")
    def test_actual_npm_full_optional_specs_against_normalized_packuments(self):
        with tempfile.TemporaryDirectory(prefix="npm normalized 한글 ") as temporary:
            root = Path(temporary)
            bundle = root / "bundle"
            bundle.mkdir()
            candidate = Candidate(bundle)
            with smoke.local_registry(bundle, candidate.manifest) as registry:
                directory = root / "consumer"
                directory.mkdir()
                env = artifacts.isolated_env(directory / "home", registry)
                npm = artifacts.npm_command()
                spec = f"{artifacts.PARENT}@{artifacts.npm_version(candidate.version)}"
                smoke.run([*npm, "install", "--global", "--ignore-scripts", spec],
                          directory=directory, env=env)
                prefix = Path(env["NPM_CONFIG_PREFIX"])
                modules = prefix / ("node_modules" if os.name == "nt" else "lib/node_modules")
                parent = artifacts.read_json((modules / "@gukhanmun/cli/package.json").read_bytes())
                self.assertEqual(parent["version"], candidate.version)
                payloads = list((modules / "@gukhanmun").rglob("cli-*/package.json"))
                self.assertEqual(len(payloads), 1)
                self.assertEqual(artifacts.read_json(payloads[0].read_bytes())["version"], candidate.version)
                # A separate cache/prefix proves --omit does not use the first install.
                omitted = root / "omitted"
                omitted.mkdir()
                artifacts.write_json(omitted / "package.json", {"private": True})
                env = artifacts.isolated_env(omitted / "home", registry)
                smoke.run([*npm, "install", "--prefix", str(omitted), "--ignore-scripts", "--omit=optional", spec],
                          directory=omitted, env=env)
                modules = omitted / "node_modules"
                self.assertFalse(list((modules / "@gukhanmun").rglob("cli-*/package.json")))

    @unittest.skipUnless(platform.system() == "Linux" and shutil.which("node") and shutil.which("npm"),
                         "synthetic executable integration uses a Linux shebang")
    def test_full_install_harness_with_synthetic_executable_not_native_evidence(self):
        with tempfile.TemporaryDirectory(prefix="npm smoke synthetic ") as temporary:
            candidate = Candidate(temporary)
            target = ("aarch64" if platform.machine().lower() in ("arm64", "aarch64") else "x86_64") + "-unknown-linux-musl"
            native = f"""#!{sys.executable}
import pathlib, sys
args = sys.argv[1:]
if args == ["--version"]:
    print("gukhanmun {candidate.version}")
    sys.exit(0)
if "--definitely-not-a-gukhanmun-option" in args:
    print("unknown option", file=sys.stderr)
    sys.exit(2)
result = "{smoke.FALLBACK}" if "--no-bundled-dictionaries" in args else "{smoke.BUNDLED}"
if "-o" in args:
    pathlib.Path(args[args.index("-o") + 1]).write_text(result, encoding="utf-8")
else:
    sys.stdin.read()
    sys.stdout.write(result)
""".encode()
            candidate.files[target][artifacts.TARGETS[target]["binary"]] = native
            candidate.repack(target)
            for filename in ("bin/gukhanmun.cjs", "lib/launcher.cjs"):
                candidate.files[None][filename] = (artifacts.ROOT / "packages/cli" / filename).read_bytes()
            candidate.repack(None)
            with smoke.local_registry(temporary, candidate.manifest) as registry, \
                    patch.object(smoke, "audit_elf", return_value={"synthetic_test_only": True}):
                result = smoke.exercise_registry(registry, artifacts.npm_version(candidate.version),
                                                 candidate.version, target)
            self.assertEqual(result["global_failure_status"], 2)
            self.assertEqual(result["npx_failure_status"], 2)
            self.assertTrue(result["omitted_optional"])


if __name__ == "__main__":
    unittest.main()
