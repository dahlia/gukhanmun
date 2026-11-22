---
links:
  '#84': https://github.com/dahlia/gukhanmun/issues/84
  '#85': https://github.com/dahlia/gukhanmun/pull/85
---
 -  Added `annotationGranularity: "hanja-run"` to `load()` to gloss only the
    hanja runs of a mixed-script word when the reading can be split uniquely.
    Whole-word glosses remain the default and are used when no unique split
    is available. [[#84], [#85]]
