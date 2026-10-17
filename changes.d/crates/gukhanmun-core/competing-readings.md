---
links:
  '#19': https://github.com/dahlia/gukhanmun/issues/19
  '#22': https://github.com/dahlia/gukhanmun/pull/22
---
 -  Added `ReadingCandidate`, `ReadingCandidates`, candidate lookup/enumeration
    methods, and `MapDictionary::insert_candidates()`. Selection happens before
    segmentation; alternatives retain their own marks and positional forms.
    Existing custom dictionary implementations remain supported.  [[#19], [#22]]

 -  Dictionary-wide homophone marking indexes only selected candidates and
    includes their suffix readings.  [[#19], [#22]]
