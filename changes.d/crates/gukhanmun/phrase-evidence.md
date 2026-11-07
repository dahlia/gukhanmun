---
links:
  '#41': https://github.com/dahlia/gukhanmun/issues/41
  '#45': https://github.com/dahlia/gukhanmun/pull/45
---
 -  Re-exported phrase evidence types for constructing and validating custom
    dictionary constraints. [[#41], [#45]]

 -  Added `HanjaDictionary::has_owned_phrase_evidence()` so dictionary
    implementations can check for owned phrase constraints without collecting
    them. Built-in dictionaries and nested chains avoid temporary evidence
    vectors during repeated source lookups. [[#41], [#45]]
