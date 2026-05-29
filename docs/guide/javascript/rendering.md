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
