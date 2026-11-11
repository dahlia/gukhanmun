gukhanmun-stdict-fst
====================

This distribution contains *Standard Korean Language Dictionary*
(標準國語大辭典) compiled into Gukhanmun FST dictionary binaries (*stdict.fst*)
for the [gukhanmun] Python package. It has no API of its own and does not
depend on `gukhanmun`.

[gukhanmun]: https://github.com/dahlia/gukhanmun


Installation
------------

Install it through the matching extra of the core package:

~~~~ sh
pip install 'gukhanmun[stdict]'
~~~~


Usage
-----

Installing the data does not change any converter. Add it explicitly:

~~~~ python
from gukhanmun import Converter
from gukhanmun.dictionaries import stdict

converter = Converter(dictionaries=[
    stdict(),
])
~~~~

To read a resource directly, use `importlib.resources`:

~~~~ python
from importlib.resources import files

data = (files("gukhanmun_stdict_fst") / "stdict.fst").read_bytes()
~~~~


License
-------

`GPL-3.0-only AND LicenseRef-CC-BY-SA-2.0-KR`. See *LICENSE* and
*ATTRIBUTION.md* for the code license and the data attribution.
*provenance.json* records the source snapshot and the hashes of the bundled
files.
