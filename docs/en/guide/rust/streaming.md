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
use gukhanmun::{Builder, Preset, write_plain_text};

let converter = Builder::with_preset(Preset::KoKr).build()?;

let output = write_plain_text(converter.convert_text_iter("漢字를 한글로"));
~~~~

`convert_text_iter` returns `impl Iterator<Item = RenderedToken>`.  Hand the
iterator to `write_plain_text` rather than concatenating it by hand; the token
type distinguishes source text from annotation text, and a writer is what turns
that distinction into safe output.


HTML and Markdown iterators
---------------------------

~~~~ rust
use gukhanmun::{MarkdownVariant, html, markdown};

// HTML
let html = html::write_html_fragment(converter.convert_html_fragment_iter("<p>漢字</p>")?);

// Markdown
let source = markdown::write_markdown(
    converter.convert_markdown_iter("# 漢字", MarkdownVariant::Gfm),
)?;
~~~~

The adapters' writers own the escaping rules for their format.  In particular,
dictionary readings end up in `RenderedToken::Annotated` tokens, and
`write_html_fragment` escapes those as character data so a reading can never
become markup.


Format-agnostic token pipeline
------------------------------

`convert_tokens` accepts any iterator of `InputToken` and returns an iterator
of `RenderedToken`.  This is the lowest-level entry point and lets you supply
pre-tokenised input from a custom reader:

~~~~ rust
use gukhanmun::InputToken;

let tokens: Vec<InputToken<&str>> = /* your tokeniser output */;
for rendered in converter.convert_tokens(tokens.into_iter()) {
    // Serialize with the writer for your output format.
}
~~~~

A custom writer must serialize `RenderedToken::Annotated` as literal text:
`Text` is source data that is already valid for the format, while `Annotated`
is generated from a dictionary or other annotation source and may contain
arbitrary characters.  Passing `Annotated` through unescaped reintroduces
GHSA-j87h-8h88-wm6g.


Token types
-----------

| Type            | Description                                                                                    |
| --------------- | ---------------------------------------------------------------------------------------------- |
| `InputToken`    | A unit of input: hanja run, hangul text, mixed text, block boundary, or section boundary       |
| `OutputToken`   | A unit after dictionary lookup and annotation: annotated hanja, plain text, or boundary marker |
| `RenderedToken` | A unit of output text after rendering: source text, generated annotation text, or a boundary   |
