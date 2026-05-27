@gukhanmun/napi
===============

Node.js native addon implementation of the Gukhanmun hanja-to-hangul converter,
built with napi-rs. Requires Node.js 20+. For environments other than Node.js
(browsers, Deno, Bun via WASM), use `@gukhanmun/wasm` instead.


Installation
------------

~~~~ bash
npm  add     @gukhanmun/napi
pnpm add     @gukhanmun/napi
yarn add     @gukhanmun/napi
bun  add     @gukhanmun/napi
deno add npm:@gukhanmun/napi
~~~~

The package declares six optional platform dependencies
(`@gukhanmun/napi-aarch64-apple-darwin`, `@gukhanmun/napi-x86_64-apple-darwin`,
etc.). npm installs only the one matching the current OS, CPU architecture, and
libc variant. The supported targets are:

| Target                    | OS            | Architecture |
| ------------------------- | ------------- | ------------ |
| aarch64-apple-darwin      | macOS         | ARM64        |
| x86\_64-apple-darwin      | macOS         | x86\_64      |
| aarch64-pc-windows-msvc   | Windows       | ARM64        |
| x86\_64-pc-windows-msvc   | Windows       | x86\_64      |
| aarch64-unknown-linux-gnu | Linux (glibc) | ARM64        |
| x86\_64-unknown-linux-gnu | Linux (glibc) | x86\_64      |

musl Linux (Alpine, etc.) is not covered by a prebuilt binary. Build from
source with `mise run napi-build` on those platforms.


Usage
-----

~~~~ ts
import { load } from "@gukhanmun/napi";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({ dictionaries: [await stdictFst()] });
console.log(g.convert("漢字를 한글로")); // "한자를 한글로"
~~~~

The `load()` factory is asynchronous for API uniformity with `@gukhanmun/wasm`,
but the native addon itself is synchronous. The async part covers reading the
dictionary bytes from disk.

### Streaming

~~~~ ts
const stream = g.convertStream("text/html");
const writer = stream.writable.getWriter();
const reader = stream.readable.getReader();

await writer.write("<p>漢字</p>");
await writer.close();

const { value } = await reader.read();
console.log(value); // "<p>한자</p>"
~~~~


License
-------

GPL-3.0-only.
