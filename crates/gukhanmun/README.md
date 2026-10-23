gukhanmun
=========

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

Umbrella library for hanja-to-hangul conversion. This crate wires together the
engine, format adapters, and dictionary backends from the workspace into a
single `Builder`/`Converter` facade.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun
[GPL badge]: https://img.shields.io/crates/l/gukhanmun
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ toml
[dependencies]
gukhanmun = "0.1"
~~~~

The `html`, `markdown`, `fst`, `cdb`, `stdict`, `opendict`, and `loanwords`
features are enabled by default. To trim the dependency tree, disable the
defaults and select only the features you need:

~~~~ toml
[dependencies]
gukhanmun = { version = "0.1", default-features = false, features = ["html"] }
~~~~

Available features: `html`, `markdown`, `fst`, `cdb`, `stdict` (implies
`fst`), `opendict` (implies `fst`), and `loanwords` (implies `fst`). The
`loanwords` feature embeds the NIKL loanword spelling examples. The `stdict`
feature embeds the South Korean *Standard Korean Dictionary*, and `opendict`
embeds *Open Korean Dictionary* (우리말샘) category data.


Usage
-----

### Default preset (South Korean)

~~~~ rust
use gukhanmun::Builder;

let converter = Builder::new().build()?;
assert_eq!(converter.convert_text_to_string("學校")?, "학교");
~~~~

### Custom dictionary

~~~~ rust
use gukhanmun::{Builder, MapDictionary};

let mut dict = MapDictionary::new();
dict.insert("外字", "외자");
let converter = Builder::new()
    .no_bundled_dictionaries()
    .push_dictionary(dict)
    .build()?;
assert_eq!(converter.convert_text_to_string("外字")?, "외자");
~~~~

### North Korean preset

~~~~ rust
use gukhanmun::{Builder, Preset};

let converter = Builder::with_preset(Preset::KoKp).build()?;
// No initial sound law: 來日 → 래일
assert_eq!(converter.convert_text_to_string("來日")?, "래일");
~~~~

### HTML fragment

~~~~ rust
use gukhanmun::Builder;

let converter = Builder::new().build()?;
let output = converter.convert_html_fragment_to_string("<p>學校</p>")?;
assert!(output.contains("학교"));
~~~~


Presets
-------

`Preset::KoKr` (the default) loads the bundled *Standard Korean Language
Dictionary* and NIKL loanword examples, and applies the initial sound law.
`Preset::KoKp` loads the bundled *Open Korean Dictionary* North Korean category
and disables the initial sound law, following North Korean orthographic
conventions where Sino-Korean words are written without the initial sound law
(래일, 류행, 녀자). Use `Builder::no_bundled_dictionaries()` to disable every
bundled dictionary, or
`Builder::no_bundled_stdict()`/`Builder::no_bundled_opendict()`/
`Builder::no_bundled_loanwords()` to disable one bundled dictionary family.


Relation to the other workspace crates
--------------------------------------

`gukhanmun` re-exports public items from `gukhanmun-core`, `gukhanmun-html`,
`gukhanmun-markdown`, `gukhanmun-fst`, `gukhanmun-cdb`, `gukhanmun-stdict`,
`gukhanmun-opendict`, and `gukhanmun-loanwords` under feature gates. Code that
needs only a subset of the pipeline can depend on those crates directly. The
full workspace is documented in *DESIGN.md* at the repository root.


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.


Bundled loanword examples
-------------------------

The bundled NIKL loanword spelling examples are enabled by default for
`Preset::KoKr`, but remain opt-in for `Preset::KoKp` through
`Builder::bundled_loanwords()`. This dictionary takes priority over the other
preset dictionaries, while user dictionaries remain first.
`no_bundled_dictionaries()` disables every family and
`no_bundled_loanwords()` disables this one. The source snapshot and explicit
choices for competing readings are maintained in `gukhanmun-loanwords`.

~~~~ rust
# #[cfg(all(feature = "loanwords", feature = "stdict"))] {
use gukhanmun::{Builder, ContextWindow};
let converter = Builder::new()
    .homophone_window(ContextWindow::Off).build()?;
assert_eq!(converter.convert_text_to_string("香港")?, "홍콩");
# }
# Ok::<(), gukhanmun::Error>(())
~~~~
