gukhanmun
=========

Umbrella library for hanja-to-hangul conversion. This crate wires together the
engine, format adapters, and dictionary backends from the workspace into a
single `Builder`/`Converter` facade.


Installation
------------

~~~~ toml
[dependencies]
gukhanmun = "0.1"
~~~~

All features are enabled by default. To trim the dependency tree, disable them
selectively:

~~~~ toml
[dependencies]
gukhanmun = { version = "0.1", default-features = false, features = ["html"] }
~~~~

Available features: `html`, `markdown`, `fst`, `cdb`, `stdict` (implies
`fst`). The `stdict` feature embeds the South Korean Standard Dictionary and
adds roughly 3 MB to the binary.


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
    .no_bundled_stdict()
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

`Preset::KoKr` (the default) loads the bundled Standard Korean Language
Dictionary and applies the initial sound law. `Preset::KoKp` omits both,
following North Korean orthographic conventions where Sino-Korean words are
written without the initial sound law (래일, 류행, 녀자).


Relation to the other workspace crates
--------------------------------------

`gukhanmun` re-exports public items from `gukhanmun-core`, `gukhanmun-html`,
`gukhanmun-markdown`, `gukhanmun-fst`, `gukhanmun-cdb`, and `gukhanmun-stdict`
under feature gates. Code that needs only a subset of the pipeline can depend
on those crates directly. The full workspace is documented in `DESIGN.md` at
the repository root.


License
-------

GPL-3.0-only. See `LICENSE` at the repository root.
