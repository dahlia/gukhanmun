Standard Korean Language Dictionary
===================================

`gukhanmun-stdict` bundles a snapshot derived from the National Institute of
Korean Language's *Standard Korean Language Dictionary* JSON download.

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
  ~/Downloads/전체\ 내려받기_표준국어대사전_JSON_20260506.zip
~~~~

The extractor writes deterministic UTF-8 TSV sorted by dictionary key.  The
`gukhanmun-stdict` build script then invokes `gukhanmun-mkdict` to build the
embedded FST at compile time.


Extraction Policy
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

The current snapshot does not derive `require_hanja` or `require_hangul`
automatically; both flags are `false` for every generated row.
