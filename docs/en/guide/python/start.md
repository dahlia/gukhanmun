---
title: Python quick start
description: Convert text, HTML and Markdown in Python with a reusable Gukhanmun converter, explicit dictionaries and typed options.
---

Python quick start
==================

[Install Gukhanmun](./install.mdx) before running these examples.
`Converter(...)` creates a reusable converter backed by Rust. Configure it
once, then pass each document to `convert()`.


Convert a document
------------------

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[
    MapDictionarySource(entries=[DictionaryEntry(hanja="漢字", reading="한자")]),
])

assert converter.convert("漢字를 한글로") == "한자를 한글로"
assert converter.convert("<p>漢字</p>", format=InputFormat.HTML) == "<p>한자</p>"
~~~~

Plain text is the default format. Use `InputFormat.MARKDOWN` for CommonMark,
or `MarkdownOptions(gfm=True)` for GitHub Flavored Markdown:

~~~~ python
from gukhanmun import MarkdownOptions

result = converter.convert("# 漢字", format=MarkdownOptions(gfm=True))
~~~~

Options take enum members, not strings. If a value arrives as a string,
convert it first, for example `Preset("ko-kr")`. Passing `preset="ko-kr"`
directly to `Converter()` raises `TypeError`.

Presets do not load official dictionaries. With no explicit
sources, conversion uses the built-in character fallback. Supply runtime
entries or compiled FST/CDB data when word readings are needed, or install and
add the [official dictionaries](./dictionaries.md).


Reuse a converter
-----------------

Dictionary resources are loaded during construction, so configure one
converter and reuse it across documents. Its settings are immutable: inspect
the resolved configuration through `converter.options`, and create a new
converter to change it. A converter can be shared across threads, as shown in
[*Parallel documents*](./streaming.md#parallel-documents).


Choose the next task
--------------------

| Task                                             | Guide                                 |
| ------------------------------------------------ | ------------------------------------- |
| Add official, in-memory or compiled dictionaries | [Dictionaries](./dictionaries.md)     |
| Set presets, numbers and annotation windows      | [Conversion options](./conversion.md) |
| Compare parentheses, ruby and original text      | [Rendering modes](./rendering.md)     |
| Force or suppress annotations for a term         | [Directives](./directives.md)         |
| Preserve HTML subtrees or handle malformed input | [HTML processing](./html.md)          |
| Convert Markdown and GFM                         | [Markdown conversion](./markdown.md)  |
| Feed chunks or convert documents in parallel     | [Streaming API](./streaming.md)       |


Handle errors
-------------

The [API reference](../../api/python.md#errors) explains `GukhanmunError`, its
`ErrorCode`, and the root-first `chain` of causes. Wrong argument types raise
`TypeError`; lone Unicode surrogates in input raise `UnicodeEncodeError`.
