Rendering and tools
===================

Inspect the destination
-----------------------

Read project instructions and rendering configuration before asking the user
for facts the project already supplies. Establish horizontal/vertical layout,
HTML permissions, ruby support, and any custom-reading facility. If the setup
remains unknown, ask before committing to a representation. There is no
universal Markdown ruby syntax.

Use HTML ruby in Markdown when HTML is allowed, or the destination's actual
ruby extension when applicable. If a configured converter can express the
required reading, use that facility. Use parentheses when ruby is unsupported.
Check titles, link labels, metadata, feeds, and Hangul-only output separately
when those channels are part of the task; do not ban ruby in all headings or
assume body support guarantees support everywhere.

Keep the selected output character sequence as the dictionary key. A mapping
for 産 does not prove that 產 has the same override. Some converters skip
foreign-language HTML spans or existing ruby; check the resulting output
instead of assuming that a `lang` attribute and a dictionary will compose.

Represent readings as correctly spelled Hangul, not phonetic approximations:
茶물 needs 찻물 in Hangul-only output. 茶禮 may read 차례 in one occurrence and
다례 in another. If the system only offers document-wide string keys, find a
supported occurrence-level mechanism or ask how to handle the collision. Do
not silently choose one reading for the entire document.

The author's blog has used Markdown front matter `reads` entries passed to
Seonbi. That is a project convention, not syntax that every Markdown renderer
supports. Inspect the current loader/configuration before adding such entries.

Primary references to consult for the chosen system:

 -  [Seonbi]
 -  [Gukhanmun documentation index]
 -  [Gukhanmun custom dictionaries]

[Seonbi]: https://github.com/dahlia/seonbi
[Gukhanmun documentation index]: https://gukhanmun.org/llms.txt
[Gukhanmun custom dictionaries]: https://gukhanmun.org/guide/cli/dictionary.md


Hangulize
---------

Use [Hangulize] to generate candidate
Korean loanword spellings where its language model supports the input. With
local mise, the author's invocation is:

~~~~ sh
mise x go:github.com/hangulize/hangulize/cmd/hangulize -- hangulize jpn 浅草寺
~~~~

Hangulize uses its supported three-letter language codes, for example `jpn`
and `vie`, rather than two-letter codes. Check the tool's supported model
list; do not assume that every ISO code or spelling system is implemented.

If the command fails specifically because the mise Go toolchain is absent,
the author authorizes installing it with:

~~~~ sh
mise use -g go
~~~~

This changes the global mise Go selection. Use it only for the missing-Go
prerequisite, not as a generic response to network, model, or input errors.
Do not replace an already working Go setup unnecessarily.

Hangulize's output is a candidate, not proof of a proper name's pronunciation
or an official Korean spelling. Verify name readings and established Korean
forms. The original spelling used as tool input need not be the form ultimately
selected for Korean mixed-script prose.

[Hangulize]: https://github.com/hangulize/hangulize


Vertical punctuation
--------------------

Ask the user about vertical punctuation before selecting a policy. Distinguish
the marks to show from the characters stored in the source:

| Stored characters     | Meaning                                   |
| --------------------- | ----------------------------------------- |
| 、 U+3001 / 。 U+3002 | Ideographic comma and full stop           |
| ︑ U+FE11 / ︒ U+FE12 | Vertical compatibility presentation forms |

Unicode lists FE11/FE12 as compatibility characters related to 3001/3002.
CSS vertical typesetting can use the font's vertical alternate glyphs, so
vertical layout does not itself require FE11/FE12 in the source. Explain
these options in terms of the actual renderer and ask; neither is an approved
automatic default. Preserve the user's single … convention in the source
unless another representation is explicitly chosen for the destination.

 -  [Unicode Vertical Forms]
 -  [CSS vertical typesetting and font features]

[Unicode Vertical Forms]: https://www.unicode.org/Public/UCD/latest/charts/nameslist/fe10/
[CSS vertical typesetting and font features]: https://www.w3.org/TR/css-writing-modes-4/#vertical-font-features


Unicode and form selection
--------------------------

Do not use NFC, NFKC, or an unqualified simplified/traditional converter as
the implementation of the author's form preferences. Some compatibility Han
characters fold even under NFC; other distinctions, such as 高/髙, do not.
Preserve self-authorized name code points and variation selectors. Keep
readings in annotation/mapping data rather than inferring them solely from a
compatibility code point.

Some visible regional differences are font glyphs of the same character;
others use distinct characters. Check code points and source evidence. Missing
font coverage for a rare character or chữ Nôm is a rendering problem, not a
reason to substitute a visually similar character.

[Unicode normalization] describes the
normalization mechanisms; it does not choose this skill's lexical spellings.

[Unicode normalization]: https://www.unicode.org/reports/tr15/
