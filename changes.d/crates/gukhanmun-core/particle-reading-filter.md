---
links:
  '#20': https://github.com/dahlia/gukhanmun/issues/20
  '#25': https://github.com/dahlia/gukhanmun/pull/25
---
 -  Added automatic reading selection from a fully delimited following Korean
    particle, including `으로/로` and combinations such as `으로는`. Conversion
    keeps the source particle and falls back to the dictionary default when
    evidence is uncertain or no reading fits; explicit dictionary overrides
    stay authoritative. [[#20], [#25]]
 -  Changed `Engine` streaming boundaries: a sole non-block root `Close` with
    pending text is emitted at the next token, `flush()`, or `finish()`.
    `finish()` supplies EOF evidence while `flush()` stays conservative;
    particle lookahead retains four extra source scalars beyond the dictionary
    tail allowance. [[#20], [#25]]
