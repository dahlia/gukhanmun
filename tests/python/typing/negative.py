# Gukhanmun: Deliberately invalid consumers, checked without ignore directives.
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

from gukhanmun import (
    Converter, CustomPunctuationProfile, DictionaryEntry, DictionaryFormat, ErrorCode, GukhanmunError, Preset, RenderMode,
)
from gukhanmun.dictionaries import stdict, stdict_bytes

Converter(preset="ko-kr")  # expected: enum
Converter(preset=RenderMode.HANGUL_ONLY)  # expected: enum
DictionaryEntry(reading="한자")  # expected: required
DictionaryEntry(hanja="漢字")  # expected: required
Converter().convert("漢字", format=Preset.KO_KR)  # expected: enum
converter = Converter()
converter.options = converter.options  # expected: immutable
converter.options.initial_sound_law = False  # expected: immutable
entry = DictionaryEntry(hanja="漢字", reading="한자")
entry.reading = "다름"  # expected: immutable
converter.stream(format="html")  # expected: enum
converter.stream().push(b"text")  # expected: enum
chunks: list[bytes] = []
converter.convert_iter(chunks)  # expected: enum
CustomPunctuationProfile(comma=object())  # expected: enum
DictionaryEntry("漢字", "한자", preference=1.5)  # expected: enum
error = GukhanmunError(ErrorCode.INVALID_INPUT, "invalid")
error.code = ErrorCode.IO  # expected: immutable
stdict(format="cdb")  # expected: enum
stdict_bytes(format=Preset.KO_KR)  # expected: enum
stdict(DictionaryFormat.FST)  # expected: keyword
