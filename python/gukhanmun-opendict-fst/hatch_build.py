# Gukhanmun: Refuses to build a data distribution without its license files.
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

"""Hatchling skips missing ``license-files`` entries silently, so a wheel
built directly from an incompletely staged project would ship without the
data's license and attribution.  This hook ships in the sdist as well."""

from pathlib import Path

from hatchling.builders.hooks.plugin.interface import BuildHookInterface

REQUIRED = ("LICENSE", "ATTRIBUTION.md")


class CustomBuildHook(BuildHookInterface):
    def initialize(self, version, build_data):
        missing = [name for name in REQUIRED if not (Path(self.root) / name).is_file()]
        if missing:
            raise RuntimeError(f"missing {', '.join(missing)}; run `mise run py-dicts-stage` first")
