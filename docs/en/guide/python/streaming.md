---
title: Streaming API
description: Use Python buffered conversion streams and iterables, handle stream finalization, and share a converter across document-processing threads.
---

Streaming API
=============

`stream()` and `convert_iter()` accept a document in string chunks. The current
Python implementation buffers the complete input and converts it at
finalization. It provides batch-equivalent results, not bounded-memory or
early-output streaming.

For any division of a document into chunks, the combined stream output equals
one `convert()` call on the combined input. A boundary inside a word, HTML tag
or Markdown construct does not change conversion. This guarantee applies to
both `stream()` and `convert_iter()`.


Push and finish
---------------

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[MapDictionarySource([
    DictionaryEntry("漢字", "한자"),
])])
stream = converter.stream(format=InputFormat.HTML)
assert stream.push("<p>漢") == ""
assert stream.push("字</p>") == ""
assert stream.finish() == "<p>한자</p>"
~~~~

Every successful `push()` returns an empty string. `finish()` joins the input
and converts it once, even when the input is empty. The format accepts the
same `InputFormat` or `MarkdownOptions` value as `convert()`.

A stream keeps a reference to its converter, so the converter stays alive
while the stream does. Give each document its own stream and each stream one
consumer. Successful `finish()`, a failed `push()`, or a failed `finish()`
makes the stream terminal; every later push or finish raises
`RuntimeError`. To retry after an error, create a new stream.


Convert an iterable
-------------------

`convert_iter()` adapts an iterable of string chunks to an iterator of
nonempty output chunks:

~~~~ python
chunks = ["<p>漢", "字</p>"]
output = "".join(converter.convert_iter(chunks, format=InputFormat.HTML))
assert output == "<p>한자</p>"
~~~~

Format validation happens when `convert_iter()` is called. Input is consumed
when the returned iterator advances. The current implementation yields one
result after normal input exhaustion, or none for empty output. If the input
iterator raises or the output iterator is closed early, no partial document
is converted.


Parallel documents
------------------

Reuse one configured converter for independent documents. Native conversion
releases the GIL and keeps parser, homophone and first-occurrence state local
to each call:

~~~~ python
from concurrent.futures import ThreadPoolExecutor

documents = ["漢字", "漢字를 읽습니다."]
with ThreadPoolExecutor(max_workers=2) as pool:
    results = list(pool.map(converter.convert, documents))
assert results == ["한자", "한자를 읽습니다."]
~~~~

This does not make a mutable stream safe for several consumers. Share the
converter and create separate streams instead. Python signals are checked
after native work returns, so a long conversion can delay
`KeyboardInterrupt`. There is no asyncio-specific interface.
