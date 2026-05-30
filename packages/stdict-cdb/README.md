@gukhanmun/stdict-cdb
=====================

[![JSR][JSR badge]][JSR]
[![npm][npm badge]][npm]
[![License: GPL-3.0-only][GPL badge]][GPL]

Standard Korean Language Dictionary (표준국어대사전, 標準國語大辭典) compiled
into a Gukhanmun CDB binary and distributed as an npm package. Use this with
`@gukhanmun/wasm` or `@gukhanmun/napi` when `ko-KR` readings are needed.

[JSR badge]: https://jsr.io/badges/@gukhanmun/stdict-cdb
[JSR]: https://jsr.io/@gukhanmun/stdict-cdb
[npm badge]: https://img.shields.io/npm/v/@gukhanmun/stdict-cdb?logo=npm
[npm]: https://www.npmjs.com/package/@gukhanmun/stdict-cdb
[GPL badge]: https://img.shields.io/npm/l/%40gukhanmun%2Fstdict-cdb
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ bash
npm  add     @gukhanmun/stdict-cdb
pnpm add     @gukhanmun/stdict-cdb
yarn add     @gukhanmun/stdict-cdb
bun  add     @gukhanmun/stdict-cdb
deno add jsr:@gukhanmun/stdict-cdb
~~~~


Usage
-----

~~~~ ts
import { load } from "@gukhanmun/wasm";
import { stdictCdb } from "@gukhanmun/stdict-cdb";

const g = await load({ dictionaries: [await stdictCdb()] });
console.log(g.convert("漢字를 한글로")); // "한자를 한글로"
~~~~

`stdictCdb()` returns a `DictionarySource` record (a
`{ format: "cdb", bytes: Uint8Array }` object). `stdictCdbBytes()` returns only
the `Uint8Array` when you need the raw bytes; pass an explicit `URL` to load a
relocated copy instead of the bundled one. `stdictCdbUrl` is the URL of the
binary inside the package, useful for preloading or caching.

`stdictCdbBytes()` picks how to load from the URL scheme, not the runtime: a
`file:` URL (the usual case for an npm install) is read from disk with
`node:fs/promises`, while any other scheme (such as the `https:` URL of a Deno
install from JSR) is fetched with `fetch`.


Relation to `@gukhanmun/stdict-fst`
-----------------------------------

Both packages contain the same dictionary data in different binary formats. The
CDB format has O(1) lookup and a layout that is straightforward to inspect
manually. The FST format supports prefix streaming and is generally preferred
for the lattice segmenter. Either works; choose based on your performance
profile or integration constraints.


License
-------

GPL-3.0-only.
