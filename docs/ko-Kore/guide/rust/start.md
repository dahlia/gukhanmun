---
title: 빠른 始作
description: |-
  Gukhanmun Rust 라이브러리의 基本 使用法.
---

빠른 始作
=========

Gukhanmun의 Rust API는 `Builder`/`Converter` 雙을 中心으로 構成됩니다.
`Builder`는 옵션을 모으고; `Converter`는 `Builder::build()`가 만들어 내는
不變의 런타임입니다.


最小 例示
---------

~~~~ rust
use gukhanmun::{Builder, Preset};

fn main() -> gukhanmun::Result<()> {
    let converter = Builder::with_preset(Preset::KoKr).build()?;
    let output = converter.convert_text_to_string("漢字를 한글로")?;
    println!("{output}");  // 한자를 한글로
    Ok(())
}
~~~~


Builder 패턴
------------

`Builder`는 流暢한 境界面을 使用합니다.  모든 setter가 `&mut Self`를 返還하므로
呼出을 連鎖할 수 있습니다:

~~~~ rust
use gukhanmun::{Builder, Preset, RenderMode, NumeralStrategy};

let converter = Builder::with_preset(Preset::KoKr)
    .rendering(RenderMode::HangulHanjaParens)
    .numerals(NumeralStrategy::Smart)
    .build()?;
~~~~

`Builder::new()`는 프리셋이 適用되지 않은 빌더를 만듭니다(모든 옵션이 各自의
基本값).  `Builder::with_preset(preset)`은 먼저 프리셋을 適用한 뒤 個別 옵션을
덮어쓸 수 있게 합니다.


다른 形式들의 變換
------------------

`Converter`는 支援하는 各 形式마다 變換 메서드를 가집니다:

~~~~ rust
// 純粹 텍스트
let text = converter.convert_text_to_string("漢字를 한글로")?;

// HTML 斷片 (`html` 피처 必要)
let html = converter.convert_html_fragment_to_string("<p>漢字</p>")?;

// Markdown (`markdown` 피처 必要)
use gukhanmun::MarkdownVariant;
let md = converter.convert_markdown_to_string("# 漢字", MarkdownVariant::CommonMark)?;
~~~~

모든 메서드는 `Result<String, gukhanmun::Error>`를 返還합니다.
