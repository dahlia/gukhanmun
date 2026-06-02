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
truth is the four canonical TSV files under
*crates/gukhanmun-opendict/data/*, split by the source `senseinfo.type` value:
general (一般語), North Korean (北韓語), dialect (方言), and archaic (옛말).


Snapshot
--------

| Field          | Value                                                              |
| -------------- | ------------------------------------------------------------------ |
| Source archive | *전체 내려받기\_우리말샘\_json\_20260503.zip*                      |
| Dump date      | 2026-05-03                                                         |
| SHA-256        | `846547ca6d80e6f8858af287aababb570ec54d591d40ea68b76266c25a0742ae` |
| Data license   | CC BY-SA 2.0 KR                                                    |

| Category     | Source label | TSV file           | Entries | SHA-256                                                            |
| ------------ | ------------ | ------------------ | ------: | ------------------------------------------------------------------ |
| General      | 일반어       | *general.tsv*      | 350,383 | `db42f9c3ee160abf2ebb46da7df2f761e1392d19f29af32fbcfdf190c401020e` |
| North Korean | 북한어       | *north-korean.tsv* |  34,093 | `cdc67c66b3c5febf870a406188e9621dd3a72654b5e878de1e9fb25b40db6256` |
| Dialect      | 방언         | *dialect.tsv*      |   5,715 | `a77ec3981ff0804ac97a59de8b44c262bd964f9031c706ba501126e793f92652` |
| Archaic      | 옛말         | *archaic.tsv*      |      16 | `0f77342de48317faf10a997b156955bf73cad442e36d150a91ed19265fea0bdc` |


Regeneration
------------

Place the downloaded zip or extracted shard directory wherever convenient,
then run:

~~~~ sh
cargo run --release -p gukhanmun-opendict --bin gukhanmun-opendict-extract -- \
  ~/Downloads/전체\ 내려받기_우리말샘_json_20260503 \
  --general-output crates/gukhanmun-opendict/data/general.tsv \
  --north-korean-output crates/gukhanmun-opendict/data/north-korean.tsv \
  --dialect-output crates/gukhanmun-opendict/data/dialect.tsv \
  --archaic-output crates/gukhanmun-opendict/data/archaic.tsv
~~~~

The extractor accepts a single JSON file, the official zip archive, or a
directory of JSON shards.  Directory entries and zip members are processed in
lexicographic order.  Each category TSV is deterministic UTF-8 sorted by
dictionary key.

The `gukhanmun-opendict` build script invokes `gukhanmun-mkdict` to build four
embedded FST dictionaries at compile time.  The JavaScript package data tasks
build matching FST and CDB binaries for `@gukhanmun/opendict-fst` and
`@gukhanmun/opendict-cdb`.


Extraction policy
-----------------

Only entries whose `word_unit` is `어휘` and whose `original_language_info` can
produce at least one hanja-bearing lookup key are included.  The dictionary
reading comes from `wordinfo.word`; homograph numbers, hyphens, and `^`
separators are removed from the word form.

The extractor reuses the shared key construction logic used by
`gukhanmun-stdict`.  Native hanja segments are copied as lookup keys, native
Korean segments may appear around hanja segments, bracketed foreign hanja
spellings such as `Beijing[北京]` are kept, and foreign-origin segments without
hanja are skipped.  Single-hanja foreign readings are skipped so they cannot
shadow Sino-Korean fallback readings recovered from unihan.

Duplicate keys are resolved within each category only: the first reading
encountered in sorted dump shard order wins for that category.  This keeps the
general (一般語) and North Korean (北韓語) readings independent, so a caller
can place the North Korean dictionary above another dictionary in a
`ChainDictionary` without changing the South Korean general snapshot.


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
