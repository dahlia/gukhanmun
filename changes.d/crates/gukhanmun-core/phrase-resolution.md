---
links:
  '#41': https://github.com/dahlia/gukhanmun/issues/41
  '#45': https://github.com/dahlia/gukhanmun/pull/45
---
 -  Added source-attested phrase resolution over fixed segmentation paths,
    including complete-form intersections, source ownership, and finite
    additional streaming context. When evidence conflicts or exceeds work
    limits, the engine keeps policy fallbacks, preserving existing
    single-character annotation merging and particle decisions, including under
    nested source policies. [[#41], [#45]]
 -  Added phrase evidence types, `HanjaDictionary::phrase_evidence()` for
    transporting constraints independently of word candidates, and
    `HanjaDictionary::has_owned_phrase_evidence()` for checking for owned
    constraints without collecting them.
    Engines ignore structurally invalid evidence, or evidence with an
    incompatible index, and emit a warning; providers must validate candidate
    links with `PhraseEvidence::validate_records()`. Binary loaders reject
    invalid evidence. [[#41], [#45]]
 -  Added reusable `PhraseIndex` lookups,
    `PhraseIndex::from_shared_evidence()`, and opt-in `Engine` resolution
    reports for policy defaults and bounded-work limits. Engines reuse
    validation for shared immutable evidence without scanning it again. Reports
    distinguish `LegacySelection` fallbacks from phrase candidates and identify
    projected fallback source ranges with `OpaqueLegacyProjection` diagnostics
    without granting them phrase authority. [[#41], [#45]]
