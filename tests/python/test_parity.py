# Gukhanmun: Executable TS correspondence, shared fixtures and concurrency checks.
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

from concurrent.futures import ThreadPoolExecutor
from dataclasses import fields
import inspect
import os
from pathlib import Path
import re
from threading import Barrier
import tomllib

import pytest

import gukhanmun as g


REPOSITORY = Path(os.environ["GUKHANMUN_REPO_ROOT"])
TS = (REPOSITORY / "packages" / "types" / "index.ts").read_text(encoding="utf-8")


def snake(name: str) -> str:
    return re.sub(r"(?<!^)([A-Z])", r"_\1", name).lower()


def interface(name: str) -> str:
    match = re.search(rf"export interface {name}\s*\{{(.*?)\n\}}", TS, re.DOTALL)
    assert match is not None, f"missing TS interface {name}"
    return match[1]


@pytest.mark.parametrize("name", [
    "Preset", "RenderMode", "OriginalGloss", "HanjaVariantSet", "Segmentation",
    "SourcePolicy", "SourceRole", "NumeralStrategy", "ContextWindow",
    "HomophoneDetection", "Recovery", "PunctuationPreset", "SpaceAfter", "ErrorCode",
])
def test_option_enum_values_equal_current_typescript_contract(name: str) -> None:
    match = re.search(rf"export type {name}\s*=\s*(.*?);", TS, re.DOTALL)
    assert match is not None
    expected = set(re.findall(r'"([a-z-]+)"', match[1]))
    actual = {member.value for member in getattr(g, name)}
    assert actual == expected


def test_dictionary_and_input_format_values_cover_typescript_shapes() -> None:
    assert re.search(r'readonly format:\s*"fst"\s*\|\s*"cdb"', interface("FileDictionarySource"))
    assert {item.value for item in g.DictionaryFormat} == {"fst", "cdb"}
    match = re.search(r"export type Format\s*=\s*(.*?);", TS, re.DOTALL)
    assert match is not None
    assert {item.value for item in g.InputFormat} == set(re.findall(r'"([a-z-]+)"', match[1]))


def test_constructor_keywords_and_resolved_fields_cover_all_ts_options() -> None:
    ts_names = {snake(name) for name in re.findall(r"readonly (\w+)\??:", interface("GukhanmunOptions"))}
    signature = inspect.signature(g.Converter)
    assert set(signature.parameters) == ts_names
    assert all(parameter.kind is inspect.Parameter.KEYWORD_ONLY for parameter in signature.parameters.values())
    excluded = {"original_gloss", "dictionaries", "directives", "html"}
    assert {field.name for field in fields(g.ResolvedOptions)} == ts_names - excluded
    entry_names = {snake(name) for name in re.findall(r"readonly (\w+)\??:", interface("DictionaryEntry"))}
    assert {field.name for field in fields(g.DictionaryEntry)} == entry_names
    for ts_name, python_type in [("Directives", g.Directives), ("HtmlOptions", g.HtmlOptions),
                                 ("StopStyle", g.StopStyle), ("DelimiterStyle", g.DelimiterStyle),
                                 ("CustomPunctuationProfile", g.CustomPunctuationProfile),
                                 ("ResolvedPunctuationProfile", g.ResolvedPunctuationProfile)]:
        ts_fields = {snake(name) for name in re.findall(r"^  readonly (\w+)\??:", interface(ts_name), re.MULTILINE)}
        assert {field.name for field in fields(python_type)} == ts_fields


SHARED_FIXTURES = [
    "text/redundant-parens-hanja-first.input.txt",
    "text/redundant-parens-hanja-first-original.input.txt",
    "html/initial-sound-raw.input.html",
    "html/preservation.input.html",
    "html/lenient-unclosed-cdata.input.html",
    "html/punctuation-modern.input.html",
    "markdown/code-span-untouched.input.md",
    "markdown/code-block-untouched.input.md",
    "markdown/punctuation-modern.input.md",
] + [str(path.relative_to(REPOSITORY / "tests" / "fixtures"))
     for path in sorted((REPOSITORY / "tests" / "fixtures" / "commonmark").glob("*.input.md"))]


def configured_fixture(relative: str) -> tuple[g.Converter, str, str, g.InputFormat | g.MarkdownOptions, dict]:
    path = REPOSITORY / "tests" / "fixtures" / relative
    stem, extension = path.name.split(".input.")
    sidecar = path.with_name(stem + ".toml")
    config = tomllib.loads(sidecar.read_text(encoding="utf-8")) if sidecar.exists() else {}
    # Fail on unsupported sidecar changes instead of silently comparing the
    # wrong preset/dictionary/settings against a Rust fixture's expected output.
    assert set(config) <= {"description", "preset", "source_policy", "rendering", "use_bundled_stdict",
                           "options", "engine", "dictionary", "recovery", "markdown", "assertion", "punctuation"}
    assert not config.get("use_bundled_stdict", False)
    options = config.get("options", {})
    assert set(options) <= {"initial_sound_law", "collapse_redundant_parens"}
    engine = config.get("engine", {})
    assert set(engine) <= {"disambiguation"}
    dictionaries = config.get("dictionary", {})
    assert set(dictionaries) <= {"records"}
    entries = [g.DictionaryEntry(**record) for record in dictionaries.get("records", [])]
    keywords = dict(preset=g.Preset(config.get("preset", "ko-kr")),
                    rendering=g.RenderMode(config.get("rendering", "hangul-only")),
                    source_policy=g.SourcePolicy(config.get("source_policy", "reference-authorized")),
                    recovery=g.Recovery(config.get("recovery", "strict")),
                    dictionaries=[g.MapDictionarySource(entries)], **options)
    if "disambiguation" in engine:
        keywords["homophone_window"] = g.ContextWindow(engine["disambiguation"])
    if "punctuation" in config:
        # Selected punctuation fixtures use a preset, so custom profiles must
        # be explicitly added here if these fixtures acquire one in the future.
        assert isinstance(config["punctuation"], str)
        keywords["punctuation"] = g.PunctuationPreset(config["punctuation"])
    variant = config.get("markdown", {})
    assert set(variant) <= {"variant"}
    assert variant.get("variant", "common-mark") in {"common-mark", "gfm"}
    format = {"txt": g.InputFormat.TEXT, "html": g.InputFormat.HTML,
              "md": g.MarkdownOptions(gfm=variant.get("variant") == "gfm")}[extension]
    assertion = config.get("assertion", {})
    assert set(assertion) <= {"kind", "needles"}
    assert assertion.get("kind", "exact") in {"exact", "contains"}
    return (g.Converter(**keywords), path.read_text(encoding="utf-8"),
            path.with_name(stem + ".expected." + extension).read_text(encoding="utf-8"), format, assertion)


@pytest.mark.parametrize("relative", SHARED_FIXTURES)
def test_shared_rust_regression_fixtures_honor_sidecars(relative: str) -> None:
    converter, text, expected, format, assertion = configured_fixture(relative)
    actual = converter.convert(text, format=format)
    if assertion.get("kind", "exact") == "contains":
        assert assertion.get("needles"), "contains fixtures require nonempty needles"
        for needle in assertion["needles"]:
            assert needle in actual
    else:
        # Match the existing Rust fixture harness's deliberate normalization
        # of one editor-added trailing newline, not arbitrary whitespace.
        assert actual.removesuffix("\n") == expected.removesuffix("\n")




def stateful_converter() -> g.Converter:
    return g.Converter(dictionaries=[g.MapDictionarySource([
        g.DictionaryEntry("甲", "한"), g.DictionaryEntry("乙", "한"),
        g.DictionaryEntry("漢字", "한자", require_hanja=True),
    ])], homophone_window=g.ContextWindow.PER_DOCUMENT,
        first_occurrence_window=g.ContextWindow.PER_DOCUMENT)


def document(text: str, format: g.InputFormat) -> str:
    if format is g.InputFormat.HTML:
        return f"<p>{text}</p>"
    if format is g.InputFormat.MARKDOWN:
        return f"# {text}\n"
    return text


@pytest.mark.parametrize("format", tuple(g.InputFormat))
def test_repeated_and_concurrent_calls_isolate_each_document(format: g.InputFormat) -> None:
    converter = stateful_converter()
    inputs = [document(text, format) for text in ("甲", "乙", "甲 乙", "漢字 漢字")]
    expected = [converter.convert(text, format=format) for text in inputs]
    assert "한(甲)" in expected[2] and "한(乙)" in expected[2]
    assert "한자(漢字) 한자" in expected[3]
    for _ in range(3):
        assert [converter.convert(text, format=format) for text in inputs] == expected
    barrier = Barrier(4)

    def worker(index: int) -> list[str]:
        barrier.wait(timeout=10)
        return [converter.convert(inputs[index], format=format) for _ in range(8)]

    with ThreadPoolExecutor(max_workers=4) as executor:
        results = list(executor.map(worker, range(4)))
    assert results == [[value] * 8 for value in expected]
    independent = stateful_converter()
    assert [independent.convert(text, format=format) for text in inputs] == expected


@pytest.mark.parametrize("relative", [
    "text/redundant-parens-hanja-first.input.txt", "html/initial-sound-raw.input.html",
    "markdown/code-span-untouched.input.md", "commonmark/01-atx-heading.input.md",
])
def test_shared_fixtures_have_chunk_partition_equivalence(relative: str) -> None:
    converter, text, _, format, _ = configured_fixture(relative)
    expected = converter.convert(text, format=format)
    for split in range(len(text) + 1):
        assert "".join(converter.convert_iter([text[:split], text[split:]], format=format)) == expected
    stream = converter.stream(format=format)
    for character in text:
        assert stream.push(character) == ""
    assert stream.finish() == expected


@pytest.mark.parametrize("format", tuple(g.InputFormat))
def test_simultaneous_streams_keep_document_state_separate(format: g.InputFormat) -> None:
    converter = stateful_converter()
    first, second = converter.stream(format=format), converter.stream(format=format)
    first_input = document("甲 乙", format)
    second_input = document("甲", format)
    for left, right in zip(first_input, second_input):
        assert first.push(left) == ""
        assert second.push(right) == ""
    first.push(first_input[len(second_input):])
    assert second.finish() == converter.convert(second_input, format=format)
    assert first.finish() == converter.convert(first_input, format=format)
