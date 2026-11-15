# Gukhanmun: Tests the CLI-specific target matrix and fail-closed evidence gate.
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

import importlib.util
import copy
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
from types import SimpleNamespace

import pytest

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SPEC = importlib.util.spec_from_file_location("cli_artifacts_test_loader", ROOT / "scripts/python/cli_wheel.py")
wheel = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(wheel)
artifacts = wheel.load("cli_artifacts")


def elf(machine=62, segments=(2,), needed=False):
    data = bytearray(256)
    data[:6] = b"\x7fELF\x02\x01"
    struct.pack_into("<H", data, 18, machine)
    struct.pack_into("<Q", data, 32, 64)
    struct.pack_into("<HH", data, 54, 56, len(segments))
    for index, kind in enumerate(segments):
        struct.pack_into("<IIQQQQ", data, 64 + index * 56, kind, 0, 192, 0, 0, 32)
    if needed:
        struct.pack_into("<qQ", data, 192, 1, 1)
    return bytes(data)


def test_static_pie_dynamic_table_permitted_but_interpreter_and_needed_forbidden():
    item = artifacts.target("linux-x86_64")
    assert artifacts.elf(elf(), item)["needed"] == []
    with pytest.raises(RuntimeError, match="PT_INTERP"):
        artifacts.elf(elf(segments=(3,)), item)
    with pytest.raises(RuntimeError, match="DT_NEEDED"):
        artifacts.elf(elf(needed=True), item)
    with pytest.raises(RuntimeError, match="machine"):
        artifacts.elf(elf(machine=183), item)


def test_exact_independent_six_target_contract():
    assert set(artifacts.expected_files("0.4.0")) == {
        f"gukhanmun_cli-0.4.0-py3-none-{tag}.whl" for tag in (
            "manylinux_2_28_x86_64.musllinux_1_2_x86_64",
            "manylinux_2_28_aarch64.musllinux_1_2_aarch64",
            "macosx_11_0_x86_64", "macosx_11_0_arm64", "win_amd64", "win_arm64")}
    assert len(wheel.load("artifacts").expected_files("0.4.0")) == 31
    linux = artifacts.target("linux-aarch64")
    required = artifacts.evidence_expected(linux)
    assert "cli-execution-linux-aarch64-musllinux_1_2-cpython315.json" in required
    assert "cli-execution-linux-aarch64-manylinux_2_28-cpython311.json" in required
    assert "cli-execution-linux-aarch64-host-pypy312.json" in required
    assert not any("musllinux" in name and "pypy" in name for name in required)
    assert not any("pypy" in name for name in artifacts.evidence_expected(artifacts.target("windows-arm64")))


def test_wrong_native_host_cannot_generate_audit(monkeypatch):
    monkeypatch.setattr(artifacts.platform, "system", lambda: "Linux")
    monkeypatch.setattr(artifacts.platform, "machine", lambda: "x86_64")
    with pytest.raises(RuntimeError, match="native"):
        artifacts.native_host(artifacts.target("linux-aarch64"))


def test_no_evidence_cannot_make_complete_bundle(tmp_path):
    data = {"version": "0.4.0", "artifacts": [], "evidence": []}
    with pytest.raises(RuntimeError, match="six"):
        artifacts.check_records(tmp_path, data)


def test_isolated_script_entry_points():
    for name, args in (("cli_artifacts.py", ["matrix"]), ("cli_wheel.py", ["--help"]),
                       ("cli_smoke.py", ["--help"]), ("cli_publish.py", ["--help"])):
        result = subprocess.run([sys.executable, "-I", ROOT / "scripts/python" / name, *args], capture_output=True)
        assert result.returncode == 0, result.stderr.decode()


def test_runtime_environment_drops_ambient_configuration(tmp_path, monkeypatch):
    smoke = wheel.load("cli_smoke")
    monkeypatch.setenv("PIP_INDEX_URL", "https://not-allowed.invalid")
    monkeypatch.setenv("PYTHONPATH", "/not-allowed")
    monkeypatch.setenv("UV_PYTHON", "/not-allowed")
    env = smoke.environment(tmp_path)
    assert "PIP_INDEX_URL" not in env and "PYTHONPATH" not in env and "UV_PYTHON" not in env
    assert env["PIP_NO_INDEX"] == "1" and env["UV_PYTHON_DOWNLOADS"] == "never"
    assert env["PATH"] == ""


def test_wheelhouse_rejects_unlocked_bytes(tmp_path):
    smoke = wheel.load("cli_smoke")
    (tmp_path / "pip-26.0-py3-none-any.whl").write_bytes(b"not the locked wheel")
    with pytest.raises(RuntimeError, match="not pinned"):
        smoke.wheelhouse_check(tmp_path)


def macho_fixture(item, minimum=0x000b0000):
    data = bytearray(56)
    data[:4] = b"\xcf\xfa\xed\xfe"
    struct.pack_into("<I", data, 4, 0x1000007 if item.machine == "x86_64" else 0x100000c)
    struct.pack_into("<II", data, 16, 1, 24)
    struct.pack_into("<IIIIII", data, 32, 0x32, 24, 1, minimum, minimum, 0)
    return bytes(data)


def pe_fixture(item, library=None, delay=False):
    data = bytearray(1024)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 60, 64)
    data[64:68] = b"PE\0\0"
    struct.pack_into("<HH", data, 68, 0x8664 if item.machine == "x86_64" else 0xaa64, 1)
    struct.pack_into("<H", data, 84, 240)
    struct.pack_into("<H", data, 88, 0x20b)
    struct.pack_into("<IIII", data, 88 + 240 + 8, 512, 0x1000, 512, 512)
    if library:
        struct.pack_into("<II", data, 88 + 112 + 8 * (13 if delay else 1), 0x1000, 64)
        if delay:
            struct.pack_into("<II", data, 512, 1, 0x1080)
        else:
            struct.pack_into("<I", data, 512 + 12, 0x1080)
        data[640:640 + len(library) + 1] = library.encode() + b"\0"
    return bytes(data)


@pytest.fixture
def complete_cli(tmp_path):
    """Entirely synthetic unit fixtures, never retained as execution evidence."""
    directory = tmp_path / "bundle"
    directory.mkdir()
    completion = tmp_path / "completion"
    completion.mkdir()
    files = {}
    for name in wheel.SHELL_FILES.values():
        data = b"# synthetic unit-test shell fixture\n"
        (completion / name).write_bytes(data)
        files[name] = wheel.sha(data)
    (completion / "manifest.json").write_bytes(wheel.json_bytes({
        "schema_version": 1, "command": "gukhanmun", "version": "0.4.0",
        "source_revision": "a" * 40, "worktree_dirty": False, "files": files}))
    compiler = tmp_path / "compiler"
    compiler.write_text("rustc synthetic-unit-test\nhost: synthetic-unit-test\n")
    for index, item in enumerate(artifacts.TARGETS):
        binary = tmp_path / f"binary-{index}"
        contents = (elf(62 if item.machine == "x86_64" else 183) if item.family == "linux"
                    else macho_fixture(item) if item.family == "macos" else pe_fixture(item))
        binary.write_bytes(contents)
        args = SimpleNamespace(target=item.id, version="0.4.0", native_version="0.4.0",
                               commit="a" * 40, binary=binary, mkdict_binary=binary, completions=completion,
                               compiler_file=compiler, output=directory)
        path = wheel.assemble(args)
        _, base = artifacts.record_base(path, item, "0.4.0")
        host = {"system": {"linux": "Linux", "macos": "Darwin", "windows": "Windows"}[item.family],
                "machine": item.machine}
        details = {"linux": artifacts.elf, "macos": artifacts.macho_headers,
                   "windows": artifacts.pe_headers}[item.family](contents, item)
        (directory / f"cli-audit-{item.id}.json").write_bytes(wheel.json_bytes(
            base | {"host": host, "details": details, "mkdict_details": details}))
        for name in artifacts.evidence_expected(item):
            if not name.startswith("cli-execution-"):
                continue
            suffix = name.removesuffix(".json").rsplit("-", 1)[1]
            environment = name.removeprefix(f"cli-execution-{item.id}-").rsplit("-", 1)[0]
            implementation = "pypy" if suffix.startswith("pypy") else "cpython"
            minor = int(suffix.removeprefix(implementation)[1:])
            libc = None
            if environment == "manylinux_2_28":
                libc = {"family": "glibc", "version": "2.28"}
            elif environment == "musllinux_1_2":
                libc = {"family": "musl", "version": "1.2.5"}
            record = base | {
                "host": host, "environment": environment, "libc": libc,
                "image": wheel.load("artifacts").IMAGES[f"{environment}_{item.machine}"] if environment != "host" else None,
                "interpreter": {"implementation": implementation, "version": [3, minor, 0],
                                "executable": "/synthetic-unit-fixture/python"},
                "checks": ["version", "help", "dictionary", "pipe", "file", "error", "completions",
                           "signals", "uninstall", "resolver"],
                "tools": ["pipx", "uv"],
                "mkdict_checks": ["version", "help", "fst", "cdb", "error"],
                "console": ({name: {"ctrl_c": 0xC000013A, "ctrl_break": 0xC000013A} for name in ("direct", "pip", "uv")}
                            if item.family == "windows" else {"2": [-2, -2], "15": [-15, -15], "broken_pipe": [1, 1]}),
            }
            (directory / name).write_bytes(wheel.json_bytes(record))
        if index == 0:
            for shell in wheel.SHELL_FILES:
                record = base | {"host": host, "shell": shell, "shell_version": "synthetic-unit-test",
                                 "checks": ["activation", "option", "value"]}
                (directory / f"cli-shell-{item.id}-{shell}.json").write_bytes(wheel.json_bytes(record))
    artifacts.manifest(SimpleNamespace(dir=directory, version="0.4.0", native_version="0.4.0",
                                       commit="a" * 40, ref="refs/heads/main", run_id="123", run_attempt="1"))
    return directory


def refresh_manifest(directory):
    data = wheel.read_json(directory / "manifest.json")
    for key in ("artifacts", "evidence"):
        for entry in data[key]:
            entry.update(artifacts.inventory(directory / entry["filename"]))
    (directory / "manifest.json").write_bytes(wheel.json_bytes(data))
    (directory / "SHA256SUMS").write_text("".join(
        f"{entry['sha256']}  {entry['filename']}\n"
        for entry in sorted(data["artifacts"] + data["evidence"], key=lambda e: e["filename"])))


def test_complete_cli_validator_is_offline_and_independent(complete_cli, monkeypatch):
    monkeypatch.setattr(artifacts.subprocess, "run", lambda *_a, **_k: pytest.fail("offline validator spawned a process"))
    result = artifacts.validate(complete_cli, "0.4.0", env={})
    assert len(result["artifacts"]) == 6
    assert result["kind"] == "gukhanmun-cli"


@pytest.mark.parametrize("filename,field,value", [
    ("cli-build-info-linux-x86_64.json", "commit", "b" * 40),
    ("cli-audit-linux-x86_64.json", "details", {"format": "ELF64", "machine": 183}),
    ("cli-audit-linux-x86_64.json", "mkdict_details", {}),
    ("cli-build-info-linux-x86_64.json", "mkdict_sha256", "b" * 64),
    ("cli-execution-linux-x86_64-host-cpython315.json", "tools", ["pipx"]),
    ("cli-execution-linux-x86_64-host-cpython311.json", "host", {"system": "Linux", "machine": "aarch64"}),
    ("cli-execution-linux-x86_64-manylinux_2_28-cpython311.json", "image", "not-pinned"),
    ("cli-execution-linux-x86_64-musllinux_1_2-cpython315.json", "libc", {"family": "glibc", "version": "2.28"}),
    ("cli-execution-linux-x86_64-host-cpython315.json", "checks", ["help"]),
    ("cli-execution-linux-x86_64-host-cpython315.json", "mkdict_checks", ["version", "help"]),
    ("cli-execution-windows-amd64-host-cpython315.json", "console", {"direct": True, "pip": True}),
    ("cli-shell-linux-x86_64-nushell.json", "checks", ["activation"]),
])
def test_stale_or_incomplete_evidence_fails_even_with_fresh_inventory(complete_cli, filename, field, value):
    path = complete_cli / filename
    data = wheel.read_json(path)
    data[field] = value
    path.write_bytes(wheel.json_bytes(data))
    refresh_manifest(complete_cli)
    with pytest.raises(RuntimeError):
        artifacts.validate(complete_cli, "0.4.0", env={})


def test_missing_floor_evidence_and_immutable_digest_fail(complete_cli):
    with pytest.raises(RuntimeError, match="digest"):
        artifacts.validate(complete_cli, "0.4.0", env={}, manifest_sha256="f" * 64)
    missing = complete_cli / "cli-execution-linux-aarch64-musllinux_1_2-cpython311.json"
    missing.unlink()
    data = wheel.read_json(complete_cli / "manifest.json")
    data["evidence"] = [entry for entry in data["evidence"] if entry["filename"] != missing.name]
    (complete_cli / "manifest.json").write_bytes(wheel.json_bytes(data))
    refresh_manifest(complete_cli)
    with pytest.raises(RuntimeError, match="mandatory"):
        artifacts.validate(complete_cli, "0.4.0", env={})


@pytest.mark.parametrize("delay", [False, True])
@pytest.mark.parametrize("library", ["python315.dll", "vcruntime140.dll", "ucrtbase.dll", "msvcrt.dll", "untrusted.dll"])
def test_pe_normal_and_delay_imports_reject_dynamic_runtime(library, delay):
    item = artifacts.target("windows-amd64")
    with pytest.raises(RuntimeError, match="unapproved"):
        artifacts.pe_headers(pe_fixture(item, library, delay), item)
    assert artifacts.pe_headers(pe_fixture(item, "kernel32.dll", delay), item)["imports"] == ["kernel32.dll"]


def test_macos_floor_architecture_and_thin_policy():
    item = artifacts.target("macos-arm64")
    assert artifacts.macho_headers(macho_fixture(item), item)["minimums"] == [[11, 0, 0]]
    with pytest.raises(RuntimeError, match="newer"):
        artifacts.macho_headers(macho_fixture(item, minimum=0x000c0000), item)
    with pytest.raises(RuntimeError, match="architecture"):
        artifacts.macho_headers(macho_fixture(artifacts.target("macos-x86_64")), item)
    with pytest.raises(RuntimeError, match="thin"):
        artifacts.macho_headers(b"\xca\xfe\xba\xbe" + bytes(64), item)
