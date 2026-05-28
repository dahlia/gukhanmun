---
title: "Dictionaries"
description: |-
  Using the bundled dictionary and loading custom dictionaries from the CLI.
---

Dictionaries
============

Gukhanmun uses dictionaries to look up the hangul readings of hanja.  By
default it ships with the bundled Standard Korean Dictionary
(標準國語大辭典).


Bundled Standard Korean Dictionary
----------------------------------

The bundled dictionary is loaded automatically.  No extra flags are needed for
most Korean text.

To disable it — for example when you want to rely entirely on a custom
dictionary — pass `--no-stdict`:

~~~~ sh
gukhanmun --no-stdict -d my-dict.gukfst input.txt
~~~~


Custom dictionaries
-------------------

Supply one or more custom dictionaries with `-d` (or `--dictionary`).  The
flag can be repeated:

~~~~ sh
gukhanmun -d legal.gukfst input.txt
gukhanmun -d legal.gukfst -d names.gukcdb input.txt
~~~~

Gukhanmun supports two binary dictionary formats:

| Format | Extension | Lookup        | Notes                                               |
| ------ | --------- | ------------- | --------------------------------------------------- |
| FST    | *.gukfst* | O(key length) | Preferred for lattice segmentation; smaller on disk |
| CDB    | *.gukcdb* | O(1)          | Simpler layout; easier to audit by hand             |

Dictionaries are tried in the order they appear on the command line, with the
bundled dictionary consulted last.  The first match wins.

See [the internals section](/internals/dictionary-format) for the full
dictionary file format specification and instructions on building your own
dictionaries with `gukhanmun-mkdict`.
