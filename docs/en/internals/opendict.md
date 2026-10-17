---
title: Open Korean Dictionary
description: Provenance, extraction policy, and runtime use for bundled Open Korean Dictionary snapshots.
---

Open Korean Dictionary
======================

`gukhanmun-opendict` bundles category snapshots derived from the National
Institute of Korean Language's *Open Korean Dictionary* (우리말샘) JSON
download.

The source dump is not committed to this repository.  The committed source of
truth is the four canonical TSV snapshots and the compatibility decisions in
*defaults.tsv* under
*crates/gukhanmun-opendict/data/*, split by the source `senseinfo.type` value:
general (一般語), North Korean (北韓語), dialect (方言), and archaic (옛말).


Snapshot
--------

| Field          | Value                                                              |
| -------------- | ------------------------------------------------------------------ |
| Source archive | *전체 내려받기\_우리말샘\_json\_20260603.zip*                      |
| Dump date      | 2026-06-03                                                         |
| SHA-256        | `345cfae71f3710cc483975a9af04773985aaaa4ba7ca4855a4bb93f390f63e8e` |
| Data license   | CC BY-SA 2.0 KR                                                    |

| Category     | Source label | TSV file           |    Keys | Candidates | Multi-candidate keys | SHA-256                                                            |
| ------------ | ------------ | ------------------ | ------: | ---------: | -------------------: | ------------------------------------------------------------------ |
| General      | 일반어       | *general.tsv*      | 350,330 |    352,435 |                2,060 | `87f74d8507a72d38c69c6a4a15f03c00b06f78bc2bf92f03ec41750310d1d53f` |
| North Korean | 북한어       | *north-korean.tsv* |  34,093 |     34,112 |                   19 | `2250764e8b0fe0726532a7e83877bd839ad4707adb81182f5009691621017462` |
| Dialect      | 방언         | *dialect.tsv*      |   5,714 |      5,726 |                   12 | `65c4bdd63028c2a8ab16f99aa695e81b065266219b2153866af8cf0fe7e52862` |
| Archaic      | 옛말         | *archaic.tsv*      |      16 |         16 |                    0 | `637713ceeaa69ce122f65eba5d89c42bbcdc34de7b6c58066dff029dc33e4549` |

Running the previous extractor on this archive reproduces the four earlier
snapshots byte for byte. The candidate migration preserves every category's
key set, selected initial/suffix reading, and annotation marks through both
FST and CDB. There are 350 compatibility pins: 349 general and one North
Korean (`齘齒 → 계치`). The other 18 North Korean conflicts use lexical
defaults, including `綠色 → 녹색`, without synthesizing or normalizing North
Korean spellings.

The source contains two positional pairs in the general category:
`年代 → 연대/년대` (sense IDs 226781 and 416516) and `年度 → 연도/년도`
(113662, 227189, and 1516768). The suffix spellings `년대` and `년도` also
remain standalone candidates and stay the selected defaults, so these pairs
introduce no default output change. The other three categories contain no
positional pairs. For single hanja, `便` keeps its pinned default `편`
alongside `변`; the conversion engine in `gukhanmun-core` continues to handle
positional sound-law behavior.


Regeneration
------------

Place the downloaded zip or extracted shard directory wherever convenient,
then run:

~~~~ sh
mise run opendict-extract -- ~/Downloads/전체\ 내려받기_우리말샘_json_20260603.zip
mise run opendict-extract-check -- ~/Downloads/전체\ 내려받기_우리말샘_json_20260603.zip
~~~~

The extractor accepts a single JSON file, the official zip archive, or a
directory of JSON shards.  Directory entries and zip members are processed in
lexicographic order.  Each category TSV is deterministic UTF-8 sorted by
dictionary key, reading, and suffix spelling, independently of shard or item
order. Check mode extracts and validates all four categories before comparing
them, reports every differing output, and never creates or overwrites files.
Normal mode also finishes extraction and pin validation before opening any
outputs.

The `gukhanmun-opendict` build script invokes `gukhanmun-mkdict` to build four
embedded FST dictionaries at compile time.  The JavaScript package data tasks
build matching FST and CDB binaries for `@gukhanmun/opendict-fst` and
`@gukhanmun/opendict-cdb`.


Extraction policy
-----------------

Only entries whose `word_unit` is `어휘` and whose `original_language_info` can
produce at least one hanja-bearing lookup key are included.  The dictionary
reading comes from `wordinfo.word`; homograph numbers, hyphens, and `^`
separators are removed after recording suffix evidence from the original
headword.

The extractor reuses the shared key construction logic used by
`gukhanmun-stdict`.  Native hanja segments are copied as lookup keys, native
Korean segments may appear around hanja segments, bracketed foreign hanja
spellings such as `Beijing[北京]` are kept, and foreign-origin segments without
hanja are skipped.  Single-hanja foreign readings are skipped so they cannot
shadow Sino-Korean fallback readings recovered from Unihan.

Readings are collected separately within each category. Identical
(reading, suffix) candidates share sorted unique numeric `target_code` sense
identifiers in `source_ids`; those identifiers stay in the TSV and are omitted
from runtime payloads. The seven TSV columns are `hanja`, `hangul`,
`require_hanja`, `require_hangul`, `preference`, `suffix_reading`, and
`source_ids`. Both annotation marks, `require_hanja` and `require_hangul`,
remain false. In the extractor's summary counters, `entries_written` counts
candidate rows, while `duplicate_keys` counts repeated key occurrences.

*data/defaults.tsv* records one decision per category and key. Each decision
records the selected reading and suffix, the complete sorted JSON candidate
signature, and a rationale. Pins preserve earlier defaults where lexical order
would choose differently. A changed signature or missing selected candidate
fails extraction for directories and ZIP archives. Single JSON files and
`extract_json_reader_to_files` accept fewer candidates than a known signature
when every present reading already appears in it, with or without its positional
suffix. They use the lexical default until the complete set is present.
Unknown candidates still fail validation. Absent or incomplete pins are counted
in `skipped_default_pins`. Unpinned conflicts use the core selector's lexical
reading/suffix tie-break and are logged at debug level for review. Every
multi-candidate key has explicit preferences: 1 for its selected default and 0
for alternatives. Source order never chooses a default.

A leading hyphen in the original headword or `senseinfo.pos` of `의존 명사`
provides suffix evidence; a trailing-hyphen prefix remains word-initial.
Pairing requires a hanja-only key of at least two characters, equal key/reading
syllable counts, and the shared initial sound law relation. All compatible
pairs and contributor identifiers are retained. Independent initial and suffix
attestations of the same spelling also retain a standalone candidate.
Every source-observed suffix-only spelling also remains a scalar lexical
alternative, even when it contributes to a pair. This retains old defaults
without suppressing positional candidates. Ambiguous or unrelated spellings
remain separate. Single-hanja keys remain
lexical candidates but are never paired, because the engine handles their
positional sound law. North Korean spellings are never normalized to South
Korean ones during extraction.

The four categories remain independent. A higher-priority dictionary replaces
the whole candidate set for a key, so placing the North Korean dictionary first
in a `ChainDictionary` retains its orthography without combining its candidates
with the general category. Unselected readings do not introduce homophone
annotations.


Runtime use
-----------

The Rust crate exposes one loader per category:

 -  `gukhanmun_opendict::general()`
 -  `gukhanmun_opendict::north_korean()`
 -  `gukhanmun_opendict::dialect()`
 -  `gukhanmun_opendict::archaic()`

`Preset::KoKp` includes the North Korean (北韓語) dictionary by default in Rust
and the CLI. The JavaScript bindings do not auto-load dictionary data for
either preset; use `@gukhanmun/opendict-fst` or `@gukhanmun/opendict-cdb`
explicitly and pass the categories you need through
`load({ dictionaries: [...] })`.
