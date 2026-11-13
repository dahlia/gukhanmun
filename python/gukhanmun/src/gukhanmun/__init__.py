# Gukhanmun: Public Python converter API.
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

from importlib.metadata import version as _distribution_version

from ._api import ConversionStream, Converter as Converter
from ._types import (
    Preset as Preset,
    RenderMode as RenderMode,
    OriginalGloss as OriginalGloss,
    HanjaVariantSet as HanjaVariantSet,
    Segmentation as Segmentation,
    SourcePolicy as SourcePolicy,
    SourceRole as SourceRole,
    NumeralStrategy as NumeralStrategy,
    ContextWindow as ContextWindow,
    HomophoneDetection as HomophoneDetection,
    Recovery as Recovery,
    PunctuationPreset as PunctuationPreset,
    SpaceAfter as SpaceAfter,
    DictionaryFormat as DictionaryFormat,
    InputFormat as InputFormat,
    ErrorCode as ErrorCode,
    UnsetType as UnsetType,
    UNSET as UNSET,
    DictionaryEntry as DictionaryEntry,
    FileDictionarySource as FileDictionarySource,
    UrlDictionarySource as UrlDictionarySource,
    MapDictionarySource as MapDictionarySource,
    Directives as Directives,
    HtmlOptions as HtmlOptions,
    AttributeMatcher as AttributeMatcher,
    StopStyle as StopStyle,
    DelimiterStyle as DelimiterStyle,
    CustomPunctuationProfile as CustomPunctuationProfile,
    ResolvedPunctuationProfile as ResolvedPunctuationProfile,
    ResolvedOptions as ResolvedOptions,
    MarkdownOptions as MarkdownOptions,
    ErrorCause as ErrorCause,
    GukhanmunError as GukhanmunError,
)

__version__: str = _distribution_version("gukhanmun")

__all__ = [
    "__version__",
    "Converter",
    "ConversionStream",
    "Preset",
    "RenderMode",
    "OriginalGloss",
    "HanjaVariantSet",
    "Segmentation",
    "SourcePolicy",
    "SourceRole",
    "NumeralStrategy",
    "ContextWindow",
    "HomophoneDetection",
    "Recovery",
    "PunctuationPreset",
    "SpaceAfter",
    "DictionaryFormat",
    "InputFormat",
    "ErrorCode",
    "UnsetType",
    "UNSET",
    "DictionaryEntry",
    "FileDictionarySource",
    "UrlDictionarySource",
    "MapDictionarySource",
    "Directives",
    "HtmlOptions",
    "AttributeMatcher",
    "StopStyle",
    "DelimiterStyle",
    "CustomPunctuationProfile",
    "ResolvedPunctuationProfile",
    "ResolvedOptions",
    "MarkdownOptions",
    "ErrorCause",
    "GukhanmunError",
]
