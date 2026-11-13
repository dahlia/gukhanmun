# Gukhanmun: Derives Python artifact and publication identities from CI events.
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

"""Share Python version derivation between build and publication contexts.

Eligibility here is an event policy, not signature verification. Tagged
publication must additionally depend on artifact-context's signed-tag gate.
Valid but unsupported publication bases return a reason rather than breaking
the independent Rust/npm version outputs.
"""

import importlib.util
import os
from pathlib import Path
import re
import sys

from packaging.version import Version

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "dahlia/gukhanmun"


def resolve(environment, root=ROOT):
    """Return the artifact version and a separate publication decision."""
    spec = importlib.util.spec_from_file_location("_release_sync_version", Path(__file__).with_name("sync_version.py"))
    sync = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sync)
    base = sync.workspace_version(root)
    parsed = Version(base)
    if parsed.local is not None or parsed.epoch or parsed.post is not None:
        raise ValueError("Python artifacts cannot have local labels, epochs or post-release versions")
    ref_type = environment.get("GITHUB_REF_TYPE")
    version = base if ref_type == "tag" or parsed.dev is not None else f"{base}.dev0"

    def decision(eligible, reason):
        return {"version": version, "eligible": eligible, "reason": reason}

    if environment.get("GITHUB_REPOSITORY") != REPOSITORY:
        return decision(False, "only dahlia/gukhanmun can publish")
    if environment.get("GITHUB_EVENT_NAME") != "push":
        return decision(False, "only push events can publish")
    ref = environment.get("GITHUB_REF", "")
    if ref_type == "tag":
        if not re.fullmatch(r"refs/tags/(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", ref):
            return decision(False, "release tags must use canonical X.Y.Z")
        if ref != f"refs/tags/{base}":
            return decision(False, "release tag does not match the workspace version")
        return decision(True, "matching release tag; signed-tag verification is also required")
    if ref_type != "branch" or not (
            ref == "refs/heads/main" or re.fullmatch(r"refs/heads/[0-9]+\.[0-9]+-maintenance", ref)):
        return decision(False, "only main and X.Y-maintenance branch pushes publish development releases")
    number = environment.get("GITHUB_RUN_NUMBER", "")
    if not re.fullmatch(r"[1-9][0-9]*", number):
        return decision(False, "development publication requires a positive workflow run number")
    if parsed.dev is not None:
        return decision(False, "a workspace development version cannot receive another .devN segment")
    version = f"{base}.dev{number}"
    return decision(True, "eligible development branch push")


def main():
    result = resolve(os.environ)
    lines = (f"python-version={result['version']}\n"
             f"python-eligible={str(result['eligible']).lower()}\n"
             f"python-reason={result['reason']}\n")
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as stream:
            stream.write(lines)
    print(lines, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
