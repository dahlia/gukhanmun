---
links:
  '#38': https://github.com/dahlia/gukhanmun/issues/38
  '#39': https://github.com/dahlia/gukhanmun/pull/39
---
 -  Added `aliases::align_morphemes()`, `aliases::derive_aliases()`, and
    `aliases::sibling_relations()` for aligning marked compound headwords,
    applying the same-source-id ambiguity gate, and emitting recognition
    aliases plus sibling relations.  [[#38], [#39]]
 -  Added `aliases::grid_aliases()` to reconstruct omitted position-wise
    combinations inside multi-spelling headwords, limited to single-reading
    groups.  [[#38], [#39]]
 -  Substitute every licensed constituent of a compound, taking the bounded
    Cartesian product instead of one chunk at a time, and match a non-initial
    constituent whose reading differs only by the word-initial sound law.
    Reviewed suffix readings such as `욕`, `암`, and `리` may act as
    single-character donors, and reviewed `역사` compounds can opt back in
    through `AliasPolicy::approved_compounds`.  [[#38], [#39]]
 -  Bound the Cartesian enumeration at `AliasPolicy::max_combinations` so a
    pathological spelling group is skipped instead of exhausting memory.
    [[#38], [#39]]
