# Gukhanmun: Verifies wheel isolation, Python syntax and native stub signatures.
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

import ast
from pathlib import Path
import sys

import gukhanmun


def test_imports_are_installed_in_the_wheel_environment():
    package = Path(gukhanmun.__file__).resolve().parent
    assert package.is_relative_to(Path(sys.prefix).resolve())
    assert (package / "py.typed").is_file()
    assert (package / "_native.pyi").is_file()
    assert not (Path.cwd() / "src" / "gukhanmun").exists()


def test_shipped_python_uses_python311_syntax():
    package = Path(gukhanmun.__file__).resolve().parent
    for path in package.glob("*.py"):
        ast.parse(path.read_text(encoding="utf-8"), feature_version=(3, 11))
