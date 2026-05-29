---
title: 렌더링 모드
description: |-
  JavaScript에서 Gukhanmun이 漢字와 한글 讀音을 어떻게 表示할지 制御하기.
---

렌더링 모드
===========

漢字와 그 한글 讀音이 出力에 어떻게 나타날지 制御하려면 `load`의 `rendering`
옵션을 設定합니다.


使用 可能한 모드
----------------

| 값                      | 入力 | 出力                                                 |
| ----------------------- | ---- | ---------------------------------------------------- |
| `"hangul-only"` (基本)  | 漢字 | 한자                                                 |
| `"hangul-hanja-parens"` | 漢字 | 한자(漢字)                                           |
| `"hanja-hangul-parens"` | 漢字 | 漢字(한자)                                           |
| `"ruby-on-hangul"`      | 漢字 | `<ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>` |
| `"ruby-on-hanja"`       | 漢字 | `<ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>` |
| `"original"`            | 漢字 | 漢字 (必要한 곳에 倂記 追加)                         |

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  rendering: "hangul-hanja-parens",
  dictionaries: [await stdictFst()],
});

console.log(g.convert("漢字"));  // 한자(漢字)
~~~~


루비 마크업
-----------

`"ruby-on-hangul"`과 `"ruby-on-hanja"`는 `<ruby>` 要素를 만듭니다.  `"html"`이나
`"markdown"` 形式과 함께 使用합니다; 純粹 텍스트에서는 括弧로 退化합니다.
註釋은 `<rp>`(ruby parenthesis) 要素로 감싸지므로, `<ruby>`를 支援하지 않는
브라우저도 讀音을 基底 텍스트에 섞지 않고 括弧로 묶인 倂記(`한자(漢字)`)로
보입니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  rendering: "ruby-on-hangul",
  dictionaries: [await stdictFst()],
});

console.log(g.convert("<p>漢字</p>", "html"));
// → <p><ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby></p>
~~~~


倂記를 곁들인 Original 모드
---------------------------

區別이 必要한 곳에만 倂記를 더하려면 `"original"`을 `originalGloss`와 함께
使用합니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  rendering: "original",
  originalGloss: "parens",  // 또는 "ruby"
  dictionaries: [await stdictFst()],
});

console.log(g.convert("東京에서 東洋까지"));
// 同音異義語 倂記됨: 東京(동경)에서 東洋(동양)까지
~~~~
