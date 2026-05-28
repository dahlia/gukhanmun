---
title: "Quick start"
description: |-
  Basic usage of the Gukhanmun Rust library.
---

Quick start
===========

Gukhanmun's Rust API is built around a `Builder`/`Converter` pair.  `Builder`
collects options; `Converter` is the immutable runtime produced by
`Builder::build()`.


Minimal example
---------------

~~~~ rust
use gukhanmun::{Builder, Preset};

fn main() -> gukhanmun::Result<()> {
    let converter = Builder::with_preset(Preset::KoKr).build()?;
    let output = converter.convert_text_to_string("漢字를 한글로")?;
    println!("{output}");  // 한자를 한글로
    Ok(())
}
~~~~


Builder pattern
---------------

`Builder` uses a fluent interface.  Every setter returns `&mut Self`, so you
can chain calls:

~~~~ rust
use gukhanmun::{Builder, Preset, RenderMode, NumeralStrategy};

let converter = Builder::with_preset(Preset::KoKr)
    .rendering(RenderMode::HangulHanjaParens)
    .numerals(NumeralStrategy::Smart)
    .build()?;
~~~~

`Builder::new()` creates a builder with no preset applied (all options at
their individual defaults).  `Builder::with_preset(preset)` applies a preset
first and then lets you override individual options.


Converting different formats
----------------------------

`Converter` has a conversion method for each supported format:

~~~~ rust
// Plain text
let text = converter.convert_text_to_string("漢字를 한글로")?;

// HTML fragment (requires the `html` feature)
let html = converter.convert_html_fragment_to_string("<p>漢字</p>")?;

// Markdown (requires the `markdown` feature)
use gukhanmun::MarkdownVariant;
let md = converter.convert_markdown_to_string("# 漢字", MarkdownVariant::CommonMark)?;
~~~~

All methods return `Result<String, gukhanmun::Error>`.
