---
title: "Markdown processing"
description: |-
  How Gukhanmun converts Markdown documents, including selected YAML front
  matter fields.
---

Markdown processing
===================

Pass `-f text/markdown` (or use a *.md*/*.markdown* extension) to convert a
Markdown document.  Gukhanmun converts hanja in the Markdown body while leaving
the syntax it has no opinion about untouched.  To enable GitHub Flavored
Markdown (tables, footnotes, strikethrough, task lists), add the `variant`
parameter:

~~~~ sh
gukhanmun -f "text/markdown; variant=GFM" input.md
~~~~


YAML front matter
-----------------

A leading YAML front matter block (the `---` delimited block at the very top of
a file) is always recognised and split off before conversion, so it is never
mangled by the Markdown converter.  By default it passes through unchanged while
the Markdown body is converted.

To also convert selected front matter values, use
`--markdown-frontmatter-convert` and address them with a [JSONPath] expression.
The flag can be repeated:

~~~~ sh
gukhanmun -f text/markdown \
  --markdown-frontmatter-convert '$.hero.tagline' \
  --markdown-frontmatter-convert '$.hero.actions[*].text' \
  page.md
~~~~

Each value matched by a selector is converted from mixed script to hangul.
Only string scalars are converted; numbers, booleans, and other non-string
matches are left untouched, as are any fields no selector points at.

:::note
Without any selector the front matter is preserved byte-for-byte.  As soon as
a selector is given, the block is parsed and re-serialised, so the whole block
is reformatted on output (comments are dropped and quoting style may change),
even though only matched values change.  A leading `---` without a closing
fence stays ordinary Markdown.  This flag is only valid with
`--format text/markdown`.
:::

[JSONPath]: https://www.rfc-editor.org/rfc/rfc9535
