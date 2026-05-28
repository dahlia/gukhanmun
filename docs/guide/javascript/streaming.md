---
title: "Streaming API"
description: |-
  Processing large documents chunk by chunk with Gukhanmun's TransformStream API.
---

Streaming API
=============

`g.stream(format?)` returns a
[`TransformStream<string, string>`]
that you can pipe through to convert an arbitrarily large document without
buffering it all at once.

[`TransformStream<string, string>`]: https://developer.mozilla.org/en-US/docs/Web/API/TransformStream


Basic usage
-----------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({ dictionaries: [await stdictFst()] });
const stream = g.stream("html");

const writer = stream.writable.getWriter();
const reader = stream.readable.getReader();

// Write chunks
await writer.write("<p>漢");
await writer.write("字</p>");
await writer.close();

// Read converted output
const parts: string[] = [];
for (;;) {
  const { done, value } = await reader.read();
  if (done) break;
  if (value) parts.push(value);
}
console.log(parts.join(""));  // <p>한자</p>
~~~~


Format argument
---------------

The format argument to `stream` accepts the same values as `convert()`:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
g.stream();                              // plain text (default)
g.stream("html");                        // HTML
g.stream("markdown");                    // CommonMark
g.stream({ format: "markdown", gfm: true });  // GFM
~~~~


Piping with the WHATWG Streams API
----------------------------------

`TransformStream` integrates with the standard Streams API:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
const response = await fetch("/large-document.html");
const converted = response.body!
  .pipeThrough(new TextDecoderStream())
  .pipeThrough(g.stream("html"))
  .pipeThrough(new TextEncoderStream());
~~~~


Streaming guarantee
-------------------

Concatenating all chunks written to the stream's writable side and all chunks
read from the readable side is equivalent to calling `g.convert()` on the full
concatenated input:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
const chunkA: string = "";
const chunkB: string = "";
const chunkC: string = "";
// ---cut-before---
// These are always equivalent:
const result1 = g.convert(chunkA + chunkB + chunkC, "html");

// and
const stream = g.stream("html");
const writer = stream.writable.getWriter();
await writer.write(chunkA);
await writer.write(chunkB);
await writer.write(chunkC);
await writer.close();
const result2 = stream.readable
  .getReader()
  .read()
  .then(({ value }) => value || "");

import assert from "node:assert/strict";
assert.strictEqual(result1, result2);
~~~~

The stream may buffer internally at context boundaries (for example,
`"per-document"` homophone tracking reads the entire document before flushing).
With the default `"per-block"` setting, output is flushed after each block
element.
