# Gukhanmun: Tests the official dictionary helpers with synthetic data packages.
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

from __future__ import annotations

from collections.abc import Callable, Iterator
import gzip
import importlib
import os
from pathlib import Path
import subprocess
import sys
import zipfile

import pytest

import gukhanmun as g
from gukhanmun import dictionaries as d
from gukhanmun._dictionary_data import DISTRIBUTIONS, Distribution, Resource

FIXTURES = Path(os.environ["GUKHANMUN_FIXTURE_DIR"])
PACKAGES = [distribution.package for distribution in DISTRIBUTIONS]

SOURCES: dict[str, tuple[Callable[..., g.FileDictionarySource], Callable[..., bytes]]] = {
    "stdict": (d.stdict, d.stdict_bytes),
    "opendict-general": (d.opendict_general, d.opendict_general_bytes),
    "opendict-north-korean": (d.opendict_north_korean, d.opendict_north_korean_bytes),
    "opendict-dialect": (d.opendict_dialect, d.opendict_dialect_bytes),
    "opendict-archaic": (d.opendict_archaic, d.opendict_archaic_bytes),
    "loanwords": (d.loanwords, d.loanwords_bytes),
}
CASES = [(distribution, resource) for distribution in DISTRIBUTIONS for resource in distribution.resources]


def case_id(case: tuple[Distribution, Resource]) -> str:
    return f"{case[1].key}-{case[0].format}"


def payload(distribution: Distribution) -> bytes:
    """A synthetic dictionary whose role matches the official one."""
    name = "loanword" if distribution.dictionary == "loanwords" else "reference"
    return (FIXTURES / f"{name}.{distribution.format}").read_bytes()


def encoded(resource: Resource, data: bytes) -> bytes:
    return gzip.compress(data, mtime=0) if resource.gzip else data


@pytest.fixture
def site(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Iterator[Path]:
    """An import path for fake resource packages, isolated per test."""
    root = tmp_path / "site"
    root.mkdir()
    monkeypatch.syspath_prepend(str(root))
    for name in PACKAGES:
        monkeypatch.delitem(sys.modules, name, raising=False)
    importlib.invalidate_caches()
    yield root
    for name in PACKAGES:
        sys.modules.pop(name, None)


def install(site: Path, distribution: Distribution,
            resources: dict[str, bytes] | None = None) -> Path:
    package = site / distribution.package
    package.mkdir()
    (package / "__init__.py").write_text("")
    data = payload(distribution)
    for resource in distribution.resources:
        content = resources[resource.name] if resources and resource.name in resources else encoded(resource, data)
        (package / resource.name).write_bytes(content)
    importlib.invalidate_caches()
    return package


def test_importing_the_helpers_imports_no_data_package() -> None:
    code = ("import sys, gukhanmun, gukhanmun.dictionaries; "
            f"print(sorted(set(sys.modules) & set({PACKAGES!r})))")
    result = subprocess.run([sys.executable, "-I", "-c", code], capture_output=True, text=True, check=True)
    assert result.stdout.strip() == "[]"


@pytest.mark.parametrize("case", CASES, ids=case_id)
def test_helpers_return_owned_bytes_with_the_official_role(site: Path, case: tuple[Distribution, Resource]) -> None:
    distribution, resource = case
    install(site, distribution)
    source_helper, bytes_helper = SOURCES[resource.key]
    format = g.DictionaryFormat(distribution.format)
    data = bytes_helper(format=format)
    assert type(data) is bytes
    assert data == payload(distribution)
    source = source_helper(format=format)
    assert source == g.FileDictionarySource(data, format, g.SourceRole(distribution.role))
    assert source.role is (g.SourceRole.LOANWORD if distribution.dictionary == "loanwords" else g.SourceRole.REFERENCE)
    # The bytes are ready for the loader and keep the fixture's readings.
    converter = g.Converter(dictionaries=[source])
    assert converter.convert("宙名") == ("누리" if distribution.dictionary == "loanwords" else "가온")


def test_default_format_is_fst(site: Path) -> None:
    for distribution in DISTRIBUTIONS:
        if distribution.format == "fst":
            install(site, distribution)
    for key, (source_helper, bytes_helper) in SOURCES.items():
        assert source_helper().format is g.DictionaryFormat.FST
        assert bytes_helper() == source_helper().data


@pytest.mark.parametrize("case", CASES, ids=case_id)
def test_missing_distribution_names_its_extra(site: Path, case: tuple[Distribution, Resource]) -> None:
    distribution, resource = case
    other = next(o for o in DISTRIBUTIONS if o.dictionary == distribution.dictionary and o is not distribution)
    install(site, other)  # the other format alone does not satisfy this one
    source_helper, bytes_helper = SOURCES[resource.key]
    for helper in (source_helper, bytes_helper):
        with pytest.raises(g.GukhanmunError) as caught:
            helper(format=g.DictionaryFormat(distribution.format))
        error = caught.value
        assert error.code is g.ErrorCode.DICTIONARY_LOAD
        assert "is not installed" in error.message
        assert f"pip install 'gukhanmun[{distribution.extra}]'" in error.message
        assert error.chain and all(cause.code is g.ErrorCode.IO for cause in error.chain)
        assert isinstance(error.__cause__, ModuleNotFoundError)
    # The installed format keeps working.
    assert SOURCES[resource.key][1](format=g.DictionaryFormat(other.format)) == payload(other)


def test_broken_packages_are_not_reported_as_missing(site: Path) -> None:
    stdict_fst = DISTRIBUTIONS[0]
    package = install(site, stdict_fst)
    (package / "__init__.py").write_text("import gukhanmun_missing_dependency\n")
    with pytest.raises(g.GukhanmunError) as caught:
        d.stdict()
    assert "could not be imported" in caught.value.message
    assert "pip install --force-reinstall 'gukhanmun[stdict]'" in caught.value.message
    assert isinstance(caught.value.__cause__, ModuleNotFoundError)
    for body in ["raise ImportError('broken')\n", "def broken(:\n", "raise RuntimeError('boom')\n"]:
        sys.modules.pop(stdict_fst.package, None)
        (package / "__init__.py").write_text(body)
        importlib.invalidate_caches()
        with pytest.raises(g.GukhanmunError) as caught:
            d.stdict()
        assert caught.value.code is g.ErrorCode.DICTIONARY_LOAD
        assert "could not be imported" in caught.value.message
        assert caught.value.__cause__ is not None


def test_interrupts_while_importing_propagate(site: Path) -> None:
    package = install(site, DISTRIBUTIONS[0])
    (package / "__init__.py").write_text("raise KeyboardInterrupt\n")
    importlib.invalidate_caches()
    with pytest.raises(KeyboardInterrupt):
        d.stdict()


def test_a_plain_module_shadowing_the_package_is_reported(site: Path) -> None:
    (site / "gukhanmun_stdict_fst.py").write_text("")
    importlib.invalidate_caches()
    with pytest.raises(g.GukhanmunError) as caught:
        d.stdict()
    assert caught.value.code is g.ErrorCode.DICTIONARY_LOAD
    assert "is a module, not the gukhanmun-stdict-fst package" in caught.value.message
    # A file beside the module must not be mistaken for the package resource.
    (site / "stdict.fst").write_bytes(b"unrelated")
    with pytest.raises(g.GukhanmunError):
        d.stdict_bytes()


def test_missing_resource_suggests_reinstalling(site: Path) -> None:
    package = install(site, DISTRIBUTIONS[0])
    (package / "stdict.fst").unlink()
    with pytest.raises(g.GukhanmunError) as caught:
        d.stdict_bytes()
    assert caught.value.code is g.ErrorCode.DICTIONARY_LOAD
    assert "stdict.fst resource could not be read" in caught.value.message
    assert "pip install --force-reinstall 'gukhanmun[stdict]'" in caught.value.message
    assert caught.value.chain[-1].code is g.ErrorCode.IO
    assert isinstance(caught.value.__cause__, OSError)


def corrupt_deflate(data: bytes) -> bytes:
    body = bytearray(gzip.compress(data, mtime=0))
    body[10] = 0xff  # the first deflate block header: an invalid block type
    return bytes(body)


@pytest.mark.parametrize("corrupt", [
    lambda data: b"\x00\x01 not gzip",
    lambda data: gzip.compress(data, mtime=0)[:-12],
    lambda data: gzip.compress(data, mtime=0)[:-8] + b"\0\0\0\0" + gzip.compress(data, mtime=0)[-4:],
    corrupt_deflate,
    lambda data: b"",
], ids=["not-gzip", "truncated", "crc", "deflate", "empty"])
def test_corrupt_gzip_resources_are_coded_load_errors(site: Path, corrupt: Callable[[bytes], bytes]) -> None:
    stdict_cdb = DISTRIBUTIONS[1]
    install(site, stdict_cdb, {"stdict.cdb.gz": corrupt(payload(stdict_cdb))})
    with pytest.raises(g.GukhanmunError) as caught:
        d.stdict(format=g.DictionaryFormat.CDB)
    error = caught.value
    assert error.code is g.ErrorCode.DICTIONARY_LOAD
    assert error.message.startswith("could not decompress stdict.cdb.gz from gukhanmun-stdict-cdb")
    assert "--force-reinstall 'gukhanmun[stdict-cdb]'" in error.message
    assert error.chain and error.chain[-1].code is g.ErrorCode.IO
    assert error.__cause__ is not None


def test_raw_resources_are_never_decompressed(site: Path) -> None:
    loanwords_cdb = DISTRIBUTIONS[-1]
    assert not loanwords_cdb.resources[0].gzip
    # A valid gzip stream in a raw resource is returned unchanged.
    raw = gzip.compress(b"not a dictionary", mtime=0)
    install(site, loanwords_cdb, {"loanwords.cdb": raw})
    assert d.loanwords_bytes(format=g.DictionaryFormat.CDB) == raw


def test_malformed_dictionary_bytes_fail_at_construction(site: Path) -> None:
    install(site, DISTRIBUTIONS[0], {"stdict.fst": b"not a dictionary"})
    source = d.stdict()
    with pytest.raises(g.GukhanmunError) as caught:
        g.Converter(dictionaries=[source])
    assert caught.value.code is g.ErrorCode.DICTIONARY_LOAD


def test_resources_load_from_a_zip_import(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    distribution = DISTRIBUTIONS[3]  # opendict-cdb: several gzip resources
    archive = tmp_path / "data.zip"
    with zipfile.ZipFile(archive, "w") as bundle:
        bundle.writestr(f"{distribution.package}/__init__.py", "")
        for resource in distribution.resources:
            bundle.writestr(f"{distribution.package}/{resource.name}", encoded(resource, payload(distribution)))
    monkeypatch.syspath_prepend(str(archive))
    monkeypatch.delitem(sys.modules, distribution.package, raising=False)
    importlib.invalidate_caches()
    try:
        assert d.opendict_dialect_bytes(format=g.DictionaryFormat.CDB) == payload(distribution)
    finally:
        sys.modules.pop(distribution.package, None)


def test_damaged_zip_members_are_coded_load_errors(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    distribution = DISTRIBUTIONS[0]
    archive = tmp_path / "data.zip"
    data = payload(distribution)
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
        bundle.writestr(f"{distribution.package}/__init__.py", "")
        bundle.writestr(f"{distribution.package}/stdict.fst", data)
    # Damage the compressed member so reading it fails inside zipfile.
    raw = bytearray(archive.read_bytes())
    start = raw.index(b"stdict.fst") + len(b"stdict.fst")
    raw[start + 2:start + 18] = b"\xff" * 16
    archive.write_bytes(bytes(raw))
    monkeypatch.syspath_prepend(str(archive))
    monkeypatch.delitem(sys.modules, distribution.package, raising=False)
    importlib.invalidate_caches()
    try:
        with pytest.raises(g.GukhanmunError) as caught:
            d.stdict_bytes()
        assert caught.value.code is g.ErrorCode.DICTIONARY_LOAD
        assert "stdict.fst resource could not be read" in caught.value.message
        assert caught.value.__cause__ is not None
    finally:
        sys.modules.pop(distribution.package, None)


@pytest.mark.parametrize("value", ["fst", "cdb", None, 1])
def test_format_must_be_a_dictionary_format(value: object) -> None:
    for source_helper, bytes_helper in SOURCES.values():
        for helper in (source_helper, bytes_helper):
            with pytest.raises(TypeError):
                helper(format=value)


def test_format_is_keyword_only() -> None:
    for source_helper, bytes_helper in SOURCES.values():
        for helper in (source_helper, bytes_helper):
            with pytest.raises(TypeError):
                helper(g.DictionaryFormat.FST)  # type: ignore[misc]


def test_public_names() -> None:
    assert sorted(d.__all__) == sorted(
        name for helpers in SOURCES.values() for name in (helpers[0].__name__, helpers[1].__name__))
