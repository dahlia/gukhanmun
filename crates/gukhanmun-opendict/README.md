gukhanmun-opendict
==================

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-or-later][GPL badge]][GPL]

Bundled *Open Korean Dictionary* (우리말샘) data for Gukhanmun, compiled into
FST binaries and embedded in the crate at build time. No external file or
network access is required at runtime.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-opendict?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-opendict
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-opendict
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-opendict = "0.2"
~~~~


Usage
-----

~~~~ rust
use gukhanmun_opendict::{general, north_korean, dialect, archaic};

let dict_general = general();
let dict_nk = north_korean();
// &'static FstDictionary, decoded once and shared for the process lifetime
~~~~

Each category loader function decodes the embedded FST bytes on the first call
and caches the result in a `OnceLock`. Subsequent calls return the same
reference with no additional work.

The categories are kept separate so callers can compose exactly the categories
they want using `ChainDictionary`.


Regeneration
------------

From the repository root, regenerate or check the canonical TSV snapshots
from the official *Open Korean Dictionary* JSON download:

~~~~ sh
mise run opendict-extract -- /path/to/우리말샘_json_directory_or_zip
mise run opendict-extract-check -- /path/to/우리말샘_json_directory_or_zip
~~~~

Check mode compares all four categories without creating or overwriting files.
Each category retains competing readings and sorted source sense identifiers.
*data/defaults.tsv* records compatibility decisions with complete candidate
signatures, so source changes that invalidate a decision fail extraction.
Identifiers stay in the source TSVs; runtime files contain candidates and
preferences. Higher-priority dictionaries replace a key's entire candidate
set. See the [extraction policy] for positional forms and snapshot provenance.

[extraction policy]: ../../docs/en/internals/opendict.md


Build-time requirements
-----------------------

The FST binaries are generated during `cargo build` from canonical TSV snapshots
stored in the *data/* directory. The snapshots are committed to the repository,
so a network connection is not required to build the crate. `gukhanmun-mkdict`
is used as a build dependency.


License and attribution
-----------------------

Code is GPL-3.0-or-later. The bundled dictionary data is CC BY-SA 2.0 KR.
See *ATTRIBUTION.md* for details.
