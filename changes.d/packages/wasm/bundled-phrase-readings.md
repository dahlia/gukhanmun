---
links:
  '#41': https://github.com/dahlia/gukhanmun/issues/41
  '#45': https://github.com/dahlia/gukhanmun/pull/45
---
 -  Added support for source-attested phrase constraints when callers load
    `@gukhanmun/stdict-fst` or `@gukhanmun/stdict-cdb` through the
    `dictionaries` configuration. Conversions such as `無條件降服` now use the
    attested `무조건항복` reading, while ambiguous readings without matching
    phrase evidence keep their existing defaults. [[#41], [#45]]
