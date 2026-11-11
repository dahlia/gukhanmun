---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#49': https://github.com/dahlia/gukhanmun/issues/49
  '#57': https://github.com/dahlia/gukhanmun/pull/57
---
 -  Added the `stdict`, `opendict` and `loanwords` extras, which install the
    FST data of the *Standard Korean Language Dictionary*, the *Open Korean
    Dictionary* and the NIKL loanword spelling examples, and the
    format-specific `stdict-fst`, `stdict-cdb`, `opendict-fst`, `opendict-cdb`,
    `loanwords-fst` and `loanwords-cdb` extras; for example,
    `pip install 'gukhanmun[stdict]'`. Each extra pins its data distribution to
    exactly the version of `gukhanmun`. Installing an extra does not add a
    dictionary to any converter, and neither preset loads official data.
    [[#49], [#15], [#57]]
