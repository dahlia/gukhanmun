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

Files start with a 64-byte fixed header beginning with the 8-byte magic
string `GUKHMFST`, followed by a format version and offsets including the CBOR
metadata block. Metadata includes string key-value pairs (source name, build
date, etc.) and, when present, typed candidate and phrase evidence. Header
offsets locate the metadata, FST map and reading table.

Values in the FST encode the hangul reading length, a 2-bit mark field
(requiring hanja or requiring hangul annotation), and a byte offset into the
reading table, all packed into a single `u64`.

Multiple readings and positional forms use candidate payloads rather than
scalar readings. Containers with source evidence use version 4; older readers
reject them, while current readers also accept evidence-free versions 1–3.
See the [dictionary format reference] for the complete layouts and loader
admission limits.

[dictionary format reference]: https://gukhanmun.org/internals/dictionary-format


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-fst = "0.4"
~~~~


Usage
-----

~~~~ rust
use gukhanmun_fst::FstDictionary;

// From a file on disk (owns the bytes internally):
let dict = FstDictionary::open("stdict.gukfst")?;

// From runtime bytes (copies them into its own backing store):
let bytes: Vec<u8> = std::fs::read("stdict.gukfst")?;
let dict = FstDictionary::from_bytes(&bytes)?;

// Zero-copy from a static byte slice (e.g. `include_bytes!`):
static BYTES: &[u8] = include_bytes!("stdict.gukfst");
let dict = FstDictionary::from_static_bytes(BYTES)?;
~~~~

`from_static_bytes()` borrows the binary bytes without copying them. Loading
still validates the automaton and decodes metadata, which can allocate.
This is how `gukhanmun-stdict` embeds the *Standard Korean Language Dictionary*.


Building dictionary files
-------------------------

Use `gukhanmun-mkdict` to compile TSV or CSV source data into a *.gukfst*
file. The `gukhanmun-stdict` crate builds the bundled dictionary at Cargo
build time using this same tool.


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.
