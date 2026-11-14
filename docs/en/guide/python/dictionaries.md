---
title: Dictionaries
description: Load official dictionaries, runtime entries and compiled FST/CDB sources in Python, and control how their roles select readings.
---

Dictionaries
============

Use runtime entries for application vocabulary, or load compiled FST/CDB
sources. The official dictionaries ship as optional data distributions,
separate from the `gukhanmun` package. An extra installs the data, and a
function from `gukhanmun.dictionaries` adds it to a converter. Installing data
never changes a converter on its own, and neither preset loads it.

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

The data distributions are platform-independent wheels without dependencies.
They do not import `gukhanmun`, so `gukhanmun` installed alone stays free of
official data.

Install the extras from PyPI as shown in
[*Installation*](./install.mdx#official-dictionaries), for example
`gukhanmun[stdict,loanwords]`. The installation guide has pip, uv and Poetry
commands, plus separate instructions for prereleases and local builds.


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

Source roles decide how dictionaries cooperate. The helpers give each source
its role, so under the default `SourcePolicy.REFERENCE_AUTHORIZED` putting the
loanword source before or after the reference source does not change which one
takes priority; here the reference dictionary attests the loanword reading:

~~~~ python
from gukhanmun.dictionaries import loanwords, stdict

converter = Converter(dictionaries=[stdict(), loanwords()])
assert converter.convert("香港") == "홍콩"
~~~~

Among several reference sources, such as *Open Korean Dictionary* categories,
registration order still decides between their competing readings. See the
[API reference](../../api/python.md#dictionary-sources-and-entries) for
`OVERRIDE` sources, the other source policies and `reference_only`.

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

The suggested commands use PyPI. For another package manager or a local build,
follow [*Installation*](./install.mdx).


Runtime entries
---------------

Use `MapDictionarySource` for application vocabulary. Its entry sequence keeps
repeated hanja forms as competing readings, rather than collapsing them into
one mapping:

~~~~ python
from gukhanmun import ContextWindow, Converter, DictionaryEntry, MapDictionarySource

source = MapDictionarySource([
    DictionaryEntry("東京", "동경"),
    DictionaryEntry("東京", "도쿄", preference=10),
    DictionaryEntry("年代", "연대", suffix_reading="년대"),
    DictionaryEntry("庫間", "곳간", require_hanja=True),
])
converter = Converter(dictionaries=[source], homophone_window=ContextWindow.OFF)
assert converter.convert("東京") == "도쿄"
assert converter.convert("東京을") == "동경을"
assert converter.convert("東京를") == "도쿄를"
assert converter.convert("1990年代 年代") == "1990년대 연대"
assert converter.convert("庫間") == "곳간(庫間)"
~~~~

A higher `preference` wins by default; following Korean particles can select
a compatible candidate instead, as `東京을` shows when 을 selects 동경.
`suffix_reading` supplies the reading used when the word is not word-initial,
for example after a numeral. Entry-level `require_hanja` and `require_hangul`
request annotation marks, as do the separate [directives](./directives.md).

`MapDictionarySource` copies its entries into an immutable tuple. Mutating the
original list later does not change a configured source or converter.


Combining source roles
----------------------

Map sources default to `SourceRole.OVERRIDE`. A user override replaces
cooperating reference/loanword candidates for the same spelling and consumed
length; earlier override sources take priority. You can combine an override
with official sources explicitly:

~~~~ python
from gukhanmun.dictionaries import loanwords, stdict

converter = Converter(dictionaries=[stdict(), loanwords(), source])
~~~~

The official extras must be installed for this example. The map source still
overrides matching cooperative candidates even though it comes last.

`source_policy` controls cooperation between sources:

| `SourcePolicy`                   | Selection                                                          |
| -------------------------------- | ------------------------------------------------------------------ |
| `REFERENCE_AUTHORIZED` (default) | Use reference evidence to authorize alternate forms                |
| `REFERENCE_FIRST`                | Prefer reference candidates                                        |
| `LOANWORD_FIRST`                 | Prefer loanword candidates                                         |
| `ORDERED`                        | Use source registration order, ignoring roles and `reference_only` |

Under the three cooperative policies, all except `ORDERED`,
`reference_only=True` excludes independent loanword choices while retaining
reference-authorized forms. A source role alone does not create the lexical
evidence needed to authorize a form.


Compiled and remote dictionaries
--------------------------------

`FileDictionarySource` accepts raw bytes or a filesystem path with an explicit
`DictionaryFormat.FST` or `CDB`. `UrlDictionarySource` loads an explicit HTTP(S)
URL during converter construction. A string path is never guessed to be a
URL. Both load once; conversion calls do no dictionary I/O.

Binary source roles come from an explicit role first, then role metadata
embedded in the file, then `OVERRIDE`. Filenames do not determine roles. See the
[API reference](../../api/python.md#dictionary-sources-and-entries) for URL
origin restrictions, timeouts and download limits.

To compile a large custom dictionary, follow the
[dictionary-building guide](../cli/dictionary.md#building-a-custom-dictionary).
The resulting FST/CDB files work with the Python sources above.


Licenses
--------

The data distributions carry the licenses of their data:

 -  *Standard Korean Language Dictionary* and *Open Korean Dictionary* data:
    Creative Commons Attribution-ShareAlike 2.0 Korea (CC BY-SA 2.0 KR).
 -  NIKL loanword spelling examples: the National Institute of Korean
    Language's free-use terms.

Each distribution includes *LICENSE* and *ATTRIBUTION.md* with the attribution
and code license. Keep them when you redistribute the data.
