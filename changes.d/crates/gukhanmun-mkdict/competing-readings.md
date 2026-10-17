---
links:
  '#19': https://github.com/dahlia/gukhanmun/issues/19
  '#22': https://github.com/dahlia/gukhanmun/pull/22
---
 -  Accept candidate rows with `preference` and `suffix_reading` in TSV/CSV,
    or `preference` and `suffixReading` in JSONL. Merge policies replace whole
    sets across files, and reading rules apply to individual candidates. The
    builder emits v1 for scalar dictionaries and v2 when candidates or suffixes
    require it; old runtimes must be updated to read v2.  [[#19], [#22]]
