# Gukhanmun: Publishes the original independently validated CLI wheel bundle.
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

"""Reuse the existing registry reconciliation, not its 31-file classifier."""

import argparse
import importlib.util
import os
from pathlib import Path
import sys

_spec = importlib.util.spec_from_file_location("_cli_publish_wheel", Path(__file__).with_name("cli_wheel.py"))
cli_wheel = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = cli_wheel
_spec.loader.exec_module(cli_wheel)
cli_artifacts = cli_wheel.load("cli_artifacts")
shared = cli_wheel.load("publish")

def validate(directory, version, *, env=None, manifest_sha256=None, root=cli_wheel.ROOT):
    try:
        return cli_artifacts.validate(directory, version, env=env, manifest_sha256=manifest_sha256, root=root)
    except (RuntimeError, OSError, ValueError, KeyError, TypeError) as error:
        raise shared.PublicationError(str(error)) from error


def publish(directory, version, artifact_id, manifest_sha256, receipt_path, **kwargs):
    return shared.publish(
        directory, version, artifact_id, manifest_sha256, receipt_path,
        validator=validate, upload_groups=lambda entries: [set(entries)],
        allow_missing_projects=("gukhanmun-cli",), **kwargs,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("validate", "publish"))
    parser.add_argument("--dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--artifact-id", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    try:
        if args.command == "publish":
            publish(args.dir, args.version, args.artifact_id, args.manifest_sha256, args.receipt)
        else:
            shared.receipt_location(args.dir, args.receipt)
            shared.require(shared.positive(args.artifact_id), "validation requires original artifact ID")
            data = validate(args.dir, args.version, manifest_sha256=args.manifest_sha256)
            shared.write_receipt(args.receipt, {
                "schema": 1, "status": "validated", "artifact_id": args.artifact_id,
                "manifest_sha256": args.manifest_sha256, "version": args.version,
                "manifest_attempt": data["source"]["run_attempt"],
                "current_attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
                "run_id": data["source"]["run_id"], "commit": data["source"]["commit"],
            })
    finally:
        if args.command == "publish":
            shared.summarize(args.receipt, os.environ)


if __name__ == "__main__":
    main()
