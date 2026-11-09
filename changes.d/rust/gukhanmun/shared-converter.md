---
links:
  '#46': https://github.com/dahlia/gukhanmun/issues/46
  '#53': https://github.com/dahlia/gukhanmun/pull/53
---
 -  Added `SharedBuilder` and `SharedConverter` so multiple threads can reuse
    one configured converter. Shared builders accept owned `Send + Sync`
    dictionaries and HTML preservation predicates; existing `Builder` and
    `Converter` still accept borrowed dictionaries and local callbacks.
    [[#46], [#53]]
