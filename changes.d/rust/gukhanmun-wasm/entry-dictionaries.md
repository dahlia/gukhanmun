---
links:
  '#33': https://github.com/dahlia/gukhanmun/issues/33
  '#34': https://github.com/dahlia/gukhanmun/pull/34
---
 -  Accepted runtime dictionary entries from the JavaScript binding: a
    dictionary record now carries either a compiled artifact or an entry list
    that becomes a `MapDictionary`, so callers need no compiled artifact for
    readings computed at runtime. Malformed entry lists report `invalid-input`
    with the offending entry index.  [[#33], [#34]]
