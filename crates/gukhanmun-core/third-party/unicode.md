Unicode Data
============

The fallback hanja reading table in `src/generated/unihan_readings.rs` is
generated from Unicode 17.0.0 `Unihan_Readings.txt`, specifically the
`kHangul` property in:

https://www.unicode.org/Public/17.0.0/ucd/Unihan.zip

The source archive is distributed under the Unicode License V3:

https://www.unicode.org/license.txt

The generated table prefers the `kHangul` reading tagged `E` as the canonical
pre-initial-sound-law fallback reading. If no `E`-tagged reading exists, it uses
the first listed reading. Additional readings are left for dictionary-backed
conversion stages.
