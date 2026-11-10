# Gukhanmun: Synchronizes the Python manifest with the Rust workspace version.
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

"""Synchronize the core Python manifest from the Rust workspace version."""
import argparse
from pathlib import Path
import re
import sys
import tomllib

from packaging.version import Version

_VERSION = re.compile(r"""(?m)^(version\s*=\s*)(["'])([^"'\n]*)\2(?=[ \t]*(?:#.*)?$)""")


def sync_version(root: Path) -> str:
    cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    raw = cargo["workspace"]["package"]["version"]
    if not isinstance(raw, str):
        raise ValueError("workspace.package.version must be a string")
    parsed = Version(raw)
    if parsed.post is not None:
        raise ValueError("a Rust prerelease cannot become a Python post-release")
    version = str(parsed)
    manifest = root / "python" / "gukhanmun" / "pyproject.toml"
    source = manifest.read_text(encoding="utf-8")
    project = tomllib.loads(source)["project"]
    if not isinstance(project.get("version"), str):
        raise ValueError("the Python manifest requires a static project.version")
    section = re.search(r"(?m)^\[project\][ \t]*(?:#.*)?$", source)
    if section is None:
        raise ValueError("missing [project] section")
    following = re.search(r"(?m)^\[", source[section.end():])
    end = len(source) if following is None else section.end() + following.start()
    body = source[section.end():end]
    matches = list(_VERSION.finditer(body))
    if len(matches) != 1:
        raise ValueError("expected exactly one static project.version assignment")
    match = matches[0]
    updated = (body[:match.start()] + match[1] + match[2] + version + match[2]
               + body[match.end():])
    changed = source[:section.end()] + updated + source[end:]
    if tomllib.loads(changed)["project"]["version"] != version:
        raise ValueError("Python version rewrite did not preserve valid project metadata")
    if changed != source:
        manifest.write_text(changed, encoding="utf-8")
    return version


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path, nargs="?", default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    try:
        print(sync_version(args.root))
    except (OSError, KeyError, TypeError, ValueError) as error:
        print(f"could not synchronize Python version: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
