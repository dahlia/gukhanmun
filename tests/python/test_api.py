# Gukhanmun: Baseline Python one-shot API and ownership regression tests.
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

import copy
import errno
import importlib.metadata
import multiprocessing
import os
import pickle
from concurrent.futures import ProcessPoolExecutor
from dataclasses import FrozenInstanceError, fields, is_dataclass
from collections.abc import Iterator
from enum import Enum
from pathlib import Path

import pytest

import gukhanmun as g


ENUMS = (
    g.Preset, g.RenderMode, g.OriginalGloss, g.HanjaVariantSet,
    g.Segmentation, g.SourcePolicy, g.SourceRole, g.NumeralStrategy,
    g.ContextWindow, g.HomophoneDetection, g.Recovery, g.PunctuationPreset,
    g.SpaceAfter, g.DictionaryFormat, g.InputFormat, g.ErrorCode,
)


def test_public_version_comes_from_installed_metadata() -> None:
    assert g.__version__ == importlib.metadata.version("gukhanmun")


def dictionary() -> g.MapDictionarySource:
    return g.MapDictionarySource(entries=[g.DictionaryEntry(hanja="漢字", reading="한자")])


@pytest.mark.parametrize("operation", ["copy", "deepcopy", "pickle"])
def test_coded_errors_survive_standard_transport(operation: str) -> None:
    error = g.GukhanmunError(
        g.ErrorCode.DICTIONARY_LOAD,
        "could not load dictionary",
        [g.ErrorCause(g.ErrorCode.IO, "missing file")],
    )
    if operation == "pickle":
        restored = pickle.loads(pickle.dumps(error))
    elif operation == "copy":
        restored = copy.copy(error)
    else:
        restored = copy.deepcopy(error)
    assert restored is not error
    assert restored.code is error.code
    assert restored.message == error.message
    assert restored.chain == error.chain
    assert isinstance(restored.chain, tuple)
    assert str(restored) == str(error)
    assert restored.args == error.args


def _convert_document_in_process(source: str) -> str:
    return g.Converter(dictionaries=[dictionary()]).convert(source, format=g.InputFormat.HTML)


def test_conversion_error_crosses_process_boundary_without_breaking_pool() -> None:
    with ProcessPoolExecutor(
        max_workers=1, mp_context=multiprocessing.get_context("spawn")
    ) as pool:
        failed = pool.submit(_convert_document_in_process, "<p>漢字 <1invalid>")
        with pytest.raises(g.GukhanmunError) as raised:
            failed.result(timeout=30)
        assert raised.value.code is g.ErrorCode.SEGMENTATION
        assert pool.submit(_convert_document_in_process, "<p>漢字</p>").result(timeout=30) == "<p>한자</p>"


def test_standard_enum_contract() -> None:
    assert len(ENUMS) == 16
    for enum in ENUMS:
        assert issubclass(enum, Enum)
        for value in enum:
            assert not isinstance(value, str)
            assert value.name == value.value.upper().replace("-", "_")
            assert enum(value.value) is value
    assert g.HanjaVariantSet.KANXI.value == "kanxi"
    assert len(g.ErrorCode) == 11
    assert pickle.loads(pickle.dumps(g.UNSET)) is g.UNSET
    assert copy.deepcopy(g.UNSET) is g.UNSET


def test_defaults_are_actual_native_options() -> None:
    options = g.Converter().options
    assert len(fields(options)) == 14
    assert options == g.ResolvedOptions(
        preset=g.Preset.KO_KR,
        rendering=g.RenderMode.HANGUL_ONLY,
        hanja_variant_set=g.HanjaVariantSet.AS_DICTIONARY,
        segmentation=g.Segmentation.LATTICE,
        source_policy=g.SourcePolicy.REFERENCE_AUTHORIZED,
        reference_only=False,
        numerals=g.NumeralStrategy.HANGUL_PHONETIC,
        initial_sound_law=True,
        homophone_window=g.ContextWindow.PER_BLOCK,
        homophone_detection=g.HomophoneDetection.CONTEXT_LOCAL,
        first_occurrence_window=g.ContextWindow.OFF,
        collapse_redundant_parens=True,
        recovery=g.Recovery.STRICT,
        punctuation=g.ResolvedPunctuationProfile(None, None, None, None),
    )
    north = g.Converter(preset=g.Preset.KO_KP).options
    assert north.initial_sound_law is False
    assert north.homophone_window is g.ContextWindow.OFF
    overrides = g.Converter(
        preset=g.Preset.KO_KP, initial_sound_law=True,
        homophone_window=g.ContextWindow.PER_DOCUMENT,
    ).options
    assert overrides.initial_sound_law is True
    assert overrides.homophone_window is g.ContextWindow.PER_DOCUMENT
    assert not hasattr(options, "original_gloss")
    assert not hasattr(options, "dictionaries")


@pytest.mark.parametrize("mode, expected", [
    (g.RenderMode.HANGUL_ONLY, "한자"),
    (g.RenderMode.HANGUL_HANJA_PARENS, "한자(漢字)"),
    (g.RenderMode.HANJA_HANGUL_PARENS, "漢字(한자)"),
    (g.RenderMode.RUBY_ON_HANGUL, "한자(漢字)"),
    (g.RenderMode.RUBY_ON_HANJA, "漢字(한자)"),
    (g.RenderMode.ORIGINAL, "漢字"),
])
def test_rendering(mode: g.RenderMode, expected: str) -> None:
    converter = g.Converter(rendering=mode, dictionaries=[dictionary()])
    assert converter.options.rendering is mode
    assert converter.convert("漢字") == expected


def test_ruby_original_gloss_and_formats() -> None:
    ruby = g.Converter(rendering=g.RenderMode.RUBY_ON_HANJA, dictionaries=[dictionary()])
    assert ruby.convert("<p>漢字</p>", format=g.InputFormat.HTML) == "<p><ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby></p>"
    marked = g.MapDictionarySource(entries=[g.DictionaryEntry("漢字", "한자", require_hangul=True)])
    original = g.Converter(rendering=g.RenderMode.ORIGINAL, dictionaries=[marked])
    assert original.convert("漢字") == "漢字(한자)"
    ruby_gloss = g.Converter(
        rendering=g.RenderMode.ORIGINAL, original_gloss=g.OriginalGloss.RUBY,
        dictionaries=[marked],
    )
    assert ruby_gloss.convert("漢字", format=g.InputFormat.HTML) == "<ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>"
    text = g.Converter(dictionaries=[dictionary()])
    assert text.convert("漢字") == text.convert("漢字", format=g.InputFormat.TEXT)
    assert "한자" in text.convert("漢字\n\n```\n漢字\n```", format=g.InputFormat.MARKDOWN)
    assert "漢字" in text.convert("漢字\n\n```\n漢字\n```", format=g.MarkdownOptions())
    assert text.convert("~~漢字~~", format=g.MarkdownOptions(gfm=True)) == "~~한자~~"


def test_map_candidates_suffix_marks_and_snapshot() -> None:
    entries = [
        g.DictionaryEntry("東京", "동경"),
        g.DictionaryEntry("東京", "도쿄", preference=10),
        g.DictionaryEntry("年代", "연대", suffix_reading="년대"),
        g.DictionaryEntry("庫間", "곳간", require_hanja=True),
    ]
    source = g.MapDictionarySource(entries=entries)
    entries.clear()
    converter = g.Converter(dictionaries=[source], homophone_window=g.ContextWindow.OFF)
    assert converter.convert("東京") == "도쿄"
    assert converter.convert("東京을") == "동경을"
    assert converter.convert("東京를") == "도쿄를"
    assert converter.convert("1990年代 年代") == "1990년대 연대"
    assert converter.convert("庫間") == "곳간(庫間)"
    assert source.entries[0].hanja == "東京"
    assert g.Converter(dictionaries=[g.MapDictionarySource([])]).convert("漢字") == g.Converter().convert("漢字")


@pytest.mark.parametrize("entry", [
    g.DictionaryEntry("", "한"), g.DictionaryEntry("漢", ""),
    g.DictionaryEntry("漢", "한", suffix_reading=""),
    g.DictionaryEntry("漢", "한", preference=-1),
    g.DictionaryEntry("漢", "한", preference=2**32),
])
def test_bad_entry_values_have_code_and_index(entry: g.DictionaryEntry) -> None:
    with pytest.raises(g.GukhanmunError) as raised:
        g.Converter(dictionaries=[g.MapDictionarySource([g.DictionaryEntry("字", "자"), entry])])
    assert raised.value.code is g.ErrorCode.INVALID_INPUT
    assert "entry 1" in raised.value.message


def test_strict_categories() -> None:
    with pytest.raises(TypeError):
        g.Converter(preset="ko-kr")
    with pytest.raises(TypeError):
        g.Converter(preset=g.InputFormat.TEXT)
    with pytest.raises(TypeError):
        g.Converter(reference_only=1)
    with pytest.raises(TypeError):
        g.Converter(initial_sound_law=1)
    with pytest.raises(TypeError):
        g.Converter(dictionaries=[{}])
    with pytest.raises(TypeError):
        g.DictionaryEntry("漢", "한", preference=True)
    with pytest.raises(TypeError):
        g.DictionaryEntry("漢", "한", require_hanja=1)
    with pytest.raises(TypeError):
        g.Directives(require_hanja="漢字")
    with pytest.raises(TypeError):
        g.MapDictionarySource(entries="漢字")
    with pytest.raises(TypeError):
        g.HtmlOptions(preserve_attributes=["data-keep"])
    with pytest.raises(TypeError):
        g.StopStyle(".", "single")
    with pytest.raises(TypeError):
        g.MarkdownOptions(gfm=1)
    with pytest.raises(TypeError):
        g.Converter().convert("漢字", format="text")
    with pytest.raises(TypeError):
        g.Converter().convert(b"text")


def test_all_records_are_frozen_and_converter_cannot_reinitialize() -> None:
    converter = g.Converter(punctuation=g.PunctuationPreset.MODERN)
    records = [
        g.DictionaryEntry("漢", "한"),
        g.FileDictionarySource(b"", g.DictionaryFormat.FST),
        g.UrlDictionarySource("https://example.com/dict", g.DictionaryFormat.FST),
        dictionary(), g.Directives(), g.HtmlOptions(), g.AttributeMatcher("keep"),
        g.StopStyle(".", g.SpaceAfter.SINGLE), g.DelimiterStyle("〈", "〉"),
        g.CustomPunctuationProfile(), converter.options.punctuation,
        converter.options, g.MarkdownOptions(), g.ErrorCause(g.ErrorCode.IO, "error"),
    ]
    assert len(records) == 14
    for record in records:
        assert is_dataclass(record)
        with pytest.raises(FrozenInstanceError):
            setattr(record, fields(record)[0].name, None)
    with pytest.raises(AttributeError):
        converter.options = converter.options
    with pytest.raises(AttributeError):
        del converter._handle
    with pytest.raises(RuntimeError):
        converter.__init__(preset=g.Preset.KO_KP)
    assert converter.options.preset is g.Preset.KO_KR


def test_directives_and_html_snapshots() -> None:
    literals = ["漢字"]
    classes = ["keep"]
    matchers = [g.AttributeMatcher("data-keep", "yes")]
    directives = g.Directives(require_hanja=literals)
    html = g.HtmlOptions(preserve_classes=classes, preserve_attributes=matchers)
    literals.clear()
    classes.clear()
    matchers.clear()
    converter = g.Converter(dictionaries=[dictionary()], directives=directives, html=html)
    assert converter.convert("漢字") == "한자(漢字)"
    assert converter.convert('<p class="keep"><b>漢字</b></p>', format=g.InputFormat.HTML) == '<p class="keep"><b>漢字</b></p>'
    assert converter.convert('<p data-keep="yes">漢字</p>', format=g.InputFormat.HTML) == '<p data-keep="yes">漢字</p>'
    assert "한자" in converter.convert('<p data-keep="no">漢字</p>', format=g.InputFormat.HTML)
    assert directives.require_hanja == ("漢字",)
    assert html.preserve_attributes == (g.AttributeMatcher("data-keep", "yes"),)


def test_punctuation_tri_state_and_validation() -> None:
    inherited = g.CustomPunctuationProfile(base=g.PunctuationPreset.MODERN, period=None)
    assert inherited.comma is g.UNSET
    converter = g.Converter(punctuation=inherited)
    assert converter.convert("가、나。 다") == "가, 나。 다"
    assert converter.options.punctuation.period is None
    assert converter.options.punctuation.comma == g.StopStyle(",", g.SpaceAfter.SINGLE)
    custom = g.Converter(punctuation=g.CustomPunctuationProfile(period=g.StopStyle("💬", g.SpaceAfter.REMOVE)))
    assert custom.convert("가。 나") == "가💬나"
    for profile in (
        g.CustomPunctuationProfile(period=g.StopStyle("xx", g.SpaceAfter.SINGLE)),
        g.CustomPunctuationProfile(period=g.StopStyle("&", g.SpaceAfter.SINGLE)),
        g.CustomPunctuationProfile(single=g.DelimiterStyle("〉", "〈")),
    ):
        with pytest.raises(g.GukhanmunError) as raised:
            g.Converter(punctuation=profile)
        assert raised.value.code is g.ErrorCode.INVALID_INPUT


def test_binary_descriptor_snapshots_and_errors(tmp_path: Path) -> None:
    mutable = bytearray(b"abcdef")
    source = g.FileDictionarySource(mutable, g.DictionaryFormat.FST)
    view_source = g.FileDictionarySource(memoryview(mutable)[::2], g.DictionaryFormat.CDB)
    mutable[:] = b"xxxxxx"
    assert source.data == b"abcdef"
    assert view_source.data == b"ace"
    view = memoryview(b"abc")
    view.release()
    with pytest.raises(g.GukhanmunError) as released:
        g.FileDictionarySource(view, g.DictionaryFormat.FST)
    assert released.value.code is g.ErrorCode.INVALID_INPUT
    missing = g.FileDictionarySource(tmp_path / "missing", g.DictionaryFormat.FST)
    with pytest.raises(g.GukhanmunError) as raised:
        g.Converter(dictionaries=[missing])
    assert raised.value.code is g.ErrorCode.DICTIONARY_LOAD
    assert raised.value.chain[0].code is g.ErrorCode.IO
    assert isinstance(raised.value.chain, tuple)
    assert str(raised.value) == raised.value.message
    with pytest.raises(g.GukhanmunError) as malformed:
        g.Converter(dictionaries=[source])
    assert malformed.value.code is g.ErrorCode.DICTIONARY_LOAD
    assert malformed.value.chain == ()
    with pytest.raises(g.GukhanmunError) as nul:
        g.Converter(dictionaries=[g.FileDictionarySource("bad\0name", g.DictionaryFormat.FST)])
    assert nul.value.code is g.ErrorCode.DICTIONARY_LOAD
    assert nul.value.chain[0].code is g.ErrorCode.IO


@pytest.mark.parametrize("url", [
    "file:///tmp/file", "ftp://example.com/file", "data:text/plain,hi",
    "https:///missing-host", "http://example.com:abc/", "http://example.com:65536/",
    "http://[bad/", "https://example.com/a b",
])
def test_malformed_urls_are_invalid_input(url: str) -> None:
    with pytest.raises(g.GukhanmunError) as raised:
        g.UrlDictionarySource(url, g.DictionaryFormat.FST)
    assert raised.value.code is g.ErrorCode.INVALID_INPUT


@pytest.mark.parametrize("timeout", [0, -1, float("inf"), float("nan")])
def test_invalid_timeouts(timeout: float) -> None:
    with pytest.raises(g.GukhanmunError) as raised:
        g.UrlDictionarySource("http://localhost/", g.DictionaryFormat.FST, timeout=timeout)
    assert raised.value.code is g.ErrorCode.INVALID_INPUT


def test_timeout_bool_is_wrong_category() -> None:
    with pytest.raises(TypeError):
        g.UrlDictionarySource("http://localhost/", g.DictionaryFormat.FST, timeout=True)


def test_unicode_source_and_configuration_boundaries(tmp_path: Path) -> None:
    converter = g.Converter(dictionaries=[dictionary()])
    assert converter.convert("😀漢字𠀀").startswith("😀한자")
    with pytest.raises(UnicodeEncodeError):
        converter.convert("\ud800")
    for build in (
        lambda: g.Converter(directives=g.Directives(require_hanja=["\ud800"])),
        lambda: g.Converter(html=g.HtmlOptions(preserve_classes=["\ud800"])),
        lambda: g.Converter(html=g.HtmlOptions(preserve_attributes=[g.AttributeMatcher("\ud800")])),
        lambda: g.Converter(dictionaries=[g.MapDictionarySource([g.DictionaryEntry("漢", "\ud800")])]),
        lambda: g.Converter(punctuation=g.CustomPunctuationProfile(period=g.StopStyle("\ud800", g.SpaceAfter.REMOVE))),
    ):
        with pytest.raises(g.GukhanmunError) as raised:
            build()
        assert raised.value.code is g.ErrorCode.INVALID_INPUT
    if os.name == "posix":
        path = os.fsdecode(os.fsencode(tmp_path) + b"/surrogate-\xff")
        expected_causes = ()
        try:
            Path(path).write_bytes(b"not a dictionary")
        except OSError as error:
            if error.errno != errno.EILSEQ:
                raise
            # Some filesystems reject invalid UTF-8 filenames before the
            # dictionary can be read, which must retain the I/O error code.
            expected_causes = (g.ErrorCode.IO,)
        source = g.FileDictionarySource(path, g.DictionaryFormat.FST)
        with pytest.raises(g.GukhanmunError) as raised:
            g.Converter(dictionaries=[source])
        assert raised.value.code is g.ErrorCode.DICTIONARY_LOAD
        assert tuple(cause.code for cause in raised.value.chain) == expected_causes


def test_constructor_revalidates_altered_descriptors() -> None:
    entry = g.DictionaryEntry("漢", "한")
    object.__setattr__(entry, "preference", True)
    with pytest.raises(TypeError):
        g.Converter(dictionaries=[g.MapDictionarySource([entry])])
    html = g.HtmlOptions()
    object.__setattr__(html, "preserve_classes", "keep")
    with pytest.raises(TypeError):
        g.Converter(html=html)


def test_url_redirects_validate_before_dispatch_and_errors_have_io_causes() -> None:
    from contextlib import contextmanager
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
    from threading import Thread

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format: str, *args: object) -> None:
            pass

        def do_GET(self) -> None:
            if self.path == "/file":
                self.send_response(302)
                self.send_header("Location", "file:///should-never-open")
            elif self.path == "/data":
                self.send_response(307)
                self.send_header("Location", "data:text/plain,should-never-open")
            elif self.path == "/relative":
                self.send_response(303)
                self.send_header("Location", "/bad-binary")
            elif self.path == "/bad-binary":
                self.send_response(200)
            else:
                self.send_response(404)
            self.end_headers()
            if self.path == "/bad-binary":
                self.wfile.write(b"not a dictionary")

    @contextmanager
    def server() -> Iterator[str]:
        httpd = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = Thread(target=httpd.serve_forever, daemon=True)
        thread.start()
        try:
            yield f"http://127.0.0.1:{httpd.server_port}"
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=5)

    with server() as base:
        for path in ("/file", "/data"):
            with pytest.raises(g.GukhanmunError) as raised:
                g.Converter(dictionaries=[g.UrlDictionarySource(base + path, g.DictionaryFormat.FST)])
            assert raised.value.code is g.ErrorCode.INVALID_INPUT
        with pytest.raises(g.GukhanmunError) as malformed:
            g.Converter(dictionaries=[g.UrlDictionarySource(base + "/relative", g.DictionaryFormat.FST)])
        assert malformed.value.code is g.ErrorCode.DICTIONARY_LOAD
        assert malformed.value.chain == ()
        with pytest.raises(g.GukhanmunError) as status:
            g.Converter(dictionaries=[g.UrlDictionarySource(base + "/missing", g.DictionaryFormat.FST)])
        assert status.value.code is g.ErrorCode.DICTIONARY_LOAD
        assert status.value.chain[0].code is g.ErrorCode.IO
