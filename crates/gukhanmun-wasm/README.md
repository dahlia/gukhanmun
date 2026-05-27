gukhanmun-wasm
==============

WebAssembly binding for Gukhanmun, generated with `wasm-bindgen`. Exposes
`WasmGukhanmun` (an owning converter instance) and `WasmStream` (a streaming
handle for chunked input) to JavaScript.

This crate is the Rust side of the `@gukhanmun/wasm` npm package. End users
interact with the TypeScript wrapper, not with this crate directly.


Exposed types
-------------

`WasmGukhanmun` wraps a `gukhanmun::Converter` and exposes:

 -  `WasmGukhanmun.load(options_json, dicts)`: static factory; `options_json` is
    a JSON-serialized `GukhanmunOptions` object and `dicts` is a JavaScript
    array of `{ format, bytes }` records.
 -  `convert(input, format)`: one-shot conversion. `format` is either a MIME
    type string (`"text/plain"`, `"text/html"`, `"text/markdown"`) or a
    `{ format, gfm }` object for Markdown.
 -  `openStream(format)`/`streamPush(handle, chunk)`/`streamFinish(handle)`:
    chunked streaming API; `handle` is a `WasmStream` returned by `openStream`.

Options and errors are passed as JSON strings so that the TypeScript wrapper
can reconstruct typed values without depending on wasm-bindgen's JS glue for
every field.


Build notes
-----------

The WASM binary is compiled with `release_max_level_off`, which strips all
`tracing` calls from the release artifact. This keeps the binary size small
and avoids a dependency on `console.log` at runtime.

`wasm-opt` is run as a post-processing step by `wasm-pack` to reduce the
binary further.


License
-------

GPL-3.0-only. See `LICENSE` at the repository root.
