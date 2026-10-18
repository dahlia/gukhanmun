---
title: "Installation"
description: |-
  Adding Gukhanmun as a Rust dependency.
---

Installation
============

Add `gukhanmun` to your *Cargo.toml*:

~~~~ sh
cargo add gukhanmun
~~~~


Feature flags
-------------

All features except `loanwords` are enabled by default. Disable the defaults
and select only the features you need to reduce compile time and binary size:

| Feature     | What it adds                                       | Default |
| ----------- | -------------------------------------------------- | ------- |
| `html`      | HTML fragment conversion                           | yes     |
| `markdown`  | Markdown conversion                                | yes     |
| `fst`       | FST dictionary backend (*.gukfst* files)           | yes     |
| `cdb`       | CDB dictionary backend (*.gukcdb* files)           | yes     |
| `stdict`    | Bundled *Standard Korean Dictionary* (~3 MB)       | yes     |
| `opendict`  | Bundled *Open Korean Dictionary* (우리말샘, ~8 MB) | yes     |
| `loanwords` | Bundled NIKL loanword examples (implies `fst`)     | no      |

Enable `loanwords` explicitly before selecting the dictionary with
`Builder::bundled_loanwords()`:

~~~~ sh
cargo add gukhanmun -F loanwords
~~~~

To build without the bundled dictionary (useful when you supply your own):

~~~~ sh
cargo add gukhanmun --no-default-features -F fst
~~~~

To build a minimal plain-text-only binary:

~~~~ sh
cargo add gukhanmun --no-default-features -F stdict
~~~~
