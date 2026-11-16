gukhanmun-cdb
=============

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

CDB dictionary backend for Gukhanmun. Implements `HanjaDictionary` over a
CDB-based file format that is trivially auditable: the on-disk layout is a
standard CDB header followed by CBOR metadata and trie records, with no
proprietary compression.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-cdb?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-cdb
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-cdb
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


File format
-----------

A `__gukhanmun_meta__` key in the CDB holds a CBOR-encoded metadata map (source
name, build date, etc.). Other keys are dictionary-key prefixes in UTF-8,
which may include hangul in mixed-script words; the
corresponding value encodes the hangul reading, a 2-bit mark byte (requiring
hanja or hangul annotation), and a length prefix in a compact binary layout.
Prefix records also represent incomplete dictionary keys. Multiple readings
and positional forms use candidate payloads. Containers with source evidence
use version 4; older readers reject them, while current readers also accept
evidence-free versions 1–3.


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-cdb = "0.4"
~~~~


Usage
-----

~~~~ rust
use gukhanmun_cdb::CdbDictionary;

// From a file on disk:
let dict = CdbDictionary::open("stdict.gukcdb")?;

// From runtime bytes (copies them into its own backing store):
let bytes: Vec<u8> = std::fs::read("stdict.gukcdb")?;
let dict = CdbDictionary::from_bytes(&bytes)?;

// Zero-copy from a static byte slice:
static BYTES: &[u8] = include_bytes!("stdict.gukcdb");
let dict = CdbDictionary::from_static_bytes(BYTES)?;
~~~~

`from_static_bytes()` borrows the binary bytes without copying them. Loading
still validates the records, allocates a lookup index and decodes metadata.


Trade-offs vs FST
-----------------

The CDB backend uses a validated index with O(log N) key comparisons per
lookup, where N is the record count, rather than trusting on-disk hash-table
probe chains. FST lookup follows the key through a compressed automaton.
CDB stores dictionary-key prefixes as trie records; FST compresses shared
structure and is generally smaller. Both loaders impose admission limits on
untrusted files; see the [dictionary format reference].

[dictionary format reference]: https://gukhanmun.org/internals/dictionary-format


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.
