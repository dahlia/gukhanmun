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

All features below are enabled by default. Disable the defaults
and select only the features you need to reduce compile time and binary size:

| Feature     | What it adds                                       | Default |
| ----------- | -------------------------------------------------- | ------- |
| `html`      | HTML fragment conversion                           | yes     |
| `markdown`  | Markdown conversion                                | yes     |
| `fst`       | FST dictionary backend (*.gukfst* files)           | yes     |
| `cdb`       | CDB dictionary backend (*.gukcdb* files)           | yes     |
| `stdict`    | Bundled *Standard Korean Dictionary* (~3 MB)       | yes     |
| `opendict`  | Bundled *Open Korean Dictionary* (우리말샘, ~8 MB) | yes     |
| `loanwords` | Bundled NIKL loanword examples (implies `fst`)     | yes     |

The `loanwords` feature includes `gukhanmun-loanwords`, the National Institute
of Korean Language's *Loanword spelling examples* dictionary. `Preset::KoKr`
selects it automatically; `Preset::KoKp` leaves it off. If you disable default
features, enable `loanwords` explicitly to make it available:

~~~~ sh
cargo add gukhanmun --no-default-features -F stdict,loanwords
~~~~

To build without the bundled dictionary (useful when you supply your own):

~~~~ sh
cargo add gukhanmun --no-default-features -F fst
~~~~

To build a minimal plain-text-only binary:

~~~~ sh
cargo add gukhanmun --no-default-features -F stdict
~~~~

When disabling default features, also disable the corresponding preset
dictionaries on the builder. With only `fst`, call
`.no_bundled_dictionaries()`; with only `stdict`, call
`.no_bundled_loanwords()` before `.build()`. Requesting a bundled dictionary
whose feature is disabled returns a configuration error.
