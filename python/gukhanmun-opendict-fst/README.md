gukhanmun-opendict-fst
======================

This distribution contains *Open Korean Dictionary* (우리말샘) compiled into
Gukhanmun FST dictionary binaries (*general.fst*, *north-korean.fst*,
*dialect.fst*, *archaic.fst*) for the [gukhanmun] Python package. It has no API
of its own and does not depend on `gukhanmun`.

The four *Open Korean Dictionary* categories (general, North Korean, dialect
and archaic words) are separate resources.

[gukhanmun]: https://github.com/dahlia/gukhanmun


Resource access
---------------

Read a resource with `importlib.resources`:

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
