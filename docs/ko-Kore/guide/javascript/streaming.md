---
title: 스트리밍 API
description: |-
  Gukhanmun의 버퍼링된 TransformStream API에 文字列 청크를 傳達하기.
---

스트리밍 API
============

`g.stream(format?)`은 文字列 청크를 받는
[`TransformStream<string, string>`]을 返還합니다. 現在 WebAssembly와 Node-API
具顯은 모두 入力 全體를 버퍼링하고 쓰기 側이 닫힐 때 變換합니다. 이는 청크
인터페이스이며, 메모리 使用量을 制限하거나 出力을 미리 내보내는 스트리밍이
아닙니다.

[`TransformStream<string, string>`]: https://developer.mozilla.org/en-US/docs/Web/API/TransformStream


基本 使用法
-----------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({ dictionaries: [await stdictFst()] });
const stream = g.stream("html");

const writer = stream.writable.getWriter();
const reader = stream.readable.getReader();

// 쓰기가 막히지 않도록 쓰기 前에 읽기를 始作합니다.
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


形式 引數
---------

`stream`의 形式 引數는 `convert()`와 같은 값을 받습니다:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
g.stream();                              // 平文 텍스트 (基本)
g.stream("html");                        // HTML
g.stream("markdown");                    // CommonMark
g.stream({ format: "markdown", gfm: true });  // GFM
~~~~


WHATWG Streams API로 파이프하기
-------------------------------

`TransformStream`은 標準 Streams API와 統合됩니다:

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


스트리밍 保證
-------------

스트림의 쓰기 側에 쓴 모든 청크와 읽기 側에서 읽은 모든 청크를 이어 붙인
것은, 全體 連結 入力에 對해 `g.convert()`를 呼出한 것과 同等합니다:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
const chunkA: string = "<p>漢";
const chunkB: string = "字";
const chunkC: string = "</p>";
// ---cut-before---
// 다음은 恒常 同等하다:
const result1 = g.convert(chunkA + chunkB + chunkC, "html");

// 그리고
const stream = g.stream("html");
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

`"per-block"`을 使用하거나 同音異義 追跡을 꺼도 最終化 前에는 出力이
없습니다. 이 옵션들은 註解(annotation) 文脈을 制御하며, JavaScript 바인딩이
청크를 내보내는 時點을 바꾸지는 않습니다. 스트림 基盤 入出力과 統合해도
文書 全體를 담는 버퍼는 必要합니다.
