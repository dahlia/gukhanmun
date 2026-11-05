---
links:
  '#38': https://github.com/dahlia/gukhanmun/issues/38
  '#39': https://github.com/dahlia/gukhanmun/pull/39
---
 -  Canonical literals in the require-hanja directive and related directives
    now reach reviewed alias spellings. The streaming plain-text path keeps
    streaming and applies each directive with the dictionary's lexeme index, so
    a reviewed alias resolves without buffering the whole document.
    [[#38], [#39]]
