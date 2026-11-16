---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#48': https://github.com/dahlia/gukhanmun/issues/48
  '#49': https://github.com/dahlia/gukhanmun/issues/49
  '#55': https://github.com/dahlia/gukhanmun/pull/55
  '#57': https://github.com/dahlia/gukhanmun/pull/57
---
 -  Added a Python 3.11+ `Converter` API for plain text, HTML and Markdown,
    with reusable immutable settings, standard-library enums, punctuation
    profiles, literal directives, HTML preservation rules and type annotations.
    `MarkdownOptions(gfm=True)` enables GFM. Conversion releases the GIL, and a
    single `Converter` can be reused from multiple threads at once. Python
    signals are checked after native conversion returns, so long conversions
    can delay `KeyboardInterrupt`. [[#48], [#15], [#55]]

 -  Added `MapDictionarySource` for competing in-memory readings and
    `FileDictionarySource` for FST/CDB bytes or filesystem paths. Dictionaries
    load once at converter construction; conversion performs no dictionary
    I/O. Configure `source_policy`, `reference_only` and source roles to
    control reference-authorized cooperation; authored overrides remain
    authoritative. Compiled dictionaries support compound aliases, source
    evidence and phrase constraints, including version 4 data.
    [[#48], [#15], [#55]]

 -  Added `UrlDictionarySource` for HTTP(S) dictionaries with configurable
    `max_bytes` (64 MiB by default), a response-reading `total_timeout`
    (30 seconds by default) shared across redirects, and a per-operation
    `timeout`. Exceeding `max_bytes` or `total_timeout` raises
    `DICTIONARY_LOAD` with an `IO` cause. Synchronous DNS lookup or connection
    setup can delay cancellation beyond `total_timeout`. Redirects must retain
    the initial URL's scheme, hostname and effective port; other origins raise
    `INVALID_INPUT` before a request is sent, so configure their final URL
    directly. Rejected redirect responses are closed. [[#48], [#15], [#55]]

 -  Added validated FST/CDB loading with `DICTIONARY_LOAD` errors for malformed
    layouts or excessive resource requirements. CDB loading checks file
    layouts before use, prevents crafted hash tables from forcing long lookup
    probe loops, and rejects record indexes requesting more than 64 MiB
    before allocation. FST loading limits accepted keys, their total byte
    length and traversal work before enumeration, even with maximum-length
    metadata. Both formats check decoded-allocation admission budgets before
    expanding aliases or typed evidence; 32,768 distinct short aliases remain
    loadable within those budgets. These limits do not bound total process
    memory: file bytes, decoded payloads and allocator overhead have additional
    costs. [[#48], [#15], [#55]]

 -  Added buffered `Converter.stream()` and `Converter.convert_iter()` methods
    for chunked text, HTML and Markdown input. Streams produce output only at
    `finish()` and reject reuse after completion or failure; they are neither
    asynchronous nor bounded-memory streams. `convert_iter()` finalizes only
    after normal input exhaustion, not after an upstream error or early close.
    [[#48], [#15], [#55]]

 -  Added `mise run py-sync-version` and integrated Python metadata and
    *uv.lock* into workspace version bumps. [[#48], [#15], [#55]]

 -  Added structured `GukhanmunError` exceptions with codes, messages and
    immutable cause tuples, preserved when copying or pickling errors,
    including errors returned from process workers. `chain` retains underlying
    parser causes for malformed FST/CDB data, such as an undecodable FST map
    or dictionary metadata; errors without an underlying cause have an empty
    chain. [[#48], [#49], [#15], [#55], [#57]]
