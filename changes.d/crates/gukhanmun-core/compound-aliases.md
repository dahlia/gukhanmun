---
links:
  '#38': https://github.com/dahlia/gukhanmun/issues/38
---
 -  Added candidate-level alias metadata (`AliasKind`, `AliasTarget`,
    `ReadingCandidate::alias`) and `LexemeIndex`, and made homophone marking,
    first-occurrence filtering, and literal directives resolve sibling and
    reviewed spellings to one canonical lexeme. [[#38]]
 -  Added `MapDictionary::insert_alias()`,
    `MapDictionary::insert_alias_with_suffix()`, and `reverse_hanja()` for
    reviewed user aliases and canonical hangul-to-hanja lookup.  [[#38]]
