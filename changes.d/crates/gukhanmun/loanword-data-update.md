---
links:
  '#18': https://github.com/dahlia/gukhanmun/issues/18
  '#26': https://github.com/dahlia/gukhanmun/pull/26
---
 -  Added the `loanwords` feature, `loanwords` module and
    `Builder::bundled_loanwords()` to compose loanword examples after user
    dictionaries and before preset dictionaries. Both presets leave it disabled
    by default. [[#18], [#26]]
 -  Changed weekly data updates to target the maintenance branch of the newest
    stable release series that ships the dictionary, with a separate PR keeping
    *main* current. Before the dictionary's first stable release, updates target
    *main* only. [[#18], [#26]]
