Dictionary Format
=================

This document specifies the canonical dictionary input format consumed by
`gukhanmun-mkdict` and the first FST dictionary file layout produced from that
input. It is the boundary between dictionary extractors, user-maintained
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


Merge policy
------------

When multiple input files are supplied, or when one input file repeats a
`hanja` key, `gukhanmun-mkdict` applies the configured merge policy:

 -  `error`: fail on the first duplicate key. This is the default.
 -  `first-wins`: keep the first entry and ignore later duplicates.
 -  `last-wins`: replace earlier entries with the last duplicate.

After merging, entries are sorted by UTF-8 key bytes before backend encoding.
This makes the generated FST deterministic for the same normalized inputs and
metadata.


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

`entry_count`, `version`, `max_word_chars`, and `max_key_bytes` are reserved
and cannot be supplied with `--metadata`. Other `--metadata KEY=VAL` pairs are
preserved as string values.

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
4.  A contiguous UTF-8 reading string table.

The fixed header fields are:

| Field           | Size | Meaning                                  |
| --------------- | ---- | ---------------------------------------- |
| magic           | 8    | `GUKHMFST`.                              |
| version         | 4    | File format version, currently `1`.      |
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


Validation
----------

With `--validate`, `gukhanmun-mkdict` writes the output, opens it again with
the `gukhanmun-fst` backend, and checks that every merged TSV entry can be
recovered with the same reading and mark bits. Validation failure is fatal.
