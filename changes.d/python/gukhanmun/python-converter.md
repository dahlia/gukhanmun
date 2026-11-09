---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#48': https://github.com/dahlia/gukhanmun/issues/48
  '#55': https://github.com/dahlia/gukhanmun/pull/55
---
 -  Added a Python 3.11+ `Converter` API for plain text, HTML and Markdown,
    with reusable immutable settings, standard-library enums, in-memory entries
    and compiled FST/CDB dictionaries, punctuation profiles, structured errors
    and type annotations. Conversion releases the GIL, and a single `Converter`
    can be reused from multiple threads at once. [[#48], [#15], [#55]]

 -  Added buffered `Converter.stream()` and `Converter.convert_iter()` methods
    for chunked text, HTML and Markdown input. Streams produce output at
    `finish()` and reject reuse after completion or failure.
    [[#48], [#15], [#55]]

 -  Added `mise run py-sync-version` and integrated Python metadata and
    *uv.lock* into workspace version bumps. [[#48], [#15], [#55]]
