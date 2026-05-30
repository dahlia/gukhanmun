@gukhanmun/wasm
===============

[![JSR][JSR badge]][JSR]
[![npm][npm badge]][npm]
[![License: GPL-3.0-only][GPL badge]][GPL]

WebAssembly implementation of the Gukhanmun hanja-to-hangul converter. Runs in
any WebAssembly-capable environment: browsers, Deno 2.0+, Node.js 20+, and Bun
1.0+.

[JSR badge]: https://jsr.io/badges/@gukhanmun/wasm
[JSR]: https://jsr.io/@gukhanmun/wasm
[npm badge]: https://img.shields.io/npm/v/@gukhanmun/wasm?logo=npm
[npm]: https://www.npmjs.com/package/@gukhanmun/wasm
[GPL badge]: https://img.shields.io/npm/l/%40gukhanmun%2Fwasm
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ bash
npm  add     @gukhanmun/wasm
pnpm add     @gukhanmun/wasm
yarn add     @gukhanmun/wasm
bun  add     @gukhanmun/wasm
deno add jsr:@gukhanmun/wasm
~~~~


Usage
-----

The `load()` factory initializes the WASM module on the first call and caches
it for all subsequent calls. Dictionary data must be supplied explicitly
because the WASM bundle does not embed any dictionary.

~~~~ ts
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({ dictionaries: [await stdictFst()] });
console.log(g.convert("漢字를 한글로")); // "한자를 한글로"
~~~~

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

### North Korean preset

~~~~ ts
const g = await load({
  preset: "ko-kp",
  dictionaries: [],
});
console.log(g.convert("來日")); // "래일"
~~~~


Relation to `@gukhanmun/napi`
-----------------------------

Both packages implement the same `Gukhanmun` interface from `@gukhanmun/types`.
`@gukhanmun/wasm` works in all environments including browsers;
`@gukhanmun/napi` uses a native Node.js addon and is faster for server-side
workloads.


License
-------

GPL-3.0-only.
