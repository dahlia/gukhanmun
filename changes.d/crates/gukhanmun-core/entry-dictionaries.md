---
links:
  '#33': https://github.com/dahlia/gukhanmun/issues/33
  '#34': https://github.com/dahlia/gukhanmun/pull/34
---
 -  Added `MapDictionary::from_candidates()` for building a dictionary from
    per-reading entries: repeated keys are grouped into one candidate set
    instead of replacing earlier readings, and preferences are normalised as
    `ReadingCandidates::new()` does. Added `CandidateError::EmptyKey` for an
    empty hanja key, which would otherwise match at every offset.
    [[#33], [#34]]
 -  Changed `MapDictionary::max_word_chars()` to report `Some(0)` while the map
    is empty, so an empty user dictionary no longer hides the word bound of the
    dictionaries chained after it while streaming.  [[#33], [#34]]
