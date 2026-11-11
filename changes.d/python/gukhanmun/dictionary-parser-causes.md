---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#49': https://github.com/dahlia/gukhanmun/issues/49
  '#57': https://github.com/dahlia/gukhanmun/pull/57
---
 -  Fixed `GukhanmunError.chain` to retain the underlying causes of malformed
    FST and CDB dictionary data, such as an undecodable FST map or dictionary
    metadata, instead of reporting only the outer error message. Errors without
    an underlying cause still have an empty chain. [[#49], [#15], [#57]]
