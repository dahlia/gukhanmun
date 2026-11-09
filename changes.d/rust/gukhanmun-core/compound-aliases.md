---
links:
  '#38': https://github.com/dahlia/gukhanmun/issues/38
  '#39': https://github.com/dahlia/gukhanmun/pull/39
---
 -  Added candidate-level alias metadata (`AliasKind`, `AliasTarget`,
    `ReadingCandidate::alias`) and `LexemeIndex` for canonical lexeme lookup.
    Homophone marking shares one identity across alias kinds through
    `CandidateRecord::homophone_key()` and
    `LexemeIndex::homophone_canonical()`, avoiding false homophone glosses for
    canonical compounds with derived aliases; first-occurrence filtering and
    literal directives resolve sibling and reviewed spellings by lexical
    identity. [[#38], [#39]]
 -  Added `MapDictionary::insert_alias()`,
    `MapDictionary::insert_alias_with_suffix()`, and `reverse_hanja()` for
    reviewed user aliases and canonical hangul-to-hanja lookup. [[#38], [#39]]
