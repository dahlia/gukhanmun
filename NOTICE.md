Gukhanmun data notices
======================

Standard Korean Language Dictionary data
----------------------------------------

The bundled dictionary data is derived from the *Standard Korean Language
Dictionary* (標準國語大辭典) JSON dump published by the National Institute of
Korean Language.

 -  Source: National Institute of Korean Language,
    [*Standard Korean Language Dictionary*]
 -  Snapshot: *전체 내려받기\_표준국어대사전\_JSON\_20260606.zip*
 -  Data license: [Creative Commons Attribution-ShareAlike 2.0 Korea]
    (CC BY-SA 2.0 KR), as stated in the
    [*Standard Korean Language Dictionary* copyright policy]

Gukhanmun extracts and normalizes dictionary records from the JSON dump into
the canonical TSV snapshot under *crates/gukhanmun-stdict/data/*. The bundled
FST and CDB files are generated from that modified snapshot.

[*Standard Korean Language Dictionary*]: https://stdict.korean.go.kr/
[Creative Commons Attribution-ShareAlike 2.0 Korea]: https://creativecommons.org/licenses/by-sa/2.0/kr/
[*Standard Korean Language Dictionary* copyright policy]: https://stdict.korean.go.kr/join/copyrightPolicy.do


Open Korean Dictionary data
---------------------------

The bundled dictionary data is derived from the *Open Korean Dictionary*
(우리말샘) JSON dump published by the National Institute of Korean Language.

 -  Source: National Institute of Korean Language, [*Open Korean Dictionary*]
 -  Snapshot: *전체 내려받기\_우리말샘\_json\_20260603.zip*
 -  Data license: [Creative Commons Attribution-ShareAlike 2.0 Korea]
    (CC BY-SA 2.0 KR), as stated in the
    [*Open Korean Dictionary* copyright policy]

Gukhanmun extracts supported dictionary records from the JSON dump, partitions
them by lexical category, and normalizes them into the canonical TSV snapshots
under *crates/gukhanmun-opendict/data/*. The bundled FST and CDB files are
generated from those modified snapshots.

[*Open Korean Dictionary*]: https://opendict.korean.go.kr/
[*Open Korean Dictionary* copyright policy]: https://opendict.korean.go.kr/service/copyrightPolicy


Han character variant data
--------------------------

Generated Han character recognition and output tables use the following
sources:

 -  [Unicode 17.0.0] Unihan
    properties, interpreted according to [Unicode Standard Annex #38]. The
    generator uses `kZVariant`, `kSimplifiedVariant`, `kTraditionalVariant`,
    and `kCompatibilityVariant`. It excludes semantic, specialized-semantic,
    and spoofing relations.
 -  The Japanese Agency for Cultural Affairs [Joyo kanji pronunciation index].
    *data/joyo-variants.tsv* is a UTF-8 snapshot of the new/old form pairs
    displayed there.
 -  Asahi Shimbun's explanations of [Asahi character history]
    and [its revised character policy],
    cross-checked against the fixed [Japanese Wikipedia revision].
    *data/asahi-variants.tsv* intentionally contains only sixteen pairs that
    can be verified from these sources. It does not attempt to reconstruct a
    larger list from images.

The generated tables are deterministic build artifacts.
`mise run generate-unihan-check` verifies that the checked-in output matches
Unicode 17.0.0 and these two snapshots.

[Unicode 17.0.0]: https://www.unicode.org/versions/Unicode17.0.0/
[Unicode Standard Annex #38]: https://www.unicode.org/reports/tr38/
[Joyo kanji pronunciation index]: https://www.bunka.go.jp/kokugo_nihongo/sisaku/joho/joho/kijun/naikaku/kanji/joyokanjisakuin/index.html
[Asahi character history]: https://www.asahi.com/special/kotoba/archive2015/moji/2013062500003.html
[its revised character policy]: https://www.asahi.com/special/kotoba/archive2015/moji/2014032500001.html
[Japanese Wikipedia revision]: https://ja.wikipedia.org/w/index.php?title=朝日文字&oldid=107230023
