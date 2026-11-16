---
links:
  '#42': https://github.com/dahlia/gukhanmun/issues/42
  '#44': https://github.com/dahlia/gukhanmun/pull/44
---
 -  `Builder` and presets now default to reference-authorized source
    cooperation, retaining reference readings unless typed dictionary relations
    authorize another form. Configure `source_policy()` and `reference_only()`,
    or select `SourcePolicy::Ordered` for legacy precedence.
    `push_dictionary()` remains an authored override. The pipeline preserves
    selected source metadata through annotations and middleware, including
    aliases, particles, authorial glosses and first-occurrence/homophone
    identity. Source-aware dictionary matches retain their own annotation
    boundaries. [[#42], [#44]]
