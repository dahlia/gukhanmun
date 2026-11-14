---
title: Conversion options
description: Choose Python presets, segmentation and numeral strategies, and control homophone annotations, first occurrences and punctuation.
---

Conversion options
==================

Pass options as keyword arguments to `Converter(...)`. Options are immutable
after construction; create another converter for a different configuration.
Use enum members, such as `Preset.KO_KP`, rather than raw strings. To read a
string from configuration, convert it explicitly with `Preset("ko-kp")`.


Presets
-------

`preset` supplies the default initial sound law and homophone window:

| Preset                   | Initial sound law | Homophone window          |
| ------------------------ | ----------------- | ------------------------- |
| `Preset.KO_KR` (default) | `True`            | `ContextWindow.PER_BLOCK` |
| `Preset.KO_KP`           | `False`           | `ContextWindow.OFF`       |

Neither preset loads official dictionaries. Install and add them explicitly;
see [*Dictionaries*](./dictionaries.md). An explicit option overrides its
preset:

~~~~ python
from gukhanmun import ContextWindow, Converter, Preset

converter = Converter(
    preset=Preset.KO_KP,
    initial_sound_law=True,
    homophone_window=ContextWindow.PER_DOCUMENT,
)
assert converter.options.initial_sound_law is True
~~~~

`converter.options` contains the resolved values, including a fully resolved
punctuation profile. It and its nested records are read-only.


Segmentation
------------

`segmentation` chooses how dictionary matches divide a hanja run into words.
The default `Segmentation.LATTICE` compares candidate paths with dynamic
programming. `Segmentation.EAGER` takes the longest match at each position;
it can choose a worse boundary when a long match leaves an unmatched suffix.

~~~~ python
from gukhanmun import Converter, Segmentation

converter = Converter(segmentation=Segmentation.LATTICE)
~~~~

Keep the default for general document conversion. Evaluate `EAGER` against
your own vocabulary before choosing it.


Numerals
--------

`numerals` controls the conversion of hanja numeral sequences. These examples
use character fallback, without a dictionary:

| `NumeralStrategy`           | 二〇一六年 | 十一月 | 一千二百三十四 |
| --------------------------- | ---------- | ------ | -------------- |
| `HANGUL_PHONETIC` (default) | 이공일륙년 | 십일월 | 일천이백삼십사 |
| `POSITIONAL_ARABIC`         | 2016년     | 십일월 | 일천이백삼십사 |
| `ADDITIVE_ARABIC`           | 이공일륙년 | 11월   | 1234           |
| `SMART`                     | 2016년     | 11월   | 1234           |

~~~~ python
from gukhanmun import Converter, NumeralStrategy

converter = Converter(numerals=NumeralStrategy.SMART)
assert converter.convert("二〇一六年 十一月") == "2016년 11월"
assert converter.convert("百濟 十長生") == "백제 십장생"
~~~~

`POSITIONAL_ARABIC` handles digit sequences; `ADDITIVE_ARABIC` handles
quantities with place-value markers such as 十 and 百. `SMART` combines both
and keeps phonetic readings for sequences that read as words, such as 十長生
in the example.


Initial sound law
-----------------

`initial_sound_law` changes certain word-initial fallback readings to South
Korean forms. For example, `來日` reads `내일` when enabled and `래일` when
disabled. Dictionary entries already supply their readings; this setting does
not rewrite them.

~~~~ python
from gukhanmun import Converter

assert Converter(initial_sound_law=True).convert("來日") == "내일"
assert Converter(initial_sound_law=False).convert("來日") == "래일"
~~~~


Parenthetical reading annotations
---------------------------------

With a dictionary entry for `庫間` read as `곳간`, the default
`collapse_redundant_parens=True` recognizes an existing reading annotation in
either order, such as `庫間(곳간)` or `곳간(庫間)`, and renders one annotation
instead of duplicating it. In hangul-only mode the result is `곳간(庫間)`.
Set the option to `False` to disable this handling. The same input `庫間(곳간)`
then becomes `곳간(곳간)`: the word converts while the existing parentheses
remain.

A gloss that matches the dictionary reading is accepted, including readings
with a linking ㅅ (사이시옷). An alternative reading must have one hangul
syllable per hanja, with each syllable a valid Sino-Korean reading of that
character. Thus `數字(수자)` can select `수자` for that occurrence. Definition
glosses and foreign transliterations that fail these rules stay unchanged.


Homophones
----------

`homophone_window` sets where two recognized words with the same reading can
cause a distinguishing hanja gloss. With hangul-only rendering:

~~~~ python
from gukhanmun import ContextWindow, Converter, DictionaryEntry, MapDictionarySource

converter = Converter(
    dictionaries=[MapDictionarySource([
        DictionaryEntry("甲", "한"),
        DictionaryEntry("乙", "한"),
    ])],
    homophone_window=ContextWindow.PER_DOCUMENT,
)
assert converter.convert("甲 乙") == "한(甲) 한(乙)"
assert converter.convert("甲") == "한"
~~~~

Each `convert()` call starts a new document and its own tracking state.

For HTML and Markdown, the windows use these structural scopes:

| Window                       | Scope                              |
| ---------------------------- | ---------------------------------- |
| `ContextWindow.OFF`          | No homophone tracking              |
| `ContextWindow.PER_BLOCK`    | Paragraph, list and heading blocks |
| `ContextWindow.PER_SECTION`  | Until the next heading             |
| `ContextWindow.PER_DOCUMENT` | The entire input                   |

With `InputFormat.TEXT`, both `PER_BLOCK` and `PER_SECTION` cover the entire
document. Blank lines and heading-like text do not create boundaries. These
format-dependent scopes also apply to `first_occurrence_window`.

`homophone_detection` defaults to `HomophoneDetection.CONTEXT_LOCAL`, which
requires a conflicting word to appear in the window. `DICTIONARY_WIDE` also
considers other forms with the same reading anywhere in the loaded dictionary.
With a large dictionary this can add many glosses. To always annotate a chosen
form, use a [directive](./directives.md) instead.

Disambiguation works on recognized dictionary units. A sequence read through
character fallback is not automatically a word with a lexical identity.
Add a [dictionary entry](./dictionaries.md) when an entire term needs to take
part in homophone detection.


First occurrences
-----------------

`first_occurrence_window` clears repeated dictionary-entry `require_hanja` and
`require_hangul` requirements within the selected window. Explicit reading
glosses in the input and directives keep their requirements, and homophone
marks remain. Rendering modes that always show both scripts still do so. The
option accepts the same four `ContextWindow` values and defaults to `OFF`,
independently of the homophone window.

~~~~ python
from gukhanmun import ContextWindow, Converter, DictionaryEntry, MapDictionarySource

converter = Converter(
    dictionaries=[MapDictionarySource([
        DictionaryEntry("漢字", "한자", require_hanja=True),
    ])],
    first_occurrence_window=ContextWindow.PER_DOCUMENT,
)
assert converter.convert("漢字 漢字") == "한자(漢字) 한자"
~~~~


Punctuation and recovery
------------------------

Punctuation normalization is independent of dictionary and rendering choices:

~~~~ python
from gukhanmun import Converter, CustomPunctuationProfile, PunctuationPreset

converter = Converter(punctuation=CustomPunctuationProfile(
    base=PunctuationPreset.MODERN,
    period=None,
))
assert converter.convert("가、나。 다") == "가, 나。 다"
~~~~

An omitted custom field inherits the base profile through `UNSET`; `None`
preserves the input punctuation for that role. See
[*Punctuation profiles*](../punctuation.md) for recognized punctuation and the
[API reference](../../api/python.md#punctuation-profiles) for Python types.

`recovery` controls malformed HTML handling. It does not change plain-text or
Markdown conversion. See [*HTML processing*](./html.md#error-recovery).
