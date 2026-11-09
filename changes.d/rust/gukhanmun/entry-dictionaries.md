---
links:
  '#33': https://github.com/dahlia/gukhanmun/issues/33
  '#34': https://github.com/dahlia/gukhanmun/pull/34
---
 -  Added `MapDictionary::from_candidates()` and `CandidateError::EmptyKey`
    through the umbrella re-export, and made `MapDictionary::max_word_chars()`
    report `Some(0)` while the map is empty so dictionaries chained after it
    keep their streaming word bound.  [[#33], [#34]]
