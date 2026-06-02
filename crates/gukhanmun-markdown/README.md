gukhanmun-markdown
==================

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

Markdown adapter for the Gukhanmun pipeline. Parses Markdown with
`pulldown-cmark`, maps each event to a core token, converts hanja in text runs
while preserving code spans, code blocks, and raw HTML, then serializes the
output back to Markdown via `pulldown-cmark-to-cmark`.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-markdown?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-markdown
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-markdown
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-markdown = "0.1"
~~~~


Key types
---------

`MarkdownScopeData` carries per-scope policy flags through the engine. A scope
represents a Markdown element (paragraph, heading, list item, etc.) or an
inline HTML node discovered inside a Markdown document. The three flags mirror
those in `gukhanmun-html`:

 -  `preserve`: text in this scope is not converted (code spans, code blocks,
    raw HTML blocks).
 -  `allows_inline_markup`: whether the renderer may emit `<ruby>` or
    parenthetical markup.
 -  `block_boundary`: whether this scope resets the homophone-disambiguation
    window.

Inline HTML inside Markdown is re-scanned for `lang` attributes, and the
`gukhanmun-html` classifier is consulted to determine the policy for any
recognized HTML elements.


Usage
-----

The adapter is typically invoked through the `gukhanmun` umbrella crate's
`Converter::convert_markdown_to_string` method. Direct use:

~~~~ rust
use gukhanmun_core::{MapDictionary, RenderMode};
use gukhanmun_markdown::{MarkdownVariant, convert_markdown};

let mut dict = MapDictionary::new();
dict.insert("漢字", "한자");

let output = convert_markdown(
    "# 漢字\n",
    &dict,
    RenderMode::HangulOnly,
    MarkdownVariant::CommonMark,
)?;
// output is semantically equivalent to "# 한자\n"
~~~~

`MarkdownVariant::CommonMark` enables no pulldown-cmark extensions.
`MarkdownVariant::Gfm` enables the GitHub Flavored Markdown extension set
(tables, task lists, strikethrough, footnotes).


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.
