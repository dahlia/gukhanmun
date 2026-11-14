---
title: HTML processing
description: Convert HTML fragments in Python, preserve selected subtrees, use ruby markup and choose strict or lenient recovery.
---

HTML processing
===============

Use `InputFormat.HTML` to convert an HTML fragment. Tags and attributes remain
in the output; conversion applies to eligible text nodes.

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[MapDictionarySource([
    DictionaryEntry("漢字", "한자"),
])])
assert converter.convert("<p>漢字</p>", format=InputFormat.HTML) == "<p>한자</p>"
~~~~


Built-in preservation
---------------------

`<pre>`, `<code>`, `<kbd>`, `<script>`, `<style>` and `<textarea>` preserve
their contents and descendants. The inherited `lang` attribute also controls
conversion: Korean language tags (`ko` or `kor`, with or without further
subtags such as `ko-KR`, case-insensitively) permit conversion; other language
tags preserve text. A nested `lang` can override the inherited language.

~~~~ python
assert converter.convert("<code>漢字</code>", format=InputFormat.HTML) == (
    "<code>漢字</code>"
)
~~~~

Use explicit rules for other preservation conventions such as
`translate="no"`.


Custom preservation
-------------------

`HtmlOptions` adds class and attribute rules. If any rule matches an element,
that element and every descendant are preserved.

~~~~ python
from gukhanmun import AttributeMatcher, HtmlOptions

converter = Converter(
    dictionaries=[MapDictionarySource([DictionaryEntry("漢字", "한자")])],
    html=HtmlOptions(
        preserve_classes=["keep"],
        preserve_attributes=[
            AttributeMatcher("data-keep"),
            AttributeMatcher("translate", "no"),
        ],
    ),
)
assert converter.convert(
    '<p class="keep"><b>漢字</b></p>', format=InputFormat.HTML,
) == '<p class="keep"><b>漢字</b></p>'
~~~~

Class names are case-sensitive tokens separated by ASCII whitespace.
`AttributeMatcher(name)` checks presence; `AttributeMatcher(name, value)`
checks the decoded value. Attribute names ignore ASCII case, while values
are case-sensitive. Pass an `AttributeMatcher`; a bare attribute-name string
is not accepted.


Ruby markup
-----------

Combine HTML with `RenderMode.RUBY_ON_HANGUL`, `RUBY_ON_HANJA`, or original
mode with `OriginalGloss.RUBY` to produce ruby annotations. Text-only elements
such as `<title>` and `<option>` use parenthetical fallback instead. See
[*Rendering modes*](./rendering.md#ruby-markup) for complete examples.


Error recovery
--------------

`Recovery.STRICT`, the default, raises `GukhanmunError` when the HTML scanner
encounters malformed input it cannot process. `Recovery.LENIENT` preserves
the malformed region verbatim and converts the parts it can recognize:

~~~~ python
from gukhanmun import Recovery

converter = Converter(recovery=Recovery.LENIENT)
assert converter.convert(
    "<p>學校 <![CDATA[北京</p>", format=InputFormat.HTML,
) == "<p>학교 <![CDATA[北京</p>"
~~~~

Lenient recovery does not repair or sanitize HTML. It retains the unrecognized
region, which can include the rest of an unfinished construct. This option
does not affect plain text or Markdown. See the
[error reference](../../api/python.md#errors) for codes and cause chains.
