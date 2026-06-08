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

All features are enabled by default.  Disable the ones you do not need to
reduce compile time and binary size:

| Feature    | What it adds                                       | Default |
| ---------- | -------------------------------------------------- | ------- |
| `html`     | HTML fragment conversion                           | yes     |
| `markdown` | Markdown conversion                                | yes     |
| `fst`      | FST dictionary backend (*.gukfst* files)           | yes     |
| `cdb`      | CDB dictionary backend (*.gukcdb* files)           | yes     |
| `stdict`   | Bundled *Standard Korean Dictionary* (~3 MB)       | yes     |
| `opendict` | Bundled *Open Korean Dictionary* (우리말샘, ~8 MB) | yes     |

To build without the bundled dictionary (useful when you supply your own):

~~~~ sh
cargo add gukhanmun --no-default-features -F fst
~~~~

To build a minimal plain-text-only binary:

~~~~ sh
cargo add gukhanmun --no-default-features -F stdict
~~~~
