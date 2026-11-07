---
links:
  '#41': https://github.com/dahlia/gukhanmun/issues/41
  '#45': https://github.com/dahlia/gukhanmun/pull/45
---
 -  Preserve legacy single-character annotation merging and particle decisions
    when phrase evidence is present, including nested source policies.
    `LegacySelection` keeps policy fallbacks separate from leaf-owned phrase
    candidates. `OpaqueLegacyProjection` diagnostics identify projected fallback
    source ranges without granting phrase authority. [[#41], [#45]]
