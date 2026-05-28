---
title: "Quick start"
description: |-
  Basic usage of the Gukhanmun JavaScript library.
---

Quick start
===========

All Gukhanmun implementations export a single async factory function `load`.
Call it once, then use the returned `Gukhanmun` instance for all conversions.


Minimal example
---------------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  dictionaries: [await stdictFst()],
});

console.log(g.convert("漢字를 한글로"));
// → 한자를 한글로
~~~~


Switching to the NAPI backend
-----------------------------

The API is identical; only the import path changes:

~~~~ ts twoslash
import { load } from "@gukhanmun/napi";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  dictionaries: [await stdictFst()],
});
~~~~


The `convert()` method
----------------------

`g.convert(source)` converts plain text by default.  Pass a format string or
object as the second argument for HTML or Markdown:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
g.convert("漢字를 한글로");             // plain text (default)
g.convert("<p>漢字</p>", "html");       // HTML fragment
g.convert("# 漢字", "markdown");        // CommonMark
g.convert("# 漢字", { format: "markdown", gfm: true });  // GFM
~~~~


Error handling
--------------

`load()` and `convert()` throw `GukhanmunError` on failure:

~~~~ ts twoslash
import { load, GukhanmunError } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

try {
  const g = await load({ dictionaries: [await stdictFst()] });
  g.convert("<invalid html>", "html");
} catch (e) {
  if (e instanceof GukhanmunError) {
    console.error(e.code, e.message);
  }
}
~~~~
