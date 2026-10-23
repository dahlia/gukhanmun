---
links:
  '#18': https://github.com/dahlia/gukhanmun/issues/18
  '#26': https://github.com/dahlia/gukhanmun/pull/26
  '#31': https://github.com/dahlia/gukhanmun/pull/31
---
 -  Enabled the bundled NIKL loanword examples by default for `ko-kr`,
    before the preset dictionary and after user dictionaries. Use
    `--no-loanwords` to disable them, or `--loanwords` to enable them for
    `ko-kp`. `--no-bundled-dictionaries` disables all bundled dictionaries.
    [[#18], [#26], [#31]]
 -  Included *NOTICE.md* in CLI binary archives to retain attribution for
    bundled dictionary data, including the NIKL loanword examples.
    [[#18], [#26]]
 -  Changed weekly data updates to target the maintenance branch of the newest
    stable release series that ships the dictionary, with a separate PR keeping
    *main* current. Before the dictionary's first stable release, updates target
    *main* only. [[#18], [#26]]
