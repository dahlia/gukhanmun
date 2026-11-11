gukhanmun-loanwords-cdb
=======================

This distribution contains NIKL loanword spelling examples compiled into
Gukhanmun CDB dictionary binaries (*loanwords.cdb*) for the [gukhanmun] Python
package. It has no API of its own and does not depend on `gukhanmun`.

[gukhanmun]: https://github.com/dahlia/gukhanmun


Resource access
---------------

Read a resource with `importlib.resources`:

~~~~ python
from importlib.resources import files

data = (files("gukhanmun_loanwords_cdb") / "loanwords.cdb").read_bytes()
~~~~


License
-------

`GPL-3.0-or-later AND LicenseRef-NIKL-Free-Use`. See *LICENSE* and
*ATTRIBUTION.md* for the code license and the data attribution.
*provenance.json* records the source snapshot and the hashes of the bundled
files.
