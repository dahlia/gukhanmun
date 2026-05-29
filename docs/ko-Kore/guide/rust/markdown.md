---
title: Markdown 變換
description: |-
  Gukhanmun Rust 라이브러리로 Markdown 文書 變換하기.
---

Markdown 變換
=============

`markdown` 피처가 必要합니다(基本으로 켜짐).


Markdown 文字列 變換
--------------------

~~~~ rust
use gukhanmun::{Builder, Preset, MarkdownVariant};

let converter = Builder::with_preset(Preset::KoKr).build()?;

// CommonMark
let output = converter.convert_markdown_to_string(
    "# 漢字\n\nHanja converted to hangul.",
    MarkdownVariant::CommonMark,
)?;

// GitHub Flavored Markdown (表, 作業 目錄, 取消線)
let output = converter.convert_markdown_to_string(
    "| 漢字 | 한자 |\n|------|------|\n| 東 | 동 |",
    MarkdownVariant::Gfm,
)?;
~~~~


變換되는 對象
-------------

Gukhanmun은 다음 안의 漢字를 變換합니다:

 -  段落 텍스트
 -  헤딩
 -  리스트 項目
 -  引用 블록 內容
 -  表 셀(GFM)
 -  인라인 HTML 텍스트 노드

다음은 손대지 않습니다:

 -  펜스/들여쓰기 코드 블록
 -  인라인 코드 스팬(`` `…` ``)
 -  날(raw) HTML 블록
 -  링크와 이미지 URL


이터레이터 版
-------------

出力을 토큰 單位로 處理하려면 `convert_markdown_iter`를 使用합니다:

~~~~ rust
use gukhanmun::MarkdownVariant;

for token in converter.convert_markdown_iter(source, MarkdownVariant::CommonMark) {
    print!("{}", token.as_str());
}
~~~~
