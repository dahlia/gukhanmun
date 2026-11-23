---
title: "Streaming API"
description: |-
  Feed string chunks through Gukhanmun's buffered TransformStream API.
---

Streaming API
=============

`g.stream(format?)` returns a
[`TransformStream<string, string>`]
that accepts string chunks. Both the WebAssembly and Node-API implementations
currently buffer the complete input and convert it when the writable side
closes. This is a chunked interface, not bounded-memory or early-output
streaming.

[`TransformStream<string, string>`]: https://developer.mozilla.org/en-US/docs/Web/API/TransformStream


Basic usage
-----------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({ dictionaries: [await stdictFst()] });
const stream = g.stream("text/html");

const writer = stream.writable.getWriter();
const reader = stream.readable.getReader();

// Start reading before writing so backpressure cannot block the writer.
const output = (async () => {
  const parts: string[] = [];
  for (;;) {
    const { done, value } = await reader.read();
    if (done) return parts.join("");
    parts.push(value);
  }
})();

await writer.write("<p>漢");
await writer.write("字</p>");
await writer.close();
console.log(await output);  // <p>한자</p>
~~~~


Format argument
---------------

The format argument to `stream()` accepts the same media types as `convert()`:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
g.stream();                             // plain text (default)
g.stream("text/html");                  // HTML
g.stream("text/markdown");              // CommonMark
g.stream("text/markdown; variant=GFM");  // GFM
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
  .pipeThrough(g.stream("text/html"))
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
const chunkA: string = "<p>漢";
const chunkB: string = "字";
const chunkC: string = "</p>";
// ---cut-before---
// These are always equivalent:
const result1 = g.convert(chunkA + chunkB + chunkC, "text/html");

// and
const stream = g.stream("text/html");
const output = new Response(
  stream.readable.pipeThrough(new TextEncoderStream()),
).text();
const writer = stream.writable.getWriter();
await writer.write(chunkA);
await writer.write(chunkB);
await writer.write(chunkC);
await writer.close();
const result2 = await output;

import assert from "node:assert/strict";
assert.strictEqual(result1, result2);
~~~~

No output is emitted before finalization, even with `"per-block"` or
homophone tracking disabled. These options control annotation context, not
when the JavaScript bindings emit chunks. Piping integrates with stream-based
I/O but does not remove the full-document buffer.
