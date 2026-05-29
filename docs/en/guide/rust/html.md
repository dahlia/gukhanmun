---
title: "HTML conversion"
description: |-
  Converting HTML documents with the Gukhanmun Rust library.
---

HTML conversion
===============

Requires the `html` feature (enabled by default).


Converting an HTML fragment
---------------------------

~~~~ rust
use gukhanmun::{Builder, Preset};

let converter = Builder::with_preset(Preset::KoKr).build()?;
let output = converter.convert_html_fragment_to_string("<p>漢字</p>")?;
// → "<p>한자</p>"
~~~~

Gukhanmun parses the input as an HTML fragment (not a full document),
converts hanja in text nodes and attributes, and serialises the result while
preserving all tags and attributes exactly.


Elements that are always preserved
----------------------------------

These elements are never modified:

 -  `<code>`, `<kbd>`, `<pre>`, `<samp>`
 -  `<script>`, `<style>`, `<textarea>`
 -  Elements with `translate="no"`
 -  `<ruby>` annotation content (`<rt>`, `<rp>`)


Custom preservation rules
-------------------------

Use `builder.html_preserve_when` to register a predicate that receives the
tag name and attributes of each element and returns `true` to skip conversion:

~~~~ rust
builder.html_preserve_when(|tag, attrs| {
    // preserve elements with class "math" or attribute "data-no-hanja"
    let has_math_class = attrs.get("class")
        .is_some_and(|c| c.split_whitespace().any(|cls| cls == "math"));
    let has_attr = attrs.contains_key("data-no-hanja");
    has_math_class || has_attr
});
~~~~

Multiple calls to `html_preserve_when` add additional predicates; any
predicate returning `true` causes the element (and its descendants) to be
preserved.
