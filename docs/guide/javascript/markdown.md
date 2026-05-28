---
title: "Markdown conversion"
description: |-
  Converting Markdown documents with the Gukhanmun JavaScript library.
---

Markdown conversion
===================

Pass `"markdown"` as the format to `convert()`:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({ dictionaries: [await stdictFst()] });

const output = g.convert("# 漢字\n\n漢字를 한글로 변환합니다.", "markdown");
// → "# 한자\n\n한자를 한글로 변환합니다."
~~~~


GitHub Flavored Markdown
------------------------

Pass a format object with `gfm: true` to enable GFM extensions (tables, task
lists, strikethrough):

~~~~ ts twoslash
import type { Gukhanmun } from "@gukhanmun/types";
const g: Gukhanmun = {} as unknown as Gukhanmun;
// ---cut-before---
const output = g.convert(
  "| 漢字 | 讀音 |\n|------|------|\n| 東 | 동 |",
  { format: "markdown", gfm: true },
);
~~~~


What gets converted
-------------------

Gukhanmun converts hanja inside paragraph text, headings, list items,
blockquotes, and table cells.

The following are always left untouched:

 -  Fenced code blocks and indented code blocks
 -  Inline code spans (`` `…` ``)
 -  Raw HTML blocks and inline HTML
 -  Link and image URLs
