---
links:
  '#19': https://github.com/dahlia/gukhanmun/issues/19
  '#22': https://github.com/dahlia/gukhanmun/pull/22
---
 -  Retain competing readings and source record IDs during extraction, with
    curated equal-priority decisions preserving all existing default readings.
    Candidate-specific suffixes now survive raw FST/CDB loading. Use
    `mise run stdict-extract` to regenerate and `mise run stdict-extract-check`
    to check a source dump without modifying outputs.  [[#19], [#22]]
