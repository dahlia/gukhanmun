# Gukhanmun: Audits and inventories the independent six-wheel CLI release.
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

"""The CLI matrix is deliberately separate from the extension's 31 files."""

import argparse
from dataclasses import asdict, dataclass
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import tempfile
import sys

_spec = importlib.util.spec_from_file_location("_cli_shared_wheel", Path(__file__).with_name("cli_wheel.py"))
wheel = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = wheel
_spec.loader.exec_module(wheel)

SCHEMA = 1
ROOT = wheel.ROOT
MACHINES = {"x86_64": "x86_64", "amd64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64"}


@dataclass(frozen=True)
class Target:
    id: str
    runner: str
    rust: str
    platform: str
    family: str
    machine: str
    pypy: bool


TARGETS = (
    Target("linux-x86_64", "ubuntu-latest", "x86_64-unknown-linux-musl",
           "manylinux_2_28_x86_64.musllinux_1_2_x86_64", "linux", "x86_64", True),
    Target("linux-aarch64", "ubuntu-24.04-arm", "aarch64-unknown-linux-musl",
           "manylinux_2_28_aarch64.musllinux_1_2_aarch64", "linux", "aarch64", True),
    Target("macos-x86_64", "macos-15-intel", "x86_64-apple-darwin",
           "macosx_11_0_x86_64", "macos", "x86_64", True),
    Target("macos-arm64", "macos-latest", "aarch64-apple-darwin",
           "macosx_11_0_arm64", "macos", "aarch64", True),
    Target("windows-amd64", "windows-latest", "x86_64-pc-windows-msvc",
           "win_amd64", "windows", "x86_64", True),
    Target("windows-arm64", "windows-11-arm", "aarch64-pc-windows-msvc",
           "win_arm64", "windows", "aarch64", False),
)


def target(name):
    for item in TARGETS:
        if item.id == name:
            return item
    raise RuntimeError(f"unknown CLI target {name!r}")


def expected_files(version, root=ROOT):
    wheel.version_check(version)
    return {wheel.filename(t, version): {
        "kind": "wheel", "distribution": "gukhanmun-cli", "target": t.id,
        "tags": {"python": "py3", "abi": "none", "platform": t.platform},
    } for t in TARGETS}


def native_host(item):
    system = platform.system()
    machine = MACHINES.get(platform.machine().lower())
    wheel.require(system == {"linux": "Linux", "macos": "Darwin", "windows": "Windows"}[item.family]
                  and machine == item.machine, f"{item.id} requires native matching OS/CPU")
    return {"system": system, "machine": machine}


def elf(data, item):
    """Read program headers directly; a static PIE may have PT_DYNAMIC."""
    wheel.require(data[:6] == b"\x7fELF\x02\x01", "CLI must be little-endian ELF64")
    machine = struct.unpack_from("<H", data, 18)[0]
    wheel.require(machine == {"x86_64": 62, "aarch64": 183}[item.machine], "ELF machine differs")
    offset = struct.unpack_from("<Q", data, 32)[0]
    size, count = struct.unpack_from("<HH", data, 54)
    wheel.require(size >= 56 and count > 0 and offset + size * count <= len(data), "invalid ELF program table")
    dynamic = []
    for index in range(count):
        kind, _, file_offset, _, _, file_size = struct.unpack_from("<IIQQQQ", data, offset + size * index)
        wheel.require(file_offset + file_size <= len(data), "ELF segment exceeds file")
        wheel.require(kind != 3, "ELF PT_INTERP is forbidden")
        if kind == 2:
            wheel.require(file_size % 16 == 0, "invalid ELF dynamic table")
            for position in range(file_offset, file_offset + file_size, 16):
                tag, value = struct.unpack_from("<qQ", data, position)
                if tag == 0:
                    break
                wheel.require(tag != 1, "ELF DT_NEEDED is forbidden")
                dynamic.append(tag)
    return {"format": "ELF64", "machine": machine, "interpreter": None, "needed": [],
            "dynamic_tags": dynamic, "floors": ["manylinux_2_28", "musllinux_1_2"]}


def output(command):
    return subprocess.run(command, check=True, capture_output=True, text=True, timeout=60).stdout


def macho_headers(data, item):
    wheel.require(data[:4] == b"\xcf\xfa\xed\xfe", "CLI must be a thin little-endian Mach-O64")
    cpu = struct.unpack_from("<I", data, 4)[0]
    wheel.require(cpu == {"x86_64": 0x1000007, "aarch64": 0x100000c}[item.machine], "Mach-O architecture differs")
    count, total = struct.unpack_from("<II", data, 16)
    position, minimums, libraries = 32, [], []
    wheel.require(position + total <= len(data), "invalid Mach-O load commands")
    for _ in range(count):
        command, size = struct.unpack_from("<II", data, position)
        wheel.require(size >= 8 and position + size <= 32 + total, "invalid Mach-O load command")
        if command in {0x24, 0x32}:
            if command == 0x32:
                wheel.require(struct.unpack_from("<I", data, position + 8)[0] == 1,
                              "Mach-O build platform must be macOS")
            value = struct.unpack_from("<I", data, position + (12 if command == 0x32 else 8))[0]
            minimums.append([value >> 16, (value >> 8) & 255, value & 255])
        if command in {0xc, 0x80000018, 0x8000001f, 0x20, 0x80000023}:
            string_offset = struct.unpack_from("<I", data, position + 8)[0]
            wheel.require(12 <= string_offset < size, "invalid Mach-O dylib name")
            raw = data[position + string_offset:position + size]
            wheel.require(b"\0" in raw, "unterminated Mach-O dylib name")
            libraries.append(raw.split(b"\0", 1)[0].decode("utf-8"))
        position += size
    wheel.require(minimums and all(v <= [11, 0, 0] for v in minimums), "Mach-O requires macOS newer than 11.0")
    wheel.require(all(name.startswith(("/usr/lib/", "/System/Library/")) for name in libraries),
                  "CLI links a non-system macOS library")
    return {"format": "Mach-O64", "machine": cpu, "minimums": minimums,
            "libraries": sorted(libraries), "signature_verified": item.machine == "aarch64"}


def macho(path, item):
    details = macho_headers(path.read_bytes(), item)
    linked = output(["otool", "-L", str(path)])
    libraries = [line.strip().split(" (", 1)[0] for line in linked.splitlines()[1:]]
    wheel.require(sorted(libraries) == details["libraries"], "otool and Mach-O import inventory differ")
    if item.machine == "aarch64":
        output(["codesign", "--verify", "--strict", str(path)])
    return details


WINDOWS_SYSTEM = {
    "advapi32.dll", "bcrypt.dll", "bcryptprimitives.dll", "cabinet.dll", "cfgmgr32.dll",
    "comctl32.dll", "comdlg32.dll", "crypt32.dll", "dbghelp.dll", "dnsapi.dll", "dwmapi.dll",
    "gdi32.dll", "imm32.dll", "iphlpapi.dll", "kernel32.dll", "msimg32.dll",
    "ncrypt.dll", "netapi32.dll", "ntdll.dll", "ole32.dll", "oleaut32.dll", "powrprof.dll",
    "propsys.dll", "psapi.dll", "rpcrt4.dll", "secur32.dll", "setupapi.dll", "shell32.dll",
    "shlwapi.dll", "user32.dll", "userenv.dll", "usp10.dll", "uxtheme.dll", "version.dll",
    "winhttp.dll", "wininet.dll", "winmm.dll", "winspool.drv", "wintrust.dll", "ws2_32.dll",
    "wtsapi32.dll",
}


def pe_headers(data, item):
    """Read both ordinary and delay imports without host tools or pefile."""
    wheel.require(len(data) >= 64 and data[:2] == b"MZ", "invalid DOS/PE header")
    offset = struct.unpack_from("<I", data, 60)[0]
    wheel.require(offset + 24 <= len(data) and data[offset:offset + 4] == b"PE\0\0", "invalid PE signature")
    machine, section_count = struct.unpack_from("<HH", data, offset + 4)
    wheel.require(machine == {"x86_64": 0x8664, "aarch64": 0xaa64}[item.machine], "PE machine differs")
    optional_size = struct.unpack_from("<H", data, offset + 20)[0]
    optional = offset + 24
    wheel.require(optional_size >= 112 + 16 * 8 and optional + optional_size + section_count * 40 <= len(data)
                  and struct.unpack_from("<H", data, optional)[0] == 0x20b, "CLI must be PE32+")
    image_base = struct.unpack_from("<Q", data, optional + 24)[0]
    section_offset = optional + optional_size
    def rva(value, size=1):
        for index in range(section_count):
            header = section_offset + 40 * index
            virtual_size, virtual_address, raw_size, raw_offset = struct.unpack_from("<IIII", data, header + 8)
            if virtual_address <= value < virtual_address + max(virtual_size, raw_size):
                relative = value - virtual_address
                wheel.require(relative + size <= raw_size and raw_offset + relative + size <= len(data),
                              "PE RVA extends outside section file data")
                return raw_offset + relative
        raise RuntimeError("PE RVA does not identify a section")
    def string(value):
        position = rva(value)
        end = data.find(b"\0", position, min(position + 4096, len(data)))
        wheel.require(end >= 0, "unterminated PE DLL name")
        return data[position:end].decode("ascii").lower()
    imports = set()
    for directory, width in ((1, 20), (13, 32)):
        address, size = struct.unpack_from("<II", data, optional + 112 + 8 * directory)
        if not address:
            wheel.require(size == 0, "invalid empty PE import directory")
            continue
        wheel.require(size >= width, "invalid PE import table")
        table = rva(address, size)
        terminated = False
        for position in range(table, table + size - width + 1, width):
            entry = data[position:position + width]
            if not any(entry):
                terminated = True
                break
            if directory == 1:
                name = struct.unpack_from("<I", entry, 12)[0]
            else:
                attributes, name = struct.unpack_from("<II", entry)
                if not attributes & 1:
                    name -= image_base
            imports.add(string(name))
        wheel.require(terminated, "unterminated PE import descriptor table")
    imports = sorted(imports)
    wheel.require(all(name in WINDOWS_SYSTEM or name.startswith("api-ms-win-core-") for name in imports),
                  f"CLI PE imports Python, dynamic CRT or unapproved libraries: {imports}")
    return {"format": "PE", "machine": machine, "imports": imports, "crt": "static",
            "policy_floor": "Windows 10 client/Windows Server 2016"}


def pe(path, item):
    return pe_headers(path.read_bytes(), item)


def record_base(path, item, version, *, root=ROOT):
    payload = wheel.check_wheel(path, item, version, root=root)
    record = {key: payload[key] for key in ("schema", "target", "version", "native_version", "commit",
                                          "executable_sha256", "mkdict_sha256")}
    record["wheel"] = {"filename": path.name, "sha256": wheel.digest(path)}
    return payload, record


def audit(args):
    item = target(args.target)
    host = native_host(item)
    path = args.dir / wheel.filename(item, args.version)
    _, record = record_base(path, item, args.version)
    members, _ = wheel.archive_files(path)
    measured = {}
    with tempfile.TemporaryDirectory(prefix="gukhanmun-cli-audit-") as temporary:
        for command in wheel.COMMANDS:
            name = f"gukhanmun_cli/_bin/{command}" + (".exe" if item.family == "windows" else "")
            executable = Path(temporary) / Path(name).name
            executable.write_bytes(members[name])
            measured[command] = (elf(members[name], item) if item.family == "linux"
                                 else (macho if item.family == "macos" else pe)(executable, item))
            key = "executable_sha256" if command == "gukhanmun" else "mkdict_sha256"
            wheel.require(wheel.digest(executable) == record[key], "audit modified the executable")
    record.update(host=host, details=measured["gukhanmun"], mkdict_details=measured["gukhanmun-mkdict"])
    (args.dir / f"cli-audit-{item.id}.json").write_bytes(wheel.json_bytes(record))
    return record


def inventory(path):
    return {"filename": path.name, "size": path.stat().st_size, "sha256": wheel.digest(path)}


def evidence_expected(item):
    """Mandatory coverage, independent of which evidence files happen to exist."""
    result = {f"cli-build-info-{item.id}.json", f"cli-audit-{item.id}.json"}
    environments = ["host", "manylinux_2_28", "musllinux_1_2"] if item.family == "linux" else ["host"]
    for environment in environments:
        implementations = ["cpython311", "cpython315"]
        if item.pypy and environment == "host":
            implementations += ["pypy311", "pypy312"]
        result.update(f"cli-execution-{item.id}-{environment}-{impl}.json" for impl in implementations)
    return result


def check_records(directory, data, *, complete=True, root=ROOT):
    expected = expected_files(data["version"])
    wheel.require(len(expected) == 6, "CLI definition must contain six wheels")
    artifacts = {entry["filename"]: entry for entry in data["artifacts"]}
    wheel.require(set(artifacts) == set(expected), "exactly six CLI wheels required")
    wheel.require(len(artifacts) == len(data["artifacts"]), "duplicate artifact inventory")
    evidence = {entry["filename"]: entry for entry in data["evidence"]}
    wheel.require(len(evidence) == len(data["evidence"]), "duplicate evidence inventory")
    shells = set()
    for item in TARGETS:
        path = directory / wheel.filename(item, data["version"])
        payload, base = record_base(path, item, data["version"], root=root)
        wheel.require(payload["commit"] == data["source"]["commit"]
                      and payload["native_version"] == data["native_version"], "wheel source/native version differs")
        wheel.require(all(artifacts[path.name].get(k) == v for k, v in expected[path.name].items()),
                      "artifact classification differs")
        if complete:
            wheel.require(evidence_expected(item) <= evidence.keys(), f"missing mandatory evidence for {item.id}")
        for name in evidence:
            if not name.startswith((f"cli-build-info-{item.id}.", f"cli-audit-{item.id}.",
                                    f"cli-execution-{item.id}-", f"cli-shell-{item.id}-")):
                continue
            record = wheel.read_json(directory / name)
            wheel.require(type(record.get("schema")) is int and
                          all(record.get(key) == value for key, value in base.items()), f"stale evidence: {name}")
            if name.startswith("cli-build-info-"):
                wheel.require(record.get("target_definition") == asdict(item) and record.get("resources") == payload["resources"]
                              and "rustc " in record.get("compiler", "") and "host: " in record.get("compiler", ""),
                              f"invalid build provenance: {name}")
                wheel.require(record.get("completion_manifest_sha256") ==
                              payload["resources"]["gukhanmun_cli/completions/manifest.json"], "completion provenance differs")
            else:
                wheel.require(record.get("host") == {"system": {"linux": "Linux", "macos": "Darwin", "windows": "Windows"}[item.family],
                                                   "machine": item.machine}, f"non-native evidence: {name}")
            if name.startswith("cli-audit-"):
                members, _ = wheel.archive_files(path)
                for command, key in (("gukhanmun", "details"), ("gukhanmun-mkdict", "mkdict_details")):
                    native_name = f"gukhanmun_cli/_bin/{command}" + (".exe" if item.family == "windows" else "")
                    measured = {"linux": elf, "macos": macho_headers, "windows": pe_headers}[item.family](members[native_name], item)
                    wheel.require(record.get(key) == measured, f"platform audit differs from packaged {command}")
            if name.startswith("cli-execution-"):
                environment = record.get("environment")
                interpreter = record.get("interpreter", {})
                implementation = interpreter.get("implementation")
                version = interpreter.get("version", [])
                wheel.require(len(version) >= 2 and implementation in {"cpython", "pypy"}, "missing interpreter identity")
                suffix = f"{implementation}{version[0]}{version[1]}"
                wheel.require(name == f"cli-execution-{item.id}-{environment}-{suffix}.json", "execution filename differs")
                wheel.require(set(record.get("checks", [])) >= {"version", "help", "dictionary", "pipe", "file", "error",
                                                                "completions", "signals", "uninstall"}, "incomplete execution checks")
                wheel.require(record.get("mkdict_checks") == ["version", "help", "fst", "cdb", "error"],
                              "incomplete mkdict execution checks")
                if environment != "host":
                    key = f"{environment}_{item.machine}"
                    wheel.require(item.family == "linux" and record.get("image") == wheel.load("artifacts").IMAGES.get(key),
                                  "floor evidence image differs")
                    libc = record.get("libc", {})
                    family, version_prefix = {"manylinux_2_28": ("glibc", "2.28"),
                                              "musllinux_1_2": ("musl", "1.2")}.get(environment, ("invalid", "invalid"))
                    wheel.require(libc.get("family") == family and
                                  (libc.get("version") == version_prefix or
                                   libc.get("version", "").startswith(version_prefix + ".")), "floor libc identity differs")
                if suffix == "cpython315":
                    tools = {"pipx", "uv"} if environment == "host" else ({"pipx"} if environment == "musllinux_1_2" else set())
                    wheel.require(set(record.get("tools", [])) >= tools, "missing tool lifecycle evidence")
                    if environment == "host":
                        wheel.require("resolver" in record["checks"], "missing prerelease resolver evidence")
                if item.family == "windows":
                    console = record.get("console", {})
                    expected_control = {"ctrl_c": 0xC000013A, "ctrl_break": 0xC000013A}
                    wheel.require(console.get("direct") == expected_control and console.get("pip") == expected_control and
                                  (suffix != "cpython315" or console.get("uv") == expected_control),
                                  "missing Windows console positive-control/shim evidence")
                else:
                    parity = record.get("console", {})
                    wheel.require(set(parity) >= {"2", "15", "broken_pipe"} and
                                  all(isinstance(parity[key], list) and len(parity[key]) == 2 and
                                      parity[key][0] == parity[key][1] for key in ("2", "15", "broken_pipe"))
                                  and parity["2"][0] != 0 and parity["15"][0] != 0, "missing POSIX signal/broken-pipe parity")
            if name.startswith("cli-shell-"):
                shell = record.get("shell")
                wheel.require(shell in wheel.SHELL_FILES and name == f"cli-shell-{item.id}-{shell}.json"
                              and record.get("checks") == ["activation", "option", "value"] and record.get("shell_version"),
                              "invalid shell activation evidence")
                shells.add(shell)
    if complete:
        wheel.require(shells == set(wheel.SHELL_FILES), "all five installed shells require activation evidence")
    allowed = set().union(*(evidence_expected(t) for t in TARGETS))
    allowed |= {f"cli-shell-{t.id}-{shell}.json" for t in TARGETS for shell in wheel.SHELL_FILES}
    wheel.require(set(evidence) <= allowed, "unknown CLI evidence")


def validate(directory, version, *, env=None, manifest_sha256=None, root=ROOT):
    """Validate the immutable CLI bundle without subprocesses or credentials."""
    directory = Path(directory)
    wheel.version_check(version)
    wheel.require(directory.is_dir() and not directory.is_symlink(), "unsafe artifact directory")
    wheel.require(all(p.is_file() and not p.is_symlink() and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", p.name)
                      for p in directory.iterdir()), "unsafe bundle entry")
    if manifest_sha256 is not None:
        wheel.require(re.fullmatch(r"[0-9a-f]{64}", manifest_sha256)
                      and wheel.digest(directory / "manifest.json") == manifest_sha256, "pinned manifest digest differs")
    data = wheel.read_json(directory / "manifest.json")
    wheel.require(type(data.get("schema")) is int and data["schema"] == 1 and data.get("kind") == "gukhanmun-cli"
                  and data.get("version") == version and isinstance(data.get("toolchain"), dict), "manifest identity differs")
    wheel.require(data["toolchain"] == {"matrix": [asdict(t) for t in TARGETS]}, "manifest matrix differs")
    source = data.get("source", {})
    wheel.require(re.fullmatch(r"[0-9a-f]{40}", source.get("commit", "")) and source.get("ref", "").startswith("refs/")
                  and all(re.fullmatch(r"[1-9]\d*", source.get(key, "")) for key in ("run_id", "run_attempt")),
                  "invalid source/run identity")
    env = os.environ if env is None else env
    if env.get("GITHUB_ACTIONS") == "true" or any(env.get(k) for k in ("GITHUB_SHA", "GITHUB_REF", "GITHUB_RUN_ID")):
        for field, key in (("commit", "GITHUB_SHA"), ("ref", "GITHUB_REF"), ("run_id", "GITHUB_RUN_ID")):
            wheel.require(source[field] == env.get(key), f"manifest differs from {key}")
        wheel.require(re.fullmatch(r"[1-9]\d*", env.get("GITHUB_RUN_ATTEMPT", ""))
                      and int(source["run_attempt"]) <= int(env["GITHUB_RUN_ATTEMPT"]), "future source attempt")
    entries = data.get("artifacts", []) + data.get("evidence", [])
    seen = set()
    for entry in entries:
        name = entry["filename"]
        wheel.require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", name) and name not in seen
                      and name not in {"manifest.json", "SHA256SUMS"}, "unsafe/duplicate inventory")
        seen.add(name)
        wheel.require(inventory(directory / name) == {k: entry[k] for k in ("filename", "size", "sha256")},
                      f"manifest hash/size differs: {name}")
    wheel.require({p.name for p in directory.iterdir()} == seen | {"manifest.json", "SHA256SUMS"}, "bundle inventory differs")
    expected_sums = "".join(f"{e['sha256']}  {e['filename']}\n" for e in sorted(entries, key=lambda e: e["filename"]))
    wheel.require((directory / "SHA256SUMS").read_text(encoding="utf-8") == expected_sums, "checksums differ")
    check_records(directory, data, complete=True, root=root)
    return data


def manifest(args):
    artifacts, evidence = [], []
    expected = expected_files(args.version)
    for path in sorted(args.dir.iterdir()):
        if path.name in {"manifest.json", "SHA256SUMS"}:
            continue
        wheel.require(path.is_file() and not path.is_symlink(), "nonregular bundle entry")
        entry = inventory(path)
        if path.name in expected:
            entry.update(expected[path.name])
            artifacts.append(entry)
        else:
            wheel.require(path.name.startswith("cli-") and path.suffix == ".json", f"unknown bundle entry {path.name}")
            evidence.append(entry)
    data = {"schema": 1, "kind": "gukhanmun-cli", "version": args.version, "native_version": args.native_version,
            "source": {"commit": args.commit, "ref": args.ref, "run_id": args.run_id, "run_attempt": args.run_attempt},
            "toolchain": {"matrix": [asdict(t) for t in TARGETS]}, "artifacts": artifacts, "evidence": evidence}
    check_records(args.dir, data)
    (args.dir / "manifest.json").write_bytes(wheel.json_bytes(data))
    (args.dir / "SHA256SUMS").write_text(
        "".join(f"{e['sha256']}  {e['filename']}\n" for e in sorted(artifacts + evidence, key=lambda e: e["filename"])),
        encoding="utf-8", newline="\n")
    validate(args.dir, args.version)
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("matrix")
    audit_parser = commands.add_parser("audit")
    audit_parser.add_argument("--target", required=True)
    create = commands.add_parser("manifest")
    for name in ("native-version", "commit", "ref", "run-id", "run-attempt"):
        create.add_argument(f"--{name}", required=True)
    verify = commands.add_parser("verify")
    verify.add_argument("--complete", action="store_true")
    for command in (audit_parser, create, verify):
        command.add_argument("--dir", type=Path, required=True)
        command.add_argument("--version", required=True)
    args = parser.parse_args()
    if args.command == "matrix":
        print(json.dumps({"include": [asdict(t) for t in TARGETS]}))
    elif args.command == "audit":
        audit(args)
    elif args.command == "manifest":
        manifest(args)
    else:
        validate(args.dir, args.version)


if __name__ == "__main__":
    main()
