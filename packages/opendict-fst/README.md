@gukhanmun/opendict-fst
=======================

[![JSR][JSR badge]][JSR]
[![npm][npm badge]][npm]
[![License: GPL-3.0-or-later AND CC BY-SA 2.0 KR][license badge]][GPL]

*Open Korean Dictionary* (우리말샘) categories compiled into FST binaries for
use with `@gukhanmun/wasm` or `@gukhanmun/napi`.

[JSR badge]: https://jsr.io/badges/@gukhanmun/opendict-fst
[JSR]: https://jsr.io/@gukhanmun/opendict-fst
[npm badge]: https://img.shields.io/npm/v/%40gukhanmun%2Fopendict-fst?logo=npm
[npm]: https://www.npmjs.com/package/@gukhanmun/opendict-fst
[license badge]: https://img.shields.io/npm/l/%40gukhanmun%2Fopendict-fst
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ bash
npm  add     @gukhanmun/opendict-fst
pnpm add     @gukhanmun/opendict-fst
yarn add     @gukhanmun/opendict-fst
bun  add     @gukhanmun/opendict-fst
deno add jsr:@gukhanmun/opendict-fst
~~~~


Usage
-----

~~~~ ts
import { load } from "@gukhanmun/wasm";
import { opendictNorthKoreanFst } from "@gukhanmun/opendict-fst";

const g = await load({
  preset: "ko-kp",
  dictionaries: [await opendictNorthKoreanFst()],
});
console.log(g.convert("歷史와 來日")); // "력사와 래일"
~~~~

The package exports separate helper functions for the 一般語, 北韓語, 方言,
and 옛말 categories, so you can select and load only the categories you need:

 -  `opendictGeneralFst()`
 -  `opendictNorthKoreanFst()`
 -  `opendictDialectFst()`
 -  `opendictArchaicFst()`

Each function returns a `DictionarySource` record (a
`{ format: "fst", bytes: Uint8Array }` object).

### Raw bytes and URLs

If you only need raw `Uint8Array` bytes, use the `*Bytes()` helpers:

 -  `opendictGeneralFstBytes()`
 -  `opendictNorthKoreanFstBytes()`
 -  `opendictDialectFstBytes()`
 -  `opendictArchaicFstBytes()`

To get the package URL of an FST binary, use the `*Url` constants:

 -  `opendictGeneralFstUrl`
 -  `opendictNorthKoreanFstUrl`
 -  `opendictDialectFstUrl`
 -  `opendictArchaicFstUrl`

The `*Bytes()` helpers read the binary from disk using `node:fs/promises` if
loaded from a `file:` URL (default for npm installations), and fall back to
`fetch` for other protocols.


Relation to `@gukhanmun/opendict-cdb`
-------------------------------------

Both packages contain the same dictionary data in different binary formats. The
FST format supports prefix streaming and is the better choice for
`@gukhanmun/wasm` workloads with the lattice segmenter. The CDB format has O(1)
lookup and a simpler layout. Either works; choose based on your performance
profile or integration constraints.


Data attribution
----------------

The bundled dictionary data is derived from the National Institute of Korean
Language's *Open Korean Dictionary* (우리말샘) JSON dump dated 2026-06-03. See
*ATTRIBUTION.md* for source and license details.


License
-------

Package code is GPL-3.0-or-later. Bundled dictionary data is CC BY-SA 2.0 KR.
