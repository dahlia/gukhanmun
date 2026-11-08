---
links:
  '#41': https://github.com/dahlia/gukhanmun/issues/41
  '#45': https://github.com/dahlia/gukhanmun/pull/45
---
 -  Added source-attested phrase resolution to the shared conversion engine,
    with phrase constraints bundled from the *Standard Korean Dictionary*.
    `無條件降服` now uses the attested `무조건항복` reading; ambiguous readings
    without matching phrase evidence keep their existing defaults. Policy
    fallbacks preserve existing single-character annotation merging and
    particle decisions, including under nested source policies. [[#41], [#45]]
 -  Re-exported phrase evidence types, indexes, source choices, and resolution
    reports for custom dictionaries, including
    `HanjaDictionary::has_owned_phrase_evidence()` and
    `PhraseIndex::from_shared_evidence()` for reusing immutable evidence and
    its validation. Reports distinguish `LegacySelection` fallbacks and
    `OpaqueLegacyProjection` source ranges from phrase candidates.
    [[#41], [#45]]
 -  Engines ignore structurally invalid custom phrase evidence, or evidence
    with an incompatible index, and emit a warning. Providers must validate
    candidate links with `PhraseEvidence::validate_records()`; binary
    dictionary loaders reject invalid evidence. [[#41], [#45]]
