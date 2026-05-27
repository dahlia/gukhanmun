@gukhanmun/stdict-cdb
=====================

Standard Korean Language Dictionary (표준국어대사전, 標準國語大辭典) compiled
into a Gukhanmun CDB binary and distributed as an npm package. Use this with
`@gukhanmun/wasm` or `@gukhanmun/napi` when `ko-KR` readings are needed.


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
the `Uint8Array` when you need the raw bytes. `stdictCdbUrl` is the URL of the
binary inside the package, useful for preloading or caching.

On Node.js the bytes are read with `node:fs/promises`. On all other runtimes
they are fetched with `fetch`.


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
