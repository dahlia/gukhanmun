---
title: "HTML processing"
description: |-
  Converting HTML and preserving specific elements in the Gukhanmun JavaScript library.
---

HTML processing
===============

Pass `"html"` as the format to `convert()`:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({ dictionaries: [await stdictFst()] });

console.log(g.convert("<p>漢字</p>", "html"));
// → <p>한자</p>
~~~~

Gukhanmun parses the input as an HTML fragment, converts hanja in text nodes,
and serialises the result while preserving all tags and attributes.


Elements that are always preserved
----------------------------------

These elements and their descendants are never modified:

 -  `<code>`, `<kbd>`, `<pre>`, `<samp>`
 -  `<script>`, `<style>`, `<textarea>`
 -  Elements with `translate="no"`
 -  `<ruby>` annotation content


Preserving elements by CSS class
--------------------------------

Use `html.preserveClasses` to skip conversion inside elements that carry
specific classes:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  dictionaries: [await stdictFst()],
  html: {
    preserveClasses: ["math", "no-translate"],
  },
});

const html = `
  <p>漢字</p>
  <span class="math">漢字</span>
`;
console.log(g.convert(html, "html"));
// → <p>한자</p>
//   <span class="math">漢字</span>   ← preserved
~~~~


Preserving elements by attribute
--------------------------------

Use `html.preserveAttributes` to skip conversion based on an attribute name
or an attribute name/value pair:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  dictionaries: [await stdictFst()],
  html: {
    preserveAttributes: [
      "data-no-hanja",            // any element with this attribute
      { name: "lang", value: "en" }, // elements with lang="en"
    ],
  },
});
~~~~


Ruby markup in HTML
-------------------

Combine `"html"` format with a ruby rendering mode:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  rendering: "ruby-on-hangul",
  dictionaries: [await stdictFst()],
});

console.log(g.convert("<p>漢字</p>", "html"));
// → <p><ruby>한자<rt>漢字</rt></ruby></p>
~~~~
