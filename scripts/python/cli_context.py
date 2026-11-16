# Gukhanmun: Resolves isolated CLI build identities and checks native stamping.
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

"""Native context is stdlib-only; Python policy is resolved in a separate step."""

import argparse
import copy
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def load(name):
    spec = importlib.util.spec_from_file_location("_cli_" + name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def native(environment, base, short_sha):
    """Preserve the existing native/npm policy, including already-dev bases."""
    ref = environment.get("GITHUB_REF", "")
    tag = environment.get("GITHUB_REF_TYPE") == "tag"
    eligible_branch = bool(ref == "refs/heads/main" or
                           re.fullmatch(r"refs/heads/[0-9]+\.[0-9]+-maintenance", ref))
    push = environment.get("GITHUB_EVENT_NAME") == "push"
    version = base
    publish = push and (tag or eligible_branch)
    if tag:
        if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", ref.removeprefix("refs/tags/")):
            raise ValueError("release tags must use x.y.z without a v prefix")
        if ref != "refs/tags/" + base:
            raise ValueError("release tag does not match workspace.package.version")
    elif push and eligible_branch:
        number = environment.get("GITHUB_RUN_NUMBER", "")
        if not re.fullmatch(r"[1-9][0-9]*", number):
            raise ValueError("native development version requires a positive run number")
        if not re.fullmatch(r"[0-9a-f]+", short_sha):
            raise ValueError("invalid Git abbreviation")
        version = f"{base}-dev.{number}+{short_sha}"
    return {"version": version, "should-publish": str(publish).lower(),
            "stamp-native-version": str(version != base).lower(),
            "commit": environment.get("GITHUB_SHA", ""), "short-sha": short_sha}


def emit(values):
    lines = "".join(f"{key}={value}\n" for key, value in values.items())
    if any("\n" in str(value) or "\r" in str(value) for value in values.values()):
        raise ValueError("multiline context output")
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as stream:
            stream.write(lines)
    print(lines, end="")


def check_stamp(before, after, lock_before, lock_after, names, version):
    """Accept only workspace version/dependency changes and their lock entries."""
    expected = copy.deepcopy(before)
    old = expected["workspace"]["package"]["version"]
    expected["workspace"]["package"]["version"] = version
    for dependency in expected["workspace"]["dependencies"].values():
        if isinstance(dependency, dict) and "path" in dependency and "version" in dependency:
            dependency["version"] = version
    if expected != after:
        raise ValueError("native stamp changed fields outside the workspace version allowlist")
    expected_lock = copy.deepcopy(lock_before)
    for package in expected_lock["package"]:
        if package["name"] in names and "source" not in package:
            if package["version"] != old:
                raise ValueError("workspace lock version disagrees before stamping")
            package["version"] = version
        if "dependencies" in package:
            package["dependencies"] = [
                f"{parts[0]} {version}" if len(parts := dependency.split()) == 2
                and parts[0] in names and parts[1] == old else dependency
                for dependency in package["dependencies"]
            ]
    if expected_lock != lock_after:
        raise ValueError("native stamp changed nonworkspace lock content")


def stamp(version, root=ROOT):
    """Stamp Cargo alone, checking a clean checkout and semantic dirty allowlist."""
    def run(*args, **kwargs):
        return subprocess.run(args, cwd=root, check=True, text=True, encoding="utf-8", **kwargs)

    if run("git", "status", "--porcelain", capture_output=True).stdout.strip():
        raise ValueError("native stamping requires a clean checkout")
    manifest = root / "Cargo.toml"
    lock = root / "Cargo.lock"
    before = tomllib.loads(manifest.read_text(encoding="utf-8"))
    lock_before = tomllib.loads(lock.read_text(encoding="utf-8"))
    metadata = json.loads(run("cargo", "metadata", "--locked", "--no-deps", "--format-version", "1",
                              capture_output=True).stdout)
    names = {package["name"] for package in metadata["packages"]
             if package["id"] in metadata["workspace_members"]}
    queries = [f'workspace.package.version = "{version}"']
    queries += [f'workspace.dependencies.get("{name}").version = "{version}"'
                for name, item in before["workspace"]["dependencies"].items()
                if isinstance(item, dict) and "path" in item and "version" in item]
    for query in queries:
        updated = run("dasel", "-i", "toml", "--root", query,
                      input=manifest.read_text(encoding="utf-8"), capture_output=True).stdout
        manifest.write_text(updated, encoding="utf-8")
    run("cargo", "update", "--workspace")
    changed = set(run("git", "diff", "--name-only", capture_output=True).stdout.splitlines())
    if not changed <= {"Cargo.toml", "Cargo.lock"}:
        raise ValueError("native stamp dirtied unexpected paths")
    check_stamp(before, tomllib.loads(manifest.read_text(encoding="utf-8")),
                lock_before, tomllib.loads(lock.read_text(encoding="utf-8")), names, version)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("native", "python", "matrix", "stamp"))
    parser.add_argument("--version")
    args = parser.parse_args()
    if args.command == "native":
        base = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
        # This is the only abbreviation derivation in the workflow.
        short = subprocess.check_output(["git", "rev-parse", "--short", os.environ["GITHUB_SHA"]],
                                        cwd=ROOT, text=True).strip()
        emit(native(os.environ, base, short))
    elif args.command == "python":
        result = load("release_context").resolve(os.environ)
        emit({"python-version": result["version"], "python-eligible": str(result["eligible"]).lower(),
              "python-reason": result["reason"]})
    elif args.command == "matrix":
        subprocess.run([sys.executable, "-I", Path(__file__).with_name("cli_artifacts.py"), "matrix"], check=True)
    else:
        if not args.version:
            parser.error("stamp requires --version")
        stamp(args.version)
    return 0


if __name__ == "__main__":
    sys.exit(main())
