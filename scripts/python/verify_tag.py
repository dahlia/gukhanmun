# Gukhanmun: Binds a verified annotated release tag to the built commit.
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

"""Additional tag identity checks after GitHub signature verification.

Kept separate from Python version policy: every registry consumes the same
verified tag and checkout, without adding a signer or ancestry policy.
"""

import argparse
import json
import os
import re
import subprocess
import sys


def validate(ref_object, tag_object, local_object, local_type, local_target, head, source_sha):
    """Require one annotated tag object and one built commit across all views."""
    if ref_object.get("type") != "tag" or local_type != "tag":
        raise ValueError("release requires an annotated tag locally and remotely")
    object_sha = ref_object.get("sha")
    if not isinstance(object_sha, str) or not re.fullmatch(r"[0-9a-f]{40}", object_sha):
        raise ValueError("remote tag object has an invalid identity")
    if tag_object.get("sha") != object_sha or local_object != object_sha:
        raise ValueError("remote signed tag object differs from the checkout tag object")
    if tag_object.get("verification", {}).get("verified") is not True:
        raise ValueError("remote tag signature is not verified")
    target = tag_object.get("object", {})
    if target.get("type") != "commit":
        raise ValueError("release tag must point directly to a commit")
    if not isinstance(source_sha, str) or not re.fullmatch(r"[0-9a-f]{40}", source_sha):
        raise ValueError("workflow source commit is missing or invalid")
    if not (target.get("sha") == local_target == head == source_sha):
        raise ValueError("signed tag target, workflow source and checkout HEAD differ")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--ref-object", required=True, type=json.loads)
    args = parser.parse_args()
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", args.tag):
        parser.error("release tag must use canonical X.Y.Z")
    ref = f"refs/tags/{args.tag}"
    git = lambda *parts: subprocess.check_output(["git", *parts], text=True).strip()
    try:
        validate(args.ref_object, json.load(sys.stdin), git("rev-parse", "--verify", ref),
                 git("cat-file", "-t", ref), git("rev-parse", "--verify", ref + "^{commit}"),
                 git("rev-parse", "HEAD"), os.environ.get("GITHUB_SHA"))
    except (KeyError, TypeError, ValueError, subprocess.CalledProcessError) as error:
        print(f"release tag identity verification failed: {error}", file=sys.stderr)
        return 1
    print("Verified signed tag object and source commit identity")
    return 0


if __name__ == "__main__":
    sys.exit(main())
