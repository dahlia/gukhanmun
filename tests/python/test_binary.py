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
import http.client
import io
import ssl
import struct
import os
from pathlib import Path
from threading import Event, Thread
import time
import urllib.request

import pytest

import gukhanmun as g
from gukhanmun import _api


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
def dictionary_server(data: bytes, *, redirects: dict[str, tuple[int, str]] | None = None) -> Iterator[tuple[str, list[str]]]:
    paths: list[str] = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format: str, *args: object) -> None:
            pass

        def do_GET(self) -> None:
            paths.append(self.path)
            if redirects is not None and self.path in redirects:
                code, target = redirects[self.path]
                self.send_response(code)
                self.send_header("Location", target)
                self.end_headers()
            elif self.path == "/redirect":
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


@pytest.mark.parametrize("code", [301, 302, 303, 307, 308])
@pytest.mark.parametrize("target_form", ["host", "port", "network-path"])
@pytest.mark.parametrize("chain", [False, True])
def test_cross_origin_redirect_never_contacts_target(code: int, target_form: str, chain: bool) -> None:
    data = binary("reference", g.DictionaryFormat.FST)
    with dictionary_server(data) as (target_base, target_paths):
        target = target_base + "/binary"
        if target_form == "host":
            target = target.replace("127.0.0.1", "localhost")
        elif target_form == "network-path":
            target = target.removeprefix("http:")
        redirects = {"/start": (code, target)}
        if chain:
            redirects = {"/start": (code, "/next"), "/next": (code, target)}
        with dictionary_server(data, redirects=redirects) as (base, paths):
            try:
                with pytest.raises(g.GukhanmunError) as caught:
                    g.Converter(dictionaries=[g.UrlDictionarySource(base + "/start", g.DictionaryFormat.FST)])
                assert caught.value.code is g.ErrorCode.INVALID_INPUT
                assert caught.value.chain == ()
            finally:
                # Even a blind GET to an unrelated local resource is forbidden.
                assert target_paths == []
            assert paths == (["/start", "/next"] if chain else ["/start"])


@pytest.mark.parametrize("code", [301, 302, 303, 307, 308])
def test_same_origin_relative_and_absolute_redirect_chain(code: int) -> None:
    data = binary("reference", g.DictionaryFormat.FST)
    redirects = {"/start": (code, "/next")}
    with dictionary_server(data, redirects=redirects) as (base, paths):
        redirects["/next"] = (code, base + "/my dict.fst")
        converter = g.Converter(dictionaries=[g.UrlDictionarySource(base + "/start", g.DictionaryFormat.FST)])
        assert paths == ["/start", "/next", "/my%20dict.fst"]
        assert converter.convert("無條件降服") == "무조건항복"


@pytest.mark.parametrize("code", [301, 302, 303, 307, 308])
def test_rejected_redirect_response_closes_before_request_creation(code: int, monkeypatch: pytest.MonkeyPatch) -> None:
    handler = _api._HttpRedirects("http://example.test/start")
    response = io.BytesIO(b"redirect body must not be read")
    headers = http.client.HTTPMessage()
    headers["Location"] = "https://example.test/private"

    def unexpected_request(*args: object) -> None:
        pytest.fail("a rejected redirect must not construct its next request")

    monkeypatch.setattr(urllib.request.HTTPRedirectHandler, "redirect_request", unexpected_request)
    callback = getattr(handler, f"http_error_{code}")
    with pytest.raises(g.GukhanmunError) as caught:
        callback(urllib.request.Request("http://example.test/start"), response, code, "Redirect", headers)
    assert caught.value.code is g.ErrorCode.INVALID_INPUT
    assert response.closed


@pytest.mark.parametrize("initial,target,allowed", [
    ("http://example.test/start", "http://EXAMPLE.TEST/end", True),
    ("http://example.test/start", "http://example.test:80/end", True),
    ("http://example.test:80/start", "http://example.test/end", True),
    ("https://example.test/start", "https://example.test:443/end", True),
    ("https://example.test:443/start", "https://example.test/end", True),
    ("http://example.test:0/start", "http://example.test:0/end", True),
    ("http://[::1]/start", "http://[::1]:80/end", True),
    ("http://[FE80::1%25ETH0]/start", "http://[fe80::1%25ETH0]/end", True),
    ("http://[fe80::1%25ETH0]/start", "http://[fe80::1%25eth0]/end", False),
    ("http://[fe80::1%25eth0]/start", "http://[fe80::1%25ETH0]/end", False),
    ("http://example.test/start", "https://example.test/end", False),
    ("https://example.test/start", "http://example.test/end", False),
    ("http://example.test/start", "http://other.test/end", False),
    ("http://example.test/start", "http://example.test:81/end", False),
    ("http://example.test/start", "http://example.test:0/end", False),
    ("http://example.test:0/start", "http://example.test/end", False),
    ("https://example.test/start", "https://example.test:0/end", False),
    ("http://example.test/start", "http://127.0.0.1/end", False),
    ("http://example.test/start", "http://[::1]/end", False),
])
def test_redirect_origin_validation_precedes_request_creation_and_closes_response(
    initial: str, target: str, allowed: bool, monkeypatch: pytest.MonkeyPatch,
) -> None:
    handler = _api._HttpRedirects(initial)
    created: list[str] = []

    def create_request(self: urllib.request.HTTPRedirectHandler, req: urllib.request.Request,
                       fp: io.BytesIO, code: int, msg: str, headers: http.client.HTTPMessage,
                       newurl: str) -> urllib.request.Request:
        created.append(newurl)
        return urllib.request.Request(newurl)

    monkeypatch.setattr(urllib.request.HTTPRedirectHandler, "redirect_request", create_request)
    response = io.BytesIO(b"redirect body must not be read")
    request = urllib.request.Request(initial)
    headers = http.client.HTTPMessage()
    if allowed:
        redirected = handler.redirect_request(request, response, 302, "Found", headers, target)
        assert redirected is not None
        assert redirected.full_url == target
        assert created == [target]
    else:
        with pytest.raises(g.GukhanmunError) as caught:
            handler.redirect_request(request, response, 302, "Found", headers, target)
        assert caught.value.code is g.ErrorCode.INVALID_INPUT
        assert caught.value.chain == ()
        assert created == []
    assert response.closed


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


@contextmanager
def bounded_dictionary_server(data: bytes, mode: str, *, tls: bool = False) -> Iterator[tuple[str, list[str]]]:
    paths: list[str] = []
    stop = Event()

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format: str, *args: object) -> None:
            pass

        def do_GET(self) -> None:
            paths.append(self.path)
            try:
                if mode == "slow-header":
                    self.wfile.write(b"HTTP/1.1 200 OK\r\nX-Slow: ")
                else:
                    redirect = mode == "redirect-body" and self.path != "/binary"
                    if mode == "redirect-chain":
                        if stop.wait(0.07):
                            return
                        redirect = True
                    self.send_response(302 if redirect else 200)
                    if redirect:
                        self.send_header("Location", "/binary" if mode == "redirect-body" else self.path + "x")
                    elif mode in ("fixed", "truncated"):
                        self.send_header("Content-Length", str(len(data) + (10 if mode == "truncated" else 0)))
                    elif mode in ("chunked", "slow-chunk-line", "slow-body"):
                        self.send_header("Transfer-Encoding", "chunked")
                    self.end_headers()
                    if mode == "stalled":
                        stop.wait(5)
                        return
                    if redirect and mode == "redirect-chain":
                        return
                    if mode == "slow-chunk-line":
                        self.wfile.write(b"1;")
                    elif mode == "slow-body":
                        self.wfile.write(b"100000\r\n")
                    elif mode == "redirect-body" and redirect:
                        # An unbounded redirect drain would never reach /binary.
                        pass
                    elif mode == "chunked":
                        for part in (data[:1], data[1:]):
                            self.wfile.write(f"{len(part):x}\r\n".encode("ascii") + part + b"\r\n")
                        self.wfile.write(b"0\r\n\r\n")
                        return
                    else:
                        self.wfile.write(data)
                        return
                for _ in range(1000):
                    if stop.wait(0.01):
                        return
                    self.wfile.write(b"a")
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    if tls:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        directory = Path(__file__).with_name("tls")
        context.load_cert_chain(directory / "localhost.crt", directory / "localhost.key")
        server.socket = context.wrap_socket(server.socket, server_side=True)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        scheme = "https" if tls else "http"
        yield f"{scheme}://127.0.0.1:{server.server_port}", paths
    finally:
        stop.set()
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
        assert not thread.is_alive()


@pytest.mark.parametrize("mode", ["fixed", "chunked", "no-length"])
def test_url_body_size_limit_including_exact_boundary(mode: str) -> None:
    data = binary("reference", g.DictionaryFormat.FST)
    with bounded_dictionary_server(data, mode) as (base, _):
        converter = g.Converter(dictionaries=[g.UrlDictionarySource(
            base, g.DictionaryFormat.FST, max_bytes=len(data))])
        assert converter.convert("無條件降服") == "무조건항복"
        with pytest.raises(g.GukhanmunError) as caught:
            g.Converter(dictionaries=[g.UrlDictionarySource(
                base, g.DictionaryFormat.FST, max_bytes=len(data) - 1)])
        assert caught.value.code is g.ErrorCode.DICTIONARY_LOAD
        assert caught.value.chain[0].code is g.ErrorCode.IO
        assert "max_bytes" in caught.value.chain[0].message


@pytest.mark.parametrize("mode", ["slow-header", "slow-chunk-line", "slow-body", "stalled", "redirect-chain"])
def test_url_deadline_bounds_each_underlying_read(mode: str) -> None:
    with bounded_dictionary_server(b"", mode) as (base, _):
        start = time.monotonic()
        with pytest.raises(g.GukhanmunError) as caught:
            g.Converter(dictionaries=[g.UrlDictionarySource(
                base, g.DictionaryFormat.FST, timeout=5.0, total_timeout=0.15)])
        elapsed = time.monotonic() - start
        assert caught.value.code is g.ErrorCode.DICTIONARY_LOAD
        assert caught.value.chain[0].code is g.ErrorCode.IO
        assert elapsed < 2.0


def test_url_redirect_does_not_drain_unbounded_body() -> None:
    data = binary("reference", g.DictionaryFormat.FST)
    with bounded_dictionary_server(data, "redirect-body") as (base, paths):
        converter = g.Converter(dictionaries=[g.UrlDictionarySource(
            base, g.DictionaryFormat.FST, total_timeout=1.0)])
        assert paths == ["/", "/binary"]
        assert converter.convert("無條件降服") == "무조건항복"


def test_url_truncated_content_length_keeps_io_error() -> None:
    # Even otherwise-valid bytes are rejected when the transport promises more.
    with bounded_dictionary_server(binary("reference", g.DictionaryFormat.FST), "truncated") as (base, _):
        with pytest.raises(g.GukhanmunError) as caught:
            g.Converter(dictionaries=[g.UrlDictionarySource(base, g.DictionaryFormat.FST)])
        assert caught.value.code is g.ErrorCode.DICTIONARY_LOAD
        assert caught.value.chain[0].code is g.ErrorCode.IO


@pytest.mark.parametrize("value", [0, -1])
def test_url_invalid_byte_limits(value: int) -> None:
    with pytest.raises(g.GukhanmunError) as caught:
        g.UrlDictionarySource("http://localhost/", g.DictionaryFormat.FST, max_bytes=value)
    assert caught.value.code is g.ErrorCode.INVALID_INPUT


@pytest.mark.parametrize("value", [True, 1.5, "64"])
def test_url_wrong_byte_limit_category(value: object) -> None:
    with pytest.raises(TypeError):
        g.UrlDictionarySource("http://localhost/", g.DictionaryFormat.FST, max_bytes=value)  # type: ignore[arg-type]


@pytest.mark.parametrize("value", [0, -1, float("inf"), float("nan")])
def test_url_invalid_total_timeout(value: float) -> None:
    with pytest.raises(g.GukhanmunError) as caught:
        g.UrlDictionarySource("http://localhost/", g.DictionaryFormat.FST, total_timeout=value)
    assert caught.value.code is g.ErrorCode.INVALID_INPUT


def test_url_wrong_total_timeout_category() -> None:
    with pytest.raises(TypeError):
        g.UrlDictionarySource("http://localhost/", g.DictionaryFormat.FST, total_timeout=True)


@pytest.mark.parametrize("field", ["max_bytes", "total_timeout"])
def test_url_bounds_are_revalidated_at_converter_boundary(field: str) -> None:
    descriptor = g.UrlDictionarySource("http://localhost/", g.DictionaryFormat.FST)
    object.__setattr__(descriptor, field, 0)
    with pytest.raises(g.GukhanmunError) as caught:
        g.Converter(dictionaries=[descriptor])
    assert caught.value.code is g.ErrorCode.INVALID_INPUT


@pytest.mark.parametrize("mode", ["fixed", "slow-body"])
def test_https_response_limits_with_verified_local_certificate(mode: str, monkeypatch: pytest.MonkeyPatch) -> None:
    context = ssl.create_default_context(cafile=str(Path(__file__).with_name("tls") / "localhost.crt"))
    # Keep certificate verification enabled while trusting only this fixture CA.
    monkeypatch.setattr(ssl, "_create_default_https_context", lambda: context)
    data = binary("reference", g.DictionaryFormat.FST)
    with bounded_dictionary_server(data, mode, tls=True) as (base, _):
        if mode == "fixed":
            converter = g.Converter(dictionaries=[g.UrlDictionarySource(base, g.DictionaryFormat.FST)])
            assert converter.convert("無條件降服") == "무조건항복"
        else:
            with pytest.raises(g.GukhanmunError) as caught:
                g.Converter(dictionaries=[g.UrlDictionarySource(base, g.DictionaryFormat.FST, total_timeout=0.15)])
            assert caught.value.chain[0].code is g.ErrorCode.IO


@pytest.mark.parametrize("transport", ["bytes", "path", "url"])
def test_malformed_cdb_hash_tables_are_rejected_before_conversion(transport: str, tmp_path: Path) -> None:
    data = bytearray(binary("plain", g.DictionaryFormat.CDB))
    # Preserve valid metadata while replacing another bucket with a full table
    # of repeated record pointers. The old metadata-only loader accepted it.
    metadata_hash = 5381
    for byte in b"__gukhanmun_meta__":
        metadata_hash = ((metadata_hash * 33) ^ byte) & 0xffffffff
    for bucket in range(256):
        position, count = struct.unpack_from("<II", data, bucket * 8)
        if count and bucket != metadata_hash & 0xff:
            for index in range(count):
                hash_value, record = struct.unpack_from("<II", data, position + index * 8)
                if record:
                    break
            else:
                continue
            table = len(data)
            data.extend(struct.pack("<II", hash_value, record) * 128)
            struct.pack_into("<II", data, bucket * 8, table, 128)
            break
    else:
        pytest.fail("synthetic CDB fixture has no non-metadata bucket")

    def reject(descriptor: g.FileDictionarySource | g.UrlDictionarySource) -> None:
        with pytest.raises(g.GukhanmunError) as caught:
            g.Converter(dictionaries=[descriptor])
        assert caught.value.code is g.ErrorCode.DICTIONARY_LOAD
        assert caught.value.chain == ()

    if transport == "url":
        with dictionary_server(bytes(data)) as (base, paths):
            reject(g.UrlDictionarySource(base + "/binary", g.DictionaryFormat.CDB))
            assert paths == ["/binary"]
    elif transport == "path":
        path = tmp_path / "malformed.cdb"
        path.write_bytes(data)
        reject(g.FileDictionarySource(path, g.DictionaryFormat.CDB))
    else:
        reject(g.FileDictionarySource(data, g.DictionaryFormat.CDB))


def aliases_binary(format: g.DictionaryFormat, aliases: str) -> bytes:
    # A small CBOR string map is enough to reach eager aliases decoding.
    def text(value: str) -> bytes:
        encoded = value.encode("utf-8")
        return b"\x7a" + struct.pack(">I", len(encoded)) + encoded

    fields = {"version": "2", "entry_count": "4", "max_word_chars": "2", "aliases": aliases}
    metadata = b"\xa4" + b"".join(text(key) + text(value) for key, value in fields.items())
    if format is g.DictionaryFormat.FST:
        template = binary("plain", format)
        _, _, fst_offset, fst_length, readings_offset, readings_length = struct.unpack_from("<6Q", template, 16)
        header = bytearray(template[:64])
        struct.pack_into("<6Q", header, 16, 64, len(metadata), 64 + len(metadata), fst_length,
                         64 + len(metadata) + fst_length, readings_length)
        return (bytes(header) + metadata + template[fst_offset:fst_offset + fst_length]
                + template[readings_offset:readings_offset + readings_length])
    key = b"__gukhanmun_meta__"
    data = bytearray(2048)
    data.extend(struct.pack("<II", len(key), len(metadata)) + key + metadata)
    hash_value = 5381
    for byte in key:
        hash_value = ((hash_value * 33) ^ byte) & 0xffffffff
    table = len(data)
    data.extend(bytes(16))
    struct.pack_into("<II", data, table + ((hash_value >> 8) % 2) * 8, hash_value, 2048)
    struct.pack_into("<II", data, (hash_value & 0xff) * 8, table, 2)
    return bytes(data)


@pytest.mark.parametrize("format", tuple(g.DictionaryFormat))
@pytest.mark.parametrize("transport", ["bytes", "path", "url"])
def test_decoded_alias_metadata_budget_is_enforced(format: g.DictionaryFormat, transport: str, tmp_path: Path) -> None:
    # Unique tiny rows expand into nested maps and three owned strings each.
    # The wire payload is well below the default 64 MiB download limit.
    aliases = "".join(f"{index:x}\tr\tlexical\tc\n" for index in range(100_000))
    data = aliases_binary(format, aliases)
    assert len(data) < 4 * 1024 * 1024

    def reject(descriptor: g.FileDictionarySource | g.UrlDictionarySource) -> None:
        with pytest.raises(g.GukhanmunError) as caught:
            g.Converter(dictionaries=[descriptor])
        assert caught.value.code is g.ErrorCode.DICTIONARY_LOAD

    if transport == "url":
        with dictionary_server(data) as (base, paths):
            reject(g.UrlDictionarySource(base + "/binary", format))
            assert paths == ["/binary"]
    elif transport == "path":
        path = tmp_path / "metadata.data"
        path.write_bytes(data)
        reject(g.FileDictionarySource(path, format))
    else:
        reject(g.FileDictionarySource(data, format))
