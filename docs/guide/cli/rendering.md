---
title: "Rendering modes"
description: |-
  How to control how Gukhanmun presents hanja and their hangul readings in the output.
---

Rendering modes
===============

`--rendering` controls how hanja and their hangul readings appear in the
output.  The default is `hangul-only`.


Available modes
---------------

### `hangul-only` (default)

Replaces each hanja word with its hangul reading.  The original hanja is
discarded.

~~~~ sh
echo "漢字" | gukhanmun --rendering hangul-only
# → 한자
~~~~

### `hangul-hanja-parens`

Outputs the hangul reading followed by the original hanja in parentheses.

~~~~ sh
echo "漢字" | gukhanmun --rendering hangul-hanja-parens
# → 한자(漢字)
~~~~

### `hanja-hangul-parens`

Outputs the original hanja followed by the hangul reading in parentheses.

~~~~ sh
echo "漢字" | gukhanmun --rendering hanja-hangul-parens
# → 漢字(한자)
~~~~

### `ruby-on-hangul`

Wraps the hangul reading in a `<ruby>` element with the hanja as the
annotation.  Requires HTML or Markdown output (`-f text/html` or
`-f text/markdown`); in plain text falls back to parentheses.

~~~~ sh
echo "漢字" | gukhanmun -f text/html --rendering ruby-on-hangul
# → <ruby>한자<rt>漢字</rt></ruby>
~~~~

### `ruby-on-hanja`

Wraps the original hanja in a `<ruby>` element with the hangul reading as the
annotation.

~~~~ sh
echo "漢字" | gukhanmun -f text/html --rendering ruby-on-hanja
# → <ruby>漢字<rt>한자</rt></ruby>
~~~~

### `original`

Keeps the original hanja in the output and adds a gloss only where needed to
disambiguate homophones.  Use `--original-gloss` to choose the gloss style:

| `--original-gloss` | Output                           |
| ------------------ | -------------------------------- |
| `parens`           | 漢字(한자)                       |
| `ruby`             | `<ruby>漢字<rt>한자</rt></ruby>` |

~~~~ sh
echo "漢字" | gukhanmun --rendering original --original-gloss parens
# → 漢字(한자)
~~~~
