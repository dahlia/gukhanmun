# Gukhanmun: Tests the release artifact table, audits and completeness checks.
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

import hashlib
import importlib.util
import json
import os
from pathlib import Path
from types import SimpleNamespace
import zipfile

import pytest

REPOSITORY = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SPEC = importlib.util.spec_from_file_location("artifacts_under_test", REPOSITORY / "scripts/python/artifacts.py")
artifacts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(artifacts)

# Written out in full so that removing a table row cannot quietly shrink the
# release: eight CPython abi3 wheels, ten PyPy wheels, six data wheels and
# seven sdists.
RELEASE = {
    *(f"gukhanmun-0.4.0-cp311-abi3-{platform}.whl" for platform in (
        "manylinux_2_28_x86_64", "manylinux_2_28_aarch64", "musllinux_1_2_x86_64", "musllinux_1_2_aarch64",
        "macosx_11_0_x86_64", "macosx_11_0_arm64", "win_amd64", "win_arm64")),
    *(f"gukhanmun-0.4.0-{abi}-{platform}.whl" for abi in ("pp311-pypy311_pp80", "pp312-pypy312_pp80")
      for platform in ("manylinux_2_28_x86_64", "manylinux_2_28_aarch64", "macosx_11_0_x86_64",
                       "macosx_11_0_arm64", "win_amd64")),
    *(f"gukhanmun_{name}-0.4.0-py3-none-any.whl" for name in (
        "stdict_fst", "stdict_cdb", "opendict_fst", "opendict_cdb", "loanwords_fst", "loanwords_cdb")),
    "gukhanmun-0.4.0.tar.gz",
    *(f"gukhanmun_{name}-0.4.0.tar.gz" for name in (
        "stdict_fst", "stdict_cdb", "opendict_fst", "opendict_cdb", "loanwords_fst", "loanwords_cdb")),
}
AUDITWHEEL = """INFO:auditwheel.wheel_abi:setting libc to glibc

gukhanmun-0.4.0-cp311-abi3-manylinux_2_28_x86_64.whl is consistent with the
following platform tag: "manylinux_2_28_x86_64".

The wheel references external versioned symbols in these
system-provided shared libraries: libgcc_s.so.1 with versions
{'GCC_3.0', 'GCC_3.3'}, libc.so.6 with versions {'GLIBC_2.28', 'GLIBC_2.2.5'}

This constrains the platform tag to "manylinux_2_28_x86_64". In order
to achieve a more compatible tag, you would need to recompile a new
wheel from source on a system with earlier versions of these
libraries, such as a recent manylinux image.
"""
OTOOL_BUILD_VERSION = """_native.abi3.so:
Load command 0
      cmd LC_SEGMENT_64
  cmdsize 552
Load command 9
      cmd LC_BUILD_VERSION
  cmdsize 32
 platform 1
    minos 11.0
      sdk 15.2
   ntools 1
Load command 10
      cmd LC_SOURCE_VERSION
"""
OTOOL_VERSION_MIN = """_native.abi3.so:
Load command 8
      cmd LC_VERSION_MIN_MACOSX
  cmdsize 16
  version 10.12
      sdk 15.2
"""


def test_the_release_is_exactly_thirty_one_files() -> None:
    expected = artifacts.expected_files("0.4.0")
    assert set(expected) == RELEASE
    assert len(RELEASE) == 31
    kinds = [data["kind"] for data in expected.values()]
    assert kinds.count("sdist") == 7
    assert sum(1 for data in expected.values() if data.get("implementation") == "pypy") == 10


def test_the_matrix_is_generated_from_the_table() -> None:
    rows = artifacts.matrix()["include"]
    assert [row["target"] for row in rows] == [item.id for item in artifacts.TARGETS]
    assert {row["runner"] for row in rows} == {
        "ubuntu-latest", "ubuntu-24.04-arm", "macos-15-intel", "macos-latest", "windows-latest", "windows-11-arm"}


def test_linux_images_are_pinned_by_digest() -> None:
    for item in artifacts.TARGETS:
        if item.family in ("manylinux", "musllinux"):
            assert item.image.startswith(f"quay.io/pypa/{item.platform}@sha256:")
            assert len(item.image.split("@sha256:")[1]) == 64
        else:
            assert item.image is None


def test_each_target_runs_the_oldest_and_newest_cpython_and_its_pypys() -> None:
    labels = {item.id: [entry["label"] for entry in artifacts.test_interpreters(item)] for item in artifacts.TARGETS}
    assert labels["linux-x86_64"] == ["cp311", "cp315", "pp311", "pp312", "cp311-floor"]
    assert labels["musl-aarch64"] == ["cp311-image", "cp315-image"]
    assert labels["macos-arm64"] == ["cp311", "cp315", "pp311", "pp312"]
    assert labels["windows-arm64"] == ["cp311", "cp315"]
    floor = artifacts.test_interpreters(artifacts.target("linux-aarch64"))[-1]
    assert floor["image"] == "/opt/python/cp311-cp311/bin/python"


def test_build_interpreters_name_the_exact_pypy_installations() -> None:
    images = [entry["image"] for entry in artifacts.build_interpreters(artifacts.target("linux-x86_64"))]
    # The images also hold an older pp73 PyPy 3.11, which must never be picked.
    assert images == ["/opt/python/cp311-cp311/bin/python", "/opt/python/pp311-pypy311_pp80/bin/python",
                      "/opt/python/pp312-pypy312_pp80/bin/python"]
    assert len(artifacts.build_interpreters(artifacts.target("musl-x86_64"))) == 1


def found(**changes):
    data = {"implementation": "pypy", "python": "3.11.17", "soabi": "pypy311-pp80-x86_64-linux-gnu",
            "machine": "x86_64", "pointer_bits": 64, "free_threaded": False, "executable": "/opt/python/x"}
    data.update(changes)
    return data


def test_interpreters_must_match_the_table() -> None:
    item = artifacts.target("linux-x86_64")
    expected = {"implementation": "pypy", "python": "3.11", "abi": "pypy311_pp80"}
    assert artifacts.check_interpreter(found(), expected, item)
    for changes, message in (({"soabi": "pypy311-pp73-x86_64-linux-gnu"}, "SOABI"),
                             ({"python": "3.12.15"}, "Python"), ({"machine": "aarch64"}, "machine"),
                             ({"implementation": "cpython"}, "implementation"),
                             ({"free_threaded": True}, "free-threaded")):
        with pytest.raises(RuntimeError, match=message):
            artifacts.check_interpreter(found(**changes), expected, item)


def test_targets_refuse_to_run_on_another_host(monkeypatch) -> None:
    monkeypatch.setattr(artifacts, "host_family", lambda: "linux")
    monkeypatch.setattr(artifacts, "host_machine", lambda: "x86_64")
    artifacts.require_host(artifacts.target("musl-x86_64"))
    with pytest.raises(RuntimeError, match="emulated"):
        artifacts.require_host(artifacts.target("linux-aarch64"))
    with pytest.raises(RuntimeError, match="emulated"):
        artifacts.require_host(artifacts.target("windows-amd64"))


def test_auditwheel_policy_must_meet_the_floor() -> None:
    item = artifacts.target("linux-x86_64")
    assert artifacts.check_policy(artifacts.parse_auditwheel(AUDITWHEEL), item) == "manylinux_2_28_x86_64"
    assert artifacts.check_policy("manylinux_2_17_x86_64", item) == "manylinux_2_17_x86_64"
    for computed in ("manylinux_2_34_x86_64", "manylinux_2_28_aarch64", "musllinux_1_2_x86_64", "linux_x86_64"):
        with pytest.raises(RuntimeError, match="does not satisfy"):
            artifacts.check_policy(computed, item)
    with pytest.raises(RuntimeError, match="unrecognized"):
        artifacts.parse_auditwheel("something else entirely")


OTOOL_LIBRARIES = """/tmp/gukhanmun-audit-x/_native.abi3.so:
	@rpath/gukhanmun._native.abi3.so (compatibility version 0.0.0, current version 0.0.0)
	/usr/lib/libiconv.2.dylib (compatibility version 7.0.0, current version 7.0.0)
	/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1351.0.0)
"""
OTOOL_IDENTITY = """/tmp/gukhanmun-audit-x/_native.abi3.so:
@rpath/gukhanmun._native.abi3.so
"""


def test_otool_libraries_leave_out_the_install_name() -> None:
    assert artifacts.parse_otool_libraries(OTOOL_LIBRARIES, OTOOL_IDENTITY) == [
        "/usr/lib/libiconv.2.dylib", "/usr/lib/libSystem.B.dylib"]
    # Another @rpath library is still reported and rejected by the audit.
    listing = OTOOL_LIBRARIES + "	@rpath/libfoo.dylib (compatibility version 1.0.0, current version 1.0.0)\n"
    assert "@rpath/libfoo.dylib" in artifacts.parse_otool_libraries(listing, OTOOL_IDENTITY)


def test_macos_minimum_reads_either_load_command() -> None:
    assert artifacts.parse_macos_minimum(OTOOL_BUILD_VERSION) == (11, 0, 0)
    assert artifacts.parse_macos_minimum(OTOOL_VERSION_MIN) == (10, 12, 0)
    patched = OTOOL_BUILD_VERSION.replace("minos 11.0", "minos 11.0.1")
    assert artifacts.parse_macos_minimum(patched) == (11, 0, 1)
    assert artifacts.parse_macos_minimum(patched) > (11, 0, 0)
    with pytest.raises(RuntimeError, match="no macOS deployment"):
        artifacts.parse_macos_minimum("Load command 0\n      cmd LC_SEGMENT_64\n")


EXTRAS = {"stdict": "stdict-fst", "stdict-fst": "stdict-fst", "stdict-cdb": "stdict-cdb",
          "opendict": "opendict-fst", "opendict-fst": "opendict-fst", "opendict-cdb": "opendict-cdb",
          "loanwords": "loanwords-fst", "loanwords-fst": "loanwords-fst", "loanwords-cdb": "loanwords-cdb"}


def native_wheel(directory: Path, name: str, *, extension: str = "gukhanmun/_native.abi3.so",
                 tag: str | None = None, extra: dict[str, bytes] | None = None, pin: str = "==0.4.0") -> Path:
    path = directory / name
    info = "gukhanmun-0.4.0.dist-info"
    tag = tag or "-".join(name.removesuffix(".whl").split("-")[2:])
    metadata = ["Metadata-Version: 2.4", "Name: gukhanmun", "Version: 0.4.0"]
    metadata += [f"Provides-Extra: {extra_name}" for extra_name in EXTRAS]
    metadata += [f'Requires-Dist: gukhanmun-{data}{pin} ; extra == "{extra_name}"'
                 for extra_name, data in EXTRAS.items()]
    files = {f"{info}/WHEEL": f"Wheel-Version: 1.0\nRoot-Is-Purelib: false\nTag: {tag}\n",
             f"{info}/METADATA": "\n".join(metadata) + "\n", f"{info}/RECORD": "",
             f"{info}/licenses/LICENSE": "GPL", "gukhanmun/__init__.py": "", "gukhanmun/py.typed": "",
             "gukhanmun/_native.pyi": "", extension: "binary", **(extra or {})}
    with zipfile.ZipFile(path, "w") as archive:
        for member, data in files.items():
            archive.writestr(member, data)
    return path


def test_wheel_structure_matches_the_target(tmp_path: Path) -> None:
    item = artifacts.target("linux-x86_64")
    name = "gukhanmun-0.4.0-cp311-abi3-manylinux_2_28_x86_64.whl"
    member, _ = artifacts.check_structure(native_wheel(tmp_path, name), item, "0.4.0", REPOSITORY)
    assert member == "gukhanmun/_native.abi3.so"
    pypy = "gukhanmun-0.4.0-pp311-pypy311_pp80-manylinux_2_28_x86_64.whl"
    artifacts.check_structure(native_wheel(tmp_path, pypy, extension="gukhanmun/_native.pypy311-pp80-x86_64-linux-gnu.so"),
                              item, "0.4.0", REPOSITORY)
    windows = "gukhanmun-0.4.0-cp311-abi3-win_amd64.whl"
    artifacts.check_structure(native_wheel(tmp_path, windows, extension="gukhanmun/_native.pyd"),
                              artifacts.target("windows-amd64"), "0.4.0", REPOSITORY)


@pytest.mark.parametrize("change,message", [
    ({"tag": "cp311-abi3-linux_x86_64"}, "WHEEL metadata"),
    ({"extension": "gukhanmun/_native.cpython-311-x86_64-linux-gnu.so"}, "does not match"),
    ({"extra": {"gukhanmun/__pycache__/x.cpython-311.pyc": b""}}, "bytecode"),
    ({"extra": {"gukhanmun/_native.abi3.so.bak": b""}}, "extensions"),
    ({"pin": ">=0.4.0"}, "unexpected requirement|extras"),
], ids=["tag", "extension", "bytecode", "two-extensions", "pin"])
def test_wheel_structure_rejects_mismatches(tmp_path: Path, change: dict, message: str) -> None:
    name = "gukhanmun-0.4.0-cp311-abi3-manylinux_2_28_x86_64.whl"
    with pytest.raises(RuntimeError, match=message):
        artifacts.check_structure(native_wheel(tmp_path, name, **change), artifacts.target("linux-x86_64"),
                                  "0.4.0", REPOSITORY)


def test_a_wheel_of_another_target_is_rejected(tmp_path: Path) -> None:
    name = "gukhanmun-0.4.0-cp311-abi3-manylinux_2_28_x86_64.whl"
    with pytest.raises(RuntimeError, match="is not a musl-x86_64 wheel"):
        artifacts.check_structure(native_wheel(tmp_path, name), artifacts.target("musl-x86_64"), "0.4.0",
                                  REPOSITORY)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_record(name: str, file_digest: str, label: str, interpreter: dict, windows: bool = False) -> dict:
    record = {"label": label}
    for mode, final in (("base", "missing-data"), ("official", "official-corpus")):
        record[mode] = {
            "installed": {"gukhanmun": {"filename": name, "sha256": file_digest}},
            "result": {"mode": mode, "checks": ["formats", final, *(["windows-import-closure"] if windows else [])],
                       "interpreter": {"implementation": interpreter["implementation"],
                                       "python": f"{interpreter['python']}.1",
                                       "libc": interpreter.get("libc", ["glibc", "2.43"])}}}
    return record


@pytest.fixture
def release(tmp_path: Path) -> Path:
    """A synthetic complete release with every record the verifier wants."""
    directory = tmp_path / "release"
    directory.mkdir()
    for name in RELEASE:
        (directory / name).write_bytes(name.encode())
    expected = artifacts.expected_files("0.4.0")
    data_installed = {}
    for name, data in expected.items():
        if data["kind"] == "wheel" and data.get("target") is None:
            data_installed[data["distribution"]] = {"sha256": digest(directory / name)}
    for item in artifacts.TARGETS:
        wheels = {name: data for name, data in expected.items() if data.get("target") == item.id}
        audit = {"records": [{"wheel": name, "sha256": digest(directory / name)} for name in wheels]}
        (directory / f"audit-{item.id}.json").write_text(json.dumps(audit))
        for interpreter in artifacts.test_interpreters(item):
            name = next(name for name, data in wheels.items() if data["implementation"] == interpreter["implementation"]
                        and (data["implementation"] == "cpython" or data["python"] == interpreter["python"]))
            record = run_record(name, digest(directory / name), f"{item.id}-{interpreter['label']}",
                                interpreter, item.family == "windows")
            record["official"]["installed"].update(data_installed)
            (directory / f"smoke-{item.id}-{interpreter['label']}.json").write_text(json.dumps(record))
    smoke = {mode: {"installed": {}, "result": {"mode": mode, "checks": [final]}}
             for mode, final in (("base", "missing-data"), ("official", "official-corpus"))}
    rebuilds = []
    for name, data in expected.items():
        if data["kind"] == "sdist":
            wheel = f"rebuilt-{name}.whl"
            installed = {"filename": wheel, "sha256": "f" * 64}
            smoke["official"]["installed"][data["distribution"]] = installed
            if data["distribution"] == "gukhanmun":
                smoke["base"]["installed"]["gukhanmun"] = installed
            rebuilds.append({"sdist": name, "sdist_sha256": digest(directory / name), "wheel": wheel,
                             "wheel_sha256": "f" * 64})
    (directory / "smoke-sdist-rebuild.json").write_text(json.dumps(smoke))
    evidence_digest = digest(directory / "smoke-sdist-rebuild.json")
    (directory / "rebuilds.json").write_text(json.dumps({"evidence_sha256": evidence_digest, "rebuilds": rebuilds}))
    artifacts.manifest(SimpleNamespace(dir=directory, version="0.4.0"))
    return directory


def test_a_complete_release_verifies(release: Path) -> None:
    artifacts.verify(SimpleNamespace(dir=release, complete=True))
    manifest = json.loads((release / "manifest.json").read_text())
    assert len(manifest["artifacts"]) == 31
    sums = (release / "SHA256SUMS").read_text().splitlines()
    assert len(sums) == 31 and all(len(line.split("  ")[0]) == 64 for line in sums)


def verify_fails(directory: Path, message: str, complete: bool = True) -> None:
    with pytest.raises(SystemExit, match=message):
        artifacts.verify(SimpleNamespace(dir=directory, complete=complete))


def test_a_directory_in_place_of_an_artifact_fails(release: Path) -> None:
    path = release / "gukhanmun-0.4.0-cp311-abi3-win_amd64.whl"
    path.unlink()
    path.mkdir()
    verify_fails(release, "is not a regular file", complete=False)


def test_a_changed_artifact_fails_even_partial_verification(release: Path) -> None:
    (release / "gukhanmun-0.4.0.tar.gz").write_bytes(b"different")
    verify_fails(release, "does not match its size or hash", complete=False)


def test_a_missing_wheel_fails_completeness(release: Path) -> None:
    (release / "gukhanmun-0.4.0-cp311-abi3-win_arm64.whl").unlink()
    artifacts.manifest(SimpleNamespace(dir=release, version="0.4.0"))
    artifacts.verify(SimpleNamespace(dir=release, complete=False))
    verify_fails(release, "missing .*win_arm64")


def test_missing_execution_evidence_fails_completeness(release: Path) -> None:
    (release / "smoke-macos-arm64-pp312.json").unlink()
    artifacts.manifest(SimpleNamespace(dir=release, version="0.4.0"))
    verify_fails(release, "no base and official execution of .*pp312.*macosx_11_0_arm64.whl with pp312")


def test_a_windows_run_without_the_import_check_fails(release: Path) -> None:
    path = release / "smoke-windows-amd64-cp311.json"
    record = json.loads(path.read_text())
    record["official"]["result"]["checks"].remove("windows-import-closure")
    path.write_text(json.dumps(record))
    artifacts.manifest(SimpleNamespace(dir=release, version="0.4.0"))
    verify_fails(release, "win_amd64.whl with cp311")


def test_evidence_for_another_file_does_not_count(release: Path) -> None:
    path = release / "audit-linux-x86_64.json"
    record = json.loads(path.read_text())
    record["records"][0]["sha256"] = "0" * 64
    path.write_text(json.dumps(record))
    artifacts.manifest(SimpleNamespace(dir=release, version="0.4.0"))
    verify_fails(release, "no audit of")


def test_an_sdist_rebuilt_from_another_archive_fails(release: Path) -> None:
    path = release / "rebuilds.json"
    record = json.loads(path.read_text())
    record["rebuilds"][0]["sdist_sha256"] = "0" * 64
    path.write_text(json.dumps(record))
    artifacts.manifest(SimpleNamespace(dir=release, version="0.4.0"))
    verify_fails(release, "was not rebuilt from the released file")


def test_changed_evidence_after_the_manifest_fails(release: Path) -> None:
    (release / "audit-musl-x86_64.json").write_text("{}")
    verify_fails(release, "evidence audit-musl-x86_64.json is missing or changed", complete=False)
    verify_fails(release, "evidence audit-musl-x86_64.json is missing or changed", complete=True)


def rewrite(release: Path, name: str, change) -> None:
    path = release / name
    record = json.loads(path.read_text())
    change(record)
    path.write_text(json.dumps(record))
    artifacts.manifest(SimpleNamespace(dir=release, version="0.4.0"))


def test_the_floor_run_must_execute_on_the_glibc_floor(release: Path) -> None:
    def newer(record: dict) -> None:
        for mode in ("base", "official"):
            record[mode]["result"]["interpreter"]["libc"] = ["glibc", "2.43"]
    rewrite(release, "smoke-linux-aarch64-cp311-floor.json", newer)
    verify_fails(release, "manylinux_2_28_aarch64.whl with cp311-floor")


def test_musl_runs_must_execute_on_musl(release: Path) -> None:
    def glibc(record: dict) -> None:
        record["base"]["result"]["interpreter"]["libc"] = ["glibc", "2.28"]
    rewrite(release, "smoke-musl-x86_64-cp315-image.json", glibc)
    verify_fails(release, "musllinux_1_2_x86_64.whl with cp315-image")


def test_an_official_run_must_execute_the_corpus(release: Path) -> None:
    def skipped(record: dict) -> None:
        record["official"]["result"]["checks"].remove("official-corpus")
    rewrite(release, "smoke-macos-x86_64-cp311.json", skipped)
    verify_fails(release, "macosx_11_0_x86_64.whl with cp311")


def test_data_wheels_need_an_official_execution(release: Path) -> None:
    for path in release.glob("smoke-*.json"):
        if path.name == "smoke-sdist-rebuild.json":
            continue
        record = json.loads(path.read_text())
        record["official"]["installed"].pop("gukhanmun-loanwords-cdb", None)
        path.write_text(json.dumps(record))
    artifacts.manifest(SimpleNamespace(dir=release, version="0.4.0"))
    verify_fails(release, "no official execution of gukhanmun_loanwords_cdb-0.4.0-py3-none-any.whl")


def test_a_staged_but_uninstalled_rebuild_fails(release: Path) -> None:
    def uninstalled(record: dict) -> None:
        record["official"]["installed"].pop("gukhanmun-stdict-fst")
    rewrite(release, "smoke-sdist-rebuild.json", uninstalled)
    # The rebuild records name the smoke run by hash; refresh that link only.
    rewrite(release, "rebuilds.json", lambda record: record.update(
        evidence_sha256=digest(release / "smoke-sdist-rebuild.json")))
    verify_fails(release, "the wheel rebuilt from gukhanmun_stdict_fst-0.4.0.tar.gz was not executed")


@pytest.mark.parametrize("change,message", [
    (lambda path: path.unlink(), "SHA256SUMS is missing"),
    (lambda path: path.write_text("not a checksum line\n"), "malformed"),
    (lambda path: path.write_text("\n".join(path.read_text().splitlines()[1:]) + "\n"), "does not match"),
], ids=["missing", "malformed", "incomplete"])
def test_sha256sums_must_match_the_manifest(release: Path, change, message: str) -> None:
    change(release / "SHA256SUMS")
    verify_fails(release, message, complete=False)


@pytest.mark.parametrize("entry,valid", [
    ("$ORIGIN", True), ("$ORIGIN/../gukhanmun.libs", True), ("$ORIGIN/sub:$ORIGIN", True),
    ("$ORIGIN/../..", False), ("$ORIGIN/../../../../opt/vendor/lib", False), ("$ORIGIN_EVIL", False),
    ("/usr/local/lib", False), ("$ORIGIN:/home/runner/lib", False),
])
def test_search_paths_must_stay_inside_the_wheel(entry: str, valid: bool) -> None:
    if valid:
        artifacts.check_search_path(entry, "gukhanmun/_native.abi3.so")
    else:
        with pytest.raises(RuntimeError, match="outside the wheel"):
            artifacts.check_search_path(entry, "gukhanmun/_native.abi3.so")


@pytest.mark.parametrize("cargo,python", [("0.4.0", "0.4.0"), ("1.2.0-rc.2", "1.2.0rc2")])
def test_ci_builds_dev0_on_branches_and_the_release_on_tags(tmp_path: Path, cargo: str, python: str) -> None:
    (tmp_path / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{cargo}"\n')
    assert artifacts.ci_version("tag", tmp_path) == python
    assert artifacts.ci_version("branch", tmp_path) == f"{python}.dev0"
    assert artifacts.ci_version(None, tmp_path) == f"{python}.dev0"


def test_ci_context_writes_the_matrix_and_version(tmp_path: Path, monkeypatch) -> None:
    output = tmp_path / "output"
    monkeypatch.setenv("GITHUB_OUTPUT", str(output))
    monkeypatch.setenv("GITHUB_REF_TYPE", "branch")
    artifacts.ci_context(SimpleNamespace())
    values = dict(line.split("=", 1) for line in output.read_text().splitlines())
    assert json.loads(values["matrix"]) == artifacts.matrix()
    assert values["pypi-version"].endswith(".dev0")


def test_each_job_installs_exactly_the_interpreters_it_runs() -> None:
    rows = {row["target"]: row["pythons"].split() for row in artifacts.matrix()["include"]}
    assert rows["musl-x86_64"] == ["python@3.15.2"]
    assert rows["windows-arm64"] == ["python@3.11.18", "python@3.15.2"]
    assert rows["linux-x86_64"] == ["python@3.11.18", "python@3.15.2", "python@pypy3.11-8.0.1",
                                    "python@pypy3.12-8.0.1"]


def test_the_workflow_uses_the_generated_matrix_and_version() -> None:
    workflow = (REPOSITORY / ".github/workflows/main.yaml").read_text()
    assert "scripts/python/artifacts.py ci-context" in workflow
    assert workflow.count("fromJSON(needs.python-context.outputs.matrix)") == 2
    for job in ("python-sdist:", "python-wheels:", "python-wheel-test:", "python-dists:"):
        section = workflow.split(f"  {job}\n", 1)[1].split("\n  python-", 1)[0]
        assert "PYPI_VERSION: ${{ needs.python-context.outputs.pypi-version }}" in section, job
        assert "artifacts.py stamp" in section, job
    assert "verify --dir python-dists" in workflow and "--complete" in workflow
