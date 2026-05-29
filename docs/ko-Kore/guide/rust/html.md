---
title: HTML 變換
description: |-
  Gukhanmun Rust 라이브러리로 HTML 文書 變換하기.
---

HTML 變換
=========

`html` 피처가 必要합니다(基本으로 켜짐).


HTML 斷片 變換
--------------

~~~~ rust
use gukhanmun::{Builder, Preset};

let converter = Builder::with_preset(Preset::KoKr).build()?;
let output = converter.convert_html_fragment_to_string("<p>漢字</p>")?;
// → "<p>한자</p>"
~~~~

Gukhanmun은 入力을 (完全한 文書가 아니라) HTML 斷片으로 解析하고, 텍스트 노드와
屬性 속의 漢字를 變換하며, 모든 태그와 屬性을 그대로 保存하면서 結果를
直列化합니다.


恒常 保存되는 要素
------------------

다음 要素는 결코 修正되지 않습니다:

 -  `<code>`, `<kbd>`, `<pre>`, `<samp>`
 -  `<script>`, `<style>`, `<textarea>`
 -  `translate="no"`를 가진 要素
 -  `<ruby>` 註釋 內容(`<rt>`, `<rp>`)


使用者 定義 保存 規則
---------------------

`builder.html_preserve_when`을 使用하여, 各 要素의 태그名과 屬性을 받고 變換을
건너뛸 때 `true`를 返還하는 述語를 登錄합니다:

~~~~ rust
builder.html_preserve_when(|tag, attrs| {
    // class "math"나 屬性 "data-no-hanja"를 가진 要素를 保存
    let has_math_class = attrs.get("class")
        .is_some_and(|c| c.split_whitespace().any(|cls| cls == "math"));
    let has_attr = attrs.contains_key("data-no-hanja");
    has_math_class || has_attr
});
~~~~

`html_preserve_when`을 여러 番 呼出하면 述語가 追加됩니다; `true`를 返還하는
述語가 하나라도 있으면 그 要素(와 그 後孫)가 保存됩니다.
