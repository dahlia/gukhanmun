---
links:
  '#42': https://github.com/dahlia/gukhanmun/issues/42
  '#44': https://github.com/dahlia/gukhanmun/pull/44
---
 -  Fixed lattice streaming for overlapping dictionary matches so chunk
    boundaries do not change the selected segmentation. Ambiguous prefixes may
    remain buffered until a word boundary or EOF settles the path.
    [[#42], [#44]]
