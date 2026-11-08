---
links:
  '#38': https://github.com/dahlia/gukhanmun/issues/38
  '#39': https://github.com/dahlia/gukhanmun/pull/39
  '#41': https://github.com/dahlia/gukhanmun/issues/41
  '#42': https://github.com/dahlia/gukhanmun/issues/42
  '#44': https://github.com/dahlia/gukhanmun/pull/44
  '#45': https://github.com/dahlia/gukhanmun/pull/45
---
 -  Added `canonical` and `alias_kind` columns and `--relations` for compound
    aliases, `--evidence` for typed source evidence, and `--phrase-evidence`
    for source-attested phrase constraints. Rust callers can use
    `build_dictionary_with_evidence()` or
    `build_dictionary_with_all_evidence()`. Embedding phrase constraints does
    not add phrase keys to word candidates. Builds use format version 4 when
    either kind of evidence is supplied, version 3 for aliases alone, and
    version 1 or 2 otherwise. [[#38], [#39], [#41], [#42], [#44], [#45]]
