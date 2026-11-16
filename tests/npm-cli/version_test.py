# Gukhanmun: Tests exact npm CLI version pins without modifying the checkout.
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
import re
import subprocess
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]


class Versions(unittest.TestCase):
    def test_stamping_keeps_all_seven_manifests_and_exact_pins_aligned(self):
        task = tomllib.loads((ROOT / "mise.toml").read_text())["tasks"]["js-sync-version"]["run"]
        with tempfile.TemporaryDirectory(prefix="gukhanmun npm version ") as temporary:
            directory = Path(temporary)
            manifests = [*(ROOT / "packages").glob("*/package.json"),
                         *(ROOT / "packages").glob("*/deno.json")]
            for source in manifests:
                # Only committed manifest inputs, never generated package data.
                destination = directory / source.relative_to(ROOT)
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(source.read_bytes())
            for version in ("0.4.0-dev.9+abc1234", "0.4.0-dev.10+def5678", "0.4.0"):
                result = subprocess.run(
                    ["nu", "-c", task], cwd=directory,
                    env={**os.environ, "VERSION": version},
                    text=True, capture_output=True, timeout=30,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                parent = json.loads((directory / "packages/cli/package.json").read_bytes())
                self.assertEqual(parent["version"], version)
                self.assertEqual(len(parent["optionalDependencies"]), 6)
                for name, pin in parent["optionalDependencies"].items():
                    self.assertEqual(pin, version)
                    payload = json.loads(
                        (directory / "packages" / name.removeprefix("@gukhanmun/") / "package.json").read_bytes()
                    )
                    self.assertEqual(payload["name"], name)
                    self.assertEqual(payload["version"], version)
                    self.assertNotIn("libc", payload)

    def test_cli_packages_stay_outside_recursive_library_publication(self):
        text = (ROOT / "pnpm-workspace.yaml").read_text()
        self.assertIn('"!packages/cli"', text)
        self.assertIn('"!packages/cli-*"', text)

    def test_pack_uses_the_shared_native_context_without_a_publication_gate(self):
        workflow = (ROOT / ".github/workflows/main.yaml").read_text()
        job = re.search(r"(?ms)^  npm-cli-pack:\n(.*?)(?=^  [\w-]+:\n|\Z)", workflow)[1]
        self.assertIn("needs: [cli-context, build-completions, build-release-binaries]", job)
        self.assertIn("ARTIFACT_VERSION: ${{ needs.cli-context.outputs.version }}", job)
        self.assertNotIn("needs.artifact-context", job)
        self.assertNotRegex(job, r"(?m)^    if:")


if __name__ == "__main__":
    unittest.main()
