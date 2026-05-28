---
title: "Markdown conversion"
description: |-
  Converting Markdown documents with the Gukhanmun Rust library.
---

Markdown conversion
===================

Requires the `markdown` feature (enabled by default).


Converting a Markdown string
----------------------------

~~~~ rust
use gukhanmun::{Builder, Preset, MarkdownVariant};

let converter = Builder::with_preset(Preset::KoKr).build()?;

// CommonMark
let output = converter.convert_markdown_to_string(
    "# 漢字\n\nHanja converted to hangul.",
    MarkdownVariant::CommonMark,
)?;

// GitHub Flavored Markdown (tables, task lists, strikethrough)
let output = converter.convert_markdown_to_string(
    "| 漢字 | 한자 |\n|------|------|\n| 東 | 동 |",
    MarkdownVariant::Gfm,
)?;
~~~~


What gets converted
-------------------

Gukhanmun converts hanja inside:

 -  Paragraph text
 -  Headings
 -  List items
 -  Blockquote content
 -  Table cells (GFM)
 -  Inline HTML text nodes

The following are left untouched:

 -  Fenced and indented code blocks
 -  Inline code spans (`` `…` ``)
 -  Raw HTML blocks
 -  Link and image URLs


Iterator version
----------------

Use `convert_markdown_iter` to process the output token by token:

~~~~ rust
use gukhanmun::MarkdownVariant;

for token in converter.convert_markdown_iter(source, MarkdownVariant::CommonMark) {
    print!("{}", token.as_str());
}
~~~~
