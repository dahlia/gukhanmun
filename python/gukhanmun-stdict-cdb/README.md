gukhanmun-stdict-cdb
====================

This distribution contains *Standard Korean Language Dictionary*
(標準國語大辭典) compiled into Gukhanmun CDB dictionary binaries
(*stdict.cdb.gz*) for the [gukhanmun] Python package. It has no API of its own
and does not depend on `gukhanmun`.

[gukhanmun]: https://github.com/dahlia/gukhanmun


Resource access
---------------

Read a resource with `importlib.resources`:

~~~~ python
from importlib.resources import files

data = (files("gukhanmun_stdict_cdb") / "stdict.cdb.gz").read_bytes()
~~~~

Resources ending in *.gz* are gzip-compressed, so direct access returns
compressed bytes. Decompress them before passing them to a dictionary loader.


License
-------

`GPL-3.0-only AND LicenseRef-CC-BY-SA-2.0-KR`. See *LICENSE* and
*ATTRIBUTION.md* for the code license and the data attribution.
*provenance.json* records the source snapshot and the hashes of the bundled
files.
