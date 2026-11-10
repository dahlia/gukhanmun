---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#48': https://github.com/dahlia/gukhanmun/issues/48
  '#55': https://github.com/dahlia/gukhanmun/pull/55
---
 -  Fixed FST dictionary loading to limit the number of accepted keys,
    their total byte length and traversal work before enumerating keys, even
    when maximum-length metadata is supplied. Files exceeding these limits
    now fail to load.
    [[#48], [#15], [#55]]
