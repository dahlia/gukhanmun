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


Version 0.1.3
-------------

To be released.

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

[#35]: https://github.com/dahlia/gukhanmun/issues/35
[#36]: https://github.com/dahlia/gukhanmun/pull/36

### gukhanmun

 -  Markdown conversion escapes source text through the re-exported adapter, so
    the Rust API no longer drops the escapes and character references in the
    input.  [[#35], [#36]]

### gukhanmun-cli

 -  `--format text/markdown` now preserves escapes and character references in
    source text both before and after the Hongdown formatting pass.
    [[#35], [#36]]

### gukhanmun-napi

 -  Markdown output escapes source text.  [[#35], [#36]]

### gukhanmun-wasm

 -  Markdown output escapes source text.  [[#35], [#36]]

### @gukhanmun/napi

 -  Markdown conversion escapes source text.  [[#35], [#36]]

### @gukhanmun/wasm

 -  Markdown conversion escapes source text.  [[#35], [#36]]


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
