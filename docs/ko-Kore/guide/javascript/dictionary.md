---
title: 辭典
description: |-
  Gukhanmun JavaScript 라이브러리로 辭典을 불러오고 使用하기.
---

辭典
====

`dictionaries` 옵션에 `DictionarySource` 客體의 配列을 넘깁니다.  辭典은
順序대로 探索됩니다; 처음 一致한 것이 採擇됩니다.


《標準國語大辭典》 패키지
-------------------------

`@gukhanmun/stdict-fst`와 `@gukhanmun/stdict-cdb`는 둘 다 內藏
《標準國語大辭典》을 各各 다른 바이너리 形式으로 提供합니다.

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
import { stdictCdb } from "@gukhanmun/stdict-cdb";

// FST: 勸奬; 디스크에서 더 작고, 래티스(lattice) 分割에 더 適合
const g = await load({ dictionaries: [await stdictFst()] });

// CDB: O(1) 찾기; 더 單純한 配置
const g = await load({ dictionaries: [await stdictCdb()] });
~~~~

`stdictFst()`와 `stdictCdb()`는 모두 `Promise<FileDictionarySource>`를
返還합니다.


《우리말샘》 패키지
-------------------

`@gukhanmun/opendict-fst`와 `@gukhanmun/opendict-cdb`는 《우리말샘》을
一般語, 北韓語, 方言, 옛말 分類別 辭典으로 나누어 提供합니다. JavaScript
프리셋은 이 패키지들을 自動으로 불러오지 않으므로, 必要한 分類를 明示的으로
넘겨야 합니다:

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

같은 分類를 CDB 形式으로 쓰려면 CDB 패키지를 使用합니다:

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/napi";
import { opendictNorthKoreanCdb } from "@gukhanmun/opendict-cdb";

const g = await load({
  preset: "ko-kp",
  dictionaries: [await opendictNorthKoreanCdb()],
});
~~~~


使用者 定義 辭典
----------------

`FileDictionarySource`는 바이너리 形式과 불러올 데이터를 指定합니다:

~~~~ ts twoslash
interface FileDictionarySource {
  format: "fst" | "cdb";
  data: ArrayBuffer | ArrayBufferView | URL | string;
}
~~~~

`data` 필드는 다음을 받습니다:

| 種類                            | 位置               | 備考                                   |
| ------------------------------- | ------------------ | -------------------------------------- |
| `ArrayBuffer`/`ArrayBufferView` | 모든 環境          | 이미 메모리에 있는 바이트              |
| `URL`                           | 모든 環境          | 遠隔 또는 로컬 URL; `fetch()`로 불러옴 |
| `string`                        | Node.js, Deno, Bun | 파일시스템 經路                        |


URL에서 불러오기
----------------

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

브라우저에서는 URL이 `fetch()`로 가져와집니다.  Node.js·Deno·Bun에서는 `file://`
URL이 디스크에서 읽힙니다.


바이트에서 불러오기
-------------------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const response = await fetch("/custom.gukcdb");
const buf = await response.arrayBuffer();

const g = await load({
  dictionaries: [{ format: "cdb", data: buf }],
});
~~~~


파일 經路에서 불러오기(Node.js/Deno/Bun)
----------------------------------------

~~~~ ts twoslash
import { load } from "@gukhanmun/napi";
// ---cut-before---
const g = await load({
  dictionaries: [{ format: "fst", data: "/data/domain.gukfst" }],
});
~~~~

브라우저에서 純粹 文字列 經路를 넘기면 `"io"` 코드와 함께 `GukhanmunError`를
던집니다.


여러 辭典 結合
--------------

~~~~ ts twoslash
import { load } from "@gukhanmun/napi";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  dictionaries: [
    { format: "fst", data: "/data/legal.gukfst" },  // 먼저 檢査
    await stdictFst(),                               // fallback
  ],
});
~~~~


使用者 定義 辭典 構築
---------------------

위에서 불러온 *.gukfst*와 *.gukcdb* 파일은 컴파일된 産出物이며,
`gukhanmun-mkdict` 道具로 純粹 텍스트 表에서 빌드됩니다.  辭典 出處를 作成하고
컴파일하는 方法은 CLI 案內書의
〈[使用者 定義 辭典 構築](../cli/dictionary.md#使用者-定義-辭典-構築)〉을
參照하십시오.
