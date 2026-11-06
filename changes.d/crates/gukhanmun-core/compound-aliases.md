---
links:
  '#38': https://github.com/dahlia/gukhanmun/issues/38
  '#39': https://github.com/dahlia/gukhanmun/pull/39
---
 -  Added candidate-level alias metadata (`AliasKind`, `AliasTarget`,
    `ReadingCandidate::alias`) and `LexemeIndex`, and made homophone marking,
    first-occurrence filtering, and literal directives resolve sibling and
    reviewed spellings to one canonical lexeme. [[#38], [#39]]
 -  Added `MapDictionary::insert_alias()`,
    `MapDictionary::insert_alias_with_suffix()`, and `reverse_hanja()` for
    reviewed user aliases and canonical hangul-to-hanja lookup.  [[#38], [#39]]
 -  Added `CandidateRecord::homophone_key()` and
    `LexemeIndex::homophone_canonical()` so every alias kind shares one
    homophone identity. A canonical compound is no longer glossed as a
    homophone merely because a derived alias exists for it, while
    first-occurrence filtering and literal directives still use lexical
    identity.  [[#38], [#39]]
