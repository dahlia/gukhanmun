@gukhanmun/opendict-cdb
=======================

[![JSR][JSR badge]][JSR]
[![npm][npm badge]][npm]
[![License: GPL-3.0-only AND CC BY-SA 2.0 KR][license badge]][GPL]

Open Korean Dictionary (우리말샘) categories bundled as CDB binaries for
`@gukhanmun/wasm` and `@gukhanmun/napi`.

[JSR badge]: https://jsr.io/badges/@gukhanmun/opendict-cdb
[JSR]: https://jsr.io/@gukhanmun/opendict-cdb
[npm badge]: https://img.shields.io/npm/v/%40gukhanmun%2Fopendict-cdb?logo=npm
[npm]: https://www.npmjs.com/package/@gukhanmun/opendict-cdb
[license badge]: https://img.shields.io/npm/l/%40gukhanmun%2Fopendict-cdb
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Usage
-----

~~~~ ts
import { load } from "@gukhanmun/napi";
import { opendictNorthKoreanCdb } from "@gukhanmun/opendict-cdb";

const g = await load({
  preset: "ko-kp",
  dictionaries: [await opendictNorthKoreanCdb()],
});
console.log(g.convert("歷史와 來日"));
~~~~

The package exports separate helpers for the 일반어, 북한어, 방언, and 옛말
categories so applications can choose only the dictionaries they need.


Data attribution
----------------

The bundled dictionary data is extracted from the National Institute of Korean
Language's Open Korean Dictionary (우리말샘) JSON dump dated 2026-05-03. See
*ATTRIBUTION.md* for source and license details.


License
-------

Package code is GPL-3.0-only. Bundled dictionary data is CC BY-SA 2.0 KR.
