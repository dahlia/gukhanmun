---
title: "Introduction"
description: |-
  An overview of Gukhanmun and how to choose between the CLI, Rust,
  JavaScript and Python interfaces.
---

Introduction
============

Gukhanmun is a hanja-to-hangul converter for Korean text.  Given mixed-script
input containing hanja (漢字) alongside hangul, it produces hangul-only output
or annotated output in several formats: plain text, Markdown, and HTML
(including `<ruby>` markup).

The converter supports the *Standard Korean Dictionary* (標準國語大辭典),
*Open Korean Dictionary* and NIKL loanword spelling examples. Rust and the CLI
bundle dictionaries; JavaScript and Python load them explicitly.
It supports South Korean (`ko-kr`) and North Korean (`ko-kp`) orthography
presets, configurable segmentation, numeral handling, homophone disambiguation,
and per-character annotation directives.


Choosing an interface
---------------------

| Interface  | When to use                                                  |
| ---------- | ------------------------------------------------------------ |
| CLI        | Convert files from the terminal or in shell pipelines        |
| Rust       | Embed the converter in a Rust application or library         |
| JavaScript | Use from Node.js, Deno, Bun, or a browser                    |
| Python     | Convert text in Python NLP and document-processing pipelines |

### CLI

The `gukhanmun` command-line tool reads from files or standard input and
writes to standard output or a file.  It auto-detects the format from the
file extension (*.html*, *.md*) and supports all conversion options via flags.

Start here: [*Installation*](./cli/install.mdx).

The standalone packages are `@gukhanmun/cli` on npm and `gukhanmun-cli` on
PyPI. They install both `gukhanmun` and `gukhanmun-mkdict`, not a library API.

### Rust

The `gukhanmun` crate exposes a `Builder`/`Converter` API.  Build a converter
once, reuse it across calls, and stream output token by token if needed.
Feature flags let you trim the binary size by disabling HTML, Markdown, or the
bundled dictionary.

Start here: [*Installation*](./rust/install.md).

### JavaScript

Two packages cover different environments:

 -  `@gukhanmun/wasm` runs in any JavaScript runtime via WebAssembly.  Use it
    in browsers, serverless functions, or anywhere native addons are not
    available.
 -  `@gukhanmun/napi` is a native Node.js addon with higher throughput.  It
    supports Node.js 20+, Deno 2.0+, and Bun 1.0+.

Both share the same `load()` API and accept identical options.

Start here: [*Installation*](./javascript/install.mdx).

### Python

The Python package provides a reusable `Converter(...)` with typed keyword
options and explicit dictionary sources. Install from PyPI, then convert
plain text, HTML or Markdown in the same process.

Start here: [*Installation*](./python/install.mdx).
