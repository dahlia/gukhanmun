---
title: "Dictionaries"
description: |-
  Loading and using dictionaries with the Gukhanmun JavaScript library.
---

Dictionaries
============

Pass an array of `DictionarySource` objects in the `dictionaries` option.
Dictionary sources are either compiled artifacts (`FileDictionarySource`) or
entry lists computed at runtime (`MapDictionarySource`).  Dictionaries are
probed in order; the first match wins.


Standard Korean Dictionary packages
-----------------------------------

`@gukhanmun/stdict-fst` and `@gukhanmun/stdict-cdb` both ship the bundled
*Standard Korean Dictionary* (標準國語大辭典) in different binary formats.

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


Open Korean Dictionary packages
-------------------------------

`@gukhanmun/opendict-fst` and `@gukhanmun/opendict-cdb` ship the *Open Korean
Dictionary* (우리말샘) as separate general (一般語), North Korean (北韓語),
dialect (方言), and archaic (옛말) category dictionaries.  JavaScript presets
do not auto-load these packages, so pass the categories you want explicitly:

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
import {
  opendictDialectFst,
  opendictNorthKoreanFst,
} from "@gukhanmun/opendict-fst";

const g = await load({
  preset: "ko-kp",
  dictionaries: [
    await opendictNorthKoreanFst(),
    await opendictDialectFst(),
  ],
});
~~~~

Use the CDB package when you want the same categories in CDB format:

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/napi";
import { opendictNorthKoreanCdb } from "@gukhanmun/opendict-cdb";

const g = await load({
  preset: "ko-kp",
  dictionaries: [await opendictNorthKoreanCdb()],
});
~~~~


Loanword spelling examples packages
-----------------------------------

`@gukhanmun/loanwords-fst` and `@gukhanmun/loanwords-cdb` provide the National
Institute of Korean Language's *Loanword spelling examples* dictionary. Both
`"ko-kr"` and `"ko-kp"` require explicit loading; presets never add it for you.
Place it after any user dictionaries and before stdict or opendict:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { loanwordsFst } from "@gukhanmun/loanwords-fst";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  preset: "ko-kr",
  dictionaries: [await loanwordsFst(), await stdictFst()],
});
g.convert("香港"); // "홍콩"
~~~~

For CDB format, import `loanwordsCdb` from `@gukhanmun/loanwords-cdb` instead.
Both loaders return a `Promise<FileDictionarySource>` and work with
`@gukhanmun/wasm` and `@gukhanmun/napi`. Installing the package alone does not
change conversion; include its loader's result in `dictionaries`.


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


Building a custom dictionary
----------------------------

The *.gukfst* and *.gukcdb* files loaded above are compiled artifacts, built
from a plain text table with the `gukhanmun-mkdict` tool.  See
[*Building a custom dictionary*](../cli/dictionary.md#building-a-custom-dictionary)
in the CLI guide for how to author and compile dictionary sources.  When the
entries are computed at runtime rather than stored on disk, pass them directly
as shown in the next section.


Entries computed at runtime
---------------------------

A `MapDictionarySource` carries entries in memory.  Use it when a caller derives
readings while building a document, in a browser, or in an edge runtime where
neither a TSV file nor `gukhanmun-mkdict` is available:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const converter = await load({
  dictionaries: [
    {
      entries: [
        { hanja: "高橋", reading: "다카하시" },
        { hanja: "東京", reading: "도쿄", preference: 10 },
        { hanja: "年代", reading: "연대", suffixReading: "년대" },
      ],
    },
    await stdictFst(),
  ],
});
~~~~

The fields mirror the Rust `ReadingCandidate` type plus the hanja key:

| Field           | Meaning                                                                |
| --------------- | ---------------------------------------------------------------------- |
| `hanja`         | The dictionary key                                                     |
| `reading`       | The word-initial hangul reading                                        |
| `suffixReading` | The reading used when the match is not word-initial, when it differs   |
| `preference`    | Relative default preference within the key; larger wins, absent is `0` |
| `requireHanja`  | Always show the original hanja alongside the reading                   |
| `requireHangul` | Always show a hangul gloss alongside the original hanja                |

Sources keep the precedence they have in `dictionaries`, so an entry list placed
before a compiled dictionary overrides it for the keys it carries; placing it
after the compiled dictionary leaves that dictionary in charge.

Repeating a `hanja` key appends another reading instead of replacing the earlier
one.  Several readings for one headword stay available to the engine, which then
selects one from a following particle when the particle is decisive:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({
  dictionaries: [
    {
      entries: [
        { hanja: "東京", reading: "동경" },
        { hanja: "東京", reading: "도쿄", preference: 10 },
      ],
    },
  ],
});
g.convert("東京");   // "도쿄": the higher preference
g.convert("東京을"); // "동경을": the particle selects the other reading
g.convert("東京를"); // "도쿄를"
~~~~

`preference` decides the context-free default.  List order never does: when two
readings share a preference, the tie is broken by reading spelling, so set
distinct preferences when the choice matters.

An empty `entries` list contributes nothing, which makes `{ entries: [] }` a
convenient placeholder for a document without overrides.
