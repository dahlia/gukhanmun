# Gukhanmun: Tests the Python dictionary helpers against the official data.
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

"""Run by `mise run py-dicts-test` in an environment that installed the
official data through the core extras, outside the checkout."""

from __future__ import annotations

from collections.abc import Callable
import gzip
import hashlib
import importlib
import importlib.metadata
import importlib.resources
import json
import os
from pathlib import Path
import sys

import pytest

import gukhanmun as g
from gukhanmun import dictionaries as d
from gukhanmun._dictionary_data import DISTRIBUTIONS, Distribution, Resource

REPOSITORY = Path(os.environ["GUKHANMUN_REPO_ROOT"])
CORPUS = json.loads((REPOSITORY / "tests/fixtures/dictionaries/official.json").read_text(encoding="utf-8"))
FORMATS = tuple(g.DictionaryFormat)
SOURCES: dict[str, tuple[Callable[..., g.FileDictionarySource], Callable[..., bytes]]] = {
    "stdict": (d.stdict, d.stdict_bytes),
    "opendict-general": (d.opendict_general, d.opendict_general_bytes),
    "opendict-north-korean": (d.opendict_north_korean, d.opendict_north_korean_bytes),
    "opendict-dialect": (d.opendict_dialect, d.opendict_dialect_bytes),
    "opendict-archaic": (d.opendict_archaic, d.opendict_archaic_bytes),
    "loanwords": (d.loanwords, d.loanwords_bytes),
}
CASES = [(distribution, resource) for distribution in DISTRIBUTIONS for resource in distribution.resources]


def locate(key: str, format: g.DictionaryFormat) -> tuple[Distribution, Resource]:
    return next((distribution, resource) for distribution, resource in CASES
                if resource.key == key and distribution.format == format.value)


def javascript_artifact(key: str, format: g.DictionaryFormat) -> bytes:
    """The decompressed binary of the existing JavaScript package."""
    _, resource = locate(key, format)
    data = (REPOSITORY / resource.artifact).read_bytes()
    return gzip.decompress(data) if resource.gzip else data


def test_the_extras_installed_every_data_distribution_at_the_core_version() -> None:
    version = importlib.metadata.version("gukhanmun")
    for distribution in DISTRIBUTIONS:
        assert importlib.metadata.version(distribution.name) == version
        assert importlib.metadata.distribution(distribution.name).requires is None
        # Imported from the environment, not from the checkout.
        module = importlib.import_module(distribution.package)
        assert Path(str(module.__file__)).resolve().is_relative_to(Path(sys.prefix).resolve())


@pytest.mark.parametrize("case", CASES, ids=lambda case: f"{case[1].key}-{case[0].format}")
def test_helpers_return_the_javascript_package_bytes(case: tuple[Distribution, Resource]) -> None:
    distribution, resource = case
    format = g.DictionaryFormat(distribution.format)
    source_helper, bytes_helper = SOURCES[resource.key]
    data = bytes_helper(format=format)
    assert data == javascript_artifact(resource.key, format)
    record = json.loads(importlib.resources.files(distribution.package).joinpath("provenance.json").read_bytes())
    entry = record["resources"][resource.name]
    assert entry["decoded_sha256"] == hashlib.sha256(data).hexdigest()
    shipped = importlib.resources.files(distribution.package).joinpath(resource.name).read_bytes()
    assert entry["sha256"] == hashlib.sha256(shipped).hexdigest()
    source = source_helper(format=format)
    assert source.data == data
    assert source.format is format
    assert source.role is g.SourceRole(distribution.role)


def build(dictionaries: list[object], format: g.DictionaryFormat, case: dict[str, object],
          load: Callable[[str, g.DictionaryFormat], g.FileDictionarySource]) -> g.Converter:
    sources: list[g.FileDictionarySource | g.MapDictionarySource] = []
    for item in dictionaries:
        if isinstance(item, dict):
            sources.append(g.MapDictionarySource([g.DictionaryEntry(hanja=item["hanja"], reading=item["reading"])]))
        else:
            sources.append(load(str(item), format))
    return g.Converter(
        dictionaries=sources,
        source_policy=g.SourcePolicy(case.get("source_policy", "reference-authorized")),
        reference_only=bool(case.get("reference_only", False)),
        homophone_window=g.ContextWindow.OFF,
    )


def from_helpers(key: str, format: g.DictionaryFormat) -> g.FileDictionarySource:
    return SOURCES[key][0](format=format)


def from_javascript(key: str, format: g.DictionaryFormat) -> g.FileDictionarySource:
    distribution, _ = locate(key, format)
    return g.FileDictionarySource(javascript_artifact(key, format), format, g.SourceRole(distribution.role))


@pytest.mark.parametrize("format", FORMATS)
@pytest.mark.parametrize("case", CORPUS["cases"], ids=lambda case: case["name"])
def test_official_corpus(case: dict[str, object], format: g.DictionaryFormat) -> None:
    dictionaries = list(case["dictionaries"])  # type: ignore[call-overload]
    text = str(case["input"])
    converter = build(dictionaries, format, case, from_helpers)
    assert converter.convert(text) == case["expected"]
    # The same data from the JavaScript packages, with the same roles.
    assert build(dictionaries, format, case, from_javascript).convert(text) == case["expected"]
    # Chunked conversion agrees at every split.
    for split in range(len(text) + 1):
        stream = converter.stream()
        assert stream.push(text[:split]) + stream.push(text[split:]) + stream.finish() == case["expected"]
    if case.get("differs_from_fallback"):
        # Without the dictionary, the character fallback reads it differently,
        # so the expected reading really comes from the data.
        assert g.Converter(homophone_window=g.ContextWindow.OFF).convert(text) != case["expected"]


def test_presets_do_not_load_installed_data() -> None:
    for preset in g.Preset:
        assert g.Converter(preset=preset, homophone_window=g.ContextWindow.OFF).convert("一杯") != "이빠이"
