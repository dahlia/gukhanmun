---
title: "Conversion options"
description: |-
  Options that control how Gukhanmun converts hanja to hangul in JavaScript.
---

Conversion options
==================

Pass these as properties of the `GukhanmunOptions` object to `load()`.


Preset
------

`preset` selects a preconfigured set of defaults:

| Value               | Dictionary                          | Initial sound law | Homophone window |
| ------------------- | ----------------------------------- | ----------------- | ---------------- |
| `"ko-kr"` (default) | None bundled—load stdict explicitly | `true`            | `"per-block"`    |
| `"ko-kp"`           | None                                | `false`           | `"off"`          |

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ preset: "ko-kp", dictionaries: [] });
~~~~

Unlike the Rust crate, the JavaScript packages never include the bundled
dictionary automatically; always pass it via `dictionaries`.


Segmentation strategy
---------------------

`segmentation` controls how Gukhanmun finds word boundaries within hanja runs:

 -  `"lattice"` (default): evaluates all dictionary matches at every position
    and selects the globally optimal segmentation using dynamic programming.
    Most accurate, especially for compound words and ambiguous boundaries.
 -  `"eager"`: greedy left-to-right longest-match.  Faster, but may
    mis-segment compound words.

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({
  segmentation: "lattice",  // default: optimal, dynamic programming
  // segmentation: "eager", // greedy, faster but less accurate
});
~~~~

Prefer `"eager"` only when throughput matters more than accuracy.


Numeral handling
----------------

`numerals` controls how hanja numeral characters such as 二〇一六 are rendered.
Chinese-style numerals can represent numbers in multiple ways depending on
whether they encode positions or quantities:

| Value                         | 二〇一六年 | 十一月 | 一千二百三十四 |
| ----------------------------- | ---------- | ------ | -------------- |
| `"hangul-phonetic"` (default) | 이공일륙년 | 십일월 | 일천이백삼십사 |
| `"positional-arabic"`         | 2016년     | (n/a)  | (n/a)          |
| `"additive-arabic"`           | (n/a)      | 11월   | 1234           |
| `"smart"`                     | 2016년     | 11월   | 1234           |

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ numerals: "hangul-phonetic" }); // 이공일륙 (default)
const g = await load({ numerals: "positional-arabic"}); // 2016
const g = await load({ numerals: "additive-arabic" });  // 11 (月), 1234
const g = await load({ numerals: "smart" });            // picks best per context
~~~~

`"smart"` chooses positional notation for year-like four-digit sequences and
additive notation for clear quantities, but keeps phonetic fallback readings
for ambiguous word-like sequences such as `百濟` or `十長生`.


Initial sound law
-----------------

The initial sound law (頭音法則) is a South Korean phonological rule
that changes certain initial consonants at the start of a word.  The rule
applies to fallback readings for characters not found in any dictionary;
dictionary entries already encode their correct readings.

| Input | `initialSoundLaw: true` (ko-kr) | `initialSoundLaw: false` (ko-kp) |
| ----- | ------------------------------- | -------------------------------- |
| 來日  | 내일                            | 래일                             |
| 理由  | 이유                            | 리유                             |
| 女子  | 여자                            | 녀자                             |

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ initialSoundLaw: true });   // default for ko-kr
const g = await load({ initialSoundLaw: false });  // default for ko-kp
~~~~

Disable it for North Korean orthography (`"ko-kp"` preset) or when processing
text that follows North Korean spelling conventions.


Parenthetical reading annotations
---------------------------------

When a word carries an explicit parenthetical reading gloss, hanja-first
(`庫間(곳간)`) or hangul-first (`곳간(庫間)`), Gukhanmun removes the redundant
parenthetical by default and shows the word in both scripts (`곳간(庫間)`).  A
parenthetical that pins an alternative reading overrides the dictionary reading
for that occurrence, so `數字(수자)` becomes `수자(數字)` even though `數字`
normally reads `숫자`.

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ collapseRedundantParens: true });   // default
const g = await load({ collapseRedundantParens: false });  // keep verbatim
~~~~

A reading annotation is told apart from a definition by two rules.  A
parenthetical that exactly matches the word's reading always collapses (this
covers 사이시옷 readings like `庫間(곳간)`).  Otherwise, an alternative reading
is accepted only when it has one hangul syllable per hanja character, each a
valid Sino-Korean reading of that character (as in `數字(수자)`).  A definition
gloss such as `庫間(물건을 간직하여 두는 곳)` and a foreign transliteration such
as `蔣介石(장제스)` match neither rule and are left untouched.


Homophone disambiguation window
-------------------------------

Different hanja words can share the same hangul reading (for example, 連霸 and
連敗 are both 연패).  In `"hangul-only"` rendering mode, Gukhanmun can keep the
hanja in parentheses for such words so readers can tell them apart.
`homophoneWindow` sets the scope across which a reading is considered ambiguous:

| Value                               | Behaviour                                        |
| ----------------------------------- | ------------------------------------------------ |
| `"off"`                             | No disambiguation tracking                       |
| `"per-block"` (default for `ko-kr`) | Reset at paragraph, list, and heading boundaries |
| `"per-section"`                     | Reset at heading boundaries only                 |
| `"per-document"`                    | Track across the entire input                    |

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ homophoneWindow: "off" });
const g = await load({ homophoneWindow: "per-block" });    // default for ko-kr
const g = await load({ homophoneWindow: "per-section" });
const g = await load({ homophoneWindow: "per-document" });
~~~~

Wider windows are appropriate for dense hanja texts where readings recur across
many sections.


Homophone detection strategy
----------------------------

`homophoneDetection` chooses *which* readings count as ambiguous within the
window:

| Value                       | Behaviour                                                                            |
| --------------------------- | ------------------------------------------------------------------------------------ |
| `"context-local"` (default) | Gloss a word only when a different-meaning homophone actually appears in the window. |
| `"dictionary-wide"`         | Also gloss readings shared by other hanja forms anywhere in the dictionary.          |

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ homophoneDetection: "context-local" });    // default
const g = await load({ homophoneDetection: "dictionary-wide" });
~~~~

`"context-local"` keeps hangul-only output clean: a word is glossed only when
the surrounding text genuinely makes it ambiguous.  `"dictionary-wide"` is
broader, but with a large reference dictionary such as the *Standard Korean
Dictionary* nearly every common reading has some homophone, so it glosses most
Sino-Korean words.  To always gloss a specific word regardless of context, use
a `requireHanja` directive instead (see [*User directives*](./directives.md)).


Only recognized words are disambiguated
---------------------------------------

Homophone disambiguation operates on words the dictionary recognizes as units.
A hanja sequence with no dictionary entry of its own is not treated as a single
word, and its fallback (non-dictionary) characters are never glossed; any
recognized single-character entries inside it (such as `紫`) are still handled
on their own.  For example, with the *Standard Korean Dictionary* loaded, `自由`
and `子游` are both entries read `자유`, so `自由와 子游` renders as
`자유(自由)와 자유(子游)`; but `紫楡` has no entry of its own, so under the
default context-local strategy `自由와 紫楡` renders as `자유와 자유` with no
gloss, because the engine never sees a second `자유` unit to collide with
`自由`.  To disambiguate the whole term, add it to a
[custom dictionary](./dictionary.md) so the engine treats it as a single unit.


First-occurrence clearing window
--------------------------------

When enabled, first-occurrence clearing stops annotating a hanja after its
first occurrence within the window.  This is useful for documents that
introduce each character once and then use it freely; subsequent occurrences
are left as plain hangul without parenthetical hanja.

| Value             | Behaviour                              |
| ----------------- | -------------------------------------- |
| `"off"` (default) | Never clear; annotate every occurrence |
| `"per-block"`     | Clear within the same paragraph/block  |
| `"per-section"`   | Clear within the same section          |
| `"per-document"`  | Clear across the entire document       |

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ firstOccurrenceWindow: "off" });        // default
const g = await load({ firstOccurrenceWindow: "per-block" });
const g = await load({ firstOccurrenceWindow: "per-section" });
const g = await load({ firstOccurrenceWindow: "per-document" });
~~~~


Error recovery
--------------

`recovery` controls what happens when the HTML parser encounters markup it
cannot interpret.  It has no effect for plain text or Markdown input.

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ recovery: "strict" });   // default: throw on error
const g = await load({ recovery: "lenient" });  // skip bad fragments (HTML)
~~~~

Use `"lenient"` when processing HTML from external sources that may contain
fragments or non-standard markup; it skips problematic parts rather than
throwing a `GukhanmunError`.
