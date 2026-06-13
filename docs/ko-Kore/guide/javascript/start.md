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

`g.convert(source)`는 基本的으로 平文 텍스트를 變換합니다.  HTML이나 Markdown을
爲해서는 두 番째 引數로 形式 文字列이나 客體를 넘깁니다:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
g.convert("漢字를 한글로");             // 平文 텍스트 (基本)
g.convert("<p>漢字</p>", "html");       // HTML 斷片
g.convert("# 漢字", "markdown");        // CommonMark
g.convert("# 漢字", { format: "markdown", gfm: true });  // GFM
~~~~


誤謬 處理
---------

`load()`와 `convert()`는 失敗 時 `GukhanmunError`를 던집니다:

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
