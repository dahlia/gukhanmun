# Gukhanmun: Tests the local-index installation used by the wheel smoke runs.
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
import os
from pathlib import Path
import sys
import zipfile

import pytest

SOURCE = Path(os.environ["GUKHANMUN_REPO_ROOT"]) / "scripts/python/wheel_smoke.py"
SPEC = importlib.util.spec_from_file_location("wheel_smoke_under_test", SOURCE)
smoke = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(smoke)

METADATA = """Metadata-Version: 2.4
Name: gukhanmun
Version: 0.4.0
Provides-Extra: stdict
Provides-Extra: loanwords-cdb
Requires-Dist: gukhanmun-stdict-fst==0.4.0; extra == "stdict"
Requires-Dist: gukhanmun-loanwords-cdb==0.4.0; extra == "loanwords-cdb"
"""


def wheel(directory: Path, name: str, distribution: str, version: str, metadata: str | None = None) -> Path:
    """Write a minimal installable pure wheel."""
    path = directory / name
    info = f"{distribution.replace('-', '_')}-{version}.dist-info"
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr(f"{info}/METADATA", metadata or
                         f"Metadata-Version: 2.4\nName: {distribution}\nVersion: {version}\n")
        archive.writestr(f"{info}/WHEEL", "Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n")
        archive.writestr(f"{info}/RECORD", "")
    return path


def test_project_names_are_normalized() -> None:
    assert smoke.artifact_project("gukhanmun_stdict_fst-0.4.0-py3-none-any.whl") == "gukhanmun-stdict-fst"
    assert smoke.artifact_project("gukhanmun-0.4.0.dev3.tar.gz") == "gukhanmun"
    assert smoke.artifact_project("Gukhanmun.Opendict_CDB-0.4.0.tar.gz") == "gukhanmun-opendict-cdb"
    with pytest.raises(ValueError):
        smoke.artifact_project("gukhanmun-0.4.0.zip")


def test_index_copies_artifacts_with_hashed_relative_links(tmp_path: Path) -> None:
    artifacts = tmp_path / "artifacts with spaces"
    artifacts.mkdir()
    first = wheel(artifacts, "gukhanmun_stdict_fst-0.4.0-py3-none-any.whl", "gukhanmun-stdict-fst", "0.4.0")
    (artifacts / "notes.txt").write_text("ignored")
    simple, staged = smoke.build_index(smoke.collect([artifacts]), tmp_path / "index")
    assert set(staged) == {first.name}
    entry = staged[first.name]
    assert entry["path"] == simple / "gukhanmun-stdict-fst" / first.name
    assert entry["sha256"] == hashlib.sha256(first.read_bytes()).hexdigest()
    page = (simple / "gukhanmun-stdict-fst" / "index.html").read_text()
    assert f'href="{first.name}#sha256={entry["sha256"]}"' in page
    assert 'href="gukhanmun-stdict-fst/"' in (simple / "index.html").read_text()


def test_two_different_artifacts_with_one_name_are_refused(tmp_path: Path) -> None:
    for directory, content in (("a", "one"), ("b", "two")):
        (tmp_path / directory).mkdir()
        (tmp_path / directory / "gukhanmun-0.4.0.tar.gz").write_text(content)
    with pytest.raises(RuntimeError, match="two different artifacts"):
        smoke.collect([tmp_path / "a", tmp_path / "b"])


def test_runtime_environment_keeps_only_the_venv_and_drops_overrides(tmp_path: Path, monkeypatch) -> None:
    venv = tmp_path / "venv"
    smoke.scripts_dir(venv).mkdir(parents=True)
    for key, value in {"PIP_INDEX_URL": "http://127.0.0.1:9/simple", "PIP_FIND_LINKS": "/elsewhere",
                       "CARGO_HOME": "/cargo", "RUSTUP_TOOLCHAIN": "stable", "LD_PRELOAD": "/x.so",
                       "LD_LIBRARY_PATH": "/lib", "DYLD_LIBRARY_PATH": "/lib", "PYTHONPATH": "/src",
                       "VIRTUAL_ENV": "/other", "PATH": os.environ.get("PATH", "")}.items():
        monkeypatch.setenv(key, value)
    environment = smoke.runtime_environment(venv)
    assert environment["PATH"] == str(smoke.scripts_dir(venv))
    assert environment["PIP_CONFIG_FILE"] == os.devnull
    assert not {"PIP_INDEX_URL", "PIP_FIND_LINKS", "CARGO_HOME", "RUSTUP_TOOLCHAIN", "LD_PRELOAD",
                "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH", "PYTHONPATH", "VIRTUAL_ENV"} & set(environment)


def test_a_reachable_cargo_fails_the_environment(tmp_path: Path) -> None:
    venv = tmp_path / "venv"
    scripts = smoke.scripts_dir(venv)
    scripts.mkdir(parents=True)
    cargo = scripts / ("cargo.exe" if os.name == "nt" else "cargo")
    cargo.write_text("#!/bin/sh\n")
    cargo.chmod(0o755)
    with pytest.raises(RuntimeError, match="cargo is reachable"):
        smoke.runtime_environment(venv)


def test_windows_creates_venvs_through_the_long_path_shim(tmp_path: Path) -> None:
    posix = smoke.venv_command("python", tmp_path, windows=False)
    assert posix == ["python", "-I", "-m", "venv", tmp_path]
    windows = smoke.venv_command("python", tmp_path, windows=True)
    assert windows[:3] == ["python", "-I", "-c"] and windows[-1] == tmp_path
    # The shim replaces only _winapi.GetLongPathName and then runs venv.
    compile(windows[3], "<venv shim>", "exec")
    assert "_winapi.GetLongPathName = long_path_name" in windows[3]
    assert "venv.main(sys.argv[1:])" in windows[3]


def report_item(path: Path, name: str, version: str, digest: str | None = None) -> dict:
    hashes = {"hashes": {"sha256": digest}} if digest else {}
    return {"metadata": {"name": name, "version": version},
            "download_info": {"url": path.as_uri(), "archive_info": hashes}}


@pytest.fixture
def staged(tmp_path: Path):
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    wheel(artifacts, "gukhanmun-0.4.0-py3-none-any.whl", "gukhanmun", "0.4.0", METADATA)
    wheel(artifacts, "gukhanmun_stdict_fst-0.4.0-py3-none-any.whl", "gukhanmun-stdict-fst", "0.4.0")
    (artifacts / "gukhanmun-0.4.0.tar.gz").write_bytes(b"sdist")
    return smoke.build_index(smoke.collect([artifacts]), tmp_path / "index")


def test_report_must_use_exactly_the_staged_wheels(staged, tmp_path: Path) -> None:
    simple, files = staged
    core = files["gukhanmun-0.4.0-py3-none-any.whl"]
    item = report_item(core["path"], "gukhanmun", "0.4.0", core["sha256"])
    installed = smoke.check_report({"install": [item]}, files, simple, {"gukhanmun": "0.4.0"})
    assert installed == {"gukhanmun": {"version": "0.4.0", "filename": core["path"].name,
                                       "sha256": core["sha256"]}}
    with pytest.raises(RuntimeError, match="expected to install"):
        smoke.check_report({"install": [item]}, files, simple,
                           {"gukhanmun": "0.4.0", "gukhanmun-stdict-fst": "0.4.0"})
    with pytest.raises(RuntimeError, match="different hash"):
        smoke.check_report({"install": [report_item(core["path"], "gukhanmun", "0.4.0", "0" * 64)]},
                           files, simple, {"gukhanmun": "0.4.0"})
    sdist = files["gukhanmun-0.4.0.tar.gz"]["path"]
    with pytest.raises(RuntimeError, match="source distribution"):
        smoke.check_report({"install": [report_item(sdist, "gukhanmun", "0.4.0")]},
                           files, simple, {"gukhanmun": "0.4.0"})
    outside = wheel(tmp_path, "gukhanmun-0.4.0-py3-none-any.whl", "gukhanmun", "0.4.0", METADATA)
    with pytest.raises(RuntimeError, match="outside the staged index"):
        smoke.check_report({"install": [report_item(outside, "gukhanmun", "0.4.0")]},
                           files, simple, {"gukhanmun": "0.4.0"})
    stray = simple / "gukhanmun" / "gukhanmun-0.3.0-py3-none-any.whl"
    stray.write_bytes(b"not staged")
    with pytest.raises(RuntimeError, match="not staged"):
        smoke.check_report({"install": [report_item(stray, "gukhanmun", "0.3.0")]},
                           files, simple, {"gukhanmun": "0.3.0"})
    with pytest.raises(RuntimeError, match="not a staged file"):
        smoke.check_report({"install": [{"metadata": {"name": "gukhanmun", "version": "0.4.0"},
                                         "download_info": {"url": "https://pypi.org/x.whl"}}]},
                           files, simple, {"gukhanmun": "0.4.0"})


def test_extras_are_read_from_the_core_wheel(staged) -> None:
    _, files = staged
    assert smoke.extras(files, "0.4.0") == {"stdict": ("gukhanmun-stdict-fst", "0.4.0"),
                                           "loanwords-cdb": ("gukhanmun-loanwords-cdb", "0.4.0")}
    with pytest.raises(RuntimeError, match="no gukhanmun 0.5.0 wheel"):
        smoke.extras(files, "0.5.0")


def test_pip_ignores_hostile_configuration(staged, tmp_path: Path, monkeypatch) -> None:
    """User pip configuration and PIP_* variables cannot redirect the install."""
    simple, files = staged
    home = tmp_path / "home"
    for directory in (home / ".config" / "pip", home / ".pip", home / "AppData" / "Roaming" / "pip"):
        directory.mkdir(parents=True)
        for name in ("pip.conf", "pip.ini"):
            (directory / name).write_text("[global]\nindex-url = http://127.0.0.1:9/simple\nno-binary = :all:\n")
    for key, value in {"HOME": str(home), "USERPROFILE": str(home), "APPDATA": str(home / "AppData" / "Roaming"),
                       "PIP_INDEX_URL": "http://127.0.0.1:9/simple", "PIP_NO_BINARY": ":all:"}.items():
        monkeypatch.setenv(key, value)
    venv = tmp_path / "venv"
    executable, version = smoke.create_venv(Path(sys.executable), venv, tmp_path)
    assert version
    report = smoke.pip_install(executable, venv, simple, "gukhanmun[stdict]==0.4.0",
                               tmp_path / "report.json", tmp_path, dry_run=True)
    installed = smoke.check_report(report, files, simple,
                                   {"gukhanmun": "0.4.0", "gukhanmun-stdict-fst": "0.4.0"})
    assert {data["filename"] for data in installed.values()} == {
        "gukhanmun-0.4.0-py3-none-any.whl", "gukhanmun_stdict_fst-0.4.0-py3-none-any.whl"}
