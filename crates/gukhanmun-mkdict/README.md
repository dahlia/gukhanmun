gukhanmun-mkdict
================

Command-line tool that compiles hanja dictionary source data into Gukhanmun's
binary dictionary formats (FST or CDB). The typical input is a tab-separated
file with a hanja key in the first column and a hangul reading in the second.


Installation
------------

~~~~ sh
cargo install gukhanmun-mkdict
~~~~


Input format
------------

Each line of the input TSV has two or three columns:

~~~~ text
漢字\t한자
北京\t베이징
學校\t학교\trequire_hanja
~~~~

The optional third column is a comma-separated list of marks:
`require_hanja` (the word is a homophone that needs disambiguation) and
`require_hangul` (the entry requires a hangul gloss in the original-script
rendering mode).

CSV and JSON Lines input are also accepted; the tool infers the format from
the file extension or from `--format`.


Usage
-----

~~~~ sh
# Compile to FST (default):
gukhanmun-mkdict --output stdict.gukfst words.tsv

# Compile to CDB:
gukhanmun-mkdict --format cdb --output stdict.gukcdb words.tsv

# Multiple input files (merged in order):
gukhanmun-mkdict --output combined.gukfst base.tsv overrides.tsv

# Attach metadata:
gukhanmun-mkdict --output stdict.gukfst \
    --metadata source="Standard Korean Language Dictionary" \
    --metadata built=$(date -I) \
    words.tsv

# Validate entries without writing output:
gukhanmun-mkdict --validate words.tsv
~~~~


Merge policies
--------------

When two input files contain the same key, `--merge` controls the outcome:
`error` (the default) rejects duplicates, `last-wins` keeps the last value,
and `first-wins` keeps the first.


License
-------

GPL-3.0-only. See `LICENSE` at the repository root.
