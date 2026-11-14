---
title: Markdown conversion
description: Convert CommonMark and GitHub Flavored Markdown in Python while preserving code, destinations and raw HTML blocks.
---

Markdown conversion
===================

`InputFormat.MARKDOWN` reads CommonMark and writes Markdown, rather than HTML:

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[MapDictionarySource([
    DictionaryEntry("漢字", "한자"),
])])
output = converter.convert("# 漢字\n\n漢字를 읽습니다.", format=InputFormat.MARKDOWN)
~~~~


GitHub Flavored Markdown
------------------------

Use `MarkdownOptions(gfm=True)` for GFM extensions such as tables, task lists
and strikethrough. `MarkdownOptions()` is equivalent to
`InputFormat.MARKDOWN`.

~~~~ python
from gukhanmun import MarkdownOptions

assert converter.convert("~~漢字~~", format=MarkdownOptions(gfm=True)) == "~~한자~~"
~~~~


Converted and preserved regions
-------------------------------

Paragraphs, headings, list items, blockquotes and link/image labels are
converted. GFM table cells are converted when GFM is enabled. Code blocks,
inline code spans, link/image destinations and raw HTML blocks are preserved.

Inline HTML tags remain markup. Text between inline tags is eligible for
conversion. An enclosing preserved tag such as `<code>`, or an inherited
non-Korean `lang` attribute, prevents conversion. Raw HTML blocks are preserved
whole. To process a complete HTML fragment,
use [HTML mode](./html.md) instead.


Output and rendering
--------------------

The Markdown adapter parses and serializes the document. It preserves the
document's structure, but it may normalize formatting details such as
whitespace or list markers; it does not promise a byte-for-byte round trip.
Text that would otherwise create Markdown syntax is escaped as needed.

[Ruby rendering](./rendering.md#ruby-markup) emits inline HTML where the
scope permits it. The application displaying the result must allow that HTML
to show the ruby annotations. Choose a parenthetical rendering mode when the
destination strips inline HTML.
