# Gukhanmun: Refuses source builds of the standalone binary distribution.
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

"""Only the explicit, verified-payload assembler can produce this package."""


def refuse(*args, **kwargs):
    raise RuntimeError(
        "gukhanmun-cli supports prebuilt wheels only; source, editable and "
        "metadata builds are disabled. Contributors: use mise run py-cli-assemble "
        "with an explicitly built native executable and completion bundle."
    )


build_wheel = refuse
build_sdist = refuse
build_editable = refuse
prepare_metadata_for_build_wheel = refuse
prepare_metadata_for_build_editable = refuse
get_requires_for_build_wheel = refuse
get_requires_for_build_sdist = refuse
get_requires_for_build_editable = refuse
