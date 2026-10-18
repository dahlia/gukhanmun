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

The committed TSV files under *crates/gukhanmun-opendict/data/* describe the
current dictionary. *metadata.json* in that directory records the source
identifier, dump date, and license used in generated binaries. Snapshots
collected by the weekly updater also have a *source.json* with the archive
checksum, record counts, and per-output hashes. See *NOTICE.md* for attribution.

The June 3, 2026 candidate migration preserved the earlier snapshot's key
sets, selected readings, and annotation marks. Its compatibility decisions
remain in *defaults.tsv*. Updates do not rewrite these decisions automatically;
a changed candidate signature requires maintainer review.


Regeneration
------------

Place the downloaded zip or extracted shard directory wherever convenient,
then run:

~~~~ sh
mise run opendict-extract -- /path/to/opendict.zip
mise run opendict-extract-check -- /path/to/opendict.zip
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
