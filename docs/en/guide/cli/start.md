---
title: "Quick start"
description: |-
  Basic usage of the Gukhanmun command-line tool.
---

Quick start
===========

Gukhanmun reads text containing hanja and writes the same text with hanja
replaced by their hangul readings.


Converting a file
-----------------

Pass a file path as the positional argument:

~~~~ sh
gukhanmun input.txt
~~~~

Output goes to standard output by default.  Use `-o` to write to a file
instead:

~~~~ sh
gukhanmun -o output.txt input.txt
~~~~

To replace a file in-place, pass the same path for both input and output.
Gukhanmun writes to a temporary file first and replaces the original
atomically, so the original is never left in a partial state:

~~~~ sh
gukhanmun -o document.txt document.txt
~~~~


Reading from standard input
---------------------------

Omit the file argument to read from standard input:

~~~~ sh
echo "漢字를 한글로" | gukhanmun
# → 한자를 한글로
~~~~


Specifying the input format
---------------------------

Gukhanmun infers the format from the file extension:

| Extension          | Format     |
| ------------------ | ---------- |
| *.txt*, *(none)*   | Plain text |
| *.html*, *.htm*    | HTML       |
| *.md*, *.markdown* | Markdown   |

Override the detected format with `-f`:

~~~~ sh
gukhanmun -f text/plain input.html      # treat as plain text
gukhanmun -f text/html  snippet.txt     # treat as HTML
gukhanmun -f text/markdown article.txt  # treat as Markdown
~~~~

The Markdown format also accepts variant parameters:

~~~~ sh
gukhanmun -f "text/markdown; variant=GFM" post.md
~~~~


Verbose logging
---------------

Add `-v` to print debug information to standard error:

~~~~ sh
gukhanmun -v input.txt
~~~~
