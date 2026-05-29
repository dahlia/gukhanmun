---
title: "Conversion options"
description: |-
  Flags that control how Gukhanmun converts hanja to hangul.
---

Conversion options
==================

These flags control the linguistic rules applied during conversion.


Preset
------

`--preset` selects a preconfigured combination of defaults:

| Preset            | Dictionary     | Initial sound law | Homophone window | Use case                 |
| ----------------- | -------------- | ----------------- | ---------------- | ------------------------ |
| `ko-kr` (default) | Bundled stdict | Enabled           | Per-block        | South Korean orthography |
| `ko-kp`           | None           | Disabled          | Off              | North Korean orthography |

~~~~ sh
gukhanmun --preset ko-kp input.txt
~~~~

Individual flags below override the preset's defaults.


Segmentation strategy
---------------------

`--segmentation` controls how word boundaries are found:

 -  `lattice` (default): finds the globally optimal segmentation by evaluating
    all dictionary matches at every position with dynamic programming.  Best for
    accuracy.
 -  `eager`: greedy left-to-right longest-match.  Faster but may mis-segment
    compound words.

~~~~ sh
gukhanmun --segmentation eager input.txt
~~~~


Numeral handling
----------------

`--numerals` controls how hanja numerals are rendered:

| Strategy                    | 二〇一六年 | 十一月 | 一千二百三十四 |
| --------------------------- | ---------- | ------ | -------------- |
| `hangul-phonetic` (default) | 이공일륙년 | 십일월 | 일천이백삼십사 |
| `positional-arabic`         | 2016년     | —      | —              |
| `additive-arabic`           | —          | 11월   | 1234           |
| `smart`                     | 2016년     | 11월   | 1234           |

~~~~ sh
gukhanmun --numerals smart input.txt
~~~~


Initial sound law
-----------------

The initial sound law (頭音法則) is enabled by default for `ko-kr` and
disabled for `ko-kp`.  It affects character-by-character fallback readings for
characters not found in any dictionary; dictionary entries already encode their
correct readings.

| Input | Law enabled (`ko-kr`) | Law disabled (`ko-kp`) |
| ----- | --------------------- | ---------------------- |
| 來日  | 내일                  | 래일                   |
| 理由  | 이유                  | 리유                   |
| 女子  | 여자                  | 녀자                   |

Override with explicit flags:

~~~~ sh
gukhanmun --no-initial-sound-law input.txt  # disable
gukhanmun --initial-sound-law input.txt     # enable (redundant for ko-kr)
~~~~


Homophone disambiguation
------------------------

Different hanja words can share the same hangul reading (for example, 連霸 and
連敗 are both 연패).  In the default `hangul-only` rendering mode, Gukhanmun
can keep the hanja in parentheses for such words so readers can tell them
apart. `--disambiguation` sets the scope across which a reading is considered
ambiguous:

| Value                             | Behaviour                                  |
| --------------------------------- | ------------------------------------------ |
| `off`                             | No disambiguation                          |
| `per-block` (default for `ko-kr`) | Reset at paragraph/list/heading boundaries |
| `per-section`                     | Reset at heading boundaries                |
| `per-document`                    | Track across the entire input              |

~~~~ sh
gukhanmun --disambiguation per-section input.txt
~~~~

`--homophone-detection` chooses which readings count as ambiguous within the
window:

| Value                     | Behaviour                                                                            |
| ------------------------- | ------------------------------------------------------------------------------------ |
| `context-local` (default) | Gloss a word only when a different-meaning homophone actually appears in the window. |
| `dictionary-wide`         | Also gloss readings shared by other hanja forms anywhere in the dictionary.          |

~~~~ sh
gukhanmun --homophone-detection dictionary-wide input.txt
~~~~

`context-local` keeps hangul-only output clean.  `dictionary-wide` is broader,
but with the bundled Standard Korean Dictionary nearly every common reading has
some homophone, so it glosses most Sino-Korean words.  To always gloss a
specific word regardless of context, use the `--require-hanja` flag instead
(see [*User directives*](./directives.md)).


First-occurrence clearing
-------------------------

`--first-occurrence` removes annotations from characters whose presentation
was already forced earlier in the window:

| Value           | Behaviour                        |
| --------------- | -------------------------------- |
| `off` (default) | Never clear                      |
| `per-block`     | Clear within a paragraph/block   |
| `per-section`   | Clear within a section           |
| `per-document`  | Clear across the entire document |

~~~~ sh
gukhanmun --first-occurrence per-section input.txt
~~~~


Error recovery
--------------

`--recovery` controls behaviour when an unrecoverable parse error occurs
(currently relevant for HTML input only):

 -  `strict` (default) — abort with an error
 -  `lenient` — skip the problematic fragment and continue

~~~~ sh
gukhanmun -f text/html --recovery lenient input.html
~~~~
