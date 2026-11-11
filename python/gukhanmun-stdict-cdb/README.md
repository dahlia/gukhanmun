gukhanmun-stdict-cdb
====================

This distribution contains *Standard Korean Language Dictionary*
(標準國語大辭典) compiled into Gukhanmun CDB dictionary binaries
(*stdict.cdb.gz*) for the [gukhanmun] Python package. It has no API of its own
and does not depend on `gukhanmun`.

[gukhanmun]: https://github.com/dahlia/gukhanmun


Installation
------------

Install it through the matching extra of the core package:

~~~~ sh
pip install 'gukhanmun[stdict-cdb]'
~~~~


Usage
-----

Installing the data does not change any converter. Add it explicitly:

~~~~ python
from gukhanmun import Converter, DictionaryFormat
from gukhanmun.dictionaries import stdict

converter = Converter(dictionaries=[
    stdict(format=DictionaryFormat.CDB),
])
~~~~

Resources ending in *.gz* are gzip-compressed; the `gukhanmun.dictionaries`
helpers decompress them, while direct resource access returns the compressed
bytes.

To read a resource directly, use `importlib.resources`:

~~~~ python
from importlib.resources import files

data = (files("gukhanmun_stdict_cdb") / "stdict.cdb.gz").read_bytes()
~~~~


License
-------

`GPL-3.0-only AND LicenseRef-CC-BY-SA-2.0-KR`. See *LICENSE* and
*ATTRIBUTION.md* for the code license and the data attribution.
*provenance.json* records the source snapshot and the hashes of the bundled
files.
