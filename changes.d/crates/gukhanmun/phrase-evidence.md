---
links:
  '#41': https://github.com/dahlia/gukhanmun/issues/41
  '#45': https://github.com/dahlia/gukhanmun/pull/45
---
 -  Re-exported phrase evidence types for constructing and validating custom
    dictionary constraints. [[#41], [#45]]

 -  Added `HanjaDictionary::has_owned_phrase_evidence()` so dictionary
    implementations can check for owned phrase constraints without collecting
    them. Built-in dictionaries and nested chains avoid temporary evidence
    vectors during repeated source lookups. [[#41], [#45]]

 -  Invalid phrase evidence supplied by custom dictionaries is now ignored
    with a warning instead of causing a panic. The same applies to evidence
    whose shared index is incompatible with it; compatible indexes remain
    shared. Binary dictionary loaders still reject invalid evidence.
    [[#41], [#45]]

 -  Added `PhraseIndex::from_shared_evidence()` to share an immutable phrase
    collection with its index. Engines reuse validation for that collection
    without scanning it again. [[#41], [#45]]
