---
pageType: home

hero:
  name: Gukhanmun
  text: Mixed-script to hangul!
  tagline: Rust/JavaScript library that converts mixed-script Korean into hangul-only text
  actions:
  - theme: brand
    text: Introduction
    link: /guide/intro
  - theme: alt
    text: CLI
    link: /guide/cli/install
  - theme: alt
    text: Rust
    link: /guide/rust/install
  - theme: alt
    text: JavaScript
    link: /guide/javascript/install
  image:
    src:
      light: /logo.svg
      dark: /logo-dark.svg
    alt: Logo
features:
- title: Hanja-to-hangul conversion
  details: Converts mixed-script Korean text containing hanja into hangul-only output, powered by the bundled Standard Korean Dictionary.
  icon: 🔤
  link: /guide/cli/start
- title: Multiple output formats
  details: Processes plain text, HTML fragments, and Markdown (CommonMark and GFM). Supports hangul-only, parenthetical, and ruby markup rendering modes.
  icon: 📝
  link: /guide/cli/rendering
- title: Custom dictionaries
  details: Load domain-specific vocabulary in FST or CDB format alongside the bundled dictionary to handle specialised terminology.
  icon: 📚
  link: /guide/cli/dictionary
- title: Per-hanja directives
  details: Force or suppress annotations for specific characters or glob patterns, inline or via a directives file.
  icon: 🎛️
  link: /guide/cli/directives
- title: CLI, Rust, and JavaScript
  details: Use from the command line, embed in a Rust crate, or run in browsers, Node.js, Deno, and Bun via WebAssembly or a native Node.js addon.
  icon: 🧩
  link: /guide/javascript/start
- title: Streaming API
  details: Process arbitrarily large documents chunk by chunk with a TransformStream interface in JavaScript or an iterator API in Rust.
  icon: 🌊
  link: /guide/javascript/streaming
---

