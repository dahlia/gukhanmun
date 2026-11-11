# Gukhanmun: Describes the official dictionary data distributions for Python.
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

"""Private table of the official dictionary data distributions.

This module is the single description of the six resource-only distributions
under *python/*: which resources each ships, how they are encoded, which source
role they carry and where their generated artifacts come from.  The public
helpers in ``gukhanmun.dictionaries`` read it at runtime, and the staging and
packaging tools load this file by path, without importing ``gukhanmun`` and
its native extension.  Keep it free of imports outside the standard library.

Resources whose ``gzip`` flag is set are always decompressed and other
resources never are.  Unlike the JavaScript loaders, the decision does not
depend on the leading bytes: a raw CDB file starts with a little-endian table
offset that can legitimately begin with the gzip magic number.
"""

from __future__ import annotations

from typing import Final, NamedTuple


class Resource(NamedTuple):
    """One dictionary binary inside a resource package."""

    # Helper key, such as `"stdict"` or `"opendict-dialect"`.
    key: str
    # File name inside the import package.
    name: str
    # Whether the shipped file is gzip-compressed.
    gzip: bool
    # Repository-relative path of the generated artifact staged from.
    artifact: str
    # *Open Korean Dictionary* category, when applicable.
    category: str | None = None


class Distribution(NamedTuple):
    """One resource-only distribution and its packaging metadata."""

    # Distribution name, such as `"gukhanmun-stdict-fst"`.
    name: str
    # Import package name, such as `"gukhanmun_stdict_fst"`.
    package: str
    # Dictionary source: `"stdict"`, `"opendict"` or `"loanwords"`.
    dictionary: str
    # Binary format value: `"fst"` or `"cdb"`.
    format: str
    # Human-readable data name used in messages and documentation.
    title: str
    # `SourceRole` value assigned by the helpers.
    role: str
    # Core extra named in installation instructions.
    extra: str
    # PEP 639 license expression of the distribution.
    license: str
    # Repository-relative crate directory holding the source metadata.
    crate: str
    resources: tuple[Resource, ...]


_OPENDICT_CATEGORIES: Final = ("general", "north-korean", "dialect", "archaic")


def _opendict(format: str, suffix: str, gzip: bool, extra: str) -> Distribution:
    return Distribution(
        name=f"gukhanmun-opendict-{format}",
        package=f"gukhanmun_opendict_{format}",
        dictionary="opendict",
        format=format,
        title="Open Korean Dictionary",
        role="reference",
        extra=extra,
        license="GPL-3.0-or-later AND LicenseRef-CC-BY-SA-2.0-KR",
        crate="crates/gukhanmun-opendict",
        resources=tuple(
            Resource(f"opendict-{category}", f"{category}{suffix}", gzip,
                     f"packages/opendict-{format}/{category}{suffix}", category)
            for category in _OPENDICT_CATEGORIES
        ),
    )


DISTRIBUTIONS: Final[tuple[Distribution, ...]] = (
    Distribution(
        name="gukhanmun-stdict-fst",
        package="gukhanmun_stdict_fst",
        dictionary="stdict",
        format="fst",
        title="Standard Korean Language Dictionary",
        role="reference",
        extra="stdict",
        license="GPL-3.0-only AND LicenseRef-CC-BY-SA-2.0-KR",
        crate="crates/gukhanmun-stdict",
        resources=(Resource("stdict", "stdict.fst", False, "packages/stdict-fst/stdict.fst"),),
    ),
    Distribution(
        name="gukhanmun-stdict-cdb",
        package="gukhanmun_stdict_cdb",
        dictionary="stdict",
        format="cdb",
        title="Standard Korean Language Dictionary",
        role="reference",
        extra="stdict-cdb",
        license="GPL-3.0-only AND LicenseRef-CC-BY-SA-2.0-KR",
        crate="crates/gukhanmun-stdict",
        resources=(Resource("stdict", "stdict.cdb.gz", True, "packages/stdict-cdb/stdict.cdb.gz"),),
    ),
    _opendict("fst", ".fst", False, "opendict"),
    _opendict("cdb", ".cdb.gz", True, "opendict-cdb"),
    # The GPL-3.0-or-later code and editorial terms and the separate NIKL
    # free-use notice are stated in crates/gukhanmun-loanwords/ATTRIBUTION.md.
    Distribution(
        name="gukhanmun-loanwords-fst",
        package="gukhanmun_loanwords_fst",
        dictionary="loanwords",
        format="fst",
        title="NIKL loanword spelling examples",
        role="loanword",
        extra="loanwords",
        license="GPL-3.0-or-later AND LicenseRef-NIKL-Free-Use",
        crate="crates/gukhanmun-loanwords",
        resources=(Resource("loanwords", "loanwords.fst", False, "packages/loanwords-fst/loanwords.fst"),),
    ),
    Distribution(
        name="gukhanmun-loanwords-cdb",
        package="gukhanmun_loanwords_cdb",
        dictionary="loanwords",
        format="cdb",
        title="NIKL loanword spelling examples",
        role="loanword",
        extra="loanwords-cdb",
        license="GPL-3.0-or-later AND LicenseRef-NIKL-Free-Use",
        crate="crates/gukhanmun-loanwords",
        resources=(Resource("loanwords", "loanwords.cdb", False, "packages/loanwords-cdb/loanwords.cdb"),),
    ),
)
