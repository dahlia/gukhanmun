---
links:
  '#84': https://github.com/dahlia/gukhanmun/issues/84
  '#85': https://github.com/dahlia/gukhanmun/pull/85
---
 -  Added `.annotation_granularity(AnnotationGranularity::HanjaRun)` to the
    converter builders. It shows `汽車길` as `기찻(汽車)길` in hangul-first
    parentheses and omits repeated hangul in ruby as well. Whole-word
    annotations remain the default. [[#84], [#85]]
