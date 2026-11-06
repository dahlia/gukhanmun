---
links:
  '#42': https://github.com/dahlia/gukhanmun/issues/42
  '#44': https://github.com/dahlia/gukhanmun/pull/44
---
 -  Added opt-in `ChainDictionary` source policies and role-aware registration.
    Reference-authorized forms and independent loanword candidates keep their
    own identities, marks, and positional readings, with one particle filter
    over the ranked pool. Direct chains retain `Ordered` by default.
    [[#42], [#44]]
 -  Preserved selected source identity through annotations, homophone and
    first-occurrence handling, directives, and parenthetical reading changes.
    Source-aware dictionary matches retain separate annotation boundaries
    beside fallback characters. [[#42], [#44]]
 -  Fixed lattice streaming for overlapping dictionary matches so chunk
    boundaries do not change the selected segmentation. Ambiguous prefixes may
    remain buffered until a word boundary or EOF settles the path.
    [[#42], [#44]]
 -  Reduced cooperative lookup overhead for dictionaries with unknown maximum
    word lengths without truncating their queries or limiting long matches.
    [[#42], [#44]]
