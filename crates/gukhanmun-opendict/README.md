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
// dict: &'static FstDictionary — decoded once, shared for the process lifetime
~~~~

Each category loader function decodes the embedded FST bytes on the first call
and caches the result in a `OnceLock`. Subsequent calls return the same
reference with no additional work.

The categories are kept separate so callers can compose exactly the categories
they want using `ChainDictionary`.


Regeneration
------------

To extract and regenerate the canonical TSVs from the official *Open Korean
Dictionary* JSON download, run the extraction binary:

~~~~ sh
cargo run --release -p gukhanmun-opendict --bin gukhanmun-opendict-extract -- \
  /path/to/우리말샘_json_directory_or_zip \
  --general-output data/general.tsv \
  --north-korean-output data/north-korean.tsv \
  --dialect-output data/dialect.tsv \
  --archaic-output data/archaic.tsv
~~~~


Build-time requirements
-----------------------

The FST binaries are generated during `cargo build` from canonical TSV snapshots
stored in the *data/* directory. The snapshots are committed to the repository,
so a network connection is not required to build the crate. `gukhanmun-mkdict`
is used as a build dependency.


License and attribution
-----------------------

GPL-3.0-or-later and CC BY-SA 2.0 KR. See *ATTRIBUTION.md* for details.
