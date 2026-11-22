---
title: "Rendering modes"
description: |-
  Controlling how Gukhanmun presents hanja and hangul readings in JavaScript.
---

Rendering modes
===============

Set the `rendering` option in `load` to control how hanja and their hangul
readings appear in the output.


Available modes
---------------

| Value                     | Input | Output                                               |
| ------------------------- | ----- | ---------------------------------------------------- |
| `"hangul-only"` (default) | 漢字  | 한자                                                 |
| `"hangul-hanja-parens"`   | 漢字  | 한자(漢字)                                           |
| `"hanja-hangul-parens"`   | 漢字  | 漢字(한자)                                           |
| `"ruby-on-hangul"`        | 漢字  | `<ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>` |
| `"ruby-on-hanja"`         | 漢字  | `<ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>` |
| `"original"`              | 漢字  | 漢字 (gloss added where needed)                      |

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  rendering: "hangul-hanja-parens",
  dictionaries: [await stdictFst()],
});

console.log(g.convert("漢字"));  // 한자(漢字)
~~~~


Omitting dictionary-required hanja
----------------------------------

Set `ignoreDictionaryRequiredHanja: true` in `load()` to omit hanja glosses
required only by dictionary entries in `"hangul-only"` mode. The option works
in Node-API and Wasm and defaults to `false`. Homophone glosses, explicit
directives, and recognized source glosses are unchanged. The configured
value is available at `converter.options.ignoreDictionaryRequiredHanja`.


Ruby markup
-----------

`"ruby-on-hangul"` and `"ruby-on-hanja"` produce `<ruby>` elements.  Use
them with `"html"` or `"markdown"` format; in plain text they fall back to
parentheses.  The annotation is wrapped in `<rp>` (ruby parenthesis)
elements, so browsers without `<ruby>` support still show the reading as a
parenthesized gloss (`한자(漢字)`) instead of running it into the base text:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  rendering: "ruby-on-hangul",
  dictionaries: [await stdictFst()],
});

console.log(g.convert("<p>漢字</p>", "html"));
// → <p><ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby></p>
~~~~


Annotating only hanja runs
--------------------------

Set `annotationGranularity: "hanja-run"` in `load()` for either Node.js or
Wasm. With `rendering: "ruby-on-hanja"`, `半달形` gets separate readings
on `半` and `形`. With `rendering: "hangul-hanja-parens"`, `汽車길` is
written as `기찻(汽車)길`. `"word"` is the default and is also used for
words whose readings cannot be split uniquely.


Original mode with a gloss
--------------------------

Use `"original"` together with `originalGloss` to add a gloss only where
disambiguation is needed:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  rendering: "original",
  originalGloss: "parens",  // or "ruby"
  dictionaries: [await stdictFst()],
});

console.log(g.convert("東京에서 東洋까지"));
// homophones glossed: 東京(동경)에서 東洋(동양)까지
~~~~


Hanja variant sets
------------------

Variant recognition is always enabled. Set `hanjaVariantSet` only when you want
to change the spelling emitted by modes that keep hanja:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({
  rendering: "original",
  hanjaVariantSet: "shinjitai",
});
~~~~

The values are `"as-dictionary"` (default), `"shinjitai"`, `"kanxi"`,
`"simplified"`, and `"asahimoji"`.
