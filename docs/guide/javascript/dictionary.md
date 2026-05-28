---
title: "Dictionaries"
description: |-
  Loading and using dictionaries with the Gukhanmun JavaScript library.
---

Dictionaries
============

Pass an array of `DictionarySource` objects in the `dictionaries` option.
Dictionaries are probed in order; the first match wins.


Standard Korean Dictionary packages
-----------------------------------

`@gukhanmun/stdict-fst` and `@gukhanmun/stdict-cdb` both ship the bundled
Standard Korean Dictionary (標準國語大辭典) in different binary formats.

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
import { stdictCdb } from "@gukhanmun/stdict-cdb";

// FST — preferred; smaller on disk, better for lattice segmentation
const g = await load({ dictionaries: [await stdictFst()] });

// CDB — O(1) lookup; simpler layout
const g = await load({ dictionaries: [await stdictCdb()] });
~~~~

Both `stdictFst()` and `stdictCdb()` return a `Promise<FileDictionarySource>`.


Custom dictionaries
-------------------

A `FileDictionarySource` specifies the binary format and the data to load from:

~~~~ ts twoslash
interface FileDictionarySource {
  format: "fst" | "cdb";
  data: ArrayBuffer | ArrayBufferView | URL | string;
}
~~~~

The `data` field accepts:

| Type                            | Where              | Notes                                     |
| ------------------------------- | ------------------ | ----------------------------------------- |
| `ArrayBuffer`/`ArrayBufferView` | All environments   | Bytes already in memory                   |
| `URL`                           | All environments   | Remote or local URL; loaded via `fetch()` |
| `string`                        | Node.js, Deno, Bun | Filesystem path                           |


Load from a URL
---------------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({
  dictionaries: [{
    format: "fst",
    data: new URL("./legal.gukfst", import.meta.url),
  }],
});
~~~~

In a browser the URL is fetched with `fetch()`.  In Node.js, Deno, and Bun a
`file://` URL is read from disk.


Load from bytes
---------------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const response = await fetch("/custom.gukcdb");
const buf = await response.arrayBuffer();

const g = await load({
  dictionaries: [{ format: "cdb", data: buf }],
});
~~~~


Load from a file path (Node.js/Deno/Bun)
----------------------------------------

~~~~ ts twoslash
import { load } from "@gukhanmun/napi";
// ---cut-before---
const g = await load({
  dictionaries: [{ format: "fst", data: "/data/domain.gukfst" }],
});
~~~~

Passing a plain string path in a browser throws `GukhanmunError` with code
`"io"`.


Combining multiple dictionaries
-------------------------------

~~~~ ts twoslash
import { load } from "@gukhanmun/napi";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  dictionaries: [
    { format: "fst", data: "/data/legal.gukfst" },  // checked first
    await stdictFst(),                               // fallback
  ],
});
~~~~
