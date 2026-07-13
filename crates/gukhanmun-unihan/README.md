gukhanmun-unihan
================

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

Code generator that downloads the Unicode Unihan database and produces the
*unihan\_readings.rs* source file that `gukhanmun-core` compiles in for
fallback hanja phonetization.

This is a development-time tool, not a library. Normal users of Gukhanmun do
not need to run it: the generated file is committed to the repository and
updated only when the Unicode version changes or the extraction logic is
revised.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-unihan?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-unihan
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-unihan
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


What it generates
-----------------

The tool reads the `kHangul` field from *Unihan\_Readings.txt* inside
*Unihan.zip* and emits a sorted `static` array mapping Unicode scalar values
to their Korean readings. `gukhanmun-core` compiles this array into its
fallback phonetizer so that characters not found in any loaded dictionary
still receive a plausible reading.

The Unicode version and the expected SHA-256 of *Unihan.zip* are pinned as
constants in the source. A checksum mismatch causes the tool to abort, so
accidental use of a different Unicode release is caught immediately.

The same run also reads `kZVariant`, `kSimplifiedVariant`,
`kTraditionalVariant`, and `kCompatibilityVariant`, then combines them with
the pinned Joyo and Asahi snapshots under *data/*. It builds equivalence
classes to validate source-data coverage, then emits the directional tables
and unambiguous reverse indexes used for dictionary recognition and hanja
variant-set output profiles. `kSemanticVariant`,
`kSpecializedSemanticVariant`, and `kSpoofingVariant` are intentionally not
used.


Running
-------

~~~~ sh
cargo run -p gukhanmun-unihan -- \
    --output crates/gukhanmun-core/src/generated/unihan_readings.rs
~~~~

The download is cached next to the output path between runs.


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.
