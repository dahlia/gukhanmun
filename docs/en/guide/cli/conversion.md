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


Parenthetical reading annotations
---------------------------------

Mixed-script input sometimes spells a word together with an explicit
parenthetical gloss, whether hanja-first (`庫間(곳간)`) or hangul-first
(`곳간(庫間)`).  By default Gukhanmun recognizes such a gloss, removes the now
redundant parenthetical, and shows the word in both scripts:

| Input        | Default output | `--no-collapse-parens` |
| ------------ | -------------- | ---------------------- |
| `庫間(곳간)` | `곳간(庫間)`   | `곳간(곳간)`           |
| `곳간(庫間)` | `곳간(庫間)`   | `곳간(곳간)`           |

A parenthetical can also pin an alternative reading.  `數字` normally reads
`숫자`, but `數字(수자)` fixes the reading to `수자` for that occurrence:

~~~~ sh
echo '數字(수자)' | gukhanmun  # 수자(數字)
~~~~

A reading annotation is told apart from a definition by two rules.  A
parenthetical that exactly matches the word's reading always collapses (this
covers 사이시옷 readings like `庫間(곳간)`).  Otherwise, an alternative reading
is accepted only when it has one hangul syllable per hanja character, each a
valid Sino-Korean reading of that character (as in `數字(수자)`).  A definition
gloss matches neither rule and passes through untouched:

~~~~ sh
echo '庫間(물건을 간직하여 두는 곳)' | gukhanmun
# 곳간(물건을 간직하여 두는 곳)
~~~~

Foreign transliterations are likewise left alone, because they are not valid
per-character readings (for example `蔣介石(장제스)`, where `介` reads `개`, not
`제`).  Pass `--no-collapse-parens` to disable the behaviour entirely.


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
but with the bundled *Standard Korean Dictionary* nearly every common reading
has some homophone, so it glosses most Sino-Korean words.  To always gloss a
specific word regardless of context, use the `--require-hanja` flag instead
(see [*User directives*](./directives.md)).


Only recognized words are disambiguated
---------------------------------------

Homophone disambiguation operates on words the dictionary recognizes as units.
A hanja sequence with no dictionary entry of its own is not treated as a single
word, and its fallback (non-dictionary) characters are never glossed; any
recognized single-character entries inside it (such as `紫`) are still handled
on their own.  For example, `自由` and `子游` are both bundled entries read
`자유`, so `自由와 子游` becomes `자유(自由)와 자유(子游)`; but `紫楡` has no
entry of its own, so under the default context-local strategy `自由와 紫楡`
becomes `자유와 자유` with no gloss, because the engine never sees a second
`자유` unit to collide with `自由`.  To disambiguate the whole term, add it to
a custom dictionary and load it with `--dictionary` (see
[*Dictionaries*](./dictionary.md)) so the engine treats it as a single unit.


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
