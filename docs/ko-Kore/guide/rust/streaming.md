---
title: 스트리밍 API
description: |-
  Gukhanmun의 이터레이터 API로 큰 文書를 漸進的으로 處理하기.
---

스트리밍 API
============

모든 `convert_*_to_string` 메서드는 返還하기 前에 全體 出力을 버퍼링합니다.
이터레이터 變種은 出力을 토큰 單位로 處理하게 해 주는데, 이는 큰 文書나 싱크에
漸進的으로 써야 할 때 有用합니다.


텍스트 이터레이터
-----------------

~~~~ rust
use gukhanmun::{Builder, Preset, write_plain_text};

let converter = Builder::with_preset(Preset::KoKr).build()?;

let output = write_plain_text(converter.convert_text_iter("漢字를 한글로"));
~~~~

`convert_text_iter`는 `impl Iterator<Item = RenderedToken>`을 返還합니다.
토큰을 손으로 이어 붙이지 말고 `write_plain_text`에 넘기십시오. 토큰 種類가
原文 텍스트와 倂記 텍스트를 區別하며, 그 區別을 안전한 出力으로 바꾸는 것이
라이터의 役割입니다.


HTML과 Markdown 이터레이터
--------------------------

~~~~ rust
use gukhanmun::{MarkdownVariant, html, markdown};

// HTML
let html = html::write_html_fragment(converter.convert_html_fragment_iter("<p>漢字</p>")?);

// Markdown
let source = markdown::write_markdown(
    converter.convert_markdown_iter("# 漢字", MarkdownVariant::Gfm),
)?;
~~~~

各 어댑터의 라이터가 그 포맷의 이스케이프 規則을 담당합니다. 特히 辭典
讀音은 `RenderedToken::Annotated` 토큰으로 到達하며, `write_html_fragment`는
이를 文字 데이터로 이스케이프하므로 讀音이 마크업이 될 수 없습니다.


形式 中立的 토큰 파이프라인
---------------------------

`convert_tokens`는 `InputToken`의 任意 이터레이터를 받아 `RenderedToken`의
이터레이터를 返還합니다.  이는 가장 低水準의 進入點이며, 使用者 定義 리더에서
미리 토큰化된 入力을 提供하게 해 줍니다:

~~~~ rust
use gukhanmun::InputToken;

let tokens: Vec<InputToken<&str>> = /* 使用者의 토크나이저 出力 */;
for rendered in converter.convert_tokens(tokens.into_iter()) {
    // 出力 포맷에 맞는 라이터로 直列化하십시오.
}
~~~~

使用者 定義 라이터는 `RenderedToken::Annotated`를 文字 그대로 直列化해야
합니다. `Text`는 이미 그 포맷에 適合한 原文 데이터이고, `Annotated`는 辭典
等의 倂記 來源에서 生成된 데이터라 任意 文字를 담을 수 있습니다.
`Annotated`를 이스케이프 없이 그대로 내보내면 GHSA-j87h-8h88-wm6g가
되살아납니다.


토큰 種類
---------

| 種類            | 說明                                                                      |
| --------------- | ------------------------------------------------------------------------- |
| `InputToken`    | 入力 單位: 漢字 連續, 한글 텍스트, 섞인 텍스트, 블록 境界, 또는 섹션 境界 |
| `OutputToken`   | 辭典 찾기와 倂記 以後의 單位: 倂記된 漢字, 純粹 텍스트, 또는 境界 標識    |
| `RenderedToken` | 렌더링 以後의 出力 토큰: 原文 텍스트, 生成된 倂記 텍스트, 또는 境界       |
