@gukhanmun/stdict-fst
=====================

[![JSR][JSR badge]][JSR]
[![npm][npm badge]][npm]
[![License: GPL-3.0-only][GPL badge]][GPL]

*Standard Korean Language Dictionary* (標準國語大辭典) compiled
into a Gukhanmun FST binary and distributed as an npm package. Use this with
`@gukhanmun/wasm` or `@gukhanmun/napi` when `ko-KR` readings are needed.

[JSR badge]: https://jsr.io/badges/@gukhanmun/stdict-fst
[JSR]: https://jsr.io/@gukhanmun/stdict-fst
[npm badge]: https://img.shields.io/npm/v/@gukhanmun/stdict-fst?logo=npm
[npm]: https://www.npmjs.com/package/@gukhanmun/stdict-fst
[GPL badge]: https://img.shields.io/npm/l/%40gukhanmun%2Fstdict-fst
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


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
the `Uint8Array` when you need the raw bytes; pass an explicit `URL` to load a
relocated copy instead of the bundled one. `stdictFstUrl` is the URL of the
binary inside the package, useful for preloading or caching.

`stdictFstBytes()` picks how to load from the URL scheme, not the runtime: a
`file:` URL (the usual case for an npm install) is read from disk with
`node:fs/promises`, while any other scheme (such as the `https:` URL of a Deno
install from JSR) is fetched with `fetch`.


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
