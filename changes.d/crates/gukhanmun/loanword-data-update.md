---
links:
  '#18': https://github.com/dahlia/gukhanmun/issues/18
  '#26': https://github.com/dahlia/gukhanmun/pull/26
  '#31': https://github.com/dahlia/gukhanmun/pull/31
---
 -  Added the `loanwords` feature, `loanwords` module and
    `Builder::bundled_loanwords()` to compose loanword examples after user
    dictionaries and before preset dictionaries. The `loanwords` feature is
    enabled by default; `ko-kr` includes the dictionary automatically, while
    `ko-kp` leaves it disabled. Use `Builder::no_bundled_loanwords()` to
    disable only this dictionary. Builds without default features must enable
    `loanwords` or call `Builder::no_bundled_loanwords()` for `ko-kr`; otherwise
    `build()` returns `Error::Config`. [[#18], [#26], [#31]]
 -  Changed weekly data updates to target the maintenance branch of the newest
    stable release series that ships the dictionary, with a separate PR keeping
    *main* current. Before the dictionary's first stable release, updates target
    *main* only. [[#18], [#26]]
