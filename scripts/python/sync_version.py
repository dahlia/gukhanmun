# Gukhanmun: Synchronizes the Python manifests with the Rust workspace version.
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

"""Synchronize every Python manifest from the Rust workspace version.

The core distribution and the six dictionary data distributions always share
one version, and each core extra pins its data distribution to exactly that
version.  All new manifest texts are computed and validated before any
file is written, so an invalid manifest leaves every manifest unchanged.
"""
import argparse
import importlib.util
import os
from pathlib import Path
import re
import sys
import tempfile
import tomllib

from packaging.version import Version

_VERSION = re.compile(r"""(?m)^(version\s*=\s*)(["'])([^"'\n]*)\2(?=[ \t]*(?:#.*)?$)""")
_TABLE = (Path(__file__).resolve().parents[2]
          / "python" / "gukhanmun" / "src" / "gukhanmun" / "_dictionary_data.py")


def table():
    """Load the private dictionary table by path, without the native module."""
    spec = importlib.util.spec_from_file_location("_gukhanmun_dictionary_data", _TABLE)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def manifests(root: Path) -> list[Path]:
    names = [distribution.name for distribution in table().DISTRIBUTIONS]
    return [root / "python" / name / "pyproject.toml" for name in ["gukhanmun", *names]]


def section(source: str, name: str) -> tuple[int, int]:
    """Return the body span of one top-level TOML table."""
    header = re.search(rf"(?m)^\[{re.escape(name)}\][ \t]*(?:#.*)?$", source)
    if header is None:
        raise ValueError(f"missing [{name}] section")
    following = re.search(r"(?m)^\[", source[header.end():])
    return header.end(), len(source) if following is None else header.end() + following.start()


def rewrite_extras(source: str, version: str) -> str:
    """Pin every data extra to exactly the version, and nothing else."""
    extras = table().EXTRAS
    start, end = section(source, "project.optional-dependencies")
    names = "|".join(map(re.escape, sorted(set(extras.values()))))
    body = re.sub(rf"""(["'])({names})==[^"'\n]*\1""",
                  lambda match: f"{match[1]}{match[2]}=={version}{match[1]}", source[start:end])
    changed = source[:start] + body + source[end:]
    actual = tomllib.loads(changed)["project"].get("optional-dependencies")
    expected = {extra: [f"{name}=={version}"] for extra, name in extras.items()}
    if actual != expected:
        raise ValueError("project.optional-dependencies must contain exactly the dictionary extras, "
                         "each pinning its data distribution with ==")
    return changed


def rewrite(source: str, version: str) -> str:
    """Replace the single static [project].version, preserving other text."""
    project = tomllib.loads(source)["project"]
    if not isinstance(project.get("version"), str):
        raise ValueError("the Python manifest requires a static project.version")
    start, end = section(source, "project")
    body = source[start:end]
    matches = list(_VERSION.finditer(body))
    if len(matches) != 1:
        raise ValueError("expected exactly one static project.version assignment")
    match = matches[0]
    updated = (body[:match.start()] + match[1] + match[2] + version + match[2]
               + body[match.end():])
    changed = source[:start] + updated + source[end:]
    if tomllib.loads(changed)["project"]["version"] != version:
        raise ValueError("Python version rewrite did not preserve valid project metadata")
    return changed


def write_atomic(path: Path, text: str) -> None:
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=path.parent,
                                     prefix=f".{path.name}.", delete=False) as temporary:
        pending = Path(temporary.name)
        temporary.write(text)
    try:
        os.replace(pending, path)
    finally:
        pending.unlink(missing_ok=True)


def sync_version(root: Path) -> str:
    cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    raw = cargo["workspace"]["package"]["version"]
    if not isinstance(raw, str):
        raise ValueError("workspace.package.version must be a string")
    parsed = Version(raw)
    if parsed.post is not None:
        raise ValueError("a Rust prerelease cannot become a Python post-release")
    version = str(parsed)
    pending = []
    core, *data = manifests(root)
    for manifest in [core, *data]:
        # Universal newlines keep the anchored patterns working on CRLF files.
        source = manifest.read_text(encoding="utf-8")
        try:
            changed = rewrite(source, version)
            if manifest == core:
                changed = rewrite_extras(changed, version)
        except ValueError as error:
            raise ValueError(f"{manifest.relative_to(root).as_posix()}: {error}") from error
        if changed != source:
            pending.append((manifest, changed))
    # Every manifest validated: only now write any of them.
    for manifest, changed in pending:
        write_atomic(manifest, changed)
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
