gukhanmun-cdb
=============

CDB dictionary backend for Gukhanmun. Implements `HanjaDictionary` over a
CDB-based file format that is trivially auditable: the on-disk layout is a
standard CDB header followed by CBOR metadata and trie records, with no
proprietary compression.


File format
-----------

A `__gukhanmun_meta__` key in the CDB holds a CBOR-encoded metadata map (source
name, build date, etc.). All other keys are hanja strings in UTF-8; the
corresponding value encodes the hangul reading, a 2-bit mark byte (requiring
hanja or hangul annotation), and a length prefix in a compact binary layout.


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-cdb = "0.1"
~~~~


Usage
-----

~~~~ rust
use gukhanmun_cdb::CdbDictionary;

// From a file on disk:
let dict = CdbDictionary::open("stdict.gukcdb")?;

// From an owned Arc<[u8]>:
let bytes: Arc<[u8]> = std::fs::read("stdict.gukcdb")?.into();
let dict = CdbDictionary::from_bytes(bytes)?;

// Zero-copy from a static byte slice:
static BYTES: &[u8] = include_bytes!("stdict.gukcdb");
let dict = CdbDictionary::from_static_bytes(BYTES)?;
~~~~

`from_static_bytes` holds the slice directly without copying, so there is no
heap allocation beyond the `CdbDictionary` struct itself.


Trade-offs vs FST
-----------------

CDB lookup is O(1) (two hash table probes), while FST lookup is O(key length)
over a compressed automaton. For short hanja keys the difference is small. The
FST format supports prefix streaming and has better locality for the lattice
segmenter's inner loop; CDB is simpler to inspect and patch by hand.


License
-------

GPL-3.0-only. See `LICENSE` at the repository root.
