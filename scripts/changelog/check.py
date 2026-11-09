# Gukhanmun: Checks the language-specific changelog fragment layout.
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

"""Reject fragment locations that Sacho's configured patterns would ignore."""

from pathlib import Path
import sys


FAMILIES = frozenset(("rust", "javascript", "python"))


def fragment_errors(root: Path) -> list[str]:
    """Report fragments outside one public component in one language family."""
    errors = []
    for path in sorted((root / "changes.d").rglob("*.md")):
        relative = path.relative_to(root / "changes.d")
        parts = relative.parts
        if len(parts) != 3 or parts[0] not in FAMILIES:
            errors.append(f"changes.d/{relative}: expected <rust|javascript|python>/<component>/<fragment>.md")
        elif parts[:2] == ("rust", "gukhanmun-python"):
            errors.append(f"changes.d/{relative}: private binding changes belong to gukhanmun (Python)")
    return errors


def main() -> int:
    """Check this checkout, allowing an explicit root for isolated fixtures."""
    if len(sys.argv) > 2:
        print("usage: check.py [project-root]", file=sys.stderr)
        return 2
    root = Path(sys.argv[1]) if len(sys.argv) == 2 else Path(__file__).resolve().parents[2]
    if not root.is_dir() or not (root / "sacho.toml").is_file():
        print(f"{root}: expected a project directory containing sacho.toml", file=sys.stderr)
        return 2
    errors = fragment_errors(root)
    for error in errors:
        print(error, file=sys.stderr)
    return int(bool(errors))


if __name__ == "__main__":
    sys.exit(main())
