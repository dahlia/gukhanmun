---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#48': https://github.com/dahlia/gukhanmun/issues/48
  '#55': https://github.com/dahlia/gukhanmun/pull/55
---
 -  Bounded `UrlDictionarySource` downloads with configurable `max_bytes`
    (64 MiB by default) and `total_timeout` (30 seconds by default) for HTTP
    response reads across redirects. Exceeding either bound raises
    `DICTIONARY_LOAD` with an `IO` cause. Cancellation during synchronous
    DNS lookup or connection setup can be delayed beyond `total_timeout`;
    the existing per-operation `timeout` is preserved.
    [[#48], [#15], [#55]]

 -  Changed URL dictionary loading to follow redirects only within the initial
    URL's origin (scheme, hostname and effective port). Redirects to another
    origin raise `INVALID_INPUT` before a request is sent; configure a different
    origin's final URL directly. [[#48], [#15], [#55]]
