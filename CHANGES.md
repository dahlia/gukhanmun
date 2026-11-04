---
title: Changelog
---

Gukhanmun changelog
===================

<!--
Format guide for contributors:

 -  Version heading: “Version x.y.z” as a level-2 heading (—- underline).

 -  Crate sections: “gukhanmun-*” as a level-3 heading (### prefix).

 -  Package sections: “@gukhanmun/*” as a level-3 heading (### prefix).

 -  Add each change as a bullet point inside the relevant crate or package
    section.  Changes that span the whole workspace go directly under the
    version heading without a subsection.

 -  End each bullet with a bracketed issue or pull request number linked to
    its URL, e.g.:

    ~~~~ md
     -  Fixed a crash when input is empty.  [[#42], [#64]]

    [#42]: https://github.com/dahlia/gukhanmun/issues/42
    [#64]: https://github.com/dahlia/gukhanmun/pull/64
    ~~~~
-->


Version 0.2.1
-------------

To be released.

### gukhanmun-core

 -  Marked annotation-derived text with the new `RenderedToken::Annotated`
    variant so that format writers serialize a dictionary reading as literal
    text instead of treating it as already-valid markup.  The variant is a
    breaking change for code that matches `RenderedToken` exhaustively; a
    `_ =>` arm still compiles but silently drops the text.  Upgrade
    *gukhanmun-core*, *gukhanmun-html*, and *gukhanmun-markdown* together.
    [[GHSA-j87h-8h88-wm6g]]

[GHSA-j87h-8h88-wm6g]: https://github.com/dahlia/gukhanmun/security/advisories/GHSA-j87h-8h88-wm6g

### gukhanmun-html

 -  Escaped dictionary readings and other annotation text when writing HTML,
    including quotes and a guarded first character, so a reading can no longer
    become an element or an attribute.  [[GHSA-j87h-8h88-wm6g]]
 -  Stopped treating `<script/>`, `<style/>`, and `<textarea/>` as self-closing
    so their contents stay raw text, matching browsers.  Text after a
    self-closed raw-text tag inside SVG or MathML is now left unconverted
    rather than risking conversion inside a script or style body.
    [[GHSA-j87h-8h88-wm6g]]

### gukhanmun-markdown

 -  Escaped source text when writing Markdown, so backslash escapes and
    character references consumed by the parser are restored instead of being
    re-emitted raw.  Interior `<`, `&`, `[`, `]`, `` ` ``, `*`, `_`, `~`, `|`,
    and `\` can no longer reform HTML, links, images, emphasis, code spans, or
    tables, and line-start characters can no longer start a heading, list,
    block quote, or definition list after conversion.  Code blocks, autolinks,
    and HTML blocks remain untouched.  Literal `&` and `<` in prose now appear
    as `&amp;` and `&lt;` in Markdown output; the rendered text is unchanged.
    [[#35], [#36]]
 -  Encoded dictionary readings as literal Markdown text, with U+200B separators
    that stop GFM from forming an autolink across a reading, so annotation text
    can no longer form links, emphasis, code spans, tables, or raw HTML.
    [[GHSA-j87h-8h88-wm6g]]

[#35]: https://github.com/dahlia/gukhanmun/issues/35
[#36]: https://github.com/dahlia/gukhanmun/pull/36

### gukhanmun

 -  Markdown conversion escapes source text through the re-exported adapter, so
    the Rust API no longer drops the escapes and character references in the
    input.  [[#35], [#36]]
 -  HTML and Markdown conversions now escape dictionary readings end to end
    through the re-exported adapters.  [[GHSA-j87h-8h88-wm6g]]

### gukhanmun-cli

 -  `--format text/markdown` now preserves escapes and character references in
    source text both before and after the Hongdown formatting pass.
    [[#35], [#36]]
 -  `--format text/html` and `--format text/markdown` now escape dictionary
    readings.  [[GHSA-j87h-8h88-wm6g]]

### gukhanmun-napi

 -  Markdown output escapes source text.  [[#35], [#36]]
 -  HTML and Markdown output escapes dictionary readings.
    [[GHSA-j87h-8h88-wm6g]]

### gukhanmun-wasm

 -  Markdown output escapes source text.  [[#35], [#36]]
 -  HTML and Markdown output escapes dictionary readings.
    [[GHSA-j87h-8h88-wm6g]]

### @gukhanmun/napi

 -  Markdown conversion escapes source text.  [[#35], [#36]]
 -  `convert(..., "html")` and `stream("html")` escape dictionary readings.
    [[GHSA-j87h-8h88-wm6g]]

### @gukhanmun/wasm

 -  Markdown conversion escapes source text.  [[#35], [#36]]
 -  `convert(..., "html")` and `stream("html")` escape dictionary readings.
    [[GHSA-j87h-8h88-wm6g]]


Version 0.2.0
-------------

Released on June 15, 2026.

### gukhanmun

 -  Collapse redundant parenthetical reading annotations by default.  The new
    `Builder::collapse_redundant_parens` opt-out disables it.  [[#3], [#4]]
 -  Added the `opendict` feature and made the `ko-kp` preset include the
    bundled *Open Korean Dictionary* (우리말샘) North Korean (北韓語) category
    by default.  Added `Builder::no_bundled_dictionaries()` to disable every
    preset-selected bundled dictionary, plus `Builder::no_bundled_stdict()` and
    `Builder::no_bundled_opendict()` for disabling only one bundled dictionary
    family.  [[#5], [#6]]

[#3]: https://github.com/dahlia/gukhanmun/issues/3
[#4]: https://github.com/dahlia/gukhanmun/pull/4
[#5]: https://github.com/dahlia/gukhanmun/issues/5
[#6]: https://github.com/dahlia/gukhanmun/pull/6

### gukhanmun-core

 -  Fixed Arabic hanja numeral strategies so dictionary calendar entries no
    longer split numeric normalization.  `NumeralStrategy::PositionalArabic`
    now renders dates such as `二〇二六年 六月 二〇日` as
    `2026년 6월 20일`, while `NumeralStrategy::HangulPhonetic`, the default
    library preset strategy, still keeps lexicalized dictionary readings such
    as `六月` as `유월`.  `NumeralStrategy::Smart` also leaves standalone large
    place markers such as `京` and `萬`, plus ambiguous small-marker words such
    as `百濟` and `十長生`, as fallback readings instead of splitting them into
    numeric text.

 -  Fixed a bug where proper names and unknown multi-character hanja words
    were split into individual character annotations when the bundled
    dictionary contained single-character entries for some (but not all) of
    the characters.  The segmenter now emits a `TrivialDictionary` segment
    variant for single-character dictionary matches that carry no special
    rendering marks, and the engine merges consecutive `TrivialDictionary`
    and `Fallback` segments into a single annotation without losing
    `from_dictionary` provenance so homophone marking still works.  [[#7], [#8]]

 -  Added `RedundantParenCollapser`, a streaming middleware that collapses an
    explicit parenthetical reading annotation into the hanja word it duplicates.
    `庫間(곳간)` and `곳간(庫間)` now render with both scripts in every mode
    instead of duplicating the reading, and a parenthetical that pins an
    alternative reading (such as “數字(수자)”) overrides the dictionary reading
    for that occurrence.  A definition gloss such as
    “庫間(物件을 간직하여 두는 곳)” is left untouched.  Regenerated the bundled
    Unihan reading data to also carry every kHangul reading per character
    (`KHANGUL_ALL_READINGS`), which the collapser uses to validate alternative
    readings.  [[#3], [#4]]

 -  Marked `Annotation` `#[non_exhaustive]` so its policy flags can grow without
    a breaking change (it gained a `from_source_gloss` flag here).  Construct it
    from `Annotation::default()` and set the fields you need.  [[#3], [#4]]

[#7]: https://github.com/dahlia/gukhanmun/issues/7
[#8]: https://github.com/dahlia/gukhanmun/pull/8

### gukhanmun-dict-extract

 -  Added a shared extraction helper crate for dictionary dump key
    normalization, original-language parsing, and mixed-script key generation.
    `gukhanmun-stdict` and `gukhanmun-opendict` now use the same core
    extraction rules.  [[#5], [#6]]

### gukhanmun-cli

 -  Collapse redundant parenthetical reading annotations by default across the
    plain-text, HTML, and Markdown pipelines.  The new `--no-collapse-parens`
    flag disables it.  [[#3], [#4]]
 -  Changed the CLI default for `--numerals` to `smart`, so omitted numeral
    options render dates such as `二〇二六年 六月 二〇日` as
    `2026년 6월 20일`.  Pass `--numerals hangul-phonetic` to keep Seonbi-style
    phonetic calendar readings such as `六月` as `유월`.
 -  The `ko-kp` preset now includes the bundled *Open Korean Dictionary* North
    Korean (北韓語) category by default.  Added `--no-bundled-dictionaries`,
    which disables every preset-selected bundled dictionary.  [[#5], [#6]]

### gukhanmun-opendict

 -  Added a bundled *Open Korean Dictionary* (우리말샘) crate generated from the
    2026-06-03 JSON dump.  The crate exposes separate `general()`,
    `north_korean()`, `dialect()`, and `archaic()` FST dictionaries so callers
    can compose the categories explicitly with `ChainDictionary`.  [[#5], [#6]]

### gukhanmun-stdict

 -  Reused the shared dictionary extraction helper and buffered direct JSON
    shard reads for large dump extraction.  [[#5], [#6]]
 -  Fixed “數字” converting to “수자” instead of the orthographically
    prescribed “숫자.”  The six *Standard Korean Orthography §30* (한글 맞춤法
    第30項) saisiot (사이시옷) compounds (곳간, 셋방, 숫자, 찻간, 툇간, 횟수)
    now win over their saisiot-free homographs regardless of dump order.
    [[#1], [#2]]
 -  Regenerated the bundled dictionary so single-hanja foreign-spelling head
    words (such as “元” → “위안” or “円” → “엔”) no longer shadow the
    Sino-Korean reading of those characters; the engine recovers their original
    sound from the bundled unihan readings instead.
 -  Regenerated the bundled *Standard Korean Language Dictionary* data from
    the 2026-06-06 JSON dump (260,690 entries, was 260,688).

[#1]: https://github.com/dahlia/gukhanmun/issues/1
[#2]: https://github.com/dahlia/gukhanmun/pull/2

### @gukhanmun/napi

 -  Collapse redundant parenthetical reading annotations by default; added the
    `collapseRedundantParens` option to disable it.  [[#3], [#4]]
 -  Documented that JavaScript presets still do not auto-load bundled
    dictionary data; use the new opendict packages explicitly when desired.
    [[#5], [#6]]

### @gukhanmun/wasm

 -  Collapse redundant parenthetical reading annotations by default; added the
    `collapseRedundantParens` option to disable it.  [[#3], [#4]]
 -  Documented that JavaScript presets still do not auto-load bundled
    dictionary data; use the new opendict packages explicitly when desired.
    [[#5], [#6]]

### @gukhanmun/opendict-cdb

 -  Added a package containing *Open Korean Dictionary* general (一般語), North
    Korean (北韓語), dialect (方言), and archaic (옛말) categories as CDB
    binaries, with category-specific byte loaders and `FileDictionarySource`
    helpers.  The binaries ship gzip-compressed (as _\*.cdb.gz_) to stay within
    the JSR per-file size limit, and the byte loaders inflate them
    transparently.  [[#5], [#6]]

### @gukhanmun/opendict-fst

 -  Added a package containing *Open Korean Dictionary* general (一般語), North
    Korean (北韓語), dialect (方言), and archaic (옛말) categories as FST
    binaries, with category-specific byte loaders and `FileDictionarySource`
    helpers.  [[#5], [#6]]

### @gukhanmun/stdict-fst

 -  Regenerated the bundled FST binary from the 2026-06-06 *Standard Korean
    Language Dictionary* JSON dump.

### @gukhanmun/stdict-cdb

 -  Regenerated the bundled CDB binary from the 2026-06-06 *Standard Korean
    Language Dictionary* JSON dump.

### @gukhanmun/types

 -  Updated the JavaScript dictionary option documentation to mention the
    opendict packages and clarify that JavaScript presets do not auto-load
    bundled dictionary data.  [[#5], [#6]]


Version 0.1.3
-------------

Released on November 5, 2026.

### gukhanmun-core

 -  Marked annotation-derived text with the new `RenderedToken::Annotated`
    variant so that format writers serialize a dictionary reading as literal
    text instead of treating it as already-valid markup.  The variant is a
    breaking change for code that matches `RenderedToken` exhaustively; a
    `_ =>` arm still compiles but silently drops the text.  Upgrade
    *gukhanmun-core*, *gukhanmun-html*, and *gukhanmun-markdown* together.
    [[GHSA-j87h-8h88-wm6g]]

### gukhanmun-html

 -  Escaped dictionary readings and other annotation text when writing HTML,
    including quotes and a guarded first character, so a reading can no longer
    become an element or an attribute.  [[GHSA-j87h-8h88-wm6g]]
 -  Stopped treating `<script/>`, `<style/>`, and `<textarea/>` as self-closing
    so their contents stay raw text, matching browsers.  Text after a
    self-closed raw-text tag inside SVG or MathML is now left unconverted
    rather than risking conversion inside a script or style body.
    [[GHSA-j87h-8h88-wm6g]]

### gukhanmun-markdown

 -  Escaped source text when writing Markdown, so backslash escapes and
    character references consumed by the parser are restored instead of being
    re-emitted raw.  Interior `<`, `&`, `[`, `]`, `` ` ``, `*`, `_`, `~`, `|`,
    and `\` can no longer reform HTML, links, images, emphasis, code spans, or
    tables, and line-start characters can no longer start a heading, list,
    block quote, or definition list after conversion.  Code blocks, autolinks,
    and HTML blocks remain untouched.  Literal `&` and `<` in prose now appear
    as `&amp;` and `&lt;` in Markdown output; the rendered text is unchanged.
    [[#35], [#36]]
 -  Encoded dictionary readings as literal Markdown text, with U+200B separators
    that stop GFM from forming an autolink across a reading, so annotation text
    can no longer form links, emphasis, code spans, tables, or raw HTML.
    [[GHSA-j87h-8h88-wm6g]]

### gukhanmun

 -  Markdown conversion escapes source text through the re-exported adapter, so
    the Rust API no longer drops the escapes and character references in the
    input.  [[#35], [#36]]
 -  HTML and Markdown conversions now escape dictionary readings end to end
    through the re-exported adapters.  [[GHSA-j87h-8h88-wm6g]]

### gukhanmun-cli

 -  `--format text/markdown` now preserves escapes and character references in
    source text both before and after the Hongdown formatting pass.
    [[#35], [#36]]
 -  `--format text/html` and `--format text/markdown` now escape dictionary
    readings.  [[GHSA-j87h-8h88-wm6g]]

### gukhanmun-napi

 -  Markdown output escapes source text.  [[#35], [#36]]
 -  HTML and Markdown output escapes dictionary readings.
    [[GHSA-j87h-8h88-wm6g]]

### gukhanmun-wasm

 -  Markdown output escapes source text.  [[#35], [#36]]
 -  HTML and Markdown output escapes dictionary readings.
    [[GHSA-j87h-8h88-wm6g]]

### @gukhanmun/napi

 -  Markdown conversion escapes source text.  [[#35], [#36]]
 -  `convert(..., "html")` and `stream("html")` escape dictionary readings.
    [[GHSA-j87h-8h88-wm6g]]

### @gukhanmun/wasm

 -  Markdown conversion escapes source text.  [[#35], [#36]]
 -  `convert(..., "html")` and `stream("html")` escape dictionary readings.
    [[GHSA-j87h-8h88-wm6g]]


Version 0.1.2
-------------

Released on June 1, 2026.

 -  Static-link the Windows release CLI executables against the MSVC C runtime
    so the release archives no longer require Visual C++ runtime DLLs.
 -  Include the license and root _README.\*.md_ files in CLI release archives.
 -  Store CLI release archive contents at the archive root instead of wrapping
    them in a top-level directory.


Version 0.1.1
-------------

Released on June 1, 2026.

### gukhanmun

 -  Fixed ruby rendering inside Markdown shortcut reference links so the
    high-level Markdown conversion API no longer emits an empty `[]:`
    reference definition.

### gukhanmun-cli

 -  Fixed ruby rendering inside Markdown shortcut reference links so CLI
    Markdown conversion no longer emits an empty `[]:` reference definition.

### gukhanmun-markdown

 -  Fixed ruby rendering inside Markdown shortcut reference links so the
    Markdown adapter no longer emits an empty `[]:` reference definition.

### @gukhanmun/napi

 -  Fixed ruby rendering inside Markdown shortcut reference links so Node-API
    Markdown conversion no longer emits an empty `[]:` reference definition.

### @gukhanmun/wasm

 -  Fixed ruby rendering inside Markdown shortcut reference links so WebAssembly
    Markdown conversion no longer emits an empty `[]:` reference definition.


Version 0.1.0
-------------

Initial release.  Released on May 30, 2026.
