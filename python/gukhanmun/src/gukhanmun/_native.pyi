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

from typing import final

__all__ = ["Converter", "NativeError"]

class NativeError(Exception): ...

@final
class Converter:
    def __new__(
        cls,
        options_json: str,
        dictionaries: list[tuple[str | None, str | None, bytes | None, str | None]],
    ) -> Converter: ...
    def convert(self, source: str, format: str, gfm: bool) -> str: ...
    def options_json(self) -> str: ...
