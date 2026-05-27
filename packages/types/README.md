@gukhanmun/types
================

TypeScript type declarations for the Gukhanmun JavaScript API. This package
carries no runtime code; it exists so that both `@gukhanmun/wasm` and
`@gukhanmun/napi` can share the same type definitions without duplicating them.


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
law, homophone window, first-occurrence window, recovery, directives, and HTML
options. `Gukhanmun` is the converter interface returned by `load()`, with
`convert()` and `convertStream()` methods. `GukhanmunError` is the structured
error class with a typed `code` property and a `chain` array of causes.

`DictionarySource` describes how to supply a dictionary: as a
`{ format, bytes }` record pointing to already-loaded binary data. `Format` is
the input format selector: `"text/plain"`, `"text/html"`, `"text/markdown"`, or
a `{ format: "text/markdown", gfm: boolean }` object.

All TSDoc comments live in this package as the single source of truth for the
JavaScript API. Refer to the declarations in `index.ts` for the full details.


License
-------

GPL-3.0-only.
