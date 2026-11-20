# Gukhanmun: Private owned native bridge signatures.
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

from typing import Never, final

__all__ = ["Converter", "LoadedDictionary", "NativeError", "load_dictionary"]

class NativeError(Exception): ...

_RawDictionary = tuple[str | None, str | None, bytes | None, str | None]

@final
class LoadedDictionary:
    def __new__(cls, _unconstructible: Never, /) -> LoadedDictionary: ...

def load_dictionary(source: _RawDictionary) -> LoadedDictionary: ...

@final
class Converter:
    def __new__(
        cls,
        options_json: str,
        dictionaries: list[_RawDictionary | LoadedDictionary],
    ) -> Converter: ...
    def convert(self, source: str, format: str, gfm: bool) -> str: ...
    def options_json(self) -> str: ...
