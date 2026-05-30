gukhanmun-core
==============

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

Format-neutral core of the Gukhanmun hanja-to-hangul pipeline. The crate is
`no_std` + `alloc`, so it is suitable for embedded and WASM targets. Format
adapters, dictionary backends, and runtime I/O live in separate crates.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-core?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-core
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-core
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


What this crate provides
------------------------

`HanjaDictionary` is the trait that all dictionary backends implement. The
engine works over any value that satisfies it. `ChainDictionary` composes two
backends so that a user-supplied dictionary is consulted before a larger
pre-built one.

The lattice segmenter covers all possible dictionary matches for a hanja span
and picks the best split using dynamic programming. `行事場所` segments as
行事 + 場所 rather than 行事場 + 所.

Fallback phonetization kicks in for characters not found in any dictionary. It
uses a table derived from the Unicode Unihan database (`kHangul` field) and
applies the initial sound law (頭音法則) when the active preset requires it.

The rendering step turns engine output into text. `RenderMode` controls the
shape: hangul-only, hangul(hanja) parentheses, hanja(hangul) parentheses, ruby
markup, or the original mixed-script form with selective glosses.


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-core = "0.1"
~~~~


Usage
-----

~~~~ rust
use gukhanmun_core::{MapDictionary, RenderMode, convert_plain_text};

let mut dict = MapDictionary::new();
dict.insert("漢字", "한자");
dict.insert("北京", "베이징");

let output = convert_plain_text("漢字 北京", &dict, RenderMode::HangulOnly);
assert_eq!(output, "한자 베이징");
~~~~


`no_std` note
-------------

The crate declares `#![no_std]` and uses `extern crate alloc`. Callers on
`std` targets get `alloc` for free; embedded targets need a global allocator.
The dictionary trait itself is synchronous and allocation-free on the hot path.


License
-------

GPL-3.0-only. See `LICENSE` at the repository root.
