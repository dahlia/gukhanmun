---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#48': https://github.com/dahlia/gukhanmun/issues/48
  '#55': https://github.com/dahlia/gukhanmun/pull/55
---
 -  Fixed CDB and FST dictionary loading to reject metadata that exceeds
    decoded-allocation admission budgets before expanding aliases or typed
    evidence. Small files with many alias rows can no longer bypass loading
    limits through metadata expansion. [[#48], [#15], [#55]]
