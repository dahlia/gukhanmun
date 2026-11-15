# Gukhanmun: Packs and validates immutable npm CLI distribution artifacts.
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

"""One pack operation; consumers only inspect the seven original tarballs."""

import argparse
import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]
TARGETS = {
    f"{arch}-{suffix}": {"os": operating_system, "cpu": cpu,
                         "binary": "gukhanmun.exe" if operating_system == "win32" else "gukhanmun"}
    for arch, cpu in (("aarch64", "arm64"), ("x86_64", "x64"))
    for suffix, operating_system in (
        ("apple-darwin", "darwin"), ("pc-windows-msvc", "win32"),
        ("unknown-linux-musl", "linux"),
    )
}
PARENT = "@gukhanmun/cli"
SHELL_FILES = {
    "bash": "gukhanmun.bash", "zsh": "_gukhanmun", "fish": "gukhanmun.fish",
    "powershell": "_gukhanmun.ps1", "nushell": "gukhanmun.nu",
}
NOTICES = {"LICENSE", "NOTICE.md", "ATTRIBUTION.loanwords.md", "ATTRIBUTION.opendict.md"}
PARENT_FILES = {"package.json", "LICENSE", "bin/gukhanmun.cjs", "lib/launcher.cjs",
                "completions/manifest.json",
                *(f"completions/{name}" for name in SHELL_FILES.values())}
MAX_MEMBER = 512 * 1024 * 1024


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise RuntimeError(f"duplicate JSON field: {key}")
        result[key] = value
    return result


def read_json(data):
    try:
        return json.loads(data, object_pairs_hook=unique_object)
    except (ValueError, UnicodeDecodeError) as error:
        raise RuntimeError("invalid JSON") from error


def write_json(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8")


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def integrity(data):
    return "sha512-" + base64.b64encode(hashlib.sha512(data).digest()).decode("ascii")


def npm_version(full):
    if not isinstance(full, str) or not re.fullmatch(
        r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
        r"(?:-dev\.(0|[1-9][0-9]*))?(?:\+[0-9a-f]+)?", full
    ):
        raise RuntimeError(f"unsupported CLI artifact version: {full!r}")
    return full.split("+", 1)[0]


def validate_version_source(version, revision):
    normalized = npm_version(version)
    if "-dev." in normalized:
        if "+" not in version or not revision.startswith(version.split("+", 1)[1]):
            raise RuntimeError("development artifact build metadata must identify its source commit")
    elif version != normalized:
        raise RuntimeError("stable artifacts must not carry build metadata")


def source_context(revision):
    if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", revision):
        raise RuntimeError("source commit must be a full lowercase Git object ID")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if head != revision:
        raise RuntimeError("candidate source commit differs from checkout HEAD")
    if os.environ.get("GITHUB_SHA") and os.environ["GITHUB_SHA"] != revision:
        raise RuntimeError("candidate source commit differs from GITHUB_SHA")
    return {"commit": revision, "ref": os.environ.get("GITHUB_REF", ""),
            "run_id": os.environ.get("GITHUB_RUN_ID", ""),
            "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT", "")}


def validate_source(source):
    if not isinstance(source, dict) or set(source) != {"commit", "ref", "run_id", "run_attempt"}:
        raise RuntimeError("invalid source identity")
    expected = source_context(source["commit"])
    for field in ("ref", "run_id"):
        if expected[field] and source[field] != expected[field]:
            raise RuntimeError(f"candidate {field} differs from current GitHub context")
    if any(not isinstance(value, str) for value in source.values()):
        raise RuntimeError("source identity fields must be strings")
    # A failed-job rerun has a newer run_attempt but must use the original bundle.


def safe_name(name):
    path = PurePosixPath(name)
    if (not name or "\\" in name or path.is_absolute()
            or any(part in ("", ".", "..") for part in name.split("/"))
            or ":" in name or "\0" in name):
        raise RuntimeError(f"unsafe archive path: {name!r}")
    return name


def archive_files(path, *, npm=False):
    """Read regular members only, without extracting archive-controlled paths."""
    files, modes = {}, {}

    def add(name, size, mode, read):
        safe_name(name)
        if npm:
            if not name.startswith("package/"):
                raise RuntimeError("npm member is outside package/")
            name = name.removeprefix("package/")
        if name in files or size > MAX_MEMBER:
            raise RuntimeError(f"duplicate or oversized archive member: {name}")
        files[name] = read()
        modes[name] = mode & 0o777

    if path.suffix == ".zip":
        with zipfile.ZipFile(path) as archive:
            for member in archive.infolist():
                safe_name(member.filename.rstrip("/"))
                mode = member.external_attr >> 16
                if member.is_dir():
                    continue
                if (mode & 0o170000) not in (0, 0o100000):
                    raise RuntimeError("nonregular ZIP member")
                add(member.filename, member.file_size, mode, lambda: archive.read(member))
    else:
        with tarfile.open(path, "r:*") as archive:
            for member in archive:
                safe_name(member.name.rstrip("/"))
                if member.isdir():
                    continue
                if not member.isfile():
                    raise RuntimeError("nonregular tar member")
                add(member.name, member.size, member.mode,
                    lambda: archive.extractfile(member).read())
    return files, modes


def verify_completions(files, version, revision, *, require_clean=False):
    """Reuse the existing verifier without generating or changing shared outputs."""
    with tempfile.TemporaryDirectory(prefix="npm-completions-") as temporary:
        directory = Path(temporary)
        expected = {*SHELL_FILES.values(), "manifest.json"}
        if set(files) != expected:
            raise RuntimeError("completion inventory differs from the five-shell bundle")
        for name, data in files.items():
            (directory / name).write_bytes(data)
        generator = load_module("npm_completion_generate", ROOT / "scripts/completions/generate.py")
        return generator.verify(directory, revision=revision, version=version,
                                require_clean=require_clean)


def npm_command():
    """Invoke npm's JS entry point, not a shell/.cmd quoting layer on Windows."""
    node = shutil.which("node")
    npm = shutil.which("npm")
    if not node or not npm:
        raise RuntimeError("Node.js and its bundled npm are required")
    resolved = Path(npm).resolve()
    candidates = [resolved] if resolved.suffix not in (".cmd", ".exe", ".bat") else []
    candidates += [
        Path(node).resolve().parent / "node_modules/npm/bin/npm-cli.js",
        resolved.parent / "node_modules/npm/bin/npm-cli.js",
        resolved.parent.parent / "lib/node_modules/npm/bin/npm-cli.js",
    ]
    for candidate in candidates:
        if candidate.is_file() and (
            candidate.suffix in (".js", ".cjs") or
            candidate.read_bytes()[:100].startswith(b"#!/usr/bin/env node")
        ):
            return [node, str(candidate)]
    raise RuntimeError("cannot locate npm-cli.js; refusing shell-based npm invocation")


def isolated_env(directory, registry="http://127.0.0.1:9/"):
    """Strip inherited npm/auth/config influence, including scope overrides."""
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    env = {key: value for key, value in os.environ.items()
           if not key.lower().startswith(("npm_", "npm_config_", "node_"))
           and key.upper() not in {
               "NPM_TOKEN", "NODE_AUTH_TOKEN", "BASH_ENV", "ENV", "ZDOTDIR",
               "XDG_CONFIG_HOME", "NU_CONFIG_DIR", "FISH_FUNCTION_PATH",
               "FISH_COMPLETE_PATH", "PROMPT_COMMAND", "PSMODULEPATH",
           }}
    for name in ("user.npmrc", "global.npmrc"):
        (directory / name).write_text("", encoding="utf-8")
    env.update({
        "HOME": str(directory), "USERPROFILE": str(directory),
        "XDG_CONFIG_HOME": str(directory),
        "NPM_CONFIG_USERCONFIG": str(directory / "user.npmrc"),
        "NPM_CONFIG_GLOBALCONFIG": str(directory / "global.npmrc"),
        "NPM_CONFIG_CACHE": str(directory / "cache"),
        "NPM_CONFIG_PREFIX": str(directory / "prefix"),
        "NPM_CONFIG_REGISTRY": registry, "NPM_CONFIG_AUDIT": "false",
        "NPM_CONFIG_FUND": "false", "NPM_CONFIG_UPDATE_NOTIFIER": "false",
        "NPM_CONFIG_FETCH_RETRIES": "0", "NPM_CONFIG_IGNORE_SCRIPTS": "true",
        "NO_COLOR": "1",
    })
    return env


def package_metadata(files, modes, record, manifest):
    target = record.get("target")
    version = manifest["version"]
    metadata = read_json(files.get("package.json", b"null"))
    if not isinstance(metadata, dict):
        raise RuntimeError("missing package.json")
    name = PARENT if target is None else f"{PARENT}-{target}"
    if metadata.get("name") != name or metadata.get("version") != version:
        raise RuntimeError("packed package name/version mismatch")
    if metadata.get("scripts") or "scripts" in metadata:
        raise RuntimeError("CLI packages must not contain lifecycle scripts")
    if any(metadata.get(key) for key in ("dependencies", "devDependencies", "peerDependencies", "bundledDependencies")):
        raise RuntimeError("unexpected dependency in CLI package")
    if target is None:
        required = PARENT_FILES
        if metadata.get("type") != "commonjs" or metadata.get("bin") != {"gukhanmun": "bin/gukhanmun.cjs"}:
            raise RuntimeError("invalid parent launcher metadata")
        if metadata.get("engines", {}).get("node") != ">=22":
            raise RuntimeError("parent must support Node.js >=22")
        if metadata.get("optionalDependencies") != {f"{PARENT}-{key}": version for key in TARGETS}:
            raise RuntimeError("optional dependency pins must match the full version exactly")
        if modes.get("bin/gukhanmun.cjs") != 0o755:
            raise RuntimeError("packed launcher is not mode 0755")
        for path in ("bin/gukhanmun.cjs", "lib/launcher.cjs"):
            if files.get(path) != (ROOT / "packages/cli" / path).read_bytes():
                raise RuntimeError(f"packed launcher differs from checkout source: {path}")
        completions = {key.removeprefix("completions/"): value for key, value in files.items()
                       if key.startswith("completions/")}
        value = verify_completions(completions, version, manifest["source"]["commit"],
                                   require_clean=manifest["source"]["ref"].startswith("refs/tags/"))
        if value != manifest["completions"]:
            raise RuntimeError("completion manifest differs from candidate manifest")
    else:
        if target not in TARGETS:
            raise RuntimeError("unknown target")
        spec = TARGETS[target]
        required = {"package.json", "provenance.json", spec["binary"], *NOTICES}
        if (metadata.get("os") != [spec["os"]] or metadata.get("cpu") != [spec["cpu"]]
                or "libc" in metadata or metadata.get("optionalDependencies")
                or metadata.get("license") != "SEE LICENSE IN LICENSE"):
            raise RuntimeError("invalid platform package constraints/license")
        if modes.get(spec["binary"]) != 0o755:
            raise RuntimeError("packed binary is not mode 0755")
        if sha256(files.get(spec["binary"], b"")) != record.get("binary_sha256"):
            raise RuntimeError("binary hash mismatch")
        provenance = read_json(files.get("provenance.json", b"null"))
        expected = {"schema_version": 1, "source": manifest["source"], "version": version,
                    "target": target, "archive": manifest["archives"][target],
                    "binary_sha256": record["binary_sha256"]}
        if provenance != expected:
            raise RuntimeError("payload provenance mismatch")
    if not required <= files.keys() or files.keys() - required - {"README.md"}:
        raise RuntimeError(f"packed allowlist mismatch for {name}: {sorted(files)}")
    if record.get("name") != name or record.get("version") != version:
        raise RuntimeError("manifest package identity mismatch")
    for notice in NOTICES & files.keys():
        source = {
            "ATTRIBUTION.loanwords.md": ROOT / "crates/gukhanmun-loanwords/ATTRIBUTION.md",
            "ATTRIBUTION.opendict.md": ROOT / "crates/gukhanmun-opendict/ATTRIBUTION.md",
        }.get(notice, ROOT / notice)
        if files[notice] != source.read_bytes():
            raise RuntimeError(f"packed notice differs from source: {notice}")
    return metadata


def validate_bundle(directory, manifest_sha256=None, artifact_id=None):
    directory = Path(directory)
    if (directory / "manifest.json").is_symlink():
        raise RuntimeError("candidate manifest must not be a symlink")
    raw = (directory / "manifest.json").read_bytes()
    if manifest_sha256 is not None and (
        not re.fullmatch(r"[0-9a-f]{64}", manifest_sha256) or sha256(raw) != manifest_sha256
    ):
        raise RuntimeError("candidate manifest SHA-256 mismatch")
    if artifact_id is not None and not re.fullmatch(r"[1-9][0-9]*", str(artifact_id)):
        raise RuntimeError("artifact ID must be a positive GitHub artifact ID")
    manifest = read_json(raw)
    if (not isinstance(manifest, dict) or type(manifest.get("schema_version")) is not int
            or manifest["schema_version"] != 1):
        raise RuntimeError("unsupported candidate schema")
    if manifest.get("npm_version") != npm_version(manifest.get("version")):
        raise RuntimeError("normalized npm version mismatch")
    validate_source(manifest["source"])
    validate_version_source(manifest["version"], manifest["source"]["commit"])
    archives = manifest.get("archives")
    if not isinstance(archives, dict) or archives.keys() != TARGETS.keys():
        raise RuntimeError("candidate must record all six source archives")
    for target, archive in archives.items():
        suffix = ".zip" if TARGETS[target]["os"] == "win32" else ".tar.bz2"
        if (not isinstance(archive, dict) or archive.get("filename") !=
                f"gukhanmun-{manifest['version']}-{target}{suffix}"
                or not re.fullmatch(r"[0-9a-f]{64}", archive.get("sha256", ""))):
            raise RuntimeError("invalid source archive record")
    packages = manifest.get("packages")
    if not isinstance(packages, list) or len(packages) != 7:
        raise RuntimeError("candidate must contain seven packages")
    names, targets, filenames = set(), set(), set()
    for record in packages:
        filename = record["filename"]
        if (safe_name(filename) != Path(filename).name or not filename.endswith(".tgz")
                or filename in filenames or record.get("name") in names):
            raise RuntimeError("duplicate or invalid candidate package")
        filenames.add(filename)
        names.add(record["name"])
        target = record.get("target")
        if target in targets:
            raise RuntimeError("duplicate package target")
        targets.add(target)
        path = directory / filename
        if path.is_symlink():
            raise RuntimeError("candidate tarball must not be a symlink")
        data = path.read_bytes()
        if sha256(data) != record.get("sha256") or integrity(data) != record.get("integrity"):
            raise RuntimeError(f"candidate tarball hash mismatch: {filename}")
        files, modes = archive_files(path, npm=True)
        package_metadata(files, modes, record, manifest)
    if targets != {*TARGETS, None}:
        raise RuntimeError("candidate platform inventory mismatch")
    if {path.name for path in directory.iterdir()} != {*filenames, "manifest.json"}:
        raise RuntimeError("candidate directory contains unexpected files")
    return manifest


def pack(archives, completions, out, version, revision):
    npm_version(version)
    source = source_context(revision)
    validate_version_source(version, revision)
    completion_files = {name: (Path(completions) / name).read_bytes()
                        for name in (*SHELL_FILES.values(), "manifest.json")}
    completion_manifest = verify_completions(
        completion_files, version, revision, require_clean=source["ref"].startswith("refs/tags/"))
    out = Path(out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    if any(out.iterdir()):
        raise RuntimeError("pack output must be empty; never overwrite an immutable candidate")
    manifest = {"schema_version": 1, "version": version, "npm_version": npm_version(version),
                "source": source, "packages": [], "archives": {}, "completions": completion_manifest}
    payloads = {}
    for target, spec in TARGETS.items():
        suffix = ".zip" if spec["os"] == "win32" else ".tar.bz2"
        filename = f"gukhanmun-{version}-{target}{suffix}"
        matches = list(Path(archives).rglob(filename))
        if len(matches) != 1 or matches[0].is_symlink():
            raise RuntimeError(f"expected exactly one source archive: {filename}")
        archive = matches[0]
        manifest["archives"][target] = {"filename": filename, "sha256": sha256(archive.read_bytes())}
        files, _ = archive_files(archive)
        required = {spec["binary"], "LICENSE", "NOTICE.md",
                    *(f"completions/{name}" for name in completion_files)}
        if not required <= files.keys():
            raise RuntimeError(f"release archive is incomplete: {filename}")
        for name, data in completion_files.items():
            if files[f"completions/{name}"] != data:
                raise RuntimeError(f"release archive completion mismatch: {filename}/{name}")
        for name in ("LICENSE", "NOTICE.md"):
            if files[name] != (ROOT / name).read_bytes():
                raise RuntimeError(f"release archive notice mismatch: {filename}/{name}")
        payloads[target] = files[spec["binary"]]
    with tempfile.TemporaryDirectory(prefix=".npm-cli-stage-", dir=out.parent) as temporary:
        staging = Path(temporary)
        env = isolated_env(staging / "environment")
        npm = npm_command()
        for target in [*TARGETS, None]:
            name = "cli" if target is None else f"cli-{target}"
            template = ROOT / "packages" / name
            stage = staging / name
            stage.mkdir()
            metadata = read_json((template / "package.json").read_bytes())
            metadata["version"] = version
            if target is None:
                metadata["optionalDependencies"] = {f"{PARENT}-{key}": version for key in TARGETS}
                for path in ("bin/gukhanmun.cjs", "lib/launcher.cjs"):
                    destination = stage / path
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(template / path, destination)
                (stage / "bin/gukhanmun.cjs").chmod(0o755)
                (stage / "completions").mkdir()
                for filename, data in completion_files.items():
                    (stage / "completions" / filename).write_bytes(data)
                shutil.copyfile(ROOT / "LICENSE", stage / "LICENSE")
            else:
                binary = stage / TARGETS[target]["binary"]
                binary.write_bytes(payloads[target])
                binary.chmod(0o755)
                for notice in ("LICENSE", "NOTICE.md"):
                    shutil.copyfile(ROOT / notice, stage / notice)
                for dictionary in ("loanwords", "opendict"):
                    shutil.copyfile(ROOT / f"crates/gukhanmun-{dictionary}/ATTRIBUTION.md",
                                    stage / f"ATTRIBUTION.{dictionary}.md")
                write_json(stage / "provenance.json", {
                    "schema_version": 1, "source": source, "version": version, "target": target,
                    "archive": manifest["archives"][target], "binary_sha256": sha256(payloads[target]),
                })
            if (template / "README.md").is_file():
                shutil.copyfile(template / "README.md", stage / "README.md")
            write_json(stage / "package.json", metadata)
            result = subprocess.run(
                [*npm, "pack", "--ignore-scripts", "--json", "--pack-destination", str(out)],
                cwd=stage, env=env, capture_output=True, text=True, timeout=180,
            )
            if result.returncode:
                raise RuntimeError(f"npm pack failed: {result.stderr}")
            packed = read_json(result.stdout)
            filename = packed[0]["filename"]
            safe_name(filename)
            if filename != Path(filename).name:
                raise RuntimeError("npm returned an unsafe tarball filename")
            original = out / filename
            # npm may clean build metadata in generated filenames; retain it here.
            filename = f"gukhanmun-{name}-{version}.tgz"
            original.rename(out / filename)
            data = (out / filename).read_bytes()
            record = {"name": metadata["name"], "version": version, "filename": filename,
                      "sha256": sha256(data), "integrity": integrity(data)}
            if target is not None:
                record.update(target=target, binary_sha256=sha256(payloads[target]))
            manifest["packages"].append(record)
        # Validate before publishing the commit marker, including the actual npm allowlist.
        for record in manifest["packages"]:
            files, modes = archive_files(out / record["filename"], npm=True)
            package_metadata(files, modes, record, manifest)
        write_json(out / "manifest.json", manifest)
    validate_bundle(out)
    print(sha256((out / "manifest.json").read_bytes()))
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    packing = subparsers.add_parser("pack")
    for name in ("archives", "completions", "out"):
        packing.add_argument(f"--{name}", type=Path, required=True)
    packing.add_argument("--version", required=True)
    packing.add_argument("--revision", required=True)
    validating = subparsers.add_parser("validate")
    validating.add_argument("--dir", type=Path, required=True)
    validating.add_argument("--manifest-sha256", required=True)
    validating.add_argument("--artifact-id")
    args = parser.parse_args()
    if args.command == "pack":
        pack(args.archives, args.completions, args.out, args.version, args.revision)
    else:
        validate_bundle(args.dir, args.manifest_sha256, args.artifact_id)
        print("npm CLI candidate verified")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
