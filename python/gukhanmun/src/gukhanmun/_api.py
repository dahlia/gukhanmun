# Gukhanmun: Owned Python resource loading and the native converter facade.
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

import http.client
import io
import socket
import time
import json
import string
import urllib.error
import urllib.request
from collections.abc import Iterable, Iterator, Sequence
from pathlib import Path
from urllib.parse import quote, urljoin, urlsplit
from typing import IO, TypeAlias, cast

from ._native import Converter as NativeConverter
from ._native import NativeError
from ._types import (
    AttributeMatcher,
    ContextWindow,
    CustomPunctuationProfile,
    DelimiterStyle,
    DictionaryEntry,
    Directives,
    ErrorCause,
    ErrorCode,
    FileDictionarySource,
    GukhanmunError,
    HanjaVariantSet,
    HomophoneDetection,
    HtmlOptions,
    InputFormat,
    MapDictionarySource,
    MarkdownOptions,
    NumeralStrategy,
    OriginalGloss,
    Preset,
    PunctuationPreset,
    Recovery,
    RenderMode,
    ResolvedOptions,
    ResolvedPunctuationProfile,
    Segmentation,
    SourcePolicy,
    SpaceAfter,
    StopStyle,
    UnsetType,
    UrlDictionarySource,
    _boolean,
    _invalid,
    _require,
    _url,
)

_Json: TypeAlias = "None | bool | int | float | str | list[_Json] | dict[str, _Json]"
_Dictionary: TypeAlias = FileDictionarySource | UrlDictionarySource | MapDictionarySource
_NativeDictionary: TypeAlias = tuple[str | None, str | None, bytes | None, str | None]


def _text(value: object, name: str) -> str:
    text = _require(value, str, name)
    try:
        text.encode("utf-8", errors="strict")
    except UnicodeEncodeError as error:
        raise _invalid(f"{name} must contain valid Unicode: {error}") from error
    return text


def _dump(value: _Json) -> str:
    return json.dumps(value, ensure_ascii=False, allow_nan=False)


def _object(value: _Json) -> dict[str, _Json]:
    if not isinstance(value, dict):
        raise ValueError("expected a JSON object from native converter")
    return value


def _string(value: _Json) -> str:
    if not isinstance(value, str):
        raise ValueError("expected a JSON string from native converter")
    return value


def _native_error(error: NativeError) -> GukhanmunError:
    try:
        payload = _object(cast(_Json, json.loads(str(error))))
        chain = payload["chain"]
        if not isinstance(chain, list):
            raise ValueError("expected native cause array")
        causes: list[ErrorCause] = []
        for item in chain:
            cause = _object(item)
            causes.append(ErrorCause(ErrorCode(_string(cause["code"])), _string(cause["message"])))
        return GukhanmunError(
            ErrorCode(_string(payload["code"])), _string(payload["message"]), causes
        )
    except (ValueError, KeyError) as decoding_error:
        return GukhanmunError(ErrorCode.INTERNAL, f"invalid native error payload: {decoding_error}")


def _io_error(error: Exception, description: str) -> GukhanmunError:
    # URLError stores its OS cause in reason rather than __cause__.
    causes: list[ErrorCause] = []
    seen: set[int] = set()
    current: BaseException | None = error
    while current is not None and id(current) not in seen:
        seen.add(id(current))
        causes.append(ErrorCause(ErrorCode.IO, str(current)))
        if isinstance(current, urllib.error.URLError) and isinstance(current.reason, Exception):
            current = current.reason
        else:
            current = current.__cause__
    causes.reverse()
    return GukhanmunError(ErrorCode.DICTIONARY_LOAD, description, causes)


def _url_origin(url: str) -> tuple[str, str, int]:
    parsed = urlsplit(_url(url))
    scheme = parsed.scheme.lower()
    port = parsed.port
    return (scheme, cast(str, parsed.hostname),
            port if port is not None else (443 if scheme == "https" else 80))


class _HttpRedirects(urllib.request.HTTPRedirectHandler):
    def __init__(self, initial_url: str) -> None:
        self.origin = _url_origin(initial_url)

    def _validate_target(self, target: str) -> None:
        if _url_origin(target) != self.origin:
            raise _invalid("dictionary URL redirects must stay on the configured origin")

    def redirect_request(
        self,
        req: urllib.request.Request,
        fp: IO[bytes],
        code: int,
        msg: str,
        headers: http.client.HTTPMessage,
        newurl: str,
    ) -> urllib.request.Request | None:
        # Validate before the parent creates or dispatches the next request.
        try:
            self._validate_target(newurl)
            return super().redirect_request(req, fp, code, msg, headers, newurl)
        finally:
            # urllib otherwise drains the redirect body with an unbounded read.
            fp.close()


    def http_error_302(
        self,
        req: urllib.request.Request,
        fp: IO[bytes],
        code: int,
        msg: str,
        headers: http.client.HTTPMessage,
    ) -> http.client.HTTPResponse | None:
        # urllib rejects some schemes before redirect_request; check the
        # original Location first so every forbidden target is invalid-input.
        location = headers.get("location", headers.get("uri"))
        if location is not None:
            # Match urllib's recovery of HTTP header bytes and quoting before
            # checking the target. Spaces in Location are normalized by urllib.
            try:
                target = quote(location, encoding="iso-8859-1", safe=string.punctuation)
                self._validate_target(urljoin(req.full_url, target))
            except (GukhanmunError, ValueError):
                fp.close()
                raise
        return cast(http.client.HTTPResponse | None, super().http_error_302(req, fp, code, msg, headers))

    http_error_301 = http_error_302
    http_error_303 = http_error_302
    http_error_307 = http_error_302
    http_error_308 = http_error_302


class _DownloadBudget:
    def __init__(self, timeout: float, total_timeout: float) -> None:
        self.timeout = timeout
        self.deadline = time.monotonic() + total_timeout

    def remaining(self) -> float:
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("dictionary download exceeded total_timeout")
        return min(self.timeout, remaining)


class _DeadlineReader(io.RawIOBase):
    """Reapply the shared deadline for each underlying socket receive."""

    def __init__(self, stream: io.BufferedReader, sock: socket.socket, budget: _DownloadBudget) -> None:
        self.stream = stream
        self.sock = sock
        self.budget = budget

    def readable(self) -> bool:
        return True

    def readinto(self, buffer: object) -> int:
        view = memoryview(cast(bytearray, buffer))
        self.sock.settimeout(self.budget.remaining())
        # read1 performs at most one raw read, even inside header/chunk parsing.
        data = self.stream.read1(len(view))
        self.budget.remaining()
        view[:len(data)] = data
        return len(data)

    def close(self) -> None:
        try:
            self.stream.close()
        finally:
            super().close()


def _fetch(source: UrlDictionarySource) -> bytes:
    budget = _DownloadBudget(source.timeout, source.total_timeout)

    class Response(http.client.HTTPResponse):
        def __init__(self, sock: socket.socket, debuglevel: int = 0,
                     method: str | None = None, url: str | None = None) -> None:
            super().__init__(sock, debuglevel=debuglevel, method=method, url=url)
            # Keep the original socket file alive when urllib closes its
            # connection socket. Wrap before begin() reads status and headers.
            self.fp = io.BufferedReader(_DeadlineReader(self.fp, sock, budget))

    class Http(urllib.request.HTTPHandler):
        def http_open(self, req: urllib.request.Request) -> http.client.HTTPResponse:
            def connect(host: str, /, *, port: int | None = None, timeout: float = 30.0,
                        source_address: tuple[str, int] | None = None, blocksize: int = 8192) -> http.client.HTTPConnection:
                connection = http.client.HTTPConnection(host, port, timeout=budget.remaining(),
                                                        source_address=source_address, blocksize=blocksize)
                connection.response_class = Response
                return connection
            return self.do_open(connect, req)

    class Https(urllib.request.HTTPSHandler):
        def https_open(self, req: urllib.request.Request) -> http.client.HTTPResponse:
            def connect(host: str, /, *, port: int | None = None, timeout: float = 30.0,
                        source_address: tuple[str, int] | None = None, blocksize: int = 8192) -> http.client.HTTPSConnection:
                connection = http.client.HTTPSConnection(host, port, timeout=budget.remaining(),
                                                         source_address=source_address, blocksize=blocksize)
                connection.response_class = Response
                return connection
            return self.do_open(connect, req)

    # No file/data/FTP handlers, proxies, or automatic redirect-body draining.
    opener = urllib.request.OpenerDirector()
    for handler in (
        urllib.request.ProxyHandler({}), Http(), Https(),
        urllib.request.HTTPDefaultErrorHandler(), _HttpRedirects(source.url),
        urllib.request.HTTPErrorProcessor(),
    ):
        opener.add_handler(handler)
    try:
        # Validate platform timeout range even if the total budget is shorter.
        with socket.socket() as probe:
            probe.settimeout(source.timeout)
        with cast(http.client.HTTPResponse, opener.open(source.url, timeout=source.timeout)) as response:
            data = bytearray()
            while True:
                budget.remaining()
                chunk = response.read1(min(64 * 1024, source.max_bytes + 1 - len(data)))
                budget.remaining()
                if not chunk:
                    if response.length:
                        raise http.client.IncompleteRead(b"", response.length)
                    return bytes(data)
                data.extend(chunk)
                if len(data) > source.max_bytes:
                    raise OSError("dictionary download exceeded max_bytes")
    except GukhanmunError:
        raise
    except UnicodeEncodeError as error:
        raise _invalid(f"invalid dictionary URL: {error}") from error
    except (ValueError, OverflowError) as error:
        raise _invalid(f"invalid dictionary URL: {error}") from error
    except (OSError, http.client.HTTPException) as error:
        if isinstance(error, urllib.error.HTTPError):
            error.close()
        raise _io_error(error, f"could not load dictionary URL {source.url!r}") from error


def _entry(value: DictionaryEntry, index: int) -> dict[str, _Json]:
    # Reconstruct records so a descriptor altered via low-level mutation is
    # checked again at the boundary, before any native work starts.
    entry = DictionaryEntry(
        value.hanja, value.reading, value.suffix_reading, value.preference,
        value.require_hanja, value.require_hangul,
    )
    label = f"dictionary entry {index}"
    if not entry.hanja or not entry.reading:
        raise _invalid(f"{label}: hanja and reading must be nonempty")
    if entry.suffix_reading == "":
        raise _invalid(f"{label}: suffix_reading must be nonempty when supplied")
    if not 0 <= entry.preference <= 2**32 - 1:
        raise _invalid(f"{label}: preference must fit a Rust u32")
    return {
        "hanja": _text(entry.hanja, f"{label}.hanja"),
        "reading": _text(entry.reading, f"{label}.reading"),
        "suffix_reading": None if entry.suffix_reading is None else _text(entry.suffix_reading, f"{label}.suffix_reading"),
        "preference": entry.preference,
        "require_hanja": entry.require_hanja,
        "require_hangul": entry.require_hangul,
    }


def _dictionaries(values: Sequence[_Dictionary]) -> list[_NativeDictionary]:
    if isinstance(values, (str, bytes, bytearray)) or not isinstance(values, Sequence):
        raise TypeError("dictionaries must be a sequence of dictionary descriptors")
    result: list[_NativeDictionary] = []
    for value in tuple(values):
        if isinstance(value, MapDictionarySource):
            source = MapDictionarySource(value.entries, value.role)
            entries: list[_Json] = [_entry(entry, index) for index, entry in enumerate(source.entries)]
            result.append((source.role.value, None, None, _dump(entries)))
        elif isinstance(value, FileDictionarySource):
            file = FileDictionarySource(value.data, value.format, value.role)
            if isinstance(file.data, bytes):
                data = file.data
            else:
                try:
                    data = Path(file.data).read_bytes()
                except (OSError, ValueError) as error:
                    raise _io_error(error, f"could not load dictionary file {file.data!r}") from error
            result.append((None if file.role is None else file.role.value, file.format.value, data, None))
        elif isinstance(value, UrlDictionarySource):
            url = UrlDictionarySource(value.url, value.format, value.role, value.timeout,
                                      value.max_bytes, value.total_timeout)
            result.append((None if url.role is None else url.role.value, url.format.value, _fetch(url), None))
        else:
            raise TypeError("dictionaries must contain dictionary descriptors")
    return result


def _stop(value: StopStyle) -> dict[str, _Json]:
    style = StopStyle(value.mark, value.after)
    return {"mark": _text(style.mark, "punctuation mark"), "after": style.after.value}


def _delimiter(value: DelimiterStyle) -> dict[str, _Json]:
    style = DelimiterStyle(value.open, value.close)
    return {"open": _text(style.open, "opening delimiter"), "close": _text(style.close, "closing delimiter")}


def _punctuation(value: PunctuationPreset | CustomPunctuationProfile) -> _Json:
    if isinstance(value, PunctuationPreset):
        return value.value
    _require(value, CustomPunctuationProfile, "punctuation")
    profile = CustomPunctuationProfile(value.base, value.comma, value.period, value.single, value.double)
    result: dict[str, _Json] = {"base": profile.base.value}
    for name, stop in (("comma", profile.comma), ("period", profile.period)):
        if isinstance(stop, UnsetType):
            continue
        result[name] = None if stop is None else _stop(stop)
    for name, delimiter in (("single", profile.single), ("double", profile.double)):
        if isinstance(delimiter, UnsetType):
            continue
        result[name] = None if delimiter is None else _delimiter(delimiter)
    return result


def _directives(value: Directives | None) -> _Json:
    if value is None:
        return None
    _require(value, Directives, "directives")
    directives = Directives(value.require_hanja, value.require_hangul, value.skip_annotation)
    return {
        name: [_text(item, f"directives.{name}") for item in items]
        for name, items in (
            ("require_hanja", directives.require_hanja),
            ("require_hangul", directives.require_hangul),
            ("skip_annotation", directives.skip_annotation),
        )
    }


def _html(value: HtmlOptions | None) -> _Json:
    if value is None:
        return None
    _require(value, HtmlOptions, "html")
    html = HtmlOptions(value.preserve_classes, value.preserve_attributes)
    attributes: list[_Json] = []
    for item in html.preserve_attributes:
        matcher = AttributeMatcher(item.name, item.value)
        attributes.append({
            "name": _text(matcher.name, "attribute name"),
            "value": None if matcher.value is None else _text(matcher.value, "attribute value"),
        })
    return {
        "preserve_classes": [_text(item, "preserve class") for item in html.preserve_classes],
        "preserve_attributes": attributes,
    }


def _resolved_stop(value: _Json) -> StopStyle | None:
    if value is None:
        return None
    style = _object(value)
    return StopStyle(_string(style["mark"]), SpaceAfter(_string(style["after"])))


def _resolved_delimiter(value: _Json) -> DelimiterStyle | None:
    if value is None:
        return None
    style = _object(value)
    return DelimiterStyle(_string(style["open"]), _string(style["close"]))


def _resolved(payload: str) -> ResolvedOptions:
    options = _object(cast(_Json, json.loads(payload)))
    profile = _object(options["punctuation"])
    return ResolvedOptions(
        preset=Preset(_string(options["preset"])),
        rendering=RenderMode(_string(options["rendering"])),
        hanja_variant_set=HanjaVariantSet(_string(options["hanja_variant_set"])),
        segmentation=Segmentation(_string(options["segmentation"])),
        source_policy=SourcePolicy(_string(options["source_policy"])),
        reference_only=_boolean(options["reference_only"], "reference_only"),
        numerals=NumeralStrategy(_string(options["numerals"])),
        initial_sound_law=_boolean(options["initial_sound_law"], "initial_sound_law"),
        homophone_window=ContextWindow(_string(options["homophone_window"])),
        homophone_detection=HomophoneDetection(_string(options["homophone_detection"])),
        first_occurrence_window=ContextWindow(_string(options["first_occurrence_window"])),
        collapse_redundant_parens=_boolean(options["collapse_redundant_parens"], "collapse_redundant_parens"),
        recovery=Recovery(_string(options["recovery"])),
        punctuation=ResolvedPunctuationProfile(
            _resolved_stop(profile["comma"]), _resolved_stop(profile["period"]),
            _resolved_delimiter(profile["single"]), _resolved_delimiter(profile["double"]),
        ),
    )


def _format(value: InputFormat | MarkdownOptions) -> tuple[str, bool]:
    if isinstance(value, MarkdownOptions):
        return InputFormat.MARKDOWN.value, _boolean(value.gfm, "gfm")
    return _require(value, InputFormat, "format").value, False


class Converter:
    """An immutable, reusable converter with exclusively owned native state.

    Dictionary resources are loaded during construction. Concurrent calls
    create independent documents. Native work releases interpreter attachment;
    Python signals are checked after it completes, rather than mid-conversion.
    """

    __slots__: tuple[str, ...] = ("_handle", "_options")
    _handle: NativeConverter
    _options: ResolvedOptions

    def __init__(
        self,
        *,
        preset: Preset = Preset.KO_KR,
        rendering: RenderMode = RenderMode.HANGUL_ONLY,
        original_gloss: OriginalGloss = OriginalGloss.PARENS,
        hanja_variant_set: HanjaVariantSet = HanjaVariantSet.AS_DICTIONARY,
        segmentation: Segmentation = Segmentation.LATTICE,
        source_policy: SourcePolicy = SourcePolicy.REFERENCE_AUTHORIZED,
        reference_only: bool = False,
        numerals: NumeralStrategy = NumeralStrategy.HANGUL_PHONETIC,
        initial_sound_law: bool | None = None,
        homophone_window: ContextWindow | None = None,
        homophone_detection: HomophoneDetection = HomophoneDetection.CONTEXT_LOCAL,
        first_occurrence_window: ContextWindow = ContextWindow.OFF,
        collapse_redundant_parens: bool = True,
        recovery: Recovery = Recovery.STRICT,
        punctuation: PunctuationPreset | CustomPunctuationProfile = PunctuationPreset.PRESERVE,
        dictionaries: Sequence[_Dictionary] = (),
        directives: Directives | None = None,
        html: HtmlOptions | None = None,
    ) -> None:
        if hasattr(self, "_handle"):
            raise RuntimeError("Converter is already initialized")
        options: dict[str, _Json] = {
            "preset": _require(preset, Preset, "preset").value,
            "rendering": _require(rendering, RenderMode, "rendering").value,
            "original_gloss": _require(original_gloss, OriginalGloss, "original_gloss").value,
            "hanja_variant_set": _require(hanja_variant_set, HanjaVariantSet, "hanja_variant_set").value,
            "segmentation": _require(segmentation, Segmentation, "segmentation").value,
            "source_policy": _require(source_policy, SourcePolicy, "source_policy").value,
            "reference_only": _boolean(reference_only, "reference_only"),
            "numerals": _require(numerals, NumeralStrategy, "numerals").value,
            "initial_sound_law": None if initial_sound_law is None else _boolean(initial_sound_law, "initial_sound_law"),
            "homophone_window": None if homophone_window is None else _require(homophone_window, ContextWindow, "homophone_window").value,
            "homophone_detection": _require(homophone_detection, HomophoneDetection, "homophone_detection").value,
            "first_occurrence_window": _require(first_occurrence_window, ContextWindow, "first_occurrence_window").value,
            "collapse_redundant_parens": _boolean(collapse_redundant_parens, "collapse_redundant_parens"),
            "recovery": _require(recovery, Recovery, "recovery").value,
            "punctuation": _punctuation(punctuation),
            "directives": _directives(directives),
            "html": _html(html),
        }
        sources = _dictionaries(dictionaries)
        try:
            handle = NativeConverter(_dump(options), sources)
            resolved = _resolved(handle.options_json())
        except NativeError as error:
            raise _native_error(error) from error
        self._handle = handle
        self._options = resolved

    def __setattr__(self, name: str, value: object) -> None:
        if name in self.__slots__ and not hasattr(self, name):
            object.__setattr__(self, name, value)
        else:
            raise AttributeError("Converter configuration is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("Converter configuration is immutable")

    @property
    def options(self) -> ResolvedOptions:
        """The frozen, actually resolved configuration reported by Rust."""
        return self._options

    def convert(
        self, source: str, *, format: InputFormat | MarkdownOptions = InputFormat.TEXT
    ) -> str:
        """Convert one complete document; source must contain valid Unicode."""
        _require(source, str, "source")
        # Source encoding failures deliberately remain UnicodeEncodeError.
        source.encode("utf-8", errors="strict")
        selector, gfm = _format(format)
        try:
            return self._handle.convert(source, selector, gfm)
        except NativeError as error:
            raise _native_error(error) from error

    def stream(
        self, *, format: InputFormat | MarkdownOptions = InputFormat.TEXT
    ) -> ConversionStream:
        """Buffer one document in a single-consumer stream retaining this converter."""
        return ConversionStream(self, format=format)

    def convert_iter(
        self,
        chunks: Iterable[str],
        *,
        format: InputFormat | MarkdownOptions = InputFormat.TEXT,
    ) -> Iterator[str]:
        """Convert chunks, yielding nonempty results after normal input exhaustion.

        Format validation is immediate. Upstream exceptions and early closing
        propagate without finalizing a partial document.
        """
        stream = self.stream(format=format)

        def results() -> Iterator[str]:
            completed = False
            try:
                for chunk in chunks:
                    output = stream.push(chunk)
                    if output:
                        yield output
                output = stream.finish()
                completed = True
                if output:
                    yield output
            finally:
                if not completed:
                    stream._abort()

        return results()


class ConversionStream:
    """A mutable, single-consumer, buffered conversion of one document.

    Successful pushes return an empty string. Finish converts all input once;
    failure or successful completion makes the stream permanently terminal.
    The stream strongly retains its converter, including after completion.
    """

    __slots__: tuple[str, ...] = ("_converter", "_format", "_chunks", "_terminal")

    def __init__(
        self,
        converter: Converter,
        *,
        format: InputFormat | MarkdownOptions = InputFormat.TEXT,
    ) -> None:
        if hasattr(self, "_converter"):
            raise RuntimeError("ConversionStream is already initialized")
        selector, gfm = _format(format)
        self._converter: Converter = _require(converter, Converter, "converter")
        self._format: InputFormat | MarkdownOptions = (
            MarkdownOptions(gfm=gfm)
            if selector == InputFormat.MARKDOWN.value
            else InputFormat(selector)
        )
        self._chunks: list[str] = []
        self._terminal: bool = False

    def _abort(self) -> None:
        self._terminal = True
        self._chunks.clear()

    def push(self, source: str) -> str:
        """Buffer valid Unicode text; any failure permanently closes the stream."""
        if self._terminal:
            raise RuntimeError("ConversionStream is terminal")
        accepted = False
        try:
            _require(source, str, "source")
            source.encode("utf-8", errors="strict")
            self._chunks.append(source)
            accepted = True
            return ""
        finally:
            if not accepted:
                self._abort()

    def finish(self) -> str:
        """Convert buffered text once; this call is terminal even if conversion fails."""
        if self._terminal:
            raise RuntimeError("ConversionStream is terminal")
        self._terminal = True
        try:
            source = "".join(self._chunks)
        finally:
            self._chunks.clear()
        return self._converter.convert(source, format=self._format)
