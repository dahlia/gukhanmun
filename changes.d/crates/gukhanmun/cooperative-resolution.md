---
links:
  '#42': https://github.com/dahlia/gukhanmun/issues/42
  '#44': https://github.com/dahlia/gukhanmun/pull/44
---
 -  The exported core pipeline preserves source-aware selection metadata through
    annotations and middleware, including aliases, particles, authorial glosses,
    and first-occurrence/homophone identity. Source-aware dictionary matches
    retain their own annotation boundaries. [[#42], [#44]]
 -  Fixed lattice streaming for overlapping dictionary matches so chunk
    boundaries do not change the selected segmentation. Ambiguous prefixes may
    remain buffered until a word boundary or EOF settles the path.
    [[#42], [#44]]
