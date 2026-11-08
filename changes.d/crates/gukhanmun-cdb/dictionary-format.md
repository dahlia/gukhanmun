---
links:
  '#38': https://github.com/dahlia/gukhanmun/issues/38
  '#39': https://github.com/dahlia/gukhanmun/pull/39
  '#41': https://github.com/dahlia/gukhanmun/issues/41
  '#42': https://github.com/dahlia/gukhanmun/issues/42
  '#44': https://github.com/dahlia/gukhanmun/pull/44
  '#45': https://github.com/dahlia/gukhanmun/pull/45
---
 -  Added support for dictionary format version 3 with compound alias metadata
    and version 4 with candidate-linked source evidence and source-attested
    phrase constraints. Version 1 and 2 files remain readable; version 4
    evidence is validated on load, and unsupported or stale constraints are
    rejected. Phrase indexes are cached per dictionary for reuse across
    engines. [[#38], [#39], [#41], [#42], [#44], [#45]]
