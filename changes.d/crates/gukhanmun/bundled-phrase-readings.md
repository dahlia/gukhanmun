---
links:
  '#41': https://github.com/dahlia/gukhanmun/issues/41
  '#45': https://github.com/dahlia/gukhanmun/pull/45
---
 -  Bundled source-attested phrase constraints from the *Standard Korean
    Dictionary*. Conversions such as `無條件降服` now use the attested
    `무조건항복` reading, while ambiguous readings without matching phrase
    evidence keep their existing defaults. [[#41], [#45]]
 -  Retain nested-policy fallbacks separately from phrase witnesses with
    `LegacySelection` and `OpaqueLegacyProjection` diagnostics. Existing
    single-character annotation merging and particle decisions are preserved.
    [[#41], [#45]]
