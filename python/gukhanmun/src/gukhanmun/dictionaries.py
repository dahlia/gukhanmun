# Gukhanmun: Loads the official dictionaries from the Python data packages.
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

"""Official dictionaries from the optional Python data distributions.

Each official dictionary ships in its own resource-only distribution, which an
extra of the ``gukhanmun`` distribution installs, for example
``pip install 'gukhanmun[stdict]'``.  Installing the data never changes a
converter; pass a helper's result to :class:`gukhanmun.Converter` explicitly::

    from gukhanmun import Converter
    from gukhanmun.dictionaries import stdict

    converter = Converter(dictionaries=[stdict()])

Every helper takes a keyword-only ``format`` that selects the FST (default) or
CDB distribution.  The source helpers return a :class:`FileDictionarySource`
holding owned bytes and the dictionary's source role; the ``*_bytes`` helpers
return the same bytes alone.  Compressed resources are decompressed, so the
bytes are always ready for the dictionary loader.  A missing distribution or an
unreadable resource raises :class:`GukhanmunError` with
:attr:`ErrorCode.DICTIONARY_LOAD` and the command that installs the data.
"""

from __future__ import annotations

import gzip
import importlib
import importlib.resources
import zlib

from ._api import _io_error
from ._dictionary_data import DISTRIBUTIONS, Distribution, Resource
from ._types import DictionaryFormat, ErrorCode, FileDictionarySource, GukhanmunError, SourceRole, _require

__all__ = [
    "stdict",
    "stdict_bytes",
    "opendict_general",
    "opendict_general_bytes",
    "opendict_north_korean",
    "opendict_north_korean_bytes",
    "opendict_dialect",
    "opendict_dialect_bytes",
    "opendict_archaic",
    "opendict_archaic_bytes",
    "loanwords",
    "loanwords_bytes",
]


def _locate(key: str, format: DictionaryFormat) -> tuple[Distribution, Resource]:
    _require(format, DictionaryFormat, "format")
    for distribution in DISTRIBUTIONS:
        if distribution.format == format.value:
            for resource in distribution.resources:
                if resource.key == key:
                    return distribution, resource
    raise AssertionError(f"no {format.value} resource for {key}")


def _install(distribution: Distribution) -> str:
    return f"pip install 'gukhanmun[{distribution.extra}]'"


def _reinstall(distribution: Distribution) -> str:
    # An installed but damaged distribution already satisfies the extra.
    return f"pip install --force-reinstall 'gukhanmun[{distribution.extra}]'"


def _load(key: str, format: DictionaryFormat) -> bytes:
    distribution, resource = _locate(key, format)
    data_name = f"{distribution.title} {distribution.format.upper()} data"
    try:
        module = importlib.import_module(distribution.package)
    except ModuleNotFoundError as error:
        if error.name != distribution.package:
            raise _io_error(error, f"{distribution.name} could not be imported; reinstall it with: "
                            f"{_reinstall(distribution)}") from error
        raise _io_error(error, f"{data_name} is not installed; install it with: "
                        f"{_install(distribution)}") from error
    except Exception as error:
        # A broken package can fail with any exception while importing;
        # KeyboardInterrupt and other BaseExceptions still propagate.
        raise _io_error(error, f"{distribution.name} could not be imported; reinstall it with: "
                        f"{_reinstall(distribution)}") from error
    if not hasattr(module, "__path__"):
        # files() of a plain module would read its containing directory.
        raise GukhanmunError(ErrorCode.DICTIONARY_LOAD,
                             f"{distribution.package} is a module, not the {distribution.name} package; "
                             f"reinstall it with: {_reinstall(distribution)}")
    try:
        data = importlib.resources.files(module).joinpath(resource.name).read_bytes()
    except Exception as error:
        # Resource readers differ: a zip import raises BadZipFile or
        # zlib.error, not OSError, for a damaged member.
        raise _io_error(error, f"{distribution.name} is installed but its {resource.name} resource could "
                        f"not be read; reinstall it with: {_reinstall(distribution)}") from error
    if not resource.gzip:
        # The table, not the leading bytes, decides: raw CDB files can start
        # with the gzip magic number.
        return data
    try:
        if not data:
            # gzip.decompress() accepts empty input and returns empty bytes.
            raise EOFError("the compressed resource is empty")
        return gzip.decompress(data)
    except (OSError, EOFError, zlib.error) as error:
        raise _io_error(error, f"could not decompress {resource.name} from {distribution.name}; "
                        f"reinstall it with: {_reinstall(distribution)}") from error


def _source(key: str, format: DictionaryFormat) -> FileDictionarySource:
    distribution, _ = _locate(key, format)
    return FileDictionarySource(_load(key, format), format, SourceRole(distribution.role))


def stdict(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource:
    """Load the *Standard Korean Language Dictionary* as a reference source.

    Requires the ``stdict`` (FST) or ``stdict-cdb`` extra.
    """
    return _source("stdict", format)


def stdict_bytes(*, format: DictionaryFormat = DictionaryFormat.FST) -> bytes:
    """Return the decompressed *Standard Korean Language Dictionary* binary."""
    return _load("stdict", format)


def opendict_general(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource:
    """Load the general words of the *Open Korean Dictionary* as a reference source.

    Requires the ``opendict`` (FST) or ``opendict-cdb`` extra.
    """
    return _source("opendict-general", format)


def opendict_general_bytes(*, format: DictionaryFormat = DictionaryFormat.FST) -> bytes:
    """Return the decompressed binary of the *Open Korean Dictionary* general words."""
    return _load("opendict-general", format)


def opendict_north_korean(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource:
    """Load the North Korean words of the *Open Korean Dictionary* as a reference source.

    Requires the ``opendict`` (FST) or ``opendict-cdb`` extra.
    """
    return _source("opendict-north-korean", format)


def opendict_north_korean_bytes(*, format: DictionaryFormat = DictionaryFormat.FST) -> bytes:
    """Return the decompressed binary of the *Open Korean Dictionary* North Korean words."""
    return _load("opendict-north-korean", format)


def opendict_dialect(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource:
    """Load the dialect words of the *Open Korean Dictionary* as a reference source.

    Requires the ``opendict`` (FST) or ``opendict-cdb`` extra.
    """
    return _source("opendict-dialect", format)


def opendict_dialect_bytes(*, format: DictionaryFormat = DictionaryFormat.FST) -> bytes:
    """Return the decompressed binary of the *Open Korean Dictionary* dialect words."""
    return _load("opendict-dialect", format)


def opendict_archaic(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource:
    """Load the archaic words of the *Open Korean Dictionary* as a reference source.

    Requires the ``opendict`` (FST) or ``opendict-cdb`` extra.
    """
    return _source("opendict-archaic", format)


def opendict_archaic_bytes(*, format: DictionaryFormat = DictionaryFormat.FST) -> bytes:
    """Return the decompressed binary of the *Open Korean Dictionary* archaic words."""
    return _load("opendict-archaic", format)


def loanwords(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource:
    """Load the NIKL loanword spelling examples as a loanword source.

    Requires the ``loanwords`` (FST) or ``loanwords-cdb`` extra.
    """
    return _source("loanwords", format)


def loanwords_bytes(*, format: DictionaryFormat = DictionaryFormat.FST) -> bytes:
    """Return the decompressed binary of the NIKL loanword spelling examples."""
    return _load("loanwords", format)
