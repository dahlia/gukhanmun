---
title: "Conversion options"
description: |-
  Builder methods that control how Gukhanmun converts hanja to hangul.
---

Conversion options
==================

All options are set on `Builder` before calling `.build()`.


Preset
------

`Builder::with_preset(preset)` configures a coherent set of defaults:

| Preset         | Dictionary     | Initial sound law | Homophone window          |
| -------------- | -------------- | ----------------- | ------------------------- |
| `Preset::KoKr` | Bundled stdict | `true`            | `ContextWindow::PerBlock` |
| `Preset::KoKp` | None           | `false`           | `ContextWindow::Off`      |

Individual options below override the preset.


Segmentation strategy
---------------------

~~~~ rust
use gukhanmun::SegmentationStrategy;

builder.segmentation(SegmentationStrategy::Lattice);  // default
builder.segmentation(SegmentationStrategy::Eager);
~~~~

`Lattice` finds the globally optimal segmentation using dynamic programming.
`Eager` is a greedy left-to-right longest-match; faster but less accurate for
compound words.


Numeral handling
----------------

`NumeralStrategy` controls how hanja numeral characters such as 二〇一六 are
rendered.  Chinese-style numerals can represent numbers in positional or
additive notation depending on context:

| Variant            | 二〇一六年 | 十一月 | 一千二百三十四 |
| ------------------ | ---------- | ------ | -------------- |
| `HangulPhonetic`   | 이공일륙년 | 십일월 | 일천이백삼십사 |
| `PositionalArabic` | 2016년     | —      | —              |
| `AdditiveArabic`   | —          | 11월   | 1234           |
| `Smart`            | 2016년     | 11월   | 1234           |

~~~~ rust
use gukhanmun::NumeralStrategy;

builder.numerals(NumeralStrategy::HangulPhonetic);   // default: 이공일륙
builder.numerals(NumeralStrategy::PositionalArabic); // 2016 (year-like)
builder.numerals(NumeralStrategy::AdditiveArabic);   // 11 (additive)
builder.numerals(NumeralStrategy::Smart);            // picks best per context
~~~~

`Smart` chooses positional notation for year-like four-digit sequences and
additive notation for quantities; use it for general-purpose documents.


Initial sound law
-----------------

~~~~ rust
builder.initial_sound_law(true);   // enabled (Preset::KoKr default)
builder.initial_sound_law(false);  // disabled (Preset::KoKp default)
~~~~

Applies the South Korean phonetic rule (頭音法則) to fallback readings for
characters not found in any dictionary:

| Input | Law enabled (`KoKr`) | Law disabled (`KoKp`) |
| ----- | -------------------- | --------------------- |
| 來日  | 내일                 | 래일                  |
| 理由  | 이유                 | 리유                  |
| 女子  | 여자                 | 녀자                  |


Homophone disambiguation window
-------------------------------

Different hanja words can share the same hangul reading (for example, 連霸 and
連敗 are both 연패).  In `RenderMode::HangulOnly`, Gukhanmun can keep the hanja
in parentheses for such words so readers can tell them apart.
`homophone_window` sets the scope across which a reading is considered
ambiguous:

| Value                                    | Behaviour                                        |
| ---------------------------------------- | ------------------------------------------------ |
| `ContextWindow::Off`                     | No disambiguation tracking                       |
| `ContextWindow::PerBlock` (KoKr default) | Reset at paragraph, list, and heading boundaries |
| `ContextWindow::PerSection`              | Reset at heading boundaries only                 |
| `ContextWindow::PerDocument`             | Track across the entire input                    |

~~~~ rust
use gukhanmun::ContextWindow;

builder.homophone_window(ContextWindow::Off);
builder.homophone_window(ContextWindow::PerBlock);    // default for KoKr
builder.homophone_window(ContextWindow::PerSection);
builder.homophone_window(ContextWindow::PerDocument);
~~~~

Wider windows are appropriate for dense hanja texts where readings recur across
many sections.


Homophone detection strategy
----------------------------

`homophone_detection` chooses *which* readings count as ambiguous within the
window:

| Value                                        | Behaviour                                                                            |
| -------------------------------------------- | ------------------------------------------------------------------------------------ |
| `HomophoneDetection::ContextLocal` (default) | Gloss a word only when a different-meaning homophone actually appears in the window. |
| `HomophoneDetection::DictionaryWide`         | Also gloss readings shared by other hanja forms anywhere in the dictionary.          |

~~~~ rust
use gukhanmun::HomophoneDetection;

builder.homophone_detection(HomophoneDetection::ContextLocal);    // default
builder.homophone_detection(HomophoneDetection::DictionaryWide);
~~~~

`ContextLocal` keeps hangul-only output clean: a word is glossed only when the
surrounding text genuinely makes it ambiguous.  `DictionaryWide` is broader, but
with a large reference dictionary such as the bundled Standard Korean Dictionary
nearly every common reading has some homophone, so it glosses most Sino-Korean
words.  To always gloss a specific word regardless of context, use a
`DirectiveAction::RequireHanja` directive instead (see
[*User directives*](./directives.md)).


Only recognized words are disambiguated
---------------------------------------

Homophone disambiguation operates on words the dictionary recognizes as units.
A hanja sequence with no dictionary entry of its own is not treated as a single
word, and its fallback (non-dictionary) characters are never glossed; any
recognized single-character entries inside it (such as `紫`) are still handled
on their own.  For example, `自由` and `子游` are both bundled entries read
`자유`, so `自由와 子游` renders as `자유(自由)와 자유(子游)`; but `紫楡` has no
entry of its own, so under the default context-local strategy `自由와 紫楡`
renders as `자유와 자유` with no gloss, because the engine never sees a second
`자유` unit to collide with `自由`.  To disambiguate the whole term, add it to a
[custom dictionary](./dictionary.md) so the engine treats it as a single unit.


First-occurrence clearing window
--------------------------------

When enabled, first-occurrence clearing stops annotating a hanja after its
first occurrence within the window.  This is useful for documents that
introduce each character once and then use it freely; subsequent occurrences
are left as plain hangul without parenthetical hanja.

| Value                          | Behaviour                              |
| ------------------------------ | -------------------------------------- |
| `ContextWindow::Off` (default) | Never clear; annotate every occurrence |
| `ContextWindow::PerBlock`      | Clear within the same paragraph/block  |
| `ContextWindow::PerSection`    | Clear within the same section          |
| `ContextWindow::PerDocument`   | Clear across the entire document       |

~~~~ rust
builder.first_occurrence_window(ContextWindow::Off);        // default
builder.first_occurrence_window(ContextWindow::PerBlock);
builder.first_occurrence_window(ContextWindow::PerSection);
builder.first_occurrence_window(ContextWindow::PerDocument);
~~~~


Error recovery
--------------

~~~~ rust
use gukhanmun::Recovery;

builder.recovery(Recovery::Strict);   // default: abort on error
builder.recovery(Recovery::Lenient);  // skip problematic fragments
~~~~

Relevant for HTML conversion; plain text and Markdown do not produce
recoverable errors.
