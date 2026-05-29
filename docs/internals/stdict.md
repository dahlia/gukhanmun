---
title: Standard Korean Language Dictionary
---

Standard Korean Language Dictionary
===================================

`gukhanmun-stdict` bundles a snapshot derived from the National Institute of
Korean Language's *Standard Korean Language Dictionary* (標準國語大辭典)
JSON download.

The source dump is not committed to this repository.  The download requires a
login on the dictionary website, and the archive is much larger than the
normalized data used by the build.  The committed source of truth is instead
the canonical TSV file at *crates/gukhanmun-stdict/data/stdict.tsv*.


Snapshot
--------

| Field          | Value                                                              |
| -------------- | ------------------------------------------------------------------ |
| Source archive | *전체 내려받기\_표준국어대사전\_JSON\_20260506.zip*                |
| Dump date      | 2026-05-06                                                         |
| SHA-256        | `0da6bef096f892d7ab44e8f52ba3f16ece1e88fc8d823e7bc816f2c2d9689e46` |
| TSV entries    | 260,697                                                            |


Regeneration
------------

Place the downloaded zip wherever convenient, then run:

~~~~ sh
cargo run -p gukhanmun-stdict --bin gukhanmun-stdict-extract -- \
  -o crates/gukhanmun-stdict/data/stdict.tsv \
  --suffix-output crates/gukhanmun-stdict/data/suffix.tsv \
  ~/Downloads/전체\ 내려받기_표준국어대사전_JSON_20260506.zip
~~~~

The extractor writes deterministic UTF-8 TSV sorted by dictionary key.  The
`gukhanmun-stdict` build script then invokes `gukhanmun-mkdict` to build the
embedded FST at compile time.


Suffix readings
---------------

South Korean initial sound law (頭音法則) makes some hanja read differently
word-initially than elsewhere, so `年` reads `연` in `年度` but `년` in
`1998년`. The engine recovers the original sound of a *single* hanja outside
word-initial position from the bundled unihan readings, so the canonical TSV
needs no extra data for single hanja.

Multi-syllable compounds are different: only the dictionary knows which ones
keep their original leading sound outside word-initial position.  The dictionary
records this through suffix head words (written with a leading hyphen, such as
`-년대`) and bound-noun head words.  The extractor collects these and, for every
hanja-only key of two or more characters whose word-initial reading `I` and
suffix reading `S` differ only in their first syllable, writes a row
`hanja<TAB>I<TAB>S` to *crates/gukhanmun-stdict/data/suffix.tsv* (for example
`年代<TAB>연대<TAB>년대`).  The first-syllable-only test excludes semantically
distinct readings such as `便` (`편`/`변`).  Single hanja are intentionally
omitted because the engine handles them from the unihan readings.

`gukhanmun_stdict::ko_kr` returns a `KoKrDictionary` that wraps the embedded FST
and attaches each suffix reading to the matching entry, so the engine can pick
the position-correct reading.


Extraction policy
-----------------

Only entries whose `word_unit` is `단어` and whose `original_language_info`
can produce at least one hanja-bearing lookup key are included.  The dictionary
reading comes from `word_info.word`, not from `pronunciation_info`; homograph
numbers, hyphens, and `^` separators are removed from the word form.

For native hanja entries, `language_type = "한자"` segments are copied as the
lookup key.  Native Korean (`고유어`) segments may appear around hanja segments,
so mixed-script entries such as native-prefix or native-suffix words are
preserved.  Source notation markers such as `▽` are stripped from lookup keys.
Inline alternate spellings marked with `/` are expanded into separate keys, so
source spellings such as `布告하다/佈告하다` produce both runtime-matchable
forms.

Chinese, Japanese, and unknown-origin loanword segments are included only when
their `original_language` contains a standalone hanja spelling or a bracketed
hanja spelling, such as `Beijing[北京]` or `haiku[俳句]`.  In those cases the
hanja spelling is used as the lookup key and romanized text is discarded.
Foreign-origin segments without such a hanja spelling are skipped so values
like `←lipoic酸` do not become mixed-script keys.

Alternate hanja spellings marked with `/(병기)` are expanded into separate TSV
rows.  Duplicate keys keep the highest-priority reading.  Entries whose senses
only redirect to another entry, such as `→ 표지03.`, lose to entries with
substantive definitions for the same hanja spelling.  Bracketed loanword hanja
spellings are preferred over native Sino-Korean readings for the same key, and
otherwise the first reading encountered in sorted dump shard order is kept.

The extractor itself writes `require_hanja` and `require_hangul` as `false` for
every row.  Annotation marks are layered on later by the rules file (see
below) so the canonical TSV remains a pure hanja↔hangul mapping.


Annotation rules
----------------

*crates/gukhanmun-stdict/data/rules.tsv* lists hand-curated rules that
OR-merge `require_hanja`/`require_hangul` marks into dictionary entries at
build time.  The stdict crate's *build.rs* passes the rules file to
`gukhanmun-mkdict`, so the embedded FST ships with the marks already encoded.

The format is a TSV with the columns `kind`, `pattern`, `require_hanja`,
`require_hangul`, `reason`.  Three kinds of rule are supported:

 -  `entry` — `pattern` matches one dictionary entry whose hanja key equals
    `pattern` exactly.
 -  `contains` — `pattern` is a hanja substring (one or more characters);
    every entry whose hanja key contains the substring is marked.  Patterns
    must consist only of hanja characters because dictionary keys can be
    mixed-script (e.g. `布告하다`); a hangul or Latin substring would
    silently mark unrelated entries.
 -  `reading` — `pattern` is a hangul reading; every entry with that reading
    is marked.

A rule must set at least one of `require_hanja`/`require_hangul`, must
include a non-empty `reason`, and must match at least one entry.  Multiple
rules that touch the same entry are OR-merged.  Stale rules that match no
entry fail the build so that the rules file does not drift out of sync with
the dictionary; pass `--allow-unmatched-rules` to `gukhanmun-mkdict` if the
rule file is shared with a smaller dictionary.

To add or edit a rule, update *rules.tsv* and run:

~~~~ sh
cargo test -p gukhanmun-stdict
~~~~

The test suite rebuilds the bundled FST, verifies the marks land on
representative entries, and exercises `convert_plain_text` end to end with
`RenderMode::HangulOnly` to confirm that a marked entry renders as
`한글(漢字)`.
