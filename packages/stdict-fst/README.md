@gukhanmun/stdict-fst
=====================

Standard Korean Language Dictionary (표준국어대사전, 標準國語大辭典) compiled
into a Gukhanmun FST binary and distributed as an npm package. Use this with
`@gukhanmun/wasm` or `@gukhanmun/napi` when `ko-KR` readings are needed.


Installation
------------

~~~~ bash
npm  add     @gukhanmun/stdict-fst
pnpm add     @gukhanmun/stdict-fst
yarn add     @gukhanmun/stdict-fst
bun  add     @gukhanmun/stdict-fst
deno add jsr:@gukhanmun/stdict-fst
~~~~


Usage
-----

~~~~ ts
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({ dictionaries: [await stdictFst()] });
console.log(g.convert("漢字를 한글로")); // "한자를 한글로"
~~~~

`stdictFst()` returns a `DictionarySource` record (a
`{ format: "fst", bytes: Uint8Array }` object). `stdictFstBytes()` returns only
the `Uint8Array` when you need the raw bytes. `stdictFstUrl` is the URL of the
binary inside the package, useful for preloading or caching.

On Node.js the bytes are read with `node:fs/promises`. On all other runtimes
they are fetched with `fetch`.


Relation to `@gukhanmun/stdict-cdb`
-----------------------------------

Both packages contain the same dictionary data in different binary formats. The
FST format supports prefix streaming and is the better choice for
`@gukhanmun/wasm` workloads with the lattice segmenter. The CDB format has O(1)
lookup and a simpler layout. Either works; choose based on your performance
profile or integration constraints.


License
-------

GPL-3.0-only.
