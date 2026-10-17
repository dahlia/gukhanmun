---
title: Dictionary format
---

Dictionary format
=================

This document specifies the normalized dictionary input formats consumed by
`gukhanmun-mkdict` and the FST/CDB dictionary file layouts produced from those
inputs. It is the boundary between dictionary extractors, user-maintained
glossaries, and backend builders.


Canonical TSV input
-------------------

The primary input format is UTF-8 TSV with a required header row. Each file
must contain at least these columns:

| Column   | Required | Meaning                                 |
| -------- | -------- | --------------------------------------- |
| `hanja`  | yes      | Source spelling used as the lookup key. |
| `hangul` | yes      | Hangul reading emitted for the key.     |

These optional columns are recognized:

| Column           | Default | Meaning                                           |
| ---------------- | ------- | ------------------------------------------------- |
| `require_hanja`  | `false` | Force renderers to keep the source hanja visible. |
| `require_hangul` | `false` | Force renderers to include a hangul gloss.        |

Boolean values are `true`, `false`, `1`, or `0`. Empty optional boolean cells
are treated as `false`.

Additional columns such as `category`, `source`, or `note` are allowed as
future extension space. The current builder does not consume them; it emits a
short warning and ignores their values.

Example:

~~~~ tsv
hanja	hangul	require_hanja	require_hangul	category
天地	천지	false	false	basic
漢字	한자	true	false	basic
色깔論	색깔론	false	true	mixed
~~~~


Candidate rows
--------------

Optional `preference` (an unsigned 32-bit integer) and `suffix_reading` columns
retain competing readings and a candidate's non-word-initial form. A nonempty
preference, including `0`, opts the row into a candidate set. All rows for that
key in the same file must opt in together. The `source_ids` column is accepted
as build-time provenance and is not embedded in the dictionary.

~~~~ tsv
hanja	hangul	preference
東京	동경	0
東京	도쿄	10
~~~~

Larger preference wins. Identical (reading, suffix) pairs merge their marks by
OR and keep their greatest preference. After deduplication, preferences are
stored relative to the smallest preference. Ties are broken by reading, then
suffix, in UTF-8 byte order. Marks never merge across unrelated readings. Empty
suffix cells mean no positional form.

JSONL accepts optional `preference` and `suffixReading`/`suffix_reading`; a null
preference is a legacy row, and an explicit empty suffix string is invalid.
Rows without a preference are legacy rows, even when they carry a suffix,
and keep the duplicate-key merge semantics described below. Across input files,
merge policies replace or reject complete sets for a key. They never merge a
user override with a lower-priority set. Annotation rules operate on each
candidate, so a `reading` rule affects only that reading.


CSV and JSONL input
-------------------

`gukhanmun-mkdict` chooses the input parser from each file extension.  *.tsv*
uses the canonical TSV parser, *.csv* uses the CSV parser, *.jsonl* uses the
JSON Lines parser, and unknown extensions are treated as TSV for compatibility.

CSV files use the same header names as TSV:

~~~~ csv
hanja,hangul,require_hanja,require_hangul
天地,천지,true,false
~~~~

Each JSONL line is one object.  The boolean fields accept either snake\_case or
camelCase spellings:

~~~~ json
{"hanja":"漢字","hangul":"한자","requireHanja":false,"requireHangul":true}
~~~~


Merge policy
------------

When multiple input files are supplied, or when one input file repeats a
`hanja` key, `gukhanmun-mkdict` applies the configured merge policy:

 -  `error`: fail on the first duplicate key. This is the default.
 -  `first-wins`: keep the first entry and ignore later duplicates.
 -  `last-wins`: replace earlier entries with the last duplicate.

After merging, entries are sorted by UTF-8 key bytes before backend encoding.
This makes generated FST and CDB artifacts deterministic for the same
normalized inputs and metadata.


Build metadata
--------------

Every generated backend file embeds CBOR metadata. The minimum keys are:

| Key              | Source                                                                       |
| ---------------- | ---------------------------------------------------------------------------- |
| `source`         | `--metadata source=...`, or an empty string.                                 |
| `license`        | `--metadata license=...`, or an empty string.                                |
| `build_date`     | `--metadata build_date=...`, `SOURCE_DATE_EPOCH`, or `1970-01-01T00:00:00Z`. |
| `entry_count`    | Number of merged dictionary entries.                                         |
| `version`        | Dictionary file format version.                                              |
| `max_word_chars` | Maximum key length in Unicode scalar values.                                 |
| `max_key_bytes`  | Maximum key length in UTF-8 bytes.                                           |
| `prefix_count`   | CDB only: number of prefix records.                                          |

`entry_count`, `version`, `max_word_chars`, `max_key_bytes`, and
`prefix_count` are reserved and cannot be supplied with `--metadata`. Other
`--metadata KEY=VAL` pairs are preserved as string values.

For reproducible builds, the builder does not use the current clock by default.
If `build_date` is not passed explicitly and `SOURCE_DATE_EPOCH` is set, the
epoch is formatted as UTC RFC 3339. If neither value is present, the fixed date
`1970-01-01T00:00:00Z` is used.


FST backend file
----------------

The first implemented backend format is `fst`. The file contains:

1.  A fixed 64-byte little-endian header.
2.  A CBOR metadata map.
3.  `fst::Map` bytes.
4.  A contiguous reading/payload table.

The fixed header fields are:

| Field           | Size | Meaning                                  |
| --------------- | ---- | ---------------------------------------- |
| magic           | 8    | `GUKHMFST`.                              |
| version         | 4    | File format version, `1` or `2`.         |
| header length   | 4    | Fixed header length, currently `64`.     |
| metadata offset | 8    | Byte offset of the CBOR metadata.        |
| metadata length | 8    | Byte length of the CBOR metadata.        |
| FST offset      | 8    | Byte offset of the `fst::Map` bytes.     |
| FST length      | 8    | Byte length of the `fst::Map` bytes.     |
| readings offset | 8    | Byte offset of the reading string table. |
| readings length | 8    | Byte length of the reading string table. |

Each FST key is the UTF-8 bytes of the `hanja` column. Each FST value is a
64-bit integer:

| Bits  | Field                                                               |
| ----- | ------------------------------------------------------------------- |
| 0-15  | Reading length in UTF-8 bytes.                                      |
| 16-23 | Mark bitfield: bit 0 is `require_hanja`, bit 1 is `require_hangul`. |
| 24-63 | Offset into the reading string table.                               |

The reading bytes are stored in the reading string table, not in the FST value,
because FST values are fixed-width integers.

At runtime, `gukhanmun-fst` decodes the fixed header and CBOR metadata eagerly.
The FST map bytes and contiguous reading table then share a single byte backing
store: owned heap bytes for `FstDictionary::open()` and
`FstDictionary::from_bytes()`, static bytes for
`FstDictionary::from_static_bytes()`. The crate intentionally does not expose a
safe file-backed mmap loader because Rust cannot enforce that another file
descriptor or process will keep the mapped file immutable while the dictionary
is live.
`entries()` and `has_homophone()` enumerate the FST and therefore remain
full-dictionary scans; callers that need repeated homophone checks should use
the core homophone middleware's batch index.


CDB backend file
----------------

The CDB backend format is `cdb`.  It uses a trie embedded in CDB key space:
each prefix of each dictionary key is written as a CDB key.  Prefix records
that are complete dictionary entries carry the reading and mark bits; prefix
records that only lead to longer entries carry no reading.

| Offset | Size          | Field                                                               |
| ------ | ------------- | ------------------------------------------------------------------- |
| 0      | 1             | `is_complete` (`1` if this prefix is itself an entry).              |
| 1      | 1             | Mark bitfield: bit 0 is `require_hanja`, bit 1 is `require_hangul`. |
| 2      | 2             | Reading length in UTF-8 bytes, little-endian; `0` for non-complete. |
| 4      | `reading_len` | Reading bytes.                                                      |

The metadata CBOR map is stored under the reserved key
`__gukhanmun_meta__`.

The runtime CDB backend uses the `cdb` crate's file-backed reader and keeps the
backend opaque behind `HanjaDictionary`.  This already avoids loading the whole
CDB file through a Gukhanmun-owned buffer without adding an unsafe mmap surface.


Version 2 candidate payloads
----------------------------

The builder writes v1 when every key has one candidate without a suffix, and
v2 otherwise. New readers accept both versions; readers released before this
extension reject v2 files. Rebuild candidate-aware files with the new builder
and update the consuming crate or binding together. A single-candidate file
continues to work with old readers.

In v2, mark value `0x80` identifies a payload instead of a scalar reading. For
FST, the existing 16-bit length and 40-bit offset address its bytes. For CDB,
the 16-bit length at offset 2 is the payload length and bytes begin at offset 4.
Scalar records retain their v1 layout. The payload starts with a little-endian
`u16` candidate count, followed by each candidate:

| Field                | Encoding                                         |
| -------------------- | ------------------------------------------------ |
| Relative preference  | Little-endian `u32`.                             |
| Marks                | `u8`, bits 0 and 1 retain their scalar meanings. |
| Reading byte length  | Little-endian `u16`.                             |
| Suffix byte length   | Little-endian `u16`, zero means absent.          |
| Reading, then suffix | UTF-8 bytes of the declared lengths.             |

Payloads must fit within 65,535 bytes and contain a nonempty canonical set.
Readers reject truncation, invalid UTF-8 or mark bits, empty readings, trailing
bytes, duplicate identities, noncanonical ordering or weights, and payload tags
in v1 files. A singleton without a suffix must use the scalar representation.
Exact lookup returns decode errors; infallible iteration retains the backend's
skip/abort policy. `lookup().candidates()` and the candidate trait methods
expose full sets; `reading()`, `mark()`, `matches_at()` and `entries()` project
defaults.


Validation
----------

With `--validate`, `gukhanmun-mkdict` writes the output, opens it again with
the selected backend, and checks that every merged entry can be recovered with
the same full candidate set, including relative preferences, positional forms
and marks. Entry counts are compared per key, not per candidate. Validation
failure is fatal.
