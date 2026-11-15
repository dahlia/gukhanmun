# Gukhanmun: Builds and verifies a portable shell completion bundle.
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

"""Generate once on the host, then verify the same scripts in every archive."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_DIRECTORY = ROOT / "target/completions"
SHELL_FILES = {
    "bash": "gukhanmun.bash",
    "zsh": "_gukhanmun",
    "fish": "gukhanmun.fish",
    "powershell": "_gukhanmun.ps1",
    "nushell": "gukhanmun.nu",
}
MANIFEST_FIELDS = {"schema_version", "command", "version", "source_revision", "worktree_dirty", "files"}
REVISION_PATTERN = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})\Z")


def run(command, *, root=ROOT, env=None):
    """Capture bytes without allowing locale or platform newline conversion."""
    result = subprocess.run(command, cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"{command[0]} exited with status {result.returncode}: {detail}")
    return result.stdout


def build_host_binary(root=ROOT):
    """Use Cargo's artifact path, even with a cross target or custom target dir."""
    env = os.environ.copy()
    for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_TARGET"):
        env.pop(name, None)
    metadata = run(["rustc", "-vV"], root=root, env=env).decode("utf-8")
    hosts = [line.removeprefix("host: ").strip() for line in metadata.splitlines() if line.startswith("host: ")]
    if len(hosts) != 1 or not re.fullmatch(r"[A-Za-z0-9_.-]+", hosts[0]):
        raise RuntimeError("rustc did not report one valid host triple")
    output = run([
        "cargo", "build", "--locked", "--package", "gukhanmun-cli", "--bin", "gukhanmun",
        "--target", hosts[0], "--message-format=json-render-diagnostics",
    ], root=root, env=env)
    manifest = (root / "crates/gukhanmun-cli/Cargo.toml").resolve()
    executables = set()
    for line in output.splitlines():
        try:
            message = json.loads(line)
        except (ValueError, UnicodeDecodeError) as error:
            raise RuntimeError("cargo emitted invalid JSON") from error
        if (message.get("reason") == "compiler-artifact"
                and message.get("target", {}).get("name") == "gukhanmun"
                and "bin" in message.get("target", {}).get("kind", [])
                and message.get("executable")
                and Path(message.get("manifest_path", "")).resolve() == manifest):
            executable = Path(message["executable"])
            executables.add((root / executable).resolve())
    if len(executables) != 1:
        raise RuntimeError("cargo did not report exactly one gukhanmun CLI executable")
    executable = executables.pop()
    if not executable.is_file():
        raise RuntimeError(f"cargo's CLI executable does not exist: {executable}")
    return executable


def check_revision(revision):
    if not isinstance(revision, str) or not REVISION_PATTERN.fullmatch(revision):
        raise RuntimeError("source revision must be a full lowercase Git object ID")


def publish(directory, files, manifest):
    """Stage all writes on the destination filesystem and publish the manifest last."""
    directory.mkdir(parents=True, exist_ok=True)
    names = [*files, "manifest.json"]
    for name in names:
        if (directory / name).is_dir():
            raise RuntimeError(f"completion destination is a directory: {directory / name}")
    with tempfile.TemporaryDirectory(prefix=".gukhanmun-completions-", dir=directory) as temporary:
        staging = Path(temporary)
        for name, data in files.items():
            (staging / name).write_bytes(data)
        (staging / "manifest.json").write_bytes(
            (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode("utf-8")
        )
        for name in names:
            os.replace(staging / name, directory / name)


def source_worktree_dirty(root, output_dir):
    """Ignore only untracked bundle outputs, retaining all source changes."""
    excluded = set()
    try:
        relative = Path(output_dir).resolve().relative_to(root)
    except ValueError:
        pass
    else:
        excluded = {os.fsencode((relative / name).as_posix())
                    for name in (*SHELL_FILES.values(), "manifest.json")}
    status = run(["git", "status", "--porcelain=v1", "-z", "--untracked-files=all"], root=root)
    return any(record and not (record.startswith(b"?? ") and record[3:] in excluded)
               for record in status.split(b"\0"))


def generate(output_dir=DEFAULT_DIRECTORY, binary=None, revision=None, version=None, *, root=ROOT, excluded_dir=None):
    """Publish scripts and source identity, optionally excluding an existing bundle.

    Regeneration can write elsewhere while excluding the original bundle's
    untracked outputs from source status, just as its initial generation did.
    """
    root = Path(root).resolve()
    with (root / "Cargo.toml").open("rb") as stream:
        workspace_version = tomllib.load(stream)["workspace"]["package"]["version"]
    if version is not None and version != workspace_version:
        raise RuntimeError(f"workspace version {workspace_version!r} does not match expected {version!r}")
    source_revision = run(["git", "rev-parse", "HEAD"], root=root).decode("ascii").strip()
    check_revision(source_revision)
    if revision is not None:
        check_revision(revision)
        if revision != source_revision:
            raise RuntimeError(f"checkout revision {source_revision} does not match expected {revision}")
    bundle_dir = output_dir if excluded_dir is None else excluded_dir
    initial_dirty = source_worktree_dirty(root, bundle_dir)
    executable = Path(binary).resolve() if binary is not None else build_host_binary(root)
    actual_version = run([str(executable), "--version"], root=root).decode("utf-8").strip()
    if actual_version != f"gukhanmun {workspace_version}":
        raise RuntimeError(f"CLI version {actual_version!r} does not match workspace version {workspace_version!r}")
    files = {}
    for shell, name in SHELL_FILES.items():
        data = run([str(executable), "--generate-completion", shell], root=root)
        data = data.replace(b"\r\n", b"\n").replace(b"\r", b"\n")
        if not data.strip():
            raise RuntimeError(f"CLI emitted an empty completion script for {shell}")
        data.decode("utf-8")
        files[name] = data if data.endswith(b"\n") else data + b"\n"
    final_dirty = source_worktree_dirty(root, bundle_dir)
    dirty = initial_dirty or final_dirty
    manifest = {
        "schema_version": 1,
        "command": "gukhanmun",
        "version": workspace_version,
        "source_revision": source_revision,
        "worktree_dirty": dirty,
        "files": {name: hashlib.sha256(data).hexdigest() for name, data in files.items()},
    }
    final_revision = run(["git", "rev-parse", "HEAD"], root=root).decode("ascii").strip()
    if final_revision != source_revision:
        raise RuntimeError("checkout revision changed during completion generation")
    publish(Path(output_dir), files, manifest)
    return manifest


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise RuntimeError(f"duplicate manifest field: {key}")
        result[key] = value
    return result


def verify(directory=DEFAULT_DIRECTORY, revision=None, version=None, require_clean=False):
    """Verify an archived bundle without requiring a checkout, Rust or its binary."""
    directory = Path(directory)
    try:
        manifest = json.loads((directory / "manifest.json").read_bytes(), object_pairs_hook=unique_object)
    except (ValueError, UnicodeDecodeError) as error:
        raise RuntimeError("invalid completion manifest JSON") from error
    if not isinstance(manifest, dict) or manifest.keys() != MANIFEST_FIELDS:
        raise RuntimeError("completion manifest has unexpected fields")
    if type(manifest["schema_version"]) is not int or manifest["schema_version"] != 1:
        raise RuntimeError("unsupported completion manifest schema version")
    if manifest["command"] != "gukhanmun":
        raise RuntimeError("completion manifest command must be gukhanmun")
    if not isinstance(manifest["version"], str) or not manifest["version"]:
        raise RuntimeError("completion manifest version must be a nonempty string")
    check_revision(manifest["source_revision"])
    if type(manifest["worktree_dirty"]) is not bool:
        raise RuntimeError("completion manifest worktree_dirty must be a boolean")
    if revision is not None:
        check_revision(revision)
        if revision != manifest["source_revision"]:
            raise RuntimeError("completion manifest source revision does not match expected revision")
    if version is not None and version != manifest["version"]:
        raise RuntimeError("completion manifest version does not match expected version")
    if require_clean and manifest["worktree_dirty"]:
        raise RuntimeError("completion manifest records a dirty worktree")
    files = manifest["files"]
    if not isinstance(files, dict) or files.keys() != set(SHELL_FILES.values()):
        raise RuntimeError("completion manifest must list exactly the five expected script filenames")
    for name, digest in files.items():
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise RuntimeError(f"invalid SHA-256 for {name}")
        data = (directory / name).read_bytes()
        if hashlib.sha256(data).hexdigest() != digest:
            raise RuntimeError(f"completion script hash mismatch: {name}")
        if not data.strip() or b"\r" in data:
            raise RuntimeError(f"completion script must be nonempty with LF line endings: {name}")
        data.decode("utf-8")
    return manifest


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    generation = commands.add_parser("generate", help="build the host CLI and generate a completion bundle")
    generation.add_argument("--output-dir", type=Path, default=DEFAULT_DIRECTORY)
    generation.add_argument("--binary", type=Path, help="reuse an existing CLI binary for focused tests")
    verification = commands.add_parser("verify", help="verify an existing completion bundle")
    verification.add_argument("--dir", type=Path, default=DEFAULT_DIRECTORY)
    verification.add_argument("--require-clean", action="store_true")
    for command in (generation, verification):
        command.add_argument("--revision", help="expected full source revision")
        command.add_argument("--version", help="expected package version")
    args = parser.parse_args(argv)
    try:
        if args.command == "generate":
            generate(args.output_dir, args.binary, args.revision, args.version)
        else:
            verify(args.dir, args.revision, args.version, args.require_clean)
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        print(f"completion bundle: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
