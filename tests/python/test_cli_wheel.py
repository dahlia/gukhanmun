# Gukhanmun: Tests strict standalone wheel assembly and source-build refusal.
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
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import sys
from types import SimpleNamespace
import zipfile
import zlib

import pytest

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SPEC = importlib.util.spec_from_file_location("cli_wheel_test", ROOT / "scripts/python/cli_wheel.py")
wheel = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(wheel)
artifacts = wheel.load("cli_artifacts")


@pytest.fixture
def assembled(tmp_path):
    completion = tmp_path / "completions"
    completion.mkdir()
    scripts = {name: b"# synthetic unit-test completion\n" for name in wheel.SHELL_FILES.values()}
    for name, data in scripts.items():
        (completion / name).write_bytes(data)
    manifest = {"schema_version": 1, "command": "gukhanmun", "version": "0.4.0",
                "source_revision": "a" * 40, "worktree_dirty": False,
                "files": {name: wheel.sha(data) for name, data in scripts.items()}}
    (completion / "manifest.json").write_bytes(wheel.json_bytes(manifest))
    binary = tmp_path / "native"
    binary.write_bytes(b"synthetic unit-test binary, not execution evidence")
    mkdict_binary = tmp_path / "mkdict"
    mkdict_binary.write_bytes(b"synthetic unit-test mkdict, not execution evidence")
    compiler = tmp_path / "compiler"
    compiler.write_text("rustc synthetic\nhost: x86_64-unknown-linux-gnu\n")
    args = SimpleNamespace(target="linux-x86_64", version="0.4.0.dev0", native_version="0.4.0",
                           commit="a" * 40, binary=binary, mkdict_binary=mkdict_binary, completions=completion,
                           compiler_file=compiler, output=tmp_path / "output")
    result = wheel.assemble(args)
    return args, result


def rewrite(path, change):
    with zipfile.ZipFile(path) as original:
        entries = [(i, original.read(i)) for i in original.infolist()]
    with zipfile.ZipFile(path, "w") as archive:
        for info, data in change(entries):
            archive.writestr(info, data)


def test_deterministic_dual_tag_archive_and_exact_resources(assembled):
    args, path = assembled
    item = artifacts.target(args.target)
    payload = wheel.check_wheel(path, item, args.version)
    contents, modes = wheel.archive_files(path)
    assert modes["gukhanmun_cli/_bin/gukhanmun"] == 0o755
    assert payload["executable_sha256"] == wheel.digest(args.binary)
    assert modes["gukhanmun_cli/_bin/gukhanmun-mkdict"] == 0o755
    assert payload["mkdict_sha256"] == wheel.digest(args.mkdict_binary)
    assert payload["commit"] == args.commit
    info = f"gukhanmun_cli-{args.version}.dist-info"
    assert contents[f"{info}/WHEEL"].count(b"Tag:") == 2
    assert len([n for n in contents if "/completions/" in n]) == 6
    assert contents[f"{info}/entry_points.txt"] == (
        b"[console_scripts]\ngukhanmun = gukhanmun_cli:main\n"
        b"gukhanmun-mkdict = gukhanmun_cli:mkdict_main\n")
    args.output = args.output.parent / "second"
    assert wheel.assemble(args).read_bytes() == path.read_bytes()


@pytest.mark.parametrize("path", ["../escape", "/absolute", "double//slash", "dot/./file",
                                 "back\\slash", "C:drive", "end/", "nul\0byte"])
def test_unsafe_archive_names_rejected(assembled, path):
    args, archive = assembled
    def add(entries):
        info = zipfile.ZipInfo(path)
        info.filename = path  # Preserve hostile spelling even on Windows.
        info.create_system = 3
        info.external_attr = (stat.S_IFREG | 0o644) << 16
        return [*entries, (info, b"bad")]
    rewrite(archive, add)
    with pytest.raises(RuntimeError, match="unsafe"):
        wheel.check_wheel(archive, artifacts.target(args.target), args.version)


def test_duplicate_archive_member_rejected(assembled):
    args, archive = assembled
    with pytest.warns(UserWarning):
        rewrite(archive, lambda entries: [*entries, entries[0]])
    with pytest.raises(RuntimeError, match="duplicate"):
        wheel.check_wheel(archive, artifacts.target(args.target), args.version)


@pytest.mark.parametrize("effective", ["gukhanmun_cli/renamed.py", "../escape.py"])
def test_unicode_path_extra_cannot_replace_launcher(assembled, effective):
    args, archive = assembled
    original = "gukhanmun_cli/__init__.py"

    def mutate(entries):
        for info, _ in entries:
            if info.filename == original:
                encoded = effective.encode("utf-8")
                info.extra = struct.pack("<HHBI", 0x7075, 5 + len(encoded), 1,
                                         zlib.crc32(original.encode("utf-8"))) + encoded
        return entries

    rewrite(archive, mutate)
    with zipfile.ZipFile(archive) as source:
        info = next(info for info in source.infolist() if info.orig_filename == original)
        if info.filename == original:
            pytest.skip("this interpreter does not decode Unicode Path extra fields")
        assert info.filename == effective
    with pytest.raises(RuntimeError, match="ambiguous wheel path"):
        wheel.check_wheel(archive, artifacts.target(args.target), args.version)


@pytest.mark.parametrize("mode", [stat.S_IFLNK | 0o777, stat.S_IFDIR | 0o755, stat.S_IFIFO | 0o644])
def test_archive_special_files_rejected(assembled, mode):
    args, archive = assembled
    def mutate(entries):
        entries[0][0].external_attr = mode << 16
        return entries
    rewrite(archive, mutate)
    with pytest.raises(RuntimeError, match="nonregular"):
        wheel.check_wheel(archive, artifacts.target(args.target), args.version)


@pytest.mark.parametrize("member,replace", [
    ("WHEEL", lambda data: data.replace(b"Root-Is-Purelib: false", b"Root-Is-Purelib: true")),
    ("WHEEL", lambda data: data.replace(b"Tag: py3-none-musllinux", b"Tag: cp311-abi3-musllinux")),
    ("METADATA", lambda data: data.replace(b"Requires-Python: >=3.11", b"Requires-Python: >=3.10")),
    ("METADATA", lambda data: data.replace(b"Summary:", b"Requires-Dist: gukhanmun\nSummary:")),
    ("entry_points.txt", lambda data: data.replace(b"gukhanmun_cli:main", b"gukhanmun:main")),
    ("RECORD", lambda data: data.replace(b"sha256=", b"sha512=", 1)),
    ("LICENSE", lambda data: data + b"not canonical"),
    ("gukhanmun.bash", lambda data: data + b"not shared"),
    ("payload.json", lambda data: data.replace(b'"commit": "aa', b'"commit": "bb')),
])
def test_metadata_payload_and_record_tampering_rejected(assembled, member, replace):
    args, archive = assembled
    rewrite(archive, lambda entries: [(info, replace(data) if info.filename.split("/")[-1] == member else data)
                                     for info, data in entries])
    with pytest.raises(RuntimeError):
        wheel.check_wheel(archive, artifacts.target(args.target), args.version)


@pytest.mark.parametrize("command", ["gukhanmun", "gukhanmun-mkdict"])
def test_executable_mode_is_not_repaired(assembled, command):
    args, archive = assembled
    def mutate(entries):
        for info, _ in entries:
            if info.filename == f"gukhanmun_cli/_bin/{command}":
                info.external_attr = (stat.S_IFREG | 0o644) << 16
        return entries
    rewrite(archive, mutate)
    with pytest.raises(RuntimeError, match="modes"):
        wheel.check_wheel(archive, artifacts.target(args.target), args.version)


def test_builder_is_required_in_wheel(assembled):
    args, archive = assembled
    rewrite(archive, lambda entries: [(info, data) for info, data in entries
                                     if info.filename != "gukhanmun_cli/_bin/gukhanmun-mkdict"])
    with pytest.raises(RuntimeError, match="missing wheel resources"):
        wheel.check_wheel(archive, artifacts.target(args.target), args.version)


def test_builder_input_must_be_present_and_nonsymlink(assembled, tmp_path):
    args, _ = assembled
    args.mkdict_binary = tmp_path / "absent"
    with pytest.raises(RuntimeError, match="mkdict payload"):
        wheel.assemble(args)
    if os.name != "nt":
        args.mkdict_binary.symlink_to(args.binary)
        with pytest.raises(RuntimeError, match="mkdict payload"):
            wheel.assemble(args)


def test_size_ceiling(assembled, monkeypatch):
    args, archive = assembled
    monkeypatch.setattr(wheel, "SIZE_LIMIT", 1)
    with pytest.raises(RuntimeError, match="ceiling"):
        wheel.check_wheel(archive, artifacts.target(args.target), args.version)


def test_refuses_overwriting_and_completion_identity_mismatch(assembled):
    args, _ = assembled
    with pytest.raises(RuntimeError, match="overwrite"):
        wheel.assemble(args)
    args.commit = "b" * 40
    with pytest.raises(RuntimeError, match="revision"):
        wheel.assemble(args)


@pytest.mark.parametrize("version", ["0.4.0+sha", "0.4.0-dev.1", "01.4.0", "0.4.0.dev01",
                                   "1!0.4.0", "0.4.0.post1", "0.4"])
def test_invalid_python_versions(version):
    with pytest.raises(RuntimeError):
        wheel.version_check(version)


@pytest.mark.parametrize("native,python", [
    ("0.4.0", "0.4.0"),
    ("0.4.0", "0.4.0.dev0"),
    ("0.4.0-rc.2", "0.4.0rc2"),
    ("0.4.0-rc.2", "0.4.0rc2.dev0"),
    ("0.4.0-alpha.1", "0.4.0a1"),
    ("0.4.0-beta.1", "0.4.0b1.dev0"),
    ("0.4.0-dev.3", "0.4.0.dev3"),
    ("0.4.0-dev.0", "0.4.0.dev0"),
    ("0.4.0-dev.12+aaaaaaa", "0.4.0.dev12"),
    ("0.4.0-dev.12+aaaaaaa", "0.4.0.dev0"),
    ("0.4.0-rc.2-dev.12+aaaaaaa", "0.4.0rc2.dev12"),
    ("0.4.0-rc.2-dev.12+aaaaaaa", "0.4.0rc2.dev0"),
    ("0.4.0-dev.3-dev.12+aaaaaaa", "0.4.0.dev3"),
    ("0.4.0-rc.2-dev.3-dev.12+aaaaaaa", "0.4.0rc2.dev3"),
])
def test_native_python_version_pair_policy(native, python):
    wheel.version_pair_check(native, python, "a" * 40)


@pytest.mark.parametrize("native,python", [
    ("9.9.9", "0.4.0"),
    ("0.4.0", "0.5.0.dev0"),
    ("0.4.0", "0.4.0.dev12"),
    ("0.4.0-rc.2", "0.4.0rc3.dev0"),
    ("0.4.0-rc.2", "0.4.0.dev0"),
    ("0.4.0-dev.12+aaaaaaa", "0.4.0.dev13"),
    ("0.4.0-dev.12+aaaaaaa", "0.4.0"),
    ("0.4.0-dev.12+bbbbbbb", "0.4.0.dev12"),
    ("0.4.0-dev.12+bbbbbbb", "0.4.0.dev0"),
    ("0.4.0-dev.12+" + "a" * 41, "0.4.0.dev12"),
    ("0.4.0-dev.3-dev.12+aaaaaaa", "0.4.0.dev12"),
    ("0.4.0-dev.3-dev.12+bbbbbbb", "0.4.0.dev3"),
    ("0.4.0-dev.3", "0.4.0.dev0"),
    ("0.4.0-dev.3", "0.4.0.dev3.dev0"),
    ("0.4.0-dev.0+aaaaaaa", "0.4.0.dev0"),
    ("0.4.0-dev.012+aaaaaaa", "0.4.0.dev12"),
    ("0.4.0-dev.12+AAAAAAA", "0.4.0.dev12"),
    ("0.4.0+aaaaaaa", "0.4.0"),
    ("v0.4.0", "0.4.0"),
    ("00.4.0", "0.4.0"),
    ("0.4.0-rc.02", "0.4.0rc2"),
    ("0.4.0-dev.03", "0.4.0.dev3"),
    ("0.4.0-dev.3-dev.12", "0.4.0.dev3"),
    ("0.4.0-rc", "0.4.0rc0"),
    ("0.4", "0.4.0"),
    ("0.4.0\n", "0.4.0"),
])
def test_native_python_version_pair_rejects_unrelated_or_malformed_identity(native, python):
    with pytest.raises(RuntimeError):
        wheel.version_pair_check(native, python, "a" * 40)


def test_assembler_rejects_unrelated_version_before_reading_completions(assembled):
    args, _ = assembled
    args.native_version = "9.9.9"
    args.completions = args.completions / "does-not-exist"
    with pytest.raises(RuntimeError, match="version pair differs"):
        wheel.assemble(args)


def test_wheel_rejects_self_consistent_unrelated_native_and_completion_version(assembled):
    args, archive = assembled
    def mutate(entries):
        members = {info.filename: data for info, data in entries}
        completion_name = "gukhanmun_cli/completions/manifest.json"
        completion = json.loads(members[completion_name])
        completion["version"] = "9.9.9"
        members[completion_name] = wheel.json_bytes(completion)
        payload_name = "gukhanmun_cli/payload.json"
        payload = json.loads(members[payload_name])
        payload["native_version"] = "9.9.9"
        payload["resources"][completion_name] = wheel.sha(members[completion_name])
        members[payload_name] = wheel.json_bytes(payload)
        record_name = f"gukhanmun_cli-{args.version}.dist-info/RECORD"
        record = wheel.io.StringIO(newline="")
        writer = wheel.csv.writer(record, lineterminator="\n")
        for name, data in sorted(members.items()):
            if name == record_name:
                continue
            checksum = wheel.base64.urlsafe_b64encode(wheel.hashlib.sha256(data).digest()).rstrip(b"=").decode()
            writer.writerow((name, f"sha256={checksum}", str(len(data))))
        writer.writerow((record_name, "", ""))
        members[record_name] = record.getvalue().encode()
        return [(info, members[info.filename]) for info, _ in entries]
    rewrite(archive, mutate)
    with pytest.raises(RuntimeError, match="version pair differs"):
        wheel.check_wheel(archive, artifacts.target(args.target), args.version)


def test_backend_refuses_every_source_hook():
    backend = wheel.load("backend", ROOT / "python/gukhanmun-cli/build_backend.py")
    for name in ("build_wheel", "build_sdist", "build_editable", "prepare_metadata_for_build_wheel",
                 "prepare_metadata_for_build_editable", "get_requires_for_build_wheel",
                 "get_requires_for_build_editable", "get_requires_for_build_sdist"):
        with pytest.raises(RuntimeError, match="mise run py-cli-assemble"):
            getattr(backend, name)("/unused")


@pytest.mark.skipif(os.name == "nt", reason="POSIX closed-descriptor subprocess parity")
@pytest.mark.parametrize("descriptor", [1, 2])
def test_launcher_preserves_native_file_output_with_closed_standard_stream(tmp_path, descriptor):
    package = tmp_path / "package"
    native = package / "_bin/gukhanmun"
    native.parent.mkdir(parents=True)
    native.write_text('#!/bin/sh\nprintf "converted\\n" > "$3"\n')
    native.chmod(0o755)
    source = tmp_path / "input.txt"
    source.write_text("input\n")
    direct, wrapped = tmp_path / "direct.txt", tmp_path / "wrapped.txt"
    launcher = ROOT / "python/gukhanmun-cli/src/gukhanmun_cli/__init__.py"
    code = (
        "import pathlib,runpy,sys;"
        f"module=runpy.run_path({str(launcher)!r});"
        f"module['main'].__globals__['files']=lambda _:pathlib.Path({str(package)!r});"
        f"sys.argv=['gukhanmun',{str(source)!r},'--output',{str(wrapped)!r}];"
        "raise SystemExit(module['main']())"
    )
    options = dict(stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20,
                   preexec_fn=lambda: os.close(descriptor))
    expected = subprocess.run([native, source, "--output", direct], **options)
    actual = subprocess.run([sys.executable, "-I", "-B", "-c", code], **options)
    assert expected.returncode == actual.returncode == 0
    assert direct.read_bytes() == wrapped.read_bytes() == b"converted\n"


@pytest.mark.skipif(os.name == "nt", reason="POSIX closed-descriptor launch error")
def test_launcher_missing_payload_with_closed_stderr_keeps_status(tmp_path):
    launcher = ROOT / "python/gukhanmun-cli/src/gukhanmun_cli/__init__.py"
    code = (
        "import pathlib,runpy,sys;"
        f"module=runpy.run_path({str(launcher)!r});"
        f"module['main'].__globals__['files']=lambda _:pathlib.Path({str(tmp_path)!r});"
        "raise SystemExit(module['main']())"
    )
    result = subprocess.run([sys.executable, "-I", "-B", "-c", code],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            preexec_fn=lambda: os.close(2), timeout=20)
    assert result.returncode == 127 and result.stdout == b""


@pytest.mark.parametrize("entry", ["main", "mkdict_main"])
def test_launcher_missing_payload_returns_127(monkeypatch, capsys, tmp_path, entry):
    launcher = wheel.load("launcher", ROOT / "python/gukhanmun-cli/src/gukhanmun_cli/__init__.py")
    monkeypatch.setattr(launcher, "files", lambda _: tmp_path)
    monkeypatch.setattr(launcher.signal, "signal", lambda *args: None)
    assert getattr(launcher, entry)() == 127
    assert "cannot launch bundled executable" in capsys.readouterr().err


@pytest.mark.skipif(os.name == "nt", reason="POSIX exec mode behavior")
@pytest.mark.parametrize("command,entry", [("gukhanmun", "main"), ("gukhanmun-mkdict", "mkdict_main")])
def test_launcher_nonexecutable_returns_126_without_chmod(monkeypatch, tmp_path, capsys, command, entry):
    launcher = wheel.load("launcher", ROOT / "python/gukhanmun-cli/src/gukhanmun_cli/__init__.py")
    directory = tmp_path / "_bin"
    directory.mkdir()
    executable = directory / command
    executable.write_text("#!/bin/sh\nexit 0\n")
    executable.chmod(0o644)
    monkeypatch.setattr(launcher, "files", lambda _: tmp_path)
    monkeypatch.setattr(launcher.signal, "signal", lambda *args: None)
    assert getattr(launcher, entry)() == 126
    assert stat.S_IMODE(executable.stat().st_mode) == 0o644
    assert "cannot launch bundled executable" in capsys.readouterr().err


@pytest.mark.parametrize("command,entry", [("gukhanmun", "main"), ("gukhanmun-mkdict", "mkdict_main")])
def test_posix_launcher_forwards_argv_and_executes_without_cwd_env_or_fd_changes(monkeypatch, tmp_path, command, entry):
    launcher = wheel.load("launcher", ROOT / "python/gukhanmun-cli/src/gukhanmun_cli/__init__.py")
    calls = []
    class Stream:
        def flush(self):
            calls.append("flush")
    launcher_sys = SimpleNamespace(argv=["shim path", "--dictionary", "space 한글.gukfst", "--", "-dash.txt"],
                                   stdout=Stream(), stderr=Stream())
    monkeypatch.setattr(launcher, "sys", launcher_sys)
    monkeypatch.setattr(launcher, "files", lambda _: tmp_path)
    class Executed(Exception):
        pass
    def execv(path, arguments):
        calls.append((path, arguments))
        raise Executed
    monkeypatch.setattr(launcher, "os", SimpleNamespace(name="posix", execv=execv))
    monkeypatch.setattr(launcher.signal, "signal", lambda *args: calls.append(("signal", args)))
    with pytest.raises(Executed):
        getattr(launcher, entry)()
    assert calls[:2] == ["flush", "flush"]
    assert calls[-1] == (tmp_path / "_bin" / command,
                          [command, "--dictionary", "space 한글.gukfst", "--", "-dash.txt"])


@pytest.mark.parametrize("status", [0, 1, 127, 130, 0xC000013A, 0xFFFFFFFF])
@pytest.mark.parametrize("command,entry", [("gukhanmun", "main"), ("gukhanmun-mkdict", "mkdict_main")])
def test_windows_launcher_preserves_dword_and_restores_control_handlers(monkeypatch, tmp_path, status, command, entry):
    launcher = wheel.load("launcher", ROOT / "python/gukhanmun-cli/src/gukhanmun_cli/__init__.py")
    events = []
    handlers = {2: object(), 21: object()}
    original = dict(handlers)
    def install(number, handler):
        previous = handlers[number]
        handlers[number] = handler
        events.append(("handler", number))
        return previous
    class Child:
        def __enter__(self):
            return self
        def __exit__(self, *args):
            return False
        def wait(self):
            events.append("wait")
            assert callable(handlers[2]) and callable(handlers[21])
            assert handlers[2](2, None) is None and handlers[21](21, None) is None
            return status
    def spawn(command):
        events.append(("spawn", command))
        assert callable(handlers[2]) and callable(handlers[21])
        return Child()
    monkeypatch.setattr(launcher, "files", lambda _: tmp_path)
    monkeypatch.setattr(launcher, "os", SimpleNamespace(name="nt"))
    monkeypatch.setattr(launcher, "signal", SimpleNamespace(SIGINT=2, SIGBREAK=21, signal=install))
    monkeypatch.setattr(launcher, "subprocess", SimpleNamespace(Popen=spawn))
    assert getattr(launcher, entry)() & 0xffffffff == status
    assert handlers == original
    assert events[2][0] == "spawn" and events[3] == "wait"
    assert events[2][1][0] == str(tmp_path / "_bin" / f"{command}.exe")
