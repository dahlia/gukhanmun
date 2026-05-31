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
