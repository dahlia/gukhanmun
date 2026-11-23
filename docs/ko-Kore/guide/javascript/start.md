---
title: 빠른 始作
description: |-
  Gukhanmun JavaScript 라이브러리의 基本 使用法.
---

빠른 始作
=========

모든 Gukhanmun 具顯은 單一 非同期 팩토리 函數 `load`를 내보냅니다.  한 番
呼出한 뒤, 返還된 `Gukhanmun` 인스턴스를 모든 變換에 使用합니다.


最小 例示
---------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  dictionaries: [await stdictFst()],
});

console.log(g.convert("漢字를 한글로"));
// → 한자를 한글로
~~~~


NAPI 백엔드로 바꾸기
--------------------

API는 同一합니다; `import` 經路만 바뀝니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/napi";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  dictionaries: [await stdictFst()],
});
~~~~


`convert()` 메서드
------------------

`g.convert(source)`는 基本的으로 平文 텍스트를 變換합니다. 두 番째 引數로
미디어 타입을 넘겨 形式을 고를 수 있습니다:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
g.convert("漢字를 한글로", "text/plain");         // 平文 텍스트 (基本)
g.convert("<p>漢字</p>", "text/html");           // HTML 조각(fragment)
g.convert("# 漢字", "text/markdown");            // CommonMark
g.convert("# 漢字", "text/markdown; variant=GFM"); // GFM
~~~~

TypeScript 宣言에서는 위 表記만 새 形式으로 받습니다. 實行 時에는 두 바인딩 모두
CLI와 같은 規則으로 미디어 타입의 媒介變數를 解析하므로
`text/markdown;variant=GFM`도 쓸 수 있습니다.

### 以前 形式 바꾸기

以前 文字列(`"text"`, `"html"`, `"markdown"`)과 Markdown 客體는 Gukhanmun 1.0
前까지 쓸 수 있습니다. 앞으로는 위의 미디어 타입을 쓰십시오. Markdown 客體는
`gfm`이 `true`이면 `"text/markdown; variant=GFM"`으로, 아니면
`"text/markdown"`으로 바꾸십시오.

以前 形式을 쓰는 呼出을 찾으려면 應用 프로그램에서 [LogTape]를 設定해
`["gukhanmun", "format"]` 分類의 `warning` 記錄을 받으십시오. 警告마다 바꿔
쓸 미디어 타입이 나옵니다. 로깅을 設定하지 않으면 아무것도 出力하지 않습니다.

[LogTape]: https://logtape.org/manual/library.md#for-application-developers


誤謬 處理
---------

`load()`와 `convert()`는 失敗 時 `GukhanmunError`를 던집니다:

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
