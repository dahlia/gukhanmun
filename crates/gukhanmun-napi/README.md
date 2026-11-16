gukhanmun-napi
==============

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

Node.js native addon for Gukhanmun, built with napi-rs v3. Exposes
`NapiGukhanmun` to JavaScript via the Node-API ABI so that Node.js 20+
processes can call the full Rust pipeline without the overhead of a WebAssembly
sandbox.

This crate is the Rust side of the `@gukhanmun/napi` npm package. End users
interact with the TypeScript wrapper, not with this crate directly.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-napi?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-napi
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-napi
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Exposed types
-------------

`NapiGukhanmun` wraps a `gukhanmun::Converter` and exposes:

 -  `NapiGukhanmun.load(options_json, dicts)`: synchronous static factory;
    options are JSON-serialized. Compiled dictionaries use
    `{ format, bytes: Buffer }`; runtime dictionaries use `{ entriesJson }`.
    Either can carry a source `role`. These internal records differ from the
    public TypeScript descriptors, whose compiled bytes are in `data`.
 -  `convert(input, format)`: one-shot synchronous conversion.
 -  `openStream(format)`: returns an opaque `External<StreamState>` handle.
 -  `streamPush(handle, chunk)`: buffers a string chunk and returns an empty
    string.
 -  `streamFinish(handle)`: converts the entire buffered input.

Format arguments at this boundary are JSON-encoded selectors such as
`"text"`, `"html"`, `"markdown"`, or `{ format: "markdown", gfm: true }`.
The TypeScript wrapper's public `load()` is asynchronous and its `stream()`
adapts the buffered handle to a `TransformStream`, not incremental output.


Error encoding
--------------

Errors thrown by the native addon are plain `Error` objects whose `message` is
a JSON string: `{ "code": "...", "message": "...", "chain": [...] }`. The
TypeScript wrapper parses this JSON and re-throws a typed `GukhanmunError` with
the correct `code` and `chain` properties.


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.
