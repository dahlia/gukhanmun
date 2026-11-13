# Gukhanmun: Tests the native sdist checks and the standalone rebuild records.
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
import io
import os
from pathlib import Path
import tarfile
import tomllib

import pytest

REPOSITORY = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SPEC = importlib.util.spec_from_file_location("dists_under_test", REPOSITORY / "scripts/python/dists.py")
dists = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dists)

EXTRAS = {"stdict": "stdict-fst", "stdict-fst": "stdict-fst", "stdict-cdb": "stdict-cdb",
          "opendict": "opendict-fst", "opendict-fst": "opendict-fst", "opendict-cdb": "opendict-cdb",
          "loanwords": "loanwords-fst", "loanwords-fst": "loanwords-fst", "loanwords-cdb": "loanwords-cdb"}
README = "# Gukhanmun\n\nConvert hanja into hangul.\n"


def pkg_info(version: str = "0.4.0") -> str:
    lines = ["Metadata-Version: 2.4", "Name: gukhanmun", f"Version: {version}",
             "License-Expression: GPL-3.0-only", "Description-Content-Type: text/markdown"]
    lines += [f"Provides-Extra: {extra}" for extra in EXTRAS]
    lines += [f'Requires-Dist: gukhanmun-{data}=={version} ; extra == "{extra}"' for extra, data in EXTRAS.items()]
    return "\n".join(lines) + "\n\n" + README


def members(version: str = "0.4.0") -> dict[str, bytes]:
    files = {
        "pyproject.toml": b"[project]\n",
        "PKG-INFO": pkg_info(version).encode(),
        "README.md": README.encode(),
        "LICENSE": b"GPL",
        "Cargo.toml": b'[workspace]\nmembers = ["crates/gukhanmun", "crates/gukhanmun-python"]\n',
        "Cargo.lock": b"version = 4\n",
        "crates/gukhanmun/Cargo.toml": b"[package]\n",
        "crates/gukhanmun-python/Cargo.toml": b"[package]\n",
        "crates/gukhanmun-python/build.rs": b"fn main() {}\n",
        "crates/gukhanmun-python/src/lib.rs": b"\n",
        "crates/gukhanmun-opendict/LICENSE": b"CC BY-SA",
        "crates/gukhanmun-opendict/ATTRIBUTION.md": b"attribution",
        "crates/gukhanmun-loanwords/LICENSE": b"KOGL",
        "crates/gukhanmun-loanwords/ATTRIBUTION.md": b"attribution",
    }
    files.update({f"src/gukhanmun/{name}": b"" for name in dists.PYTHON_SOURCES})
    return files


def archive(path: Path, files: dict[str, bytes], version: str = "0.4.0", extra=None) -> Path:
    with tarfile.open(path, "w:gz") as tar:
        for name, data in files.items():
            info = tarfile.TarInfo(f"gukhanmun-{version}/{name}")
            info.size = len(data)
            tar.addfile(info, io.BytesIO(data))
        if extra is not None:
            extra(tar)
    return path


def test_a_complete_archive_passes(tmp_path: Path) -> None:
    names = dists.check_sdist(archive(tmp_path / "gukhanmun-0.4.0.tar.gz", members()), "0.4.0")
    assert "crates/gukhanmun-python/src/lib.rs" in names


@pytest.mark.parametrize("change,message", [
    (lambda files: files.update({"crates/gukhanmun-stdict/data/stdict.tsv": b"x"}), "unused dictionary data"),
    (lambda files: files.update({"crates/gukhanmun-opendict/data/general.tsv": b"x"}), "unused dictionary data"),
    (lambda files: files.update({"src/gukhanmun/__pycache__/x.cpython-315.pyc": b"x"}), "contains"),
    (lambda files: files.update({".git/HEAD": b"x"}), "contains"),
    (lambda files: files.pop("Cargo.lock"), "lacks"),
    (lambda files: files.pop("README.md"), "lacks"),
    (lambda files: files.update({"PKG-INFO": pkg_info().split("\n\n", 1)[0].encode()}), "README"),
    (lambda files: files.update({"PKG-INFO": pkg_info().replace("text/markdown", "text/plain").encode()}), "README"),
    (lambda files: files.update({"README.md": b"Another README"}), "README"),
    (lambda files: files.pop("src/gukhanmun/_native.pyi"), "lacks"),
    (lambda files: files.pop("crates/gukhanmun-opendict/ATTRIBUTION.md"), "lacks"),
    (lambda files: files.pop("crates/gukhanmun/Cargo.toml"), "workspace members"),
    (lambda files: files.update({"PKG-INFO": pkg_info().replace("GPL-3.0-only", "MIT").encode()}), "PKG-INFO"),
    (lambda files: files.update({"PKG-INFO": pkg_info().replace("==0.4.0 ; extra == \"stdict\"",
                                                                ">=0.4.0 ; extra == \"stdict\"").encode()}),
     "extras"),
], ids=["stdict-data", "opendict-data", "bytecode", "git", "lock", "readme-file", "readme-empty",
        "readme-type", "readme-mismatch", "stub", "attribution", "member",
        "license", "pin"])
def test_incomplete_or_unclean_archives_are_rejected(tmp_path: Path, change, message: str) -> None:
    files = members()
    change(files)
    with pytest.raises(RuntimeError, match=message):
        dists.check_sdist(archive(tmp_path / "gukhanmun-0.4.0.tar.gz", files), "0.4.0")


def test_links_and_escaping_members_are_rejected(tmp_path: Path) -> None:
    def link(tar: tarfile.TarFile) -> None:
        info = tarfile.TarInfo("gukhanmun-0.4.0/src/gukhanmun/link.py")
        info.type = tarfile.SYMTYPE
        info.linkname = "/etc/passwd"
        tar.addfile(info)

    def escape(tar: tarfile.TarFile) -> None:
        info = tarfile.TarInfo("gukhanmun-0.4.0/../evil")
        tar.addfile(info, io.BytesIO(b""))

    for name, extra in (("link", link), ("escape", escape)):
        (tmp_path / name).mkdir()
        path = archive(tmp_path / name / "gukhanmun-0.4.0.tar.gz", members(), extra=extra)
        with pytest.raises(RuntimeError, match="link or special|unsafe"):
            dists.check_sdist(path, "0.4.0")


def test_the_archive_name_and_version_must_match(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="unexpected sdist name"):
        dists.check_sdist(archive(tmp_path / "gukhanmun-0.4.1.tar.gz", members()), "0.4.0")
    path = archive(tmp_path / "gukhanmun-0.4.0.tar.gz", members("0.4.1"), version="0.4.0")
    with pytest.raises(RuntimeError, match="PKG-INFO"):
        dists.check_sdist(path, "0.4.0")


def test_every_source_of_a_packaged_crate_is_required(tmp_path: Path) -> None:
    for name in ("crates/gukhanmun-python/src/lib.rs", "crates/gukhanmun-python/src/html.rs",
                 "crates/gukhanmun-python/build.rs", "crates/gukhanmun/src/lib.rs",
                 "crates/gukhanmun-stdict/src/lib.rs", "crates/gukhanmun-stdict/data/stdict.tsv"):
        (tmp_path / name).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / name).write_text("")
    names = ["crates/gukhanmun-python/Cargo.toml", "crates/gukhanmun-python/src/lib.rs",
             "crates/gukhanmun-python/src/html.rs", "crates/gukhanmun-python/build.rs",
             "crates/gukhanmun/Cargo.toml", "crates/gukhanmun/src/lib.rs",
             "crates/gukhanmun-stdict/Cargo.toml", "crates/gukhanmun-stdict/src/lib.rs"]
    # Data outside src/ is not required; the excludes drop it on purpose.
    assert dists.check_sources(names, tmp_path) == [
        "crates/gukhanmun", "crates/gukhanmun-python", "crates/gukhanmun-stdict"]
    with pytest.raises(RuntimeError, match="html.rs"):
        dists.check_sources([name for name in names if not name.endswith("html.rs")], tmp_path)
    with pytest.raises(RuntimeError, match="build.rs"):
        dists.check_sources([name for name in names if not name.endswith("build.rs")], tmp_path)


def lock(path: Path, packages: list[dict[str, object]]) -> Path:
    blocks = []
    for package in packages:
        lines = []
        for key, value in package.items():
            if isinstance(value, list):
                lines.append(f"{key} = [" + ", ".join(f'"{item}"' for item in value) + "]\n")
            else:
                lines.append(f'{key} = "{value}"\n')
        blocks.append("[[package]]\n" + "".join(lines))
    path.write_text("version = 4\n\n" + "\n".join(blocks), encoding="utf-8")
    return path


REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


def test_rebuilt_lock_may_only_prune_the_shipped_lock(tmp_path: Path) -> None:
    shipped = lock(tmp_path / "shipped.lock", [
        {"name": "gukhanmun-python", "version": "0.4.0"},
        {"name": "gukhanmun-cli", "version": "0.4.0"},
        {"name": "serde", "version": "1.0.228", "source": REGISTRY, "checksum": "aa"},
        {"name": "clap", "version": "4.6.0", "source": REGISTRY, "checksum": "bb"},
    ])
    pruned = lock(tmp_path / "pruned.lock", [
        {"name": "gukhanmun-python", "version": "0.4.0"},
        {"name": "serde", "version": "1.0.228", "source": REGISTRY, "checksum": "aa"},
    ])
    assert dists.check_lock_subset(shipped, pruned) == {"shipped_packages": 4, "built_packages": 2}
    added = lock(tmp_path / "added.lock", [
        {"name": "gukhanmun-python", "version": "0.4.0"},
        {"name": "serde", "version": "1.0.229", "source": REGISTRY, "checksum": "cc"},
    ])
    with pytest.raises(RuntimeError, match="added"):
        dists.check_lock_subset(shipped, added)
    changed = lock(tmp_path / "changed.lock", [
        {"name": "gukhanmun-python", "version": "0.4.0"},
        {"name": "serde", "version": "1.0.228", "source": REGISTRY, "checksum": "dd"},
    ])
    with pytest.raises(RuntimeError, match="changed"):
        dists.check_lock_subset(shipped, changed)
    empty = lock(tmp_path / "empty.lock", [])
    with pytest.raises(RuntimeError, match="gukhanmun-python"):
        dists.check_lock_subset(shipped, empty)


def test_rebuilt_lock_keeps_every_dependency_edge(tmp_path: Path) -> None:
    helper = [{"name": "helper", "version": version, "source": REGISTRY, "checksum": version}
              for version in ("1.0.0", "2.0.0")]
    shipped = lock(tmp_path / "shipped.lock", [
        {"name": "gukhanmun-python", "version": "0.4.0", "dependencies": ["helper 1.0.0"]},
        {"name": "gukhanmun-cli", "version": "0.4.0", "dependencies": ["helper 2.0.0"]},
        *helper,
    ])
    # Pruning the CLI leaves one helper, so Cargo shortens the notation.
    pruned = lock(tmp_path / "pruned.lock", [
        {"name": "gukhanmun-python", "version": "0.4.0", "dependencies": ["helper"]}, helper[0]])
    assert dists.check_lock_subset(shipped, pruned)["built_packages"] == 2
    # A feature only the pruned members enabled can drop an edge.
    dropped = lock(tmp_path / "dropped.lock", [
        {"name": "gukhanmun-python", "version": "0.4.0"}, helper[0]])
    assert dists.check_lock_subset(shipped, dropped)["built_packages"] == 2
    switched = lock(tmp_path / "switched.lock", [
        {"name": "gukhanmun-python", "version": "0.4.0", "dependencies": ["helper 2.0.0"]}, *helper])
    with pytest.raises(RuntimeError, match="changed"):
        dists.check_lock_subset(shipped, switched)
    added = lock(tmp_path / "edge.lock", [
        {"name": "gukhanmun-python", "version": "0.4.0", "dependencies": ["helper 1.0.0", "helper 2.0.0"]},
        *helper])
    with pytest.raises(RuntimeError, match="changed"):
        dists.check_lock_subset(shipped, added)


def test_the_rebuild_directory_must_be_outside_the_checkout(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="outside the checkout"):
        dists.outside(REPOSITORY / "target")
    assert dists.outside(tmp_path, root=REPOSITORY) == Path(os.path.realpath(tmp_path))


def test_native_sdist_excludes_only_the_unused_dictionary_data() -> None:
    manifest = tomllib.loads((REPOSITORY / "python/gukhanmun/pyproject.toml").read_text(encoding="utf-8"))
    excluded = {entry["path"] for entry in manifest["tool"]["maturin"]["exclude"]
                if isinstance(entry, dict) and entry.get("format") == "sdist"}
    assert excluded == {f"**/{crate}/data/**" for crate in dists.DICTIONARY_CRATES}


def test_build_group_pins_the_native_build_backend() -> None:
    root = tomllib.loads((REPOSITORY / "pyproject.toml").read_text(encoding="utf-8"))
    native = tomllib.loads((REPOSITORY / "python/gukhanmun/pyproject.toml").read_text(encoding="utf-8"))
    assert native["build-system"]["requires"] == ["maturin==1.16.0"]
    assert set(native["build-system"]["requires"]) <= set(root["dependency-groups"]["build"])


def test_the_python_binding_crate_stays_private() -> None:
    crate = tomllib.loads((REPOSITORY / "crates/gukhanmun-python/Cargo.toml").read_text(encoding="utf-8"))
    assert crate["package"]["publish"] is False


def test_data_records_must_match_the_sdist_and_the_rebuilt_wheel(tmp_path: Path) -> None:
    import hashlib

    sdist = tmp_path / "gukhanmun-stdict-fst" / "gukhanmun_stdict_fst-0.4.0.tar.gz"
    wheel = tmp_path / "rebuilt" / "gukhanmun-stdict-fst" / "gukhanmun_stdict_fst-0.4.0-py3-none-any.whl"
    for path in (sdist, wheel):
        path.parent.mkdir(parents=True)
        path.write_bytes(path.name.encode())
    record = {"distribution": "gukhanmun-stdict-fst", "version": "0.4.0", "sdist": sdist.name,
              "sdist_sha256": hashlib.sha256(sdist.read_bytes()).hexdigest(), "wheel": wheel.name,
              "wheel_sha256": hashlib.sha256(wheel.read_bytes()).hexdigest()}
    dists.check_data_record(record, "0.4.0", tmp_path)
    with pytest.raises(RuntimeError, match="is not version 0.4.1"):
        dists.check_data_record(record, "0.4.1", tmp_path)
    for path in (sdist, wheel):
        original = path.read_bytes()
        path.write_bytes(b"replaced")
        with pytest.raises(RuntimeError, match="missing or changed"):
            dists.check_data_record(record, "0.4.0", tmp_path)
        path.unlink()
        with pytest.raises(RuntimeError, match="missing or changed"):
            dists.check_data_record(record, "0.4.0", tmp_path)
        path.write_bytes(original)
