---
links:
  '#11': https://github.com/dahlia/gukhanmun/issues/11
  '#12': https://github.com/dahlia/gukhanmun/pull/12
---
 -  Added dictionary-guided recognition of Unicode z-variants,
    traditional/simplified forms, CJK compatibility ideographs, Joyo kanji
    old/new forms, and Asahi variants.  Ambiguous variant relations are
    accepted only when the dictionary identifies one matching sense, while
    exact source matches remain authoritative.  Compatibility ideographs are
    folded before fallback reading and numeral lookup.  [[#11], [#12]]

 -  Added `HanjaVariantSet` with `AsDictionary`, `Shinjitai`, `Kanxi`,
    `Simplified`, and `Asahimoji` output profiles.
    `Annotation::canonical_hanja()` exposes the dictionary spelling for a
    dictionary match and the source hanja for a fallback annotation, so
    rendering and policy middlewares use the same lexical identity.
    [[#11], [#12]]
