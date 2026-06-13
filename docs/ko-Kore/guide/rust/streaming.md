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
use gukhanmun::{Builder, Preset};

let converter = Builder::with_preset(Preset::KoKr).build()?;

let mut output = String::new();
for token in converter.convert_text_iter("漢字를 한글로") {
    output.push_str(token.as_str());
}
~~~~

`convert_text_iter`는 `impl Iterator<Item = RenderedToken>`을 返還합니다.  各
`RenderedToken`은 그 토큰의 出力 텍스트를 담은 文字列 슬라이스를 들고 있습니다.


HTML과 Markdown 이터레이터
--------------------------

~~~~ rust
use gukhanmun::MarkdownVariant;

// HTML
for token in converter.convert_html_fragment_iter("<p>漢字</p>")? {
    print!("{}", token.as_str());
}

// Markdown
for token in converter.convert_markdown_iter("# 漢字", MarkdownVariant::Gfm) {
    print!("{}", token.as_str());
}
~~~~


形式 中立的 토큰 파이프라인
---------------------------

`convert_tokens`는 `InputToken`의 任意 이터레이터를 받아 `RenderedToken`의
이터레이터를 返還합니다.  이는 가장 低水準의 進入點이며, 使用者 定義 리더에서
미리 토큰化된 入力을 提供하게 해 줍니다:

~~~~ rust
use gukhanmun::{InputToken, RenderedToken};

let tokens: Vec<InputToken<&str>> = /* 使用者의 토크나이저 出力 */;
for rendered in converter.convert_tokens(tokens.into_iter()) {
    print!("{}", rendered.as_str());
}
~~~~

形式 어댑터(`gukhanmun-html`, `gukhanmun-markdown`)는 內部的으로 이를
使用합니다.


토큰 種類
---------

| 種類            | 說明                                                                      |
| --------------- | ------------------------------------------------------------------------- |
| `InputToken`    | 入力 單位: 漢字 連續, 한글 텍스트, 섞인 텍스트, 블록 境界, 또는 섹션 境界 |
| `OutputToken`   | 辭典 찾기와 倂記 以後의 單位: 倂記된 漢字, 平文 텍스트, 또는 境界 標識    |
| `RenderedToken` | 렌더링 以後의 出力 텍스트 單位; 最終 文字列을 담습니다                    |
