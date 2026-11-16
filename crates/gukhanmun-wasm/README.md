gukhanmun-wasm
==============

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

WebAssembly binding for Gukhanmun, generated with `wasm-bindgen`. Exposes
`WasmGukhanmun` (an owning converter instance) and `WasmStream` (a streaming
handle for chunked input) to JavaScript.

This crate is the Rust side of the `@gukhanmun/wasm` npm package. End users
interact with the TypeScript wrapper, not with this crate directly.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-wasm?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-wasm
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-wasm
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Exposed types
-------------

`WasmGukhanmun` wraps a `gukhanmun::Converter` and exposes:

 -  `new WasmGukhanmun(options, dictionaries)`: constructor taking a JavaScript
    options object and pre-resolved dictionary records. Compiled dictionaries
    use `{ format, bytes }`; runtime dictionaries use `{ entriesJson }`. Either
    can carry a source `role`. These internal records differ from the public
    TypeScript descriptors, whose compiled bytes are in `data`.
 -  `convert(input, format)`: one-shot conversion. `format` is `"text"`,
    `"html"`, `"markdown"`, or a `{ format: "markdown", gfm }` object.
 -  `open_stream(format)`: returns a `WasmStream`. Its `push(chunk)` method
    buffers a string and returns an empty string; `finish()` converts the
    entire buffered input.

Errors are JavaScript objects with `code`, `message` and `chain` fields. The
TypeScript wrapper reconstructs a typed `GukhanmunError`. Its public `stream()`
method adapts the buffered handle to a `TransformStream`, not incremental
output.


Build notes
-----------

The WASM binary is compiled with `release_max_level_off`, which strips all
`tracing` calls from the release artifact. This keeps the binary size small
and avoids a dependency on `console.log` at runtime.

`wasm-opt` is run as a post-processing step by `wasm-pack` to reduce the
binary further.


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.
