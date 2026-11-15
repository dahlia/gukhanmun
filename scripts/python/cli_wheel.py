# Gukhanmun: Assembles and structurally verifies standalone command wheels.
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

"""Deterministic, stdlib-only packaging; never downloads or builds payloads."""

import argparse
import base64
import csv
import email.parser
import hashlib
import importlib.util
import io
import json
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SIZE_LIMIT = 100 * 1024 * 1024
LICENSE_EXPRESSION = "GPL-3.0-only AND LicenseRef-CC-BY-SA-2.0-KR AND LicenseRef-NIKL-Free-Use"
COMMANDS = {"gukhanmun": "main", "gukhanmun-mkdict": "mkdict_main"}
ENTRY_POINTS = ("[console_scripts]\n" + "".join(
    f"{command} = gukhanmun_cli:{entry}\n" for command, entry in COMMANDS.items())).encode()
SHELL_FILES = {
    "bash": "gukhanmun.bash", "zsh": "_gukhanmun", "fish": "gukhanmun.fish",
    "powershell": "_gukhanmun.ps1", "nushell": "gukhanmun.nu",
}


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def load(name, path=None):
    path = path or Path(__file__).with_name(f"{name}.py")
    spec = importlib.util.spec_from_file_location(f"_cli_{name}", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def sha(data):
    return hashlib.sha256(data).hexdigest()


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def json_bytes(value):
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False) + "\n").encode()


def pairs(entries):
    result = {}
    for name, value in entries:
        require(name not in result, f"duplicate JSON key {name}")
        result[name] = value
    return result


def read_json(path):
    return json.loads(Path(path).read_bytes(), object_pairs_hook=pairs)


def version_check(version):
    require(isinstance(version, str) and re.fullmatch(
        r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)"
        r"(?:(?:a|b|rc)(?:0|[1-9]\d*))?(?:\.dev(?:0|[1-9]\d*))?", version),
        "version must be canonical PEP 440 without local labels")


def version_pair_check(native_version, python_version, commit):
    """Bind the supported native stamping policy to Python spelling and source.

    Unstamped final/prerelease builds allow the matching Python version or its
    PR/bootstrap ``.dev0`` spelling. Branch stamping adds a positive run number
    and a Git abbreviation. Already-development workspace versions retain their
    original Python development number even when native stamping adds another.
    Fork branch pushes retain native stamping but Python uses ``.dev0``.
    This nonpublishing pair is allowed here; upload authorization independently
    requires the eligible upstream context and its positive run-number version.
    """
    version_check(python_version)
    require(isinstance(commit, str) and re.fullmatch(r"[0-9a-f]{40}", commit),
            "full source SHA required")
    require(isinstance(native_version, str), "unsupported or malformed native version")
    stamped = re.fullmatch(r"(.+)-dev\.([1-9][0-9]*)\+([0-9a-f]+)", native_version)
    base = stamped[1] if stamped else native_version
    number = r"(?:0|[1-9][0-9]*)"
    parsed = re.fullmatch(
        rf"({number}\.{number}\.{number})(?:-(alpha|beta|rc|a|b)\.({number}))?"
        rf"(?:-dev\.({number}))?", base,
    )
    require(parsed is not None, "unsupported or malformed native version")
    converted = parsed[1]
    if parsed[2] is not None:
        converted += {"alpha": "a", "beta": "b", "a": "a", "b": "b", "rc": "rc"}[parsed[2]] + parsed[3]
    if parsed[4] is not None:
        converted += ".dev" + parsed[4]
    if stamped:
        require(commit.startswith(stamped[3]), "native version SHA is not a prefix of the full source commit")
        expected = converted if parsed[4] is not None else converted + ".dev" + stamped[2]
        allowed = {expected}
        if parsed[4] is None:
            allowed.add(converted + ".dev0")
        require(python_version in allowed, "native/Python version pair differs")
    else:
        allowed = {converted}
        if parsed[4] is None:
            allowed.add(converted + ".dev0")
        require(python_version in allowed, "native/Python version pair differs")


def licenses(root=ROOT):
    """Use the exact canonical software/data texts used by dictionary staging."""
    dictionaries = load("dictionaries")
    result = {"LICENSE": (root / "LICENSE").read_bytes(),
              "NOTICE.md": (root / "NOTICE.md").read_bytes()}
    for distribution in dictionaries.load_table(
            root / "python/gukhanmun/src/gukhanmun/_dictionary_data.py"):
        for name, data in dictionaries.license_files(root, distribution).items():
            result[f"{distribution.name}/{name}"] = data
    return result


def project(root=ROOT):
    data = tomllib.loads((root / "python/gukhanmun-cli/pyproject.toml").read_text(encoding="utf-8"))
    metadata = data["project"]
    require(data["build-system"] == {"requires": [], "build-backend": "build_backend", "backend-path": ["."]},
            "source backend contract differs")
    require(metadata["name"] == "gukhanmun-cli" and metadata.get("dependencies") == []
            and not metadata.get("optional-dependencies") and not metadata.get("dynamic")
            and metadata["requires-python"] == ">=3.11" and metadata["license"] == LICENSE_EXPRESSION
            and metadata["scripts"] == {command: f"gukhanmun_cli:{entry}" for command, entry in COMMANDS.items()},
            "static metadata contract differs")
    version_check(metadata["version"])
    return metadata


def completion_files(directory, native_version, commit):
    generator = load("completion_generate", ROOT / "scripts/completions/generate.py")
    directory = Path(directory)
    require(not directory.is_symlink(), "completion directory must not be a symlink")
    require({p.name for p in directory.iterdir()} == {*SHELL_FILES.values(), "manifest.json"},
            "completion directory must contain exactly the shared bundle")
    generator.verify(directory, revision=commit, version=native_version)
    result = {}
    for name in (*SHELL_FILES.values(), "manifest.json"):
        path = directory / name
        require(path.is_file() and not path.is_symlink(), f"unsafe completion resource {name}")
        result[name] = path.read_bytes()
    return result


def filename(item, version):
    return f"gukhanmun_cli-{version}-py3-none-{item.platform}.whl"


def archive_files(path):
    """Reject ambiguous paths, special files, duplicates and excessive archives."""
    require(Path(path).stat().st_size <= SIZE_LIMIT, "wheel exceeds PyPI per-file size ceiling")
    result, modes = {}, {}
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        require(len(names) == len(set(names)), "duplicate wheel member")
        require(sum(i.file_size for i in archive.infolist()) <= 1024 * 1024 * 1024,
                "wheel decompressed size exceeds safety limit")
        for info in archive.infolist():
            name = info.filename
            require(name and "\\" not in name and "\0" not in name and ":" not in name
                    and not name.startswith("/") and str(PurePosixPath(name)) == name
                    and all(p not in {"", ".", ".."} for p in name.split("/")),
                    f"unsafe wheel path {name!r}")
            require(name not in result, f"duplicate wheel member {name}")
            mode = info.external_attr >> 16
            require(info.create_system == 3 and stat.S_ISREG(mode), f"nonregular wheel member {name}")
            require(not info.flag_bits & 1, "encrypted wheel member")
            result[name], modes[name] = archive.read(info), stat.S_IMODE(mode)
    return result, modes


def check_wheel(path, item, version, *, root=ROOT):
    """Verify exact payload, metadata, modes, resources and every RECORD row."""
    version_check(version)
    require(Path(path).name == filename(item, version), "wheel filename differs from target/version")
    members, modes = archive_files(path)
    info = f"gukhanmun_cli-{version}.dist-info"
    package = "gukhanmun_cli"
    natives = {command: f"{package}/_bin/{command}" + (".exe" if item.family == "windows" else "")
               for command in COMMANDS}
    required = {
        f"{package}/__init__.py", *natives.values(), f"{package}/payload.json",
        f"{info}/METADATA", f"{info}/WHEEL", f"{info}/entry_points.txt",
        f"{info}/RECORD",
        *(f"{package}/completions/{name}" for name in (*SHELL_FILES.values(), "manifest.json")),
        *(f"{package}/licenses/{name}" for name in licenses(root)),
        *(f"{info}/licenses/{name}" for name in licenses(root)),
    }
    require(set(members) == required, "unexpected or missing wheel resources")
    require(all(modes[name] == 0o755 for name in natives.values()) and
            all(mode == 0o644 for name, mode in modes.items() if name not in natives.values()),
            "incorrect executable/resource modes")
    expected_launcher = (root / "python/gukhanmun-cli/src/gukhanmun_cli/__init__.py").read_bytes()
    require(members[f"{package}/__init__.py"] == expected_launcher, "launcher source differs")
    for name, data in licenses(root).items():
        require(members[f"{package}/licenses/{name}"] == data, f"canonical license differs: {name}")
        require(members[f"{info}/licenses/{name}"] == data, f"dist-info license differs: {name}")
    fields = email.parser.BytesParser().parsebytes(members[f"{info}/METADATA"])
    metadata = project(root)
    expected_fields = {
        "Metadata-Version": "2.4", "Name": "gukhanmun-cli", "Version": version,
        "Requires-Python": ">=3.11", "License-Expression": LICENSE_EXPRESSION,
        "Description-Content-Type": "text/markdown",
        "Summary": metadata["description"],
        "Author-email": ", ".join(f"{a['name']} <{a['email']}>" for a in metadata["authors"]),
    }
    for key, value in expected_fields.items():
        require(fields.get_all(key) == [value], f"invalid metadata {key}")
    require(fields.get_all("License-File") == sorted(licenses(root)), "license metadata differs")
    require(fields.get_all("Project-URL") == [f"{name}, {url}" for name, url in sorted(metadata["urls"].items())],
            "project URL metadata differs")
    require(not fields.get_all("Requires-Dist") and not fields.get_all("Provides-Extra"), "dependencies/extras forbidden")
    require(fields.get_payload() == (root / "python/gukhanmun-cli/README.md").read_text(encoding="utf-8"),
            "wheel README differs")
    wheel = email.parser.BytesParser().parsebytes(members[f"{info}/WHEEL"])
    require(wheel.get_all("Wheel-Version") == ["1.0"] and wheel.get_all("Root-Is-Purelib") == ["false"],
            "invalid WHEEL properties")
    require(wheel.get_all("Tag") == [f"py3-none-{p}" for p in item.platform.split(".")], "expanded tags differ")
    require(members[f"{info}/entry_points.txt"] == ENTRY_POINTS,
            "entry point differs")
    rows = list(csv.reader(io.StringIO(members[f"{info}/RECORD"].decode())))
    require(len(rows) == len(members) and all(len(row) == 3 for row in rows), "invalid RECORD rows")
    require(len({row[0] for row in rows}) == len(rows) and {row[0] for row in rows} == set(members),
            "RECORD inventory differs")
    for name, checksum, size in rows:
        if name == f"{info}/RECORD":
            require(checksum == size == "", "RECORD must not hash itself")
        else:
            wanted = base64.urlsafe_b64encode(hashlib.sha256(members[name]).digest()).rstrip(b"=").decode()
            require(checksum == f"sha256={wanted}" and size == str(len(members[name])), f"RECORD mismatch: {name}")
    payload = json.loads(members[f"{package}/payload.json"], object_pairs_hook=pairs)
    require(type(payload.get("schema")) is int and payload["schema"] == 1
            and payload.get("target") == item.id and payload.get("version") == version,
            "payload identity differs")
    require(isinstance(payload.get("native_version"), str) and payload["native_version"]
            and re.fullmatch(r"[0-9A-Za-z.+-]+", payload["native_version"]), "invalid native version")
    require(re.fullmatch(r"[0-9a-f]{40}", payload.get("commit", "")), "invalid source identity")
    version_pair_check(payload["native_version"], version, payload["commit"])
    resources = {name: sha(data) for name, data in members.items()
                 if name.startswith(f"{package}/") and name != f"{package}/payload.json"}
    require(payload.get("resources") == resources, "payload resource hashes differ")
    require(payload.get("executable_sha256") == sha(members[natives["gukhanmun"]]), "native executable hash differs")
    require(payload.get("mkdict_sha256") == sha(members[natives["gukhanmun-mkdict"]]), "mkdict executable hash differs")
    completion = json.loads(members[f"{package}/completions/manifest.json"], object_pairs_hook=pairs)
    require(set(completion) == {"schema_version", "command", "version", "source_revision", "worktree_dirty", "files"}
            and type(completion["schema_version"]) is int and completion["schema_version"] == 1
            and completion["command"] == "gukhanmun" and type(completion["worktree_dirty"]) is bool,
            "invalid completion manifest")
    require(completion["version"] == payload.get("native_version") and completion["source_revision"] == payload["commit"],
            "completion identity differs")
    require(completion["files"] == {name: sha(members[f"{package}/completions/{name}"])
                                    for name in SHELL_FILES.values()}, "completion hashes differ")
    for name in SHELL_FILES.values():
        data = members[f"{package}/completions/{name}"]
        require(data.strip() and b"\r" not in data, "invalid completion contents")
        data.decode("utf-8")
    return payload


def assemble(args, *, root=ROOT):
    item = load("cli_artifacts").target(args.target)
    version_check(args.version)
    require(re.fullmatch(r"[0-9a-f]{40}", args.commit), "full source SHA required")
    version_pair_check(args.native_version, args.version, args.commit)
    require(args.binary.is_file() and not args.binary.is_symlink(), "native payload must be a regular nonsymlink file")
    require(args.mkdict_binary.is_file() and not args.mkdict_binary.is_symlink(),
            "mkdict payload must be a regular nonsymlink file")
    require(args.native_version and "\n" not in args.native_version, "invalid native version")
    compiler = args.compiler_file.read_text(encoding="utf-8")
    require("rustc " in compiler and "host: " in compiler, "rustc -vV compiler record required")
    completions = completion_files(args.completions, args.native_version, args.commit)
    metadata = project(root)
    package, info = "gukhanmun_cli", f"gukhanmun_cli-{args.version}.dist-info"
    native = f"{package}/_bin/gukhanmun" + (".exe" if item.family == "windows" else "")
    mkdict = f"{package}/_bin/gukhanmun-mkdict" + (".exe" if item.family == "windows" else "")
    contents = {f"{package}/__init__.py": (root / "python/gukhanmun-cli/src/gukhanmun_cli/__init__.py").read_bytes(),
                 native: args.binary.read_bytes(), mkdict: args.mkdict_binary.read_bytes()}
    contents.update({f"{package}/completions/{name}": data for name, data in completions.items()})
    contents.update({f"{package}/licenses/{name}": data for name, data in licenses(root).items()})
    payload = {"schema": 1, "target": item.id, "version": args.version, "native_version": args.native_version,
                "commit": args.commit, "executable_sha256": sha(contents[native]), "mkdict_sha256": sha(contents[mkdict]),
               "resources": {name: sha(data) for name, data in sorted(contents.items())}}
    contents[f"{package}/payload.json"] = json_bytes(payload)
    contents.update({f"{info}/licenses/{name}": data for name, data in licenses(root).items()})
    contents[f"{info}/METADATA"] = (
        f"Metadata-Version: 2.4\nName: gukhanmun-cli\nVersion: {args.version}\n"
        f"Summary: {metadata['description']}\nRequires-Python: >=3.11\n"
        "Author-email: " + ", ".join(f"{a['name']} <{a['email']}>" for a in metadata["authors"]) + "\n"
        + "".join(f"Project-URL: {name}, {url}\n" for name, url in sorted(metadata["urls"].items()))
        +
        f"License-Expression: {LICENSE_EXPRESSION}\n"
        + "".join(f"License-File: {name}\n" for name in sorted(licenses(root)))
        +
        "Description-Content-Type: text/markdown\n\n"
        + (root / "python/gukhanmun-cli/README.md").read_text(encoding="utf-8")
    ).encode()
    contents[f"{info}/WHEEL"] = (
        "Wheel-Version: 1.0\nGenerator: gukhanmun-cli-assembler\nRoot-Is-Purelib: false\n"
        + "".join(f"Tag: py3-none-{p}\n" for p in item.platform.split("."))
    ).encode()
    contents[f"{info}/entry_points.txt"] = ENTRY_POINTS
    record = io.StringIO(newline="")
    writer = csv.writer(record, lineterminator="\n")
    for name, data in sorted(contents.items()):
        checksum = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()
        writer.writerow((name, f"sha256={checksum}", str(len(data))))
    writer.writerow((f"{info}/RECORD", "", ""))
    contents[f"{info}/RECORD"] = record.getvalue().encode()
    args.output.mkdir(parents=True, exist_ok=True)
    wheel = args.output / filename(item, args.version)
    require(not wheel.exists(), f"refusing to overwrite {wheel}")
    try:
        with zipfile.ZipFile(wheel, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for name, data in sorted(contents.items()):
                entry = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                entry.create_system = 3
                entry.external_attr = (stat.S_IFREG | (0o755 if name in (native, mkdict) else 0o644)) << 16
                entry.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(entry, data)
        check_wheel(wheel, item, args.version, root=root)
    except BaseException:
        wheel.unlink(missing_ok=True)
        raise
    record = {key: payload[key] for key in ("schema", "target", "version", "native_version", "commit",
                                          "executable_sha256", "mkdict_sha256")}
    record.update(wheel={"filename": wheel.name, "sha256": digest(wheel)},
                  target_definition=vars(item), compiler=compiler,
                  completion_manifest_sha256=sha(completions["manifest.json"]), resources=payload["resources"])
    (args.output / f"cli-build-info-{item.id}.json").write_bytes(json_bytes(record))
    return wheel


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("target", "native-version", "version", "commit"):
        parser.add_argument(f"--{name}", required=True)
    for name in ("binary", "mkdict-binary", "completions", "compiler-file", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    print(assemble(args))


if __name__ == "__main__":
    main()
