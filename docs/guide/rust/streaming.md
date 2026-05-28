---
title: "Streaming API"
description: |-
  Processing large documents incrementally with Gukhanmun's iterator API.
---

Streaming API
=============

All `convert_*_to_string` methods buffer the full output before returning.
The iterator variants let you process the output token by token, which is
useful for large documents or when you need to write to a sink incrementally.


Text iterator
-------------

~~~~ rust
use gukhanmun::{Builder, Preset};

let converter = Builder::with_preset(Preset::KoKr).build()?;

let mut output = String::new();
for token in converter.convert_text_iter("漢字를 한글로") {
    output.push_str(token.as_str());
}
~~~~

`convert_text_iter` returns `impl Iterator<Item = RenderedToken>`.  Each
`RenderedToken` carries a string slice with the output text for that token.


HTML and Markdown iterators
---------------------------

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


Format-agnostic token pipeline
------------------------------

`convert_tokens` accepts any iterator of `InputToken` and returns an iterator
of `RenderedToken`.  This is the lowest-level entry point and lets you supply
pre-tokenised input from a custom reader:

~~~~ rust
use gukhanmun::{InputToken, RenderedToken};

let tokens: Vec<InputToken<&str>> = /* your tokeniser output */;
for rendered in converter.convert_tokens(tokens.into_iter()) {
    print!("{}", rendered.as_str());
}
~~~~

The format adapters (`gukhanmun-html`, `gukhanmun-markdown`) use this
internally.


Token types
-----------

| Type            | Description                                                                                    |
| --------------- | ---------------------------------------------------------------------------------------------- |
| `InputToken`    | A unit of input: hanja run, hangul text, mixed text, block boundary, or section boundary       |
| `OutputToken`   | A unit after dictionary lookup and annotation: annotated hanja, plain text, or boundary marker |
| `RenderedToken` | A unit of output text after rendering; carries the final string                                |
