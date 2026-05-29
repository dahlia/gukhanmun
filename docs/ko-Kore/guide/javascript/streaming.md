---
title: 스트리밍 API
description: |-
  Gukhanmun의 TransformStream API로 큰 文書를 청크 單位로 處理하기.
---

스트리밍 API
============

`g.stream(format?)`은 [`TransformStream<string, string>`]을 返還하며, 이를 通해
파이프하면 任意의 큰 文書를 한꺼번에 버퍼링하지 않고 變換할 수 있습니다.

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

// 청크 쓰기
await writer.write("<p>漢");
await writer.write("字</p>");
await writer.close();

// 變換된 出力 읽기
const parts: string[] = [];
for (;;) {
  const { done, value } = await reader.read();
  if (done) break;
  if (value) parts.push(value);
}
console.log(parts.join(""));  // <p>한자</p>
~~~~


形式 引數
---------

`stream`의 形式 引數는 `convert()`와 같은 값을 받습니다:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
g.stream();                              // 純粹 텍스트 (基本)
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

스트림의 writable 側에 쓴 모든 청크와 readable 側에서 읽은 모든 청크를 이어 붙인
것은, 全體 連結 入力에 對해 `g.convert()`를 呼出한 것과 同等합니다:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
const chunkA: string = "";
const chunkB: string = "";
const chunkC: string = "";
// ---cut-before---
// 다음은 恒常 同等하다:
const result1 = g.convert(chunkA + chunkB + chunkC, "html");

// 그리고
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

스트림은 文脈 境界에서 內部的으로 버퍼링할 수 있습니다(例를 들어
`"per-document"` 同音異義 追跡은 플러시(flush) 前에 全體 文書를 읽습니다).
基本 `"per-block"` 設定에서는 各 블록 要素 다음에 出力이 플러시됩니다.
