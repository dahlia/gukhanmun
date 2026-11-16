gukhanmun-html
==============

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

HTML fragment adapter for the Gukhanmun pipeline. A fragment-oriented scanner,
not a fully HTML5-conformant parser, classifies elements by preservation,
inline-markup and block-boundary policy. It produces a token stream for the
core engine. The writer serializes the engine's output back into HTML,
keeping all original attributes and tag structure intact.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-html?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-html
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-html
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-html = "0.4"
~~~~


Key types
---------

`HtmlScopeData` is the adapter-owned scope value attached to each open HTML
element. It records the tag name, raw attribute text, the original start tag
for serialization, and three policy flags:

 -  `preserve`: text inside this element passes through unchanged (applies to
    `<code>`, `<kbd>`, `<pre>`, `<script>`, `<style>`, `<textarea>`, and
    elements whose effective inherited language is not Korean). Custom
    preservation predicates can add class or attribute rules.
 -  `allows_inline_markup`: whether the renderer may emit `<ruby>` or
    parenthetical spans inside this element.
 -  `block_boundary`: whether this element resets block-oriented middleware
    state such as the homophone-disambiguation window.

`lang` attributes are tracked across nesting levels and inherited by child
elements that do not specify their own.


Usage
-----

The adapter is typically invoked through the `gukhanmun` umbrella crate's
`Converter::convert_html_fragment_to_string()` method. Direct use:

~~~~ rust
use gukhanmun_core::{MapDictionary, RenderMode};
use gukhanmun_html::{HtmlFragmentReader, HtmlFragmentWriter};

let mut dict = MapDictionary::new();
dict.insert("漢字", "한자");

// Low-level: construct a reader and writer explicitly via the pipeline.
// For most uses the umbrella crate is more convenient.
~~~~


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.
