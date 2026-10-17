---
links:
  '#18': https://github.com/dahlia/gukhanmun/issues/18
  '#26': https://github.com/dahlia/gukhanmun/pull/26
---
 -  Added loanword collection and deterministic extraction with explicit
    editorial decisions, provenance reports and complete matching sweeps.
    Enable `loanwords-cli` for offline commands or `loanwords-collect` for
    HTTPS collection using `NIKL_LOANWORD_API_KEY`. [[#18], [#26]]
 -  Loanword output preflight rejects filesystem aliases before publication,
    and generated reports can be checked beyond the offline input size limit.
    Encoded service keys are decoded once, and unsafe Trace logging
    configurations are rejected before authentication. Partial response bodies
    count toward the collection byte limit even when the request is retried.
    [[#18], [#26]]
