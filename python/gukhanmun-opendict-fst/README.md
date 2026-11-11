gukhanmun-opendict-fst
======================

This distribution contains *Open Korean Dictionary* (우리말샘) compiled into
Gukhanmun FST dictionary binaries (*general.fst*, *north-korean.fst*,
*dialect.fst*, *archaic.fst*) for the [gukhanmun] Python package. It has no API
of its own and does not depend on `gukhanmun`.

[gukhanmun]: https://github.com/dahlia/gukhanmun


Installation
------------

Install it through the matching extra of the core package:

~~~~ sh
pip install 'gukhanmun[opendict]'
~~~~


Usage
-----

Installing the data does not change any converter. Add it explicitly:

~~~~ python
from gukhanmun import Converter
from gukhanmun.dictionaries import opendict_general, opendict_north_korean, opendict_dialect, opendict_archaic

converter = Converter(dictionaries=[
    opendict_general(),
    opendict_north_korean(),
    opendict_dialect(),
    opendict_archaic(),
])
~~~~

The four *Open Korean Dictionary* categories are separate resources:
general words, North Korean words, dialect words and archaic words. Add only
the categories you need.

To read a resource directly, use `importlib.resources`:

~~~~ python
from importlib.resources import files

data = (files("gukhanmun_opendict_fst") / "general.fst").read_bytes()
~~~~


License
-------

`GPL-3.0-or-later AND LicenseRef-CC-BY-SA-2.0-KR`. See *LICENSE* and
*ATTRIBUTION.md* for the code license and the data attribution.
*provenance.json* records the source snapshot and the hashes of the bundled
files.
