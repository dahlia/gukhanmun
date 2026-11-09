# Gukhanmun: Synthetic binary ownership, evidence and source policy tests.
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

from contextlib import contextmanager
from collections.abc import Iterator
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
from threading import Thread

import pytest

import gukhanmun as g


FIXTURES = Path(os.environ["GUKHANMUN_FIXTURE_DIR"])


def binary(name: str, format: g.DictionaryFormat) -> bytes:
    return (FIXTURES / f"{name}.{format.value}").read_bytes()


def source(name: str, format: g.DictionaryFormat, role: g.SourceRole | None = None) -> g.FileDictionarySource:
    return g.FileDictionarySource(binary(name, format), format, role)


@pytest.mark.parametrize("format", tuple(g.DictionaryFormat))
def test_binary_candidates_positional_readings_and_marks(format: g.DictionaryFormat) -> None:
    converter = g.Converter(dictionaries=[source("plain", format)], homophone_window=g.ContextWindow.OFF)
    assert converter.convert("東京를 東京을 1990年代 漢字") == "도쿄를 동경을 1990년대 한자(漢字)"
    original = g.Converter(dictionaries=[source("plain", format)], rendering=g.RenderMode.ORIGINAL)
    assert original.convert("學校") == "學校(학교)"


@pytest.mark.parametrize("format", tuple(g.DictionaryFormat))
def test_bytes_buffers_noncontiguous_views_and_paths_are_owned(format: g.DictionaryFormat, tmp_path: Path) -> None:
    data = binary("plain", format)
    mutable = bytearray(data)
    padded = bytearray(len(data) * 2)
    padded[::2] = data
    view = memoryview(padded)[::2]
    descriptors = [g.FileDictionarySource(data, format), g.FileDictionarySource(mutable, format),
                   g.FileDictionarySource(memoryview(data), format), g.FileDictionarySource(view, format)]
    # Descriptor creation snapshots logical buffer content, before native work.
    mutable[:] = bytes(len(mutable))
    padded[:] = bytes(len(padded))
    view.release()
    for descriptor in descriptors:
        assert g.Converter(dictionaries=[descriptor]).convert("1990年代") == "1990년대"
    path = tmp_path / "dictionary.data"
    path.write_bytes(data)

    class CustomPath:
        def __fspath__(self) -> str:
            return str(path)

    converters = [g.Converter(dictionaries=[g.FileDictionarySource(value, format)])
                  for value in (str(path), path, CustomPath())]
    path.write_bytes(b"changed after construction")
    path.unlink()
    for converter in converters:
        assert converter.convert("1990年代") == "1990년대"


@pytest.mark.parametrize("format", tuple(g.DictionaryFormat))
@pytest.mark.parametrize("policy,normal,reference_only", [
    (g.SourcePolicy.REFERENCE_AUTHORIZED, "누리", "누리"),
    (g.SourcePolicy.REFERENCE_FIRST, "가온", "가온"),
    (g.SourcePolicy.LOANWORD_FIRST, "누리", "가온"),
    (g.SourcePolicy.ORDERED, "누리", "누리"),
])
def test_embedded_roles_and_authorized_evidence_survive(format: g.DictionaryFormat, policy: g.SourcePolicy, normal: str, reference_only: str) -> None:
    # Neither descriptor states its role: both must come from embedded evidence.
    sources = [source("loanword", format), source("reference", format)]
    for restricted, expected in [(False, normal), (True, reference_only)]:
        converter = g.Converter(dictionaries=sources, source_policy=policy, reference_only=restricted)
        assert converter.convert("宙名") == expected
    reversed_converter = g.Converter(dictionaries=list(reversed(sources)), source_policy=policy)
    assert reversed_converter.convert("宙名") == ("가온" if policy is g.SourcePolicy.ORDERED else normal)


@pytest.mark.parametrize("format", tuple(g.DictionaryFormat))
def test_explicit_role_overrides_embedded_role_and_authored_entries_win(format: g.DictionaryFormat) -> None:
    override = g.MapDictionarySource([g.DictionaryEntry("宙名", "다솜")])
    inherited = g.Converter(dictionaries=[source("reference", format), override])
    explicit = g.Converter(dictionaries=[source("reference", format, g.SourceRole.OVERRIDE), override])
    assert inherited.convert("宙名") == "다솜"
    assert explicit.convert("宙名") == "가온"
    # Reference-authorized forms belong to the reference's candidate. A user
    # candidate with the same key must not acquire those authorizations.
    blocked = g.Converter(dictionaries=[override, source("reference", format), source("loanword", format)])
    assert blocked.convert("宙名") == "다솜"


@pytest.mark.parametrize("format", tuple(g.DictionaryFormat))
@pytest.mark.parametrize("segmentation", tuple(g.Segmentation))
def test_phrase_constraints_preserve_candidate_ownership_and_boundaries(format: g.DictionaryFormat, segmentation: g.Segmentation) -> None:
    converter = g.Converter(dictionaries=[source("reference", format)], segmentation=segmentation,
                            homophone_window=g.ContextWindow.OFF)
    for text, expected in [
        ("降服", "강복"), ("無條件降服", "무조건항복"),
        ("無條件 降服", "무조건 항복"), ("무조건 降服", "무조건 항복"),
        ("無條件  降服", "무조건  강복"), ("無條件\t降服", "무조건\t강복"),
        ("無條件\n降服", "무조건\n강복"),
    ]:
        assert converter.convert(text) == expected
    overridden = g.Converter(dictionaries=[
        g.MapDictionarySource([g.DictionaryEntry("降服", "다솜")]), source("reference", format),
    ], segmentation=segmentation, homophone_window=g.ContextWindow.OFF)
    assert overridden.convert("無條件降服") == "무조건다솜"


@contextmanager
def dictionary_server(data: bytes) -> Iterator[tuple[str, list[str]]]:
    paths: list[str] = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format: str, *args: object) -> None:
            pass

        def do_GET(self) -> None:
            paths.append(self.path)
            if self.path == "/redirect":
                self.send_response(302)
                self.send_header("Location", "/binary")
                self.end_headers()
            elif self.path == "/spaced-redirect":
                self.send_response(302)
                self.send_header("Location", "/my dict.fst")
                self.end_headers()
            elif self.path in ("/binary", "/my%20dict.fst"):
                self.send_response(200)
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
            else:
                self.send_error(404)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}", paths
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
        assert not thread.is_alive()


@pytest.mark.parametrize("format", tuple(g.DictionaryFormat))
def test_explicit_url_loads_valid_binary_once_and_retains_evidence(format: g.DictionaryFormat) -> None:
    with dictionary_server(binary("reference", format)) as (base, requests):
        converter = g.Converter(dictionaries=[g.UrlDictionarySource(base + "/redirect", format)])
        assert requests == ["/redirect", "/binary"]
    # The server is gone. Native conversion must use the owned binary.
    for _ in range(2):
        assert converter.convert("無條件降服") == "무조건항복"
    assert requests == ["/redirect", "/binary"]


@pytest.mark.parametrize("format", tuple(g.DictionaryFormat))
def test_redirects_percent_quote_spaces_before_validation(format: g.DictionaryFormat) -> None:
    with dictionary_server(binary("reference", format)) as (base, requests):
        converter = g.Converter(dictionaries=[g.UrlDictionarySource(base + "/spaced-redirect", format)])
        assert requests == ["/spaced-redirect", "/my%20dict.fst"]
        assert converter.convert("無條件降服") == "무조건항복"


@pytest.mark.parametrize("timeout", [1e300, 10**30])
def test_large_finite_timeout_has_a_coded_error(timeout: float) -> None:
    with dictionary_server(binary("plain", g.DictionaryFormat.FST)) as (base, _):
        with pytest.raises(g.GukhanmunError) as raised:
            g.Converter(dictionaries=[g.UrlDictionarySource(base + "/binary", g.DictionaryFormat.FST, timeout=timeout)])
        assert raised.value.code is g.ErrorCode.INVALID_INPUT


@pytest.mark.parametrize("format", tuple(g.DictionaryFormat))
def test_binary_streams_keep_phrase_and_candidate_evidence_at_every_split(format: g.DictionaryFormat) -> None:
    converter = g.Converter(dictionaries=[source("reference", format), source("plain", format)],
                            homophone_window=g.ContextWindow.OFF)
    for text in ("無條件降服", "無條件 降服", "無條件  降服", "1990年代 東京을"):
        expected = converter.convert(text)
        for split in range(len(text) + 1):
            stream = converter.stream()
            assert stream.push(text[:split]) == ""
            assert stream.push(text[split:]) == ""
            assert stream.finish() == expected
            assert "".join(converter.convert_iter([text[:split], text[split:]])) == expected
