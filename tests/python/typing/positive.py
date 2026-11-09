# Gukhanmun: Consumer typing expectations for the installed Python package.
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

from typing import assert_type

from gukhanmun import (
    Converter, DictionaryEntry, Directives, ErrorCause, ErrorCode,
    GukhanmunError, HtmlOptions, InputFormat, MapDictionarySource,
    MarkdownOptions, Preset, ResolvedOptions, SourceRole,
)

entry = DictionaryEntry(hanja="漢字", reading="한자")
dictionary = MapDictionarySource(entries=[entry], role=SourceRole.OVERRIDE)
assert_type(dictionary.entries, tuple[DictionaryEntry, ...])
assert_type(Directives(require_hanja=["漢字"]).require_hanja, tuple[str, ...])
assert_type(HtmlOptions(preserve_classes=["keep"]).preserve_classes, tuple[str, ...])
converter = Converter(preset=Preset.KO_KR, dictionaries=[dictionary])
assert_type(converter.convert("漢字"), str)
assert_type(converter.convert("漢字", format=InputFormat.HTML), str)
assert_type(converter.convert("漢字", format=MarkdownOptions(gfm=True)), str)
assert_type(converter.options, ResolvedOptions)
assert_type(converter.options.preset, Preset)
assert_type(converter.options.initial_sound_law, bool)

def inspect_error(error: GukhanmunError) -> None:
    assert_type(error.code, ErrorCode)
    assert_type(error.message, str)
    assert_type(error.chain, tuple[ErrorCause, ...])
