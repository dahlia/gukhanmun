---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#48': https://github.com/dahlia/gukhanmun/issues/48
  '#55': https://github.com/dahlia/gukhanmun/pull/55
---
 -  Added `check_metadata_resources()` and `check_phrase_index_resources()`
    for checking decoded-allocation admission budgets before expanding
    dictionary aliases, typed evidence or phrase indexes. CDB and FST loaders
    use these checks to reject excessive metadata before allocation, including
    small files with many alias rows. Dictionaries with 32,768 distinct short
    aliases remain loadable within the budgets. Admission charges are
    conservative estimates, not limits on total process memory.
    [[#48], [#15], [#55]]
