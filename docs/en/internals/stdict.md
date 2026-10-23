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

The committed TSV files under *crates/gukhanmun-stdict/data/* describe the
current dictionary. *metadata.json* in that directory records the source
identifier, dump date, and license used in generated binaries. Snapshots
collected by the weekly updater also have a *source.json* with the archive
checksum, record counts, and per-output hashes. See *NOTICE.md* for attribution.


Regeneration
------------

Place the downloaded zip wherever convenient, then run:

~~~~ sh
mise run stdict-extract -- /path/to/stdict.zip
mise run stdict-extract-check -- /path/to/stdict.zip
~~~~

The extractor writes deterministic UTF-8 TSV sorted by dictionary key.  The
`gukhanmun-stdict` build script then invokes `gukhanmun-mkdict` to build the
embedded FST at compile time.


Suffix readings
---------------

South Korean initial sound law (頭音法則) makes some hanja read differently
word-initially than elsewhere, so `年` reads `연` in `年度` but `년` in
`1998년`. The engine recovers the original sound of a single hanja outside
word-initial position from the bundled unihan readings, so the canonical TSV
needs no extra data for single hanja.

Multi-syllable positional pairs are stored in each candidate's `suffix_reading`
and survive both FST and CDB loading. Automatic pairing requires an all-hanja
key of at least two characters, matching key/reading character counts, and an
exact first-syllable initial-sound-law pair with identical remainders.
Suffix-only source records are folded into the candidate with the matching
word-initial reading; a reading with independent word-initial provenance also
retains its standalone candidate. Single-character `年` and `率` keep separate
lexical readings, leaving the engine's positional sound rules intact.

The extractor still writes *data/suffix.tsv* for compatibility, but runtime
lookup obtains suffix forms from the binary itself. `KoKrDictionary` and
raw FST/CDB lookup therefore expose the same positional data.


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
like `←lipoic酸` do not become mixed-script keys.  A single hanja borrowed for
a foreign reading (such as `引` from Japanese `引き`) is dropped, however, so it
cannot shadow that character's Sino-Korean reading in every compound that
contains it (`引` → `인`, recovered from the bundled unihan readings, not
`삐끼`).  Multi-character foreign spellings such as `北京` → `베이징` are kept.

Alternate hanja spellings marked with `/(병기)` are expanded into separate TSV
rows.  Duplicate keys retain every distinct reading and its source
`target_code` IDs. The default uses the highest-priority reading.  Entries
whose senses only redirect to another entry, such as `→ 표지03.`, lose to
entries with substantive definitions for the same hanja spelling.
Multi-character bracketed loanword hanja spellings are preferred over native
Sino-Korean readings for the same key, and otherwise ties use lexical order
unless a curated decision preserves the previous effective default.

The six Sino-Korean compounds that Standard Korean Orthography §30 spells with
a saisiot (사이시옷), namely 곳간(庫間), 셋방(貰房), 숫자(數字), 찻간(車間),
툇간(退間), and 횟수(回數), are promoted above the default priority, so the
prescribed saisiot spelling wins over a saisiot-free homograph for those keys
(數字 → 숫자, not 수자).  The list is closed and named directly by the
orthographic standard, so no general saisiot heuristic is involved.

The extractor itself writes `require_hanja` and `require_hangul` as `false` for
every row.  Annotation marks are layered on later by the rules file (see
below) so the canonical TSV remains a pure hanja↔hangul mapping.


Curated defaults and provenance
-------------------------------

*crates/gukhanmun-stdict/data/defaults.tsv* records 46 equal-priority decisions
needed to preserve the 2026-06-06 snapshot's defaults. Each row names a key,
the chosen reading and suffix pair, the complete candidate-identity signature
for that key, and a reason. Keys are unique within the file. A pin adds one
point inside the highest priority band. It cannot promote a redirect over a
substantive entry. Changed candidate signatures or missing chosen readings fail
extraction; absent keys are counted as skipped decisions so subset extraction
remains possible. Full regeneration uses every decision.

The source TSV keeps merged `target_code` IDs in `source_ids`, making candidate
records traceable in the pinned JSON dump. Runtime binaries retain only
readings, marks and relative preferences. They contain no definitions or source
IDs. The existing priority bands are redirect 0, ordinary 10, prescribed
saisiot 20, and foreign hanja spelling 30. Reading profiles are still
unimplemented. Conversion selects the recorded default for each key, narrowed
by a following Korean particle when one is present. The *Open Korean
Dictionary* extractor retains competing readings under the same rules.


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
