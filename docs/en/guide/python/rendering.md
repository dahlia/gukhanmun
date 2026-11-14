---
title: Rendering modes
description: Compare Python rendering modes, add ruby or parenthetical glosses, and select the hanja variants used in output.
---

Rendering modes
===============

`rendering` controls how a recognized word and its reading appear. The table
shows `漢字` with a dictionary reading of `한자`, without homophone or directive
marks:

| `RenderMode`            | Plain-text output |
| ----------------------- | ----------------- |
| `HANGUL_ONLY` (default) | 한자              |
| `HANGUL_HANJA_PARENS`   | 한자(漢字)        |
| `HANJA_HANGUL_PARENS`   | 漢字(한자)        |
| `RUBY_ON_HANGUL`        | 한자(漢字)        |
| `RUBY_ON_HANJA`         | 漢字(한자)        |
| `ORIGINAL`              | 漢字              |

~~~~ python
from gukhanmun import Converter, DictionaryEntry, MapDictionarySource, RenderMode

source = MapDictionarySource([DictionaryEntry("漢字", "한자")])
converter = Converter(
    dictionaries=[source],
    rendering=RenderMode.HANGUL_HANJA_PARENS,
)
assert converter.convert("漢字") == "한자(漢字)"
~~~~


Ruby markup
-----------

Ruby modes produce `<ruby>` elements in HTML and Markdown where inline markup
is allowed. In plain text, and in text-only HTML elements such as `<title>`
and `<option>`, they fall back to parentheses.

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource, RenderMode

converter = Converter(
    dictionaries=[MapDictionarySource([DictionaryEntry("漢字", "한자")])],
    rendering=RenderMode.RUBY_ON_HANJA,
)
assert converter.convert("<p>漢字</p>", format=InputFormat.HTML) == (
    "<p><ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby></p>"
)
~~~~

`RUBY_ON_HANGUL` uses hangul as the base and hanja inside `<rt>` instead.
The `<rp>` elements provide parentheses for browsers without ruby support.


Original text with a gloss
--------------------------

`ORIGINAL` keeps hanja and adds a reading when an annotation carries a
`require_hangul` mark. Homophone disambiguation alone does not add a reading
in this mode.
`original_gloss` chooses parentheses or ruby for those readings:

~~~~ python
from gukhanmun import (
    Converter, DictionaryEntry, InputFormat, MapDictionarySource,
    OriginalGloss, RenderMode,
)

converter = Converter(
    dictionaries=[MapDictionarySource([
        DictionaryEntry("漢字", "한자", require_hangul=True),
    ])],
    rendering=RenderMode.ORIGINAL,
    original_gloss=OriginalGloss.RUBY,
)
assert converter.convert("漢字", format=InputFormat.HTML) == (
    "<ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>"
)
assert converter.convert("漢字") == "漢字(한자)"
~~~~

`OriginalGloss.PARENS` is the default. `original_gloss` affects only
`RenderMode.ORIGINAL`; it does not select the main rendering mode.


Hanja variant sets
------------------

A hanja character can have several variant spellings. Recognition of supported
variants is always enabled. `hanja_variant_set` controls the
spelling emitted after recognition by modes that retain hanja; it does not
choose a dictionary or a reading.

~~~~ python
from gukhanmun import Converter, HanjaVariantSet, RenderMode

converter = Converter(
    rendering=RenderMode.ORIGINAL,
    hanja_variant_set=HanjaVariantSet.SHINJITAI,
)
~~~~

| `HanjaVariantSet`         | Output spelling                                                    |
| ------------------------- | ------------------------------------------------------------------ |
| `AS_DICTIONARY` (default) | Keep the spelling selected by the dictionary                       |
| `SHINJITAI`               | Japanese Joyo kanji new forms                                      |
| `KANXI`                   | Prefer traditional and compatibility-folded forms                  |
| `SIMPLIFIED`              | Use the first Unicode `kSimplifiedVariant` mapping where available |
| `ASAHIMOJI`               | Shinjitai plus Gukhanmun's verified Asahi mappings                 |
