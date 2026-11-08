---
links:
  '#38': https://github.com/dahlia/gukhanmun/issues/38
  '#39': https://github.com/dahlia/gukhanmun/pull/39
---
 -  Added `aliases::align_morphemes()`, `aliases::derive_aliases()`,
    `aliases::sibling_relations()`, and `aliases::grid_aliases()` for deriving
    compound recognition aliases and sibling relations from marked headwords.
    Derivation checks source identifiers for ambiguity and reconstructs
    omitted position-wise spelling combinations inside multi-spelling headwords
    only for single-reading groups.
    [[#38], [#39]]
 -  Added `AliasPolicy` controls for compound derivation, including reviewed
    single-character suffix donors, `AliasPolicy::approved_compounds`, and
    `max_combinations`. Derivation combines licensed constituent substitutions,
    accepts word-initial sound-law variants in non-initial constituents, and
    skips spelling groups that exceed the combination limit. [[#38], [#39]]
