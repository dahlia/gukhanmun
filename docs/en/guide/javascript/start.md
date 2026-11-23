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

`g.convert(source)` converts plain text by default. Pass a media type as the
second argument to choose a format:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
g.convert("漢字를 한글로", "text/plain");         // plain text (default)
g.convert("<p>漢字</p>", "text/html");           // HTML fragment
g.convert("# 漢字", "text/markdown");            // CommonMark
g.convert("# 漢字", "text/markdown; variant=GFM"); // GFM
~~~~

These are the spellings in the TypeScript declarations. At runtime, both
bindings parse media type parameters like the CLI, so
`text/markdown;variant=GFM` also works.

### Migrating from legacy formats

The old strings (`"text"`, `"html"`, and `"markdown"`) and Markdown objects
still work until Gukhanmun 1.0. Use the media types above instead. For a
Markdown object, choose `"text/markdown; variant=GFM"` if `gfm` is `true`,
or `"text/markdown"` otherwise.

To find calls that still use the old formats at runtime, configure [LogTape]
in your application to capture `warning` records under
`["gukhanmun", "format"]`. Each warning names the replacement. Without a
logging configuration, nothing is printed.

[LogTape]: https://logtape.org/manual/library.md#for-application-developers


Error handling
--------------

`load()` and `convert()` throw `GukhanmunError` on failure:

~~~~ ts twoslash
import { load, GukhanmunError } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

try {
  const g = await load({ dictionaries: [await stdictFst()] });
  g.convert("<invalid html>", "text/html");
} catch (e) {
  if (e instanceof GukhanmunError) {
    console.error(e.code, e.message);
  }
}
~~~~
