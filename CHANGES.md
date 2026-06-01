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


Version 0.2.0
-------------

To be released.

### gukhanmun

 -  Collapse redundant parenthetical reading annotations by default.  The new
    `Builder::collapse_redundant_parens` opt-out disables it.  [[#3], [#4]]

[#3]: https://github.com/dahlia/gukhanmun/issues/3
[#4]: https://github.com/dahlia/gukhanmun/pull/4

### gukhanmun-core

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

### gukhanmun-cli

 -  Collapse redundant parenthetical reading annotations by default across the
    plain-text, HTML, and Markdown pipelines.  The new `--no-collapse-parens`
    flag disables it.  [[#3], [#4]]

### gukhanmun-stdict

 -  Fixed “數字” converting to “수자” instead of the orthographically
    prescribed “숫자.”  The six *Standard Korean Orthography §30* (한글 맞춤法
    第30項) saisiot (사이시옷) compounds (곳간, 셋방, 숫자, 찻간, 툇간, 횟수)
    now win over their saisiot-free homographs regardless of dump order.
    [[#1], [#2]]
 -  Regenerated the bundled dictionary so single-hanja foreign-spelling head
    words (such as “元” → “위안” or “円” → “엔”) no longer shadow the
    Sino-Korean reading of those characters; the engine recovers their original
    sound from the bundled unihan readings instead.

[#1]: https://github.com/dahlia/gukhanmun/issues/1
[#2]: https://github.com/dahlia/gukhanmun/pull/2

### @gukhanmun/napi

 -  Collapse redundant parenthetical reading annotations by default; added the
    `collapseRedundantParens` option to disable it.  [[#3], [#4]]

### @gukhanmun/wasm

 -  Collapse redundant parenthetical reading annotations by default; added the
    `collapseRedundantParens` option to disable it.  [[#3], [#4]]


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
