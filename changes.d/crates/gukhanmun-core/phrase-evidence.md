---
links:
  '#41': https://github.com/dahlia/gukhanmun/issues/41
  '#45': https://github.com/dahlia/gukhanmun/pull/45
---
 -  Added validated phrase evidence types and
    `HanjaDictionary::phrase_evidence()` for transporting source-attested
    constraints independently of word candidates. [[#41], [#45]]

 -  Added `HanjaDictionary::has_owned_phrase_evidence()` so dictionary
    implementations can check for owned phrase constraints without collecting
    them. Built-in dictionaries and nested chains avoid temporary evidence
    vectors during repeated source lookups. [[#41], [#45]]
