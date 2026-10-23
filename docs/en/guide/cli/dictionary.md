---
title: "Dictionaries"
description: |-
  Using the bundled dictionary and loading custom dictionaries from the CLI.
---

Dictionaries
============

Gukhanmun uses dictionaries to look up the hangul readings of hanja.  The
`ko-kr` preset uses the bundled *Loanword spelling examples* and *Standard
Korean Dictionary* (標準國語大辭典), and the `ko-kp` preset uses the bundled
*Open Korean Dictionary* (우리말샘) North Korean category.


Bundled dictionaries
--------------------

The bundled dictionary selected by the preset is loaded automatically.  No
extra flags are needed for most Korean text.

To disable every bundled dictionary, for example when you want to rely entirely
on a custom dictionary, pass `--no-bundled-dictionaries`:

~~~~ sh
gukhanmun --no-bundled-dictionaries -d my-dict.gukfst input.txt
~~~~

Use the `ko-kp` preset for North Korean orthography.  It disables the initial
sound law and uses the bundled *Open Korean Dictionary* (우리말샘) North Korean
(北韓語) dictionary:

~~~~ sh
gukhanmun --preset ko-kp input.txt
~~~~


Loanword spelling examples
--------------------------

The `ko-kr` preset includes the National Institute of Korean Language's
*Loanword spelling examples* dictionary by default. It supplies Korean
spellings of foreign names written in hanja, such as `香港` → `홍콩`:

~~~~ sh
echo '香港' | gukhanmun
# 홍콩
~~~~

Use `--no-loanwords` to disable only loanword examples while keeping the
preset's other dictionaries. The `ko-kp` preset leaves loanword examples off;
pass `--loanwords` to enable them explicitly. Custom dictionaries take priority,
followed by loanword examples, then the preset's other dictionaries.
`--loanwords` cannot be combined with `--no-bundled-dictionaries`.


Custom dictionaries
-------------------

Supply one or more custom dictionaries with `-d` (or `--dictionary`).  The
flag can be repeated:

~~~~ sh
gukhanmun -d legal.gukfst input.txt
gukhanmun -d legal.gukfst -d names.gukcdb input.txt
~~~~

Gukhanmun supports two binary dictionary formats:

| Format | Extension | Lookup                 | Notes                                               |
| ------ | --------- | ---------------------- | --------------------------------------------------- |
| FST    | *.gukfst* | $O(\text{key length})$ | Preferred for lattice segmentation; smaller on disk |
| CDB    | *.gukcdb* | $O(1)$                 | Simpler layout; easier to audit by hand             |

Later `-d` arguments take priority over earlier ones and over every bundled
dictionary. The first match in that priority order wins.


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
