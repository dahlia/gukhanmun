gukhanmun-fst
=============

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

FST dictionary backend for Gukhanmun. Implements `HanjaDictionary` over a
custom binary format built on top of the `fst` crate's sorted-map structure.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-fst?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-fst
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-fst
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


File format
-----------

Files start with an 8-byte magic string (`GUKHMFST`) and a 64-byte fixed
header containing a format version and the byte offset of the CBOR metadata
block. The metadata block is a CBOR map of string key-value pairs (source
name, build date, etc.). The FST map and reading table follow immediately after
the header.

Values in the FST encode the hangul reading length, a 2-bit mark field
(requiring hanja or requiring hangul annotation), and a byte offset into the
reading table, all packed into a single `u64`.


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-fst = "0.1"
~~~~


Usage
-----

~~~~ rust
use gukhanmun_fst::FstDictionary;

// From a file on disk (owns the bytes internally):
let dict = FstDictionary::open("stdict.gukfst")?;

// From an owned byte vec (e.g. fetched over HTTP):
let bytes: Vec<u8> = std::fs::read("stdict.gukfst")?;
let dict = FstDictionary::from_bytes(bytes.into())?;

// Zero-copy from a static byte slice (e.g. `include_bytes!`):
static BYTES: &[u8] = include_bytes!("stdict.gukfst");
let dict = FstDictionary::from_static_bytes(BYTES)?;
~~~~

`from_static_bytes` holds a reference to the slice directly, so the FST map
and reading table share the same backing memory without any heap allocation.
This is how `gukhanmun-stdict` embeds the Standard Korean Dictionary.


Building dictionary files
-------------------------

Use `gukhanmun-mkdict` to compile TSV or CSV source data into a *.gukfst*
file. The `gukhanmun-stdict` crate builds the bundled dictionary at Cargo
build time using this same tool.


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.
