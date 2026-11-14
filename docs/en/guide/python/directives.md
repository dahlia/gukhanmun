---
title: Directives
description: Use Python literal-hanja directives to require hanja or hangul glosses, or suppress annotation without changing reading selection.
---

Directives
==========

`Directives` adjusts how selected readings are presented. It does not add
dictionary entries or choose a different reading. Its three sequences contain
literal hanja forms, rather than glob patterns or Python callbacks.

~~~~ python
from gukhanmun import Converter, DictionaryEntry, Directives, MapDictionarySource

converter = Converter(
    dictionaries=[MapDictionarySource([DictionaryEntry("漢字", "한자")])],
    directives=Directives(require_hanja=["漢字"]),
)
assert converter.convert("漢字") == "한자(漢字)"
~~~~

Rules operate on conversion annotations. Add the whole term to a
[dictionary](./dictionaries.md) if it needs to be recognized as a single unit.
Dictionary identity metadata can mark an alternate spelling as a reviewed
alias of a canonical form. A literal rule for that canonical form then also
applies to the alias.


Combining directives with rendering
-----------------------------------

| Field             | Effect                                             |
| ----------------- | -------------------------------------------------- |
| `require_hanja`   | Keep hanja visible, including in hangul-only mode  |
| `require_hangul`  | Include a hangul gloss, including in original mode |
| `skip_annotation` | Use only the active mode's primary text            |

For `ORIGINAL`, a required hangul reading becomes a gloss:

~~~~ python
from gukhanmun import Converter, DictionaryEntry, Directives, MapDictionarySource, RenderMode

converter = Converter(
    dictionaries=[MapDictionarySource([DictionaryEntry("漢字", "한자")])],
    rendering=RenderMode.ORIGINAL,
    directives=Directives(require_hangul=["漢字"]),
)
assert converter.convert("漢字") == "漢字(한자)"
~~~~

`skip_annotation` suppresses parentheses and ruby rather than preserving the
input verbatim. Modes with hangul as their primary text keep the reading; modes
with hanja as their primary text keep hanja. To leave an HTML subtree
unconverted, use [HTML preservation rules](./html.md#custom-preservation).

One form may appear in both `require_hanja` and `require_hangul`.
`skip_annotation` suppresses annotation rendering even when a requirement is
also present. Input sequences are copied into immutable tuples at construction.
