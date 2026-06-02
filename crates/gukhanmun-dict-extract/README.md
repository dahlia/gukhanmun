gukhanmun-dict-extract
======================

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

Shared dictionary dump extraction helpers for Gukhanmun.

This library crate contains shared key assembly and word normalization logic
for National Institute of Korean Language dictionary dumps such as the *Standard
Korean Language Dictionary* (標準國語大辭典) and the *Open Korean Dictionary*
(우리말샘). Source-specific extractors depend on this crate to focus on parsing
logic and category-specific policies.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-dict-extract?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-dict-extract
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-dict-extract
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-dict-extract = "0.2"
~~~~


Usage
-----

This crate provides helpers to extract and normalize dictionary headwords:

~~~~ rust
use gukhanmun_dict_extract::{keys_from_originals, normalize_word};

// Assemble lookup keys from original-language records
let keys = keys_from_originals(&original_language_infos);

// Normalize dictionary head words (stripping homograph digits, hyphens, and spaces)
let clean_word = normalize_word("힐난-조03");
assert_eq!(clean_word, "힐난조");
~~~~


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.
