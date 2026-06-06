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

Or write the dependency by hand:

~~~~ toml
[dependencies]
gukhanmun = "0.1"
~~~~


Feature flags
-------------

All features are enabled by default.  Disable the ones you do not need to
reduce compile time and binary size:

| Feature    | What it adds                                 | Default |
| ---------- | -------------------------------------------- | ------- |
| `html`     | HTML fragment conversion                     | yes     |
| `markdown` | Markdown conversion                          | yes     |
| `fst`      | FST dictionary backend (*.gukfst* files)     | yes     |
| `cdb`      | CDB dictionary backend (*.gukcdb* files)     | yes     |
| `stdict`   | Bundled *Standard Korean Dictionary* (~3 MB) | yes     |

To build without the bundled dictionary (useful when you supply your own):

~~~~ toml
[dependencies]
gukhanmun = { version = "0.1", default-features = false, features = ["fst"] }
~~~~

To build a minimal plain-text-only binary:

~~~~ toml
[dependencies]
gukhanmun = { version = "0.1", default-features = false, features = ["stdict"] }
~~~~
