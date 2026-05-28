---
title: "Directives"
description: |-
  Per-hanja overrides that force or suppress annotations for specific characters.
---

Directives
==========

Directives let you override the dictionary's annotation marks for specific
hanja.  You can require that a hanja always show its reading, always show
the original hanja, or skip annotation entirely.


Inline flags
------------

### `--require-hanja`

Forces the hanja to always appear in the output (relevant for `hangul-only`
mode, where it would otherwise be dropped):

~~~~ sh
gukhanmun --require-hanja 漢 --require-hanja 字 input.txt
~~~~

### `--require-hangul`

Forces the hangul reading to appear alongside the hanja (relevant for
`original` mode):

~~~~ sh
gukhanmun --rendering original --require-hangul 東 input.txt
~~~~

### `--skip-annotation`

Suppresses any annotation for the hanja, leaving it as-is in the output:

~~~~ sh
gukhanmun --skip-annotation 中 input.txt
~~~~


Glob patterns
-------------

Each directive has a `-glob` variant that matches a shell-style glob against
the hanja key:

~~~~ sh
gukhanmun --require-hanja-glob "東*" input.txt
gukhanmun --require-hangul-glob "北[京津]" input.txt
gukhanmun --skip-annotation-glob "中*" input.txt
~~~~


Directives file
---------------

For many directives, use a TSV file with `--directives`:

~~~~ sh
gukhanmun --directives overrides.tsv input.txt
~~~~

The file has three tab-separated columns:

| Column    | Values                                               |
| --------- | ---------------------------------------------------- |
| `action`  | `require-hanja`, `require-hangul`, `skip-annotation` |
| `pattern` | The hanja string to match                            |
| `kind`    | `literal` or `glob`                                  |

Lines starting with `#` are comments.  Blank lines are ignored.

Example *overrides.tsv*:

~~~~ tsv
# Force reading for proper nouns
require-hanja	東京	literal
require-hanja	北京	literal
# Suppress annotation for all middle-dot separated names
skip-annotation	*·*	glob
~~~~
