---
links:
  '#42': https://github.com/dahlia/gukhanmun/issues/42
  '#44': https://github.com/dahlia/gukhanmun/pull/44
---
 -  `Builder` and presets now default to reference-authorized source
    cooperation, retaining reference readings unless typed dictionary relations
    authorize another form. Configure `source_policy()` and `reference_only()`,
    or select `SourcePolicy::Ordered` for legacy precedence.
    `push_dictionary()` remains an authored override. [[#42], [#44]]
