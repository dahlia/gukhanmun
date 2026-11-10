---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#48': https://github.com/dahlia/gukhanmun/issues/48
  '#55': https://github.com/dahlia/gukhanmun/pull/55
---
 -  Fixed CDB dictionary loading to reject malformed file layouts with a
    `DICTIONARY_LOAD` error. Crafted hash tables can no longer force long
    lookup probe loops during conversion. [[#48], [#15], [#55]]
