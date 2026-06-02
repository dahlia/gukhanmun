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

 -  `NapiGukhanmun.load(options_json, dicts)`: static async factory; options are
    JSON-serialized and dicts are `{ format, bytes: Buffer }` records.
 -  `convert(input, format)`: one-shot synchronous conversion.
 -  `openStream(format)`: returns an opaque `External<StreamState>` handle.
 -  `streamPush(handle, chunk)`: feeds a `Buffer` chunk to the stream; returns
    whatever output is ready.
 -  `streamFinish(handle)`: flushes and finalizes the stream.


Error encoding
--------------

Errors thrown by the native addon are plain `Error` objects whose `message` is
a JSON string: `{ "code": "...", "message": "...", "chain": [...] }`. The
TypeScript wrapper parses this JSON and re-throws a typed `GukhanmunError` with
the correct `code` and `chain` properties.


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.
