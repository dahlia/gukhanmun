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

To disable it—for example when you want to rely entirely on a custom
dictionary—pass `--no-stdict`:

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


Building a custom dictionary
----------------------------

The *.gukfst* and *.gukcdb* files are compiled artifacts, not something you
edit by hand.  You author your entries as a plain text table and compile them
with `gukhanmun-mkdict`.

The `gukhanmun-mkdict` builder is installed together with `gukhanmun`, whether
you [install via mise](./install.md#via-mise) or
[download a prebuilt archive](./install.md#prebuilt-binaries). If you instead
built from crates.io, install the builder the same way:

~~~~ sh
cargo install gukhanmun-mkdict
~~~~

Write your entries as a tab-separated file with a `hanja` key column and a
`hangul` reading column:

~~~~ tsv
hanja	hangul
北京	베이징
學校	학교
~~~~

Two optional columns control how renderers treat each entry: set
`require_hanja` to `true` to keep the source hanja visible (for homophones that
need disambiguation), and `require_hangul` to `true` to force a hangul gloss in
the original-script rendering mode.

~~~~ tsv
hanja	hangul	require_hanja	require_hangul
北京	베이징	false	false
色깔論	색깔론	false	true
~~~~

Compile the table into an FST dictionary (the default format):

~~~~ sh
gukhanmun-mkdict --output legal.gukfst legal.tsv
~~~~

Pass `--format cdb` to produce a *.gukcdb* file instead.  You can supply
several input files, which are merged in order; `--merge` selects how duplicate
keys are resolved (`error`, `first-wins`, or `last-wins`).  Add `--validate` to
reopen the output and confirm every entry round-trips, and `--metadata KEY=VAL`
to embed provenance such as the source or license.

Then load the result like any other custom dictionary:

~~~~ sh
gukhanmun -d legal.gukfst input.txt
~~~~

CSV and JSON Lines inputs are also accepted, and a few more advanced options
are available.  See [the internals section](/internals/dictionary-format) for
the full dictionary file format specification.
