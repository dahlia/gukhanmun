---
title: HTML 處理
description: |-
  Gukhanmun JavaScript 라이브러리에서 HTML을 變換하고 特定 要素를 保存하기.
---

HTML 處理
=========

`convert()`에 形式으로 `"html"`을 넘깁니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({ dictionaries: [await stdictFst()] });

console.log(g.convert("<p>漢字</p>", "html"));
// → <p>한자</p>
~~~~

Gukhanmun은 入力을 HTML 斷片으로 解析하고, 텍스트 노드의 漢字를 變換하며, 모든
태그와 屬性을 保存하면서 結果를 直列化합니다.


恒常 保存되는 要素
------------------

다음 要素와 그 後孫은 결코 修正되지 않습니다:

 -  `<code>`, `<kbd>`, `<pre>`, `<samp>`
 -  `<script>`, `<style>`, `<textarea>`
 -  `translate="no"`를 가진 要素
 -  `<ruby>` 註釋 內容


CSS 클래스로 要素 保存
----------------------

特定 클래스를 가진 要素 안의 變換을 건너뛰려면 `html.preserveClasses`를
使用합니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  dictionaries: [await stdictFst()],
  html: {
    preserveClasses: ["math", "no-translate"],
  },
});

const html = `
  <p>漢字</p>
  <span class="math">漢字</span>
`;
console.log(g.convert(html, "html"));
// → <p>한자</p>
//   <span class="math">漢字</span>   ← 保存됨
~~~~


屬性으로 要素 保存
------------------

屬性 이름 또는 屬性 이름/값 雙에 따라 變換을 건너뛰려면
`html.preserveAttributes`를 使用합니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  dictionaries: [await stdictFst()],
  html: {
    preserveAttributes: [
      "data-no-hanja",            // 이 屬性을 가진 任意의 要素
      { name: "lang", value: "en" }, // lang="en"인 要素
    ],
  },
});
~~~~


HTML에서의 루비 마크업
----------------------

`"html"` 形式을 루비 렌더링 모드와 結合합니다:

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

`<rp>`(ruby parenthesis) 要素는 fallback을 提供합니다: `<ruby>`를 支援하지 않는
브라우저는 讀音을 基底 텍스트에 섞지 않고 括弧 안에(`한자(漢字)`) 表示합니다.
