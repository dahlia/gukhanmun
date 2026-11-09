# Gukhanmun: Buffered Python stream lifecycle and partition regression tests.
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

from collections.abc import Generator, Iterator

import pytest

import gukhanmun as g


FORMATS = [
    (g.InputFormat.TEXT, "東京을 漢字😀 年代。 1990年代"),
    (g.InputFormat.HTML, '<p>東京을 <b>漢字😀</b>。</p><code>漢字</code>'),
    (g.InputFormat.MARKDOWN, "**東京을** 漢字😀。\n\n```\n漢字\n```\n"),
    (g.MarkdownOptions(gfm=True), "~~東京을~~ 漢字😀。\n\n| 漢字 |\n| --- |\n| 年代 |\n"),
]


def converter(rendering: g.RenderMode = g.RenderMode.HANGUL_ONLY) -> g.Converter:
    return g.Converter(
        dictionaries=[g.MapDictionarySource([
            g.DictionaryEntry("漢字", "한자", require_hanja=True),
            g.DictionaryEntry("東京", "동경"),
            g.DictionaryEntry("東京", "도쿄", preference=10),
            g.DictionaryEntry("年代", "연대", suffix_reading="년대"),
        ])],
        rendering=rendering,
        punctuation=g.PunctuationPreset.MODERN,
        homophone_window=g.ContextWindow.PER_DOCUMENT,
        first_occurrence_window=g.ContextWindow.PER_DOCUMENT,
    )


@pytest.mark.parametrize("format, source", FORMATS)
@pytest.mark.parametrize("rendering", list(g.RenderMode))
def test_every_scalar_split_matches_batch(
    format: g.InputFormat | g.MarkdownOptions, source: str, rendering: g.RenderMode
) -> None:
    shared = converter(rendering)
    expected = shared.convert(source, format=format)
    for split in range(len(source) + 1):
        stream = shared.stream(format=format)
        assert isinstance(stream, g.ConversionStream)
        assert stream.push(source[:split]) == ""
        assert stream.push("") == ""
        assert stream.push(source[split:]) == ""
        assert stream.finish() == expected
    stream = shared.stream(format=format)
    for scalar in source:
        assert stream.push(scalar) == ""
    assert stream.finish() == expected
    assert "".join(shared.convert_iter(source, format=format)) == expected


def test_every_partition_of_a_small_document() -> None:
    shared = converter()
    source = "漢字😀。"
    expected = shared.convert(source)
    for mask in range(1 << (len(source) - 1)):
        chunks: list[str] = []
        start = 0
        for index in range(1, len(source)):
            if mask & (1 << (index - 1)):
                chunks.append(source[start:index])
                start = index
        chunks.append(source[start:])
        assert list(shared.convert_iter(chunks)) == [expected]


@pytest.mark.parametrize("format, source", FORMATS)
def test_streams_retain_converter_and_isolate_documents(
    format: g.InputFormat | g.MarkdownOptions, source: str
) -> None:
    shared = converter()
    expected = shared.convert(source, format=format)
    first = shared.stream(format=format)
    second = shared.stream(format=format)
    first.push(source[:3])
    second.push(source)
    first.push(source[3:])
    del shared
    assert first.finish() == expected
    assert second.finish() == expected


def test_empty_chunks_input_and_terminal_success() -> None:
    shared = converter()
    stream = shared.stream()
    assert stream.push("") == ""
    assert stream.finish() == shared.convert("") == ""
    for call in (stream.finish, lambda: stream.push(""), lambda: stream.push("漢字")):
        with pytest.raises(RuntimeError):
            call()
    assert list(shared.convert_iter([])) == []
    assert list(shared.convert_iter(["", "", ""])) == []
    assert list(shared.convert_iter("")) == []
    assert list(shared.convert_iter(["漢", "", "字"])) == [shared.convert("漢字")]


@pytest.mark.parametrize("invalid", [None, b"text", 1, [], "\ud800"])
def test_push_failure_is_terminal_and_clears_chunks(invalid: object) -> None:
    stream = converter().stream()
    stream.push("漢字")
    error = UnicodeEncodeError if isinstance(invalid, str) else TypeError
    with pytest.raises(error):
        stream.push(invalid)
    with pytest.raises(RuntimeError):
        stream.push("字")
    with pytest.raises(RuntimeError):
        stream.finish()


def test_keyboard_interrupt_during_push_propagates_and_closes() -> None:
    class Interrupted(str):
        def encode(self, encoding: str = "utf-8", errors: str = "strict") -> bytes:
            raise KeyboardInterrupt

    stream = converter().stream()
    stream.push("漢")
    with pytest.raises(KeyboardInterrupt):
        stream.push(Interrupted("字"))
    with pytest.raises(RuntimeError):
        stream.finish()


def test_failed_finish_is_terminal_and_clears_chunks() -> None:
    shared = converter()
    source = "<p>漢字 <1invalid>"
    with pytest.raises(g.GukhanmunError) as batch:
        shared.convert(source, format=g.InputFormat.HTML)
    stream = shared.stream(format=g.InputFormat.HTML)
    stream.push(source)
    with pytest.raises(g.GukhanmunError) as raised:
        stream.finish()
    assert raised.value.code is batch.value.code is g.ErrorCode.SEGMENTATION
    assert raised.value.message == batch.value.message
    with pytest.raises(RuntimeError):
        stream.finish()
    with pytest.raises(RuntimeError):
        stream.push("recovery")


def test_direct_stream_constructor_and_reinitialization_guard() -> None:
    shared = converter()
    stream = g.ConversionStream(shared, format=g.InputFormat.TEXT)
    stream.push("漢字")
    assert stream.finish() == shared.convert("漢字")
    with pytest.raises(RuntimeError):
        stream.__init__(shared)
    with pytest.raises(TypeError):
        g.ConversionStream(object())
    with pytest.raises(TypeError):
        shared.stream(format="text")


def test_iterator_validates_format_eagerly_without_touching_input() -> None:
    visited: list[bool] = []

    class Input:
        def __iter__(self) -> Iterator[str]:
            visited.append(True)
            raise AssertionError("input must not be touched by invalid format")

    with pytest.raises(TypeError):
        converter().convert_iter(Input(), format="text")
    assert visited == []
    options = g.MarkdownOptions()
    object.__setattr__(options, "gfm", 1)
    with pytest.raises(TypeError):
        converter().convert_iter(Input(), format=options)
    assert visited == []


@pytest.mark.parametrize("upstream_error", [RuntimeError("upstream"), KeyboardInterrupt(), GeneratorExit()])
def test_iterator_upstream_failure_never_finishes(
    monkeypatch: pytest.MonkeyPatch, upstream_error: BaseException
) -> None:
    finishes: list[bool] = []
    aborted: list[g.ConversionStream] = []
    original_finish = g.ConversionStream.finish
    original_abort = g.ConversionStream._abort

    def finish(stream: g.ConversionStream) -> str:
        finishes.append(True)
        return original_finish(stream)

    def abort(stream: g.ConversionStream) -> None:
        aborted.append(stream)
        original_abort(stream)

    monkeypatch.setattr(g.ConversionStream, "finish", finish)
    monkeypatch.setattr(g.ConversionStream, "_abort", abort)

    def chunks() -> Iterator[str]:
        yield "漢字"
        raise upstream_error

    iterator = converter().convert_iter(chunks())
    with pytest.raises(type(upstream_error)) as raised:
        next(iterator)
    assert raised.value is upstream_error
    assert finishes == []
    assert len(aborted) == 1
    assert aborted[0]._chunks == []
    with pytest.raises(RuntimeError):
        aborted[0].finish()


def test_iterator_close_before_start_does_not_consume_or_finish(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    visited: list[str] = []

    def finish(stream: g.ConversionStream) -> str:
        visited.append("finish")
        raise AssertionError("must not finalize a closed iterator")

    monkeypatch.setattr(g.ConversionStream, "finish", finish)

    def chunks() -> Iterator[str]:
        visited.append("upstream")
        yield "漢字"

    iterator = converter().convert_iter(chunks())
    assert isinstance(iterator, Generator)
    iterator.close()
    assert visited == []


def test_iterator_iter_failure_and_bad_chunk_do_not_finish(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    finishes: list[bool] = []

    def finish(stream: g.ConversionStream) -> str:
        finishes.append(True)
        raise AssertionError("must not finalize an invalid input")

    monkeypatch.setattr(g.ConversionStream, "finish", finish)

    class Input:
        def __iter__(self) -> Iterator[str]:
            raise ValueError("cannot iterate")

    with pytest.raises(ValueError, match="cannot iterate"):
        next(converter().convert_iter(Input()))
    with pytest.raises(TypeError):
        next(converter().convert_iter(["漢字", None]))
    assert finishes == []


@pytest.mark.parametrize("failure", [KeyboardInterrupt(), SystemExit(2)])
def test_finish_propagates_base_exceptions_and_remains_terminal(
    monkeypatch: pytest.MonkeyPatch, failure: BaseException
) -> None:
    stream = converter().stream()
    stream.push("漢字")

    def fail(
        self: g.Converter,
        source: str,
        *,
        format: g.InputFormat | g.MarkdownOptions = g.InputFormat.TEXT,
    ) -> str:
        raise failure

    monkeypatch.setattr(g.Converter, "convert", fail)
    with pytest.raises(type(failure)) as raised:
        stream.finish()
    assert raised.value is failure
    with pytest.raises(RuntimeError):
        stream.finish()
