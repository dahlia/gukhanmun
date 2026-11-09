# Gukhanmun: Immutable Python configuration and public enum types.
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

import math
import os
from collections.abc import Sequence
from dataclasses import dataclass
from enum import Enum
from typing import TypeVar
from urllib.parse import urlsplit


class Preset(Enum):
    KO_KR = "ko-kr"
    KO_KP = "ko-kp"


class RenderMode(Enum):
    HANGUL_ONLY = "hangul-only"
    HANGUL_HANJA_PARENS = "hangul-hanja-parens"
    HANJA_HANGUL_PARENS = "hanja-hangul-parens"
    RUBY_ON_HANGUL = "ruby-on-hangul"
    RUBY_ON_HANJA = "ruby-on-hanja"
    ORIGINAL = "original"


class OriginalGloss(Enum):
    PARENS = "parens"
    RUBY = "ruby"


class HanjaVariantSet(Enum):
    AS_DICTIONARY = "as-dictionary"
    SHINJITAI = "shinjitai"
    KANXI = "kanxi"
    SIMPLIFIED = "simplified"
    ASAHIMOJI = "asahimoji"


class Segmentation(Enum):
    LATTICE = "lattice"
    EAGER = "eager"


class SourcePolicy(Enum):
    REFERENCE_AUTHORIZED = "reference-authorized"
    REFERENCE_FIRST = "reference-first"
    LOANWORD_FIRST = "loanword-first"
    ORDERED = "ordered"


class SourceRole(Enum):
    OVERRIDE = "override"
    REFERENCE = "reference"
    LOANWORD = "loanword"


class NumeralStrategy(Enum):
    HANGUL_PHONETIC = "hangul-phonetic"
    POSITIONAL_ARABIC = "positional-arabic"
    ADDITIVE_ARABIC = "additive-arabic"
    SMART = "smart"


class ContextWindow(Enum):
    OFF = "off"
    PER_BLOCK = "per-block"
    PER_SECTION = "per-section"
    PER_DOCUMENT = "per-document"


class HomophoneDetection(Enum):
    CONTEXT_LOCAL = "context-local"
    DICTIONARY_WIDE = "dictionary-wide"


class Recovery(Enum):
    STRICT = "strict"
    LENIENT = "lenient"


class PunctuationPreset(Enum):
    PRESERVE = "preserve"
    MODERN = "modern"
    TRADITIONAL = "traditional"
    TRADITIONAL_VERTICAL = "traditional-vertical"


class SpaceAfter(Enum):
    PRESERVE = "preserve"
    REMOVE = "remove"
    SINGLE = "single"


class DictionaryFormat(Enum):
    FST = "fst"
    CDB = "cdb"


class InputFormat(Enum):
    TEXT = "text"
    HTML = "html"
    MARKDOWN = "markdown"


class ErrorCode(Enum):
    DICTIONARY_LOAD = "dictionary-load"
    SEGMENTATION = "segmentation"
    INVALID_READING = "invalid-reading"
    HTML_SCAN = "html-scan"
    HTML_MALFORMED_ATTR = "html-malformed-attr"
    MARKDOWN = "markdown"
    UNSUPPORTED_CONTENT_TYPE = "unsupported-content-type"
    INVALID_INPUT = "invalid-input"
    IO = "io"
    INTERNAL = "internal"
    OTHER = "other"


class UnsetType(Enum):
    """Identity-preserving sentinel for an inherited punctuation role."""

    UNSET = "unset"


UNSET = UnsetType.UNSET
_T = TypeVar("_T")


def _require(value: object, category: type[_T], name: str) -> _T:
    if not isinstance(value, category):
        raise TypeError(f"{name} must be {category.__name__}")
    return value


def _boolean(value: object, name: str) -> bool:
    if type(value) is not bool:
        raise TypeError(f"{name} must be bool")
    return value


def _sequence(value: Sequence[_T], category: type[_T], name: str) -> tuple[_T, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        raise TypeError(f"{name} must be a sequence of {category.__name__}")
    return tuple(_require(item, category, name) for item in value)


@dataclass(frozen=True, slots=True)
class ErrorCause:
    """One underlying error; chains list the deepest cause first."""

    code: ErrorCode
    message: str

    def __post_init__(self) -> None:
        _require(self.code, ErrorCode, "code")
        _require(self.message, str, "message")


class GukhanmunError(Exception):
    """A coded conversion/configuration failure with immutable cause records."""

    def __init__(
        self, code: ErrorCode, message: str, chain: Sequence[ErrorCause] = ()
    ) -> None:
        self._code = _require(code, ErrorCode, "code")
        self._message = _require(message, str, "message")
        self._chain = _sequence(chain, ErrorCause, "chain")
        super().__init__(message)

    def __reduce__(
        self,
    ) -> tuple[type[GukhanmunError], tuple[ErrorCode, str, tuple[ErrorCause, ...]]]:
        return (type(self), (self._code, self._message, self._chain))

    @property
    def code(self) -> ErrorCode:
        return self._code

    @property
    def message(self) -> str:
        return self._message

    @property
    def chain(self) -> tuple[ErrorCause, ...]:
        return self._chain


def _invalid(message: str) -> GukhanmunError:
    return GukhanmunError(ErrorCode.INVALID_INPUT, message)


def _url(value: object) -> str:
    url = _require(value, str, "url")
    try:
        url.encode("utf-8", errors="strict")
        if any(ord(char) <= 32 or ord(char) == 127 for char in url):
            raise ValueError("URL contains whitespace or control characters")
        parsed = urlsplit(url)
        if parsed.scheme.lower() not in ("http", "https") or not parsed.hostname:
            raise ValueError("dictionary URL must use HTTP(S) and include a host")
        # Accessing port checks both non-numeric and out-of-range ports.
        _ = parsed.port
    except ValueError as error:
        raise _invalid(f"invalid dictionary URL: {error}") from error
    return url


@dataclass(frozen=True, slots=True)
class DictionaryEntry:
    """An authored reading candidate, with optional suffix reading and marks."""

    hanja: str
    reading: str
    suffix_reading: str | None = None
    preference: int = 0
    require_hanja: bool = False
    require_hangul: bool = False

    def __post_init__(self) -> None:
        _require(self.hanja, str, "hanja")
        _require(self.reading, str, "reading")
        if self.suffix_reading is not None:
            _require(self.suffix_reading, str, "suffix_reading")
        if isinstance(self.preference, bool) or not isinstance(self.preference, int):
            raise TypeError("preference must be a non-boolean int")
        _boolean(self.require_hanja, "require_hanja")
        _boolean(self.require_hangul, "require_hangul")


@dataclass(frozen=True, slots=True, init=False)
class FileDictionarySource:
    """Owned binary bytes or a filesystem path read once by Converter."""

    data: bytes | str
    format: DictionaryFormat
    role: SourceRole | None

    def __init__(
        self,
        data: bytes | bytearray | memoryview | str | os.PathLike[str],
        format: DictionaryFormat,
        role: SourceRole | None = None,
    ) -> None:
        _require(format, DictionaryFormat, "format")
        if role is not None:
            _require(role, SourceRole, "role")
        normalized: bytes | str
        if isinstance(data, memoryview):
            try:
                normalized = data.tobytes()
            except (ValueError, BufferError) as error:
                raise _invalid(f"unreadable dictionary buffer: {error}") from error
        elif isinstance(data, (bytes, bytearray)):
            normalized = bytes(data)
        elif isinstance(data, (str, os.PathLike)):
            normalized = _require(os.fspath(data), str, "filesystem path")
        else:
            raise TypeError("data must be bytes, bytearray, memoryview, str or PathLike[str]")
        object.__setattr__(self, "data", normalized)
        object.__setattr__(self, "format", format)
        object.__setattr__(self, "role", role)


@dataclass(frozen=True, slots=True)
class UrlDictionarySource:
    """An explicit HTTP(S) resource fetched once with an operation timeout."""

    url: str
    format: DictionaryFormat
    role: SourceRole | None = None
    timeout: float = 30.0

    def __post_init__(self) -> None:
        _url(self.url)
        _require(self.format, DictionaryFormat, "format")
        if self.role is not None:
            _require(self.role, SourceRole, "role")
        if isinstance(self.timeout, bool) or not isinstance(self.timeout, (int, float)):
            raise TypeError("timeout must be a non-boolean number")
        try:
            valid = math.isfinite(self.timeout) and self.timeout > 0
        except OverflowError:
            valid = False
        if not valid:
            raise _invalid("timeout must be finite and positive")


@dataclass(frozen=True, slots=True, init=False)
class MapDictionarySource:
    """An immutable snapshot of ordered, possibly competing candidates."""

    entries: tuple[DictionaryEntry, ...]
    role: SourceRole

    def __init__(
        self, entries: Sequence[DictionaryEntry], role: SourceRole = SourceRole.OVERRIDE
    ) -> None:
        object.__setattr__(self, "entries", _sequence(entries, DictionaryEntry, "entries"))
        object.__setattr__(self, "role", _require(role, SourceRole, "role"))


@dataclass(frozen=True, slots=True, init=False)
class Directives:
    """Literal per-hanja annotations, registered in the listed order."""

    require_hanja: tuple[str, ...]
    require_hangul: tuple[str, ...]
    skip_annotation: tuple[str, ...]

    def __init__(
        self,
        require_hanja: Sequence[str] = (),
        require_hangul: Sequence[str] = (),
        skip_annotation: Sequence[str] = (),
    ) -> None:
        object.__setattr__(self, "require_hanja", _sequence(require_hanja, str, "require_hanja"))
        object.__setattr__(self, "require_hangul", _sequence(require_hangul, str, "require_hangul"))
        object.__setattr__(self, "skip_annotation", _sequence(skip_annotation, str, "skip_annotation"))


@dataclass(frozen=True, slots=True)
class AttributeMatcher:
    """Match an HTML attribute's presence or its exact decoded value."""

    name: str
    value: str | None = None

    def __post_init__(self) -> None:
        _require(self.name, str, "name")
        if self.value is not None:
            _require(self.value, str, "value")


@dataclass(frozen=True, slots=True, init=False)
class HtmlOptions:
    """Additive HTML subtree preservation classes and attribute predicates."""

    preserve_classes: tuple[str, ...]
    preserve_attributes: tuple[AttributeMatcher, ...]

    def __init__(
        self,
        preserve_classes: Sequence[str] = (),
        preserve_attributes: Sequence[AttributeMatcher] = (),
    ) -> None:
        object.__setattr__(self, "preserve_classes", _sequence(preserve_classes, str, "preserve_classes"))
        object.__setattr__(self, "preserve_attributes", _sequence(preserve_attributes, AttributeMatcher, "preserve_attributes"))


@dataclass(frozen=True, slots=True)
class StopStyle:
    """An output punctuation scalar and its following-space policy."""

    mark: str
    after: SpaceAfter

    def __post_init__(self) -> None:
        _require(self.mark, str, "mark")
        _require(self.after, SpaceAfter, "after")


@dataclass(frozen=True, slots=True)
class DelimiterStyle:
    """An output opening/closing punctuation scalar pair."""

    open: str
    close: str

    def __post_init__(self) -> None:
        _require(self.open, str, "open")
        _require(self.close, str, "close")


@dataclass(frozen=True, slots=True)
class CustomPunctuationProfile:
    """UNSET inherits a role; None preserves it; a style replaces it."""

    base: PunctuationPreset = PunctuationPreset.PRESERVE
    comma: StopStyle | None | UnsetType = UNSET
    period: StopStyle | None | UnsetType = UNSET
    single: DelimiterStyle | None | UnsetType = UNSET
    double: DelimiterStyle | None | UnsetType = UNSET

    def __post_init__(self) -> None:
        _require(self.base, PunctuationPreset, "base")
        for name, value, category in (
            ("comma", self.comma, StopStyle),
            ("period", self.period, StopStyle),
            ("single", self.single, DelimiterStyle),
            ("double", self.double, DelimiterStyle),
        ):
            if value is not UNSET and value is not None:
                _require(value, category, name)


@dataclass(frozen=True, slots=True)
class ResolvedPunctuationProfile:
    """A fully resolved immutable output profile; None preserves a role."""

    comma: StopStyle | None
    period: StopStyle | None
    single: DelimiterStyle | None
    double: DelimiterStyle | None

    def __post_init__(self) -> None:
        for name, value, category in (
            ("comma", self.comma, StopStyle),
            ("period", self.period, StopStyle),
            ("single", self.single, DelimiterStyle),
            ("double", self.double, DelimiterStyle),
        ):
            if value is not None:
                _require(value, category, name)


@dataclass(frozen=True, slots=True)
class ResolvedOptions:
    """The fourteen resolved scalar settings, including the preset identity."""

    preset: Preset
    rendering: RenderMode
    hanja_variant_set: HanjaVariantSet
    segmentation: Segmentation
    source_policy: SourcePolicy
    reference_only: bool
    numerals: NumeralStrategy
    initial_sound_law: bool
    homophone_window: ContextWindow
    homophone_detection: HomophoneDetection
    first_occurrence_window: ContextWindow
    collapse_redundant_parens: bool
    recovery: Recovery
    punctuation: ResolvedPunctuationProfile

    def __post_init__(self) -> None:
        _require(self.preset, Preset, "preset")
        _require(self.rendering, RenderMode, "rendering")
        _require(self.hanja_variant_set, HanjaVariantSet, "hanja_variant_set")
        _require(self.segmentation, Segmentation, "segmentation")
        _require(self.source_policy, SourcePolicy, "source_policy")
        _boolean(self.reference_only, "reference_only")
        _require(self.numerals, NumeralStrategy, "numerals")
        _boolean(self.initial_sound_law, "initial_sound_law")
        _require(self.homophone_window, ContextWindow, "homophone_window")
        _require(self.homophone_detection, HomophoneDetection, "homophone_detection")
        _require(self.first_occurrence_window, ContextWindow, "first_occurrence_window")
        _boolean(self.collapse_redundant_parens, "collapse_redundant_parens")
        _require(self.recovery, Recovery, "recovery")
        _require(self.punctuation, ResolvedPunctuationProfile, "punctuation")


@dataclass(frozen=True, slots=True)
class MarkdownOptions:
    """Select Markdown conversion, optionally enabling GFM extensions."""

    gfm: bool = False

    def __post_init__(self) -> None:
        _boolean(self.gfm, "gfm")
