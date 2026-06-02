@gukhanmun/opendict-cdb
=======================

[![JSR][JSR badge]][JSR]
[![npm][npm badge]][npm]
[![License: GPL-3.0-or-later AND CC BY-SA 2.0 KR][license badge]][GPL]

*Open Korean Dictionary* (우리말샘) categories compiled into CDB binaries for
use with `@gukhanmun/wasm` or `@gukhanmun/napi`.

[JSR badge]: https://jsr.io/badges/@gukhanmun/opendict-cdb
[JSR]: https://jsr.io/@gukhanmun/opendict-cdb
[npm badge]: https://img.shields.io/npm/v/%40gukhanmun%2Fopendict-cdb?logo=npm
[npm]: https://www.npmjs.com/package/@gukhanmun/opendict-cdb
[license badge]: https://img.shields.io/npm/l/%40gukhanmun%2Fopendict-cdb
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ bash
npm  add     @gukhanmun/opendict-cdb
pnpm add     @gukhanmun/opendict-cdb
yarn add     @gukhanmun/opendict-cdb
bun  add     @gukhanmun/opendict-cdb
deno add jsr:@gukhanmun/opendict-cdb
~~~~


Usage
-----

~~~~ ts
import { load } from "@gukhanmun/napi";
import { opendictNorthKoreanCdb } from "@gukhanmun/opendict-cdb";

const g = await load({
  preset: "ko-kp",
  dictionaries: [await opendictNorthKoreanCdb()],
});
console.log(g.convert("歷史와 來日")); // "력사와 래일"
~~~~

The package exports separate helper functions for the 一般語, 北韓語, 方言,
and 옛말 categories, so you can select and load only the categories you need:

 -  `opendictGeneralCdb()`
 -  `opendictNorthKoreanCdb()`
 -  `opendictDialectCdb()`
 -  `opendictArchaicCdb()`

Each function returns a `DictionarySource` record (a `{ format: "cdb", bytes: Uint8Array }`
object).

### Raw bytes and URLs

If you only need raw `Uint8Array` bytes, use the `*Bytes()` helpers:

 -  `opendictGeneralCdbBytes()`
 -  `opendictNorthKoreanCdbBytes()`
 -  `opendictDialectCdbBytes()`
 -  `opendictArchaicCdbBytes()`

To get the package URL of a CDB binary, use the `*Url` constants:

 -  `opendictGeneralCdbUrl`
 -  `opendictNorthKoreanCdbUrl`
 -  `opendictDialectCdbUrl`
 -  `opendictArchaicCdbUrl`

The `*Bytes()` helpers read the binary from disk using `node:fs/promises` if loaded from
a `file:` URL (default for npm installations), and fall back to `fetch` for other protocols.

The bundled binaries ship gzip-compressed (the `*Url` constants point at
*\*.cdb.gz* files) to stay within the JSR per-file size limit. The `*Bytes()`
helpers inflate them transparently, so their return value is always the raw
CDB; only reach past them to the `*Url` constants if you intend to handle the
gzip yourself.


Relation to `@gukhanmun/opendict-fst`
-------------------------------------

Both packages contain the same dictionary data in different binary formats. The
CDB format has O(1) lookup and a layout that is straightforward to inspect
manually. The FST format supports prefix streaming and is generally preferred
for the lattice segmenter. Either works; choose based on your performance
profile or integration constraints.


Data attribution
----------------

The bundled dictionary data is derived from the National Institute of Korean
Language's *Open Korean Dictionary* (우리말샘) JSON dump dated 2026-05-03. See
*ATTRIBUTION.md* for source and license details.


License
-------

Package code is GPL-3.0-or-later. Bundled dictionary data is CC BY-SA 2.0 KR.
