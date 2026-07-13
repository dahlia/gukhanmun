---
title: 렌더링 모드
description: |-
  Rust에서 Gukhanmun이 漢字와 한글 讀音을 어떻게 表示할지 制御하기.
---

렌더링 모드
===========

`builder.rendering(mode)`은 漢字와 그 한글 讀音이 出力에 어떻게 나타날지
設定합니다.


`RenderMode` 變種
-----------------

~~~~ rust
use gukhanmun::RenderMode;

builder.rendering(RenderMode::HangulOnly);          // "한자" (基本)
builder.rendering(RenderMode::HangulHanjaParens);   // "한자(漢字)"
builder.rendering(RenderMode::HanjaHangulParens);   // "漢字(한자)"
builder.rendering(RenderMode::RubyOnHangul);        // <ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>
builder.rendering(RenderMode::RubyOnHanja);         // <ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>
builder.rendering(RenderMode::Original);            // 漢字 維持, 必要한 곳에 倂記
~~~~

`RubyOnHangul`과 `RubyOnHanja`는 `<ruby>` 마크업을 만듭니다; HTML이나 Markdown
出力에서 가장 有用합니다.  平文 텍스트 모드에서는 括弧로 退化합니다.  註釋은
`<rp>`(ruby parenthesis) 要素로 감싸지므로, `<ruby>`를 支援하지 않는 브라우저도
讀音을 基底 텍스트에 섞지 않고 括弧로 묶인 倂記(`한자(漢字)`)로 렌더링합니다.


倂記 樣式을 곁들인 `Original` 모드
----------------------------------

`RenderMode::Original`은 漢字를 제자리에 維持하고 區別이 必要한 同音異義語에만
倂記를 더합니다.  倂記 樣式까지 設定하려면 `RenderOptions`를 使用합니다:

~~~~ rust
use gukhanmun::{RenderOptions, RenderMode, OriginalGloss};

builder.rendering(RenderOptions {
    mode: RenderMode::Original,
    original_gloss: OriginalGloss::Parens,  // 또는 OriginalGloss::Ruby
    ..RenderOptions::default()
});
~~~~


異體字 集合
-----------

異體字 認識은 언제나 活性化됩니다. `Builder::hanja_variant_set`는 렌더러가
漢字를 내보낼 때의 表記를 獨立的으로 選擇합니다.

~~~~ rust
use gukhanmun::HanjaVariantSet;

builder.hanja_variant_set(HanjaVariantSet::Shinjitai);
~~~~

프로필은 `AsDictionary`(基本), `Shinjitai`, `Kanxi`, `Simplified`,
`Asahimoji`입니다.
