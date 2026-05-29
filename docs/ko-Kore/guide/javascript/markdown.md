---
title: Markdown 變換
description: |-
  Gukhanmun JavaScript 라이브러리로 Markdown 文書 變換하기.
---

Markdown 變換
=============

`convert()`에 形式으로 `"markdown"`을 넘깁니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({ dictionaries: [await stdictFst()] });

const output = g.convert("# 漢字\n\n漢字를 한글로 변환합니다.", "markdown");
// → "# 한자\n\n한자를 한글로 변환합니다."
~~~~


GitHub Flavored Markdown
------------------------

GFM 擴張(表, 作業 目錄, 取消線)을 켜려면 `gfm: true`인 形式 客體를 넘깁니다:

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
const output = g.convert(
  "| 漢字 | 讀音 |\n|------|------|\n| 東 | 동 |",
  { format: "markdown", gfm: true },
);
~~~~


變換되는 對象
-------------

Gukhanmun은 段落 텍스트·헤딩·리스트 項目·引用 블록·表 셀 안의 漢字를 變換합니다.

다음은 恒常 손대지 않습니다:

 -  펜스 코드 블록과 들여쓰기 코드 블록
 -  인라인 코드 스팬(`` `…` ``)
 -  날(raw) HTML 블록과 인라인 HTML
 -  링크와 이미지 URL
