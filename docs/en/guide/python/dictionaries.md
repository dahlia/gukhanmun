---
title: Official dictionaries
description: Install the Standard Korean Language Dictionary, Open Korean Dictionary and NIKL loanword data for Python through extras, and activate them explicitly with gukhanmun.dictionaries.
---

Official dictionaries
=====================

The official dictionaries ship as optional data distributions, separate from
the `gukhanmun` package. An extra installs the data, and a function from
`gukhanmun.dictionaries` adds it to a converter. Installing data never changes
a converter on its own, and neither preset loads it.

~~~~ python
from gukhanmun import Converter
from gukhanmun.dictionaries import stdict

converter = Converter(dictionaries=[stdict()])

assert converter.convert("無條件降服") == "무조건항복"
~~~~


Install the data
----------------

Each dictionary has an extra that installs its data at exactly the same
version as `gukhanmun`:

| Extra                        | Distribution              | Data                                                        |
| ---------------------------- | ------------------------- | ----------------------------------------------------------- |
| `stdict`, `stdict-fst`       | `gukhanmun-stdict-fst`    | *Standard Korean Language Dictionary* (標準國語大辭典), FST |
| `stdict-cdb`                 | `gukhanmun-stdict-cdb`    | *Standard Korean Language Dictionary*, CDB                  |
| `opendict`, `opendict-fst`   | `gukhanmun-opendict-fst`  | *Open Korean Dictionary* (우리말샘), FST                    |
| `opendict-cdb`               | `gukhanmun-opendict-cdb`  | *Open Korean Dictionary*, CDB                               |
| `loanwords`, `loanwords-fst` | `gukhanmun-loanwords-fst` | NIKL loanword spelling examples, FST                        |
| `loanwords-cdb`              | `gukhanmun-loanwords-cdb` | NIKL loanword spelling examples, CDB                        |

~~~~ sh
pip install 'gukhanmun[stdict,loanwords]'
~~~~

The data distributions are platform-independent wheels without dependencies.
They do not import `gukhanmun`, so `gukhanmun` installed alone stays free of
official data.

The packages are not on PyPI yet. Until they are, build them from a checkout of
the repository and install from the local artifacts:

~~~~ sh
mise run py-build
mise run py-dicts-build
pip install --find-links target/python/wheels \
            --find-links target/python/dictionaries/gukhanmun-stdict-fst \
            'gukhanmun[stdict]==0.4.0'
~~~~

Replace `0.4.0` with the version you built. *target/python/wheels/* holds a
`gukhanmun` wheel for the building platform only; add one `--find-links` for
each data distribution you install.


Add dictionaries to a converter
-------------------------------

`gukhanmun.dictionaries` has one function per dictionary:

| Function                  | Data                                         | Source role |
| ------------------------- | -------------------------------------------- | ----------- |
| `stdict()`                | *Standard Korean Language Dictionary*        | `REFERENCE` |
| `opendict_general()`      | *Open Korean Dictionary*, general words      | `REFERENCE` |
| `opendict_north_korean()` | *Open Korean Dictionary*, North Korean words | `REFERENCE` |
| `opendict_dialect()`      | *Open Korean Dictionary*, dialect words      | `REFERENCE` |
| `opendict_archaic()`      | *Open Korean Dictionary*, archaic words      | `REFERENCE` |
| `loanwords()`             | NIKL loanword spelling examples              | `LOANWORD`  |

Each returns a `FileDictionarySource` holding the dictionary bytes, the format
and the source role. The four *Open Korean Dictionary* categories are separate
dictionaries, so add only the ones you need:

~~~~ python
from gukhanmun import Converter, Preset
from gukhanmun.dictionaries import opendict_general, opendict_north_korean

converter = Converter(
    preset=Preset.KO_KP,
    dictionaries=[opendict_north_korean(), opendict_general()],
)
~~~~

Source roles decide how dictionaries cooperate. Under the default
`SourcePolicy.REFERENCE_AUTHORIZED`, a loanword reading that the reference
dictionaries attest wins regardless of order, unless a source with the
`OVERRIDE` role supplies the same entry:

~~~~ python
from gukhanmun.dictionaries import loanwords, stdict

converter = Converter(dictionaries=[stdict(), loanwords()])
assert converter.convert("香港") == "홍콩"
~~~~

See the [API reference](../../api/python.md#dictionary-sources-and-entries) for
the other source policies and `reference_only`.

A converter reads the data once when it is constructed. Reuse the converter
rather than calling the functions for every document.


Choose a format
---------------

Every function takes a keyword-only `format`, `DictionaryFormat.FST` by
default:

~~~~ python
from gukhanmun import DictionaryFormat
from gukhanmun.dictionaries import stdict

converter = Converter(dictionaries=[stdict(format=DictionaryFormat.CDB)])
~~~~

Both formats contain the same data and produce the same conversions. FST data
is smaller; CDB has constant-time lookups. Install the extra of the format you
use.

The CDB distributions of the *Standard Korean Language Dictionary* and the
*Open Korean Dictionary* store their files gzip-compressed. The functions
decompress them, so the bytes are always ready for the dictionary loader.


Read the bytes
--------------

Each function has a `*_bytes()` counterpart, such as `stdict_bytes()`, that
returns the decompressed dictionary bytes alone, for example to cache them or
to pass a different role:

~~~~ python
from gukhanmun import DictionaryFormat, FileDictionarySource, SourceRole
from gukhanmun.dictionaries import stdict_bytes

source = FileDictionarySource(stdict_bytes(), DictionaryFormat.FST, SourceRole.OVERRIDE)
~~~~

The packaged files are also available through `importlib.resources`. Each
distribution's import package has the dictionary files and a *provenance.json*
that records the source snapshot and the hashes of the files:

~~~~ python
from importlib.resources import files

data = (files("gukhanmun_stdict_cdb") / "stdict.cdb.gz").read_bytes()
~~~~

Read this way, *.gz* files are still compressed.


Handle missing data
-------------------

When the data distribution is not installed, the function raises
`GukhanmunError` with `ErrorCode.DICTIONARY_LOAD`. The message names the extra
that installs it:

~~~~ python
from gukhanmun import ErrorCode, GukhanmunError
from gukhanmun.dictionaries import stdict

try:
    source = stdict()
except GukhanmunError as error:
    assert error.code is ErrorCode.DICTIONARY_LOAD
    print(error.message)
    # Standard Korean Language Dictionary FST data is not installed;
    # install it with: pip install 'gukhanmun[stdict]'
~~~~

Other formats stay usable when one is missing. If an installed distribution is
damaged, for example a file is missing or no longer matches the size and hash
recorded in *provenance.json*, the error suggests reinstalling it with
`pip install --force-reinstall`, and `error.chain` keeps any underlying cause.

The suggested commands install from PyPI. Until the packages are published
there, add the `--find-links` options from
[*Install the data*](#install-the-data), for example:

~~~~ sh
pip install --force-reinstall \
            --find-links target/python/wheels \
            --find-links target/python/dictionaries/gukhanmun-stdict-fst \
            'gukhanmun[stdict]==0.4.0'
~~~~


Licenses
--------

The data distributions carry the licenses of their data:

 -  *Standard Korean Language Dictionary* and *Open Korean Dictionary* data:
    Creative Commons Attribution-ShareAlike 2.0 Korea (CC BY-SA 2.0 KR).
 -  NIKL loanword spelling examples: the National Institute of Korean
    Language's free-use terms.

Each distribution includes *LICENSE* and *ATTRIBUTION.md* with the attribution
and code license. Keep them when you redistribute the data.
