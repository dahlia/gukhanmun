---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#48': https://github.com/dahlia/gukhanmun/issues/48
  '#55': https://github.com/dahlia/gukhanmun/pull/55
---
 -  Fixed CDB dictionary loading to reject malformed file layouts with a
    `dictionary-load` error. Crafted hash tables can no longer force long
    lookup probe loops during conversion. [[#48], [#15], [#55]]

 -  Changed CDB loading to reject dictionaries whose record index requests
    more than 64 MiB, before allocating the index. Loaded file bytes,
    decoded metadata and payloads, and allocator overhead use additional
    memory. [[#48], [#15], [#55]]
