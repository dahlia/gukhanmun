@gukhanmun/types
================

[![JSR][JSR badge]][JSR]
[![npm][npm badge]][npm]
[![License: GPL-3.0-only][GPL badge]][GPL]

TypeScript type declarations for the Gukhanmun JavaScript API. This package
carries no runtime code; it exists so that both `@gukhanmun/wasm` and
`@gukhanmun/napi` can share the same type definitions without duplicating them.

[JSR badge]: https://jsr.io/badges/@gukhanmun/types
[JSR]: https://jsr.io/@gukhanmun/types
[npm badge]: https://img.shields.io/npm/v/@gukhanmun/types?logo=npm
[npm]: https://www.npmjs.com/package/@gukhanmun/types
[GPL badge]: https://img.shields.io/npm/l/%40gukhanmun%2Ftypes
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ bash
npm  add     @gukhanmun/types
pnpm add     @gukhanmun/types
yarn add     @gukhanmun/types
bun  add     @gukhanmun/types
deno add jsr:@gukhanmun/types
~~~~

This package is listed as a peer dependency of `@gukhanmun/wasm` and
`@gukhanmun/napi`, so it is usually installed transitively.


What is declared here
---------------------

`Preset` (`"ko-kr"` | `"ko-kp"`) and `RenderMode` control orthographic
conventions and output format. `GukhanmunOptions` is the options bag passed to
`load()`; it covers preset, rendering, segmentation, numerals, initial sound
law, homophone window, first-occurrence window, recovery, directives, HTML
options, source policy, and punctuation profiles. `Gukhanmun` is the converter
interface returned by `load()`, with `convert()` and `stream()` methods.
`GukhanmunError` is the structured error class with a typed `code` property and
a `chain` array of causes.

`DictionarySource` describes how to supply a dictionary, either as a
`FileDictionarySource` (`{ format, data }` pointing at FST or CDB bytes, a URL,
or a path) or as a `MapDictionarySource` (`{ entries }` with hanja, reading,
positional reading, preference, and mark fields) for readings computed at
runtime. A source's `role` distinguishes user overrides from cooperating
reference and loanword dictionaries.

`MediaType` lists the four format strings for `convert()` and `stream()`:
`"text/plain"` (the default), `"text/html"`, `"text/markdown"` (CommonMark),
and `"text/markdown; variant=GFM"` (GitHub Flavored Markdown). `Format` also
includes the legacy strings and Markdown objects for compatibility until
Gukhanmun 1.0.

All TSDoc comments live in this package as the single source of truth for the
JavaScript API. Refer to the declarations in `index.ts` for the full details.


License
-------

GPL-3.0-only.
