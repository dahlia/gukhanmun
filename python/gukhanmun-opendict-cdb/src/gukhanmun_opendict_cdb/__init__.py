# Gukhanmun: Resource-only package with Open Korean Dictionary CDB data.
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

"""Open Korean Dictionary CDB data for ``gukhanmun.dictionaries``.

This package only ships dictionary binaries (general.cdb.gz, north-korean.cdb.gz, dialect.cdb.gz, archaic.cdb.gz) and
their *provenance.json*; it has no API and does not import ``gukhanmun``.
Load the data through ``gukhanmun.dictionaries``, or read a resource directly
with ``importlib.resources.files(__name__)``.
"""
