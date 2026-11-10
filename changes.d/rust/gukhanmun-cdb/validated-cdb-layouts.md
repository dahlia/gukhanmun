---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#48': https://github.com/dahlia/gukhanmun/issues/48
  '#55': https://github.com/dahlia/gukhanmun/pull/55
---
 -  Fixed CDB loading to reject malformed file layouts before use. Crafted hash
    tables can no longer force long lookup probe loops during conversion; valid
    dictionary files remain supported. [[#48], [#15], [#55]]
