gukhanmun-cli
=============

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

The `gukhanmun` command-line binary. Reads Korean mixed-script text from a
file or standard input and writes hangul-converted text to a file or standard
output. HTML and Markdown formats are supported in addition to plain text.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-cli?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-cli
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-cli
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ sh
cargo install gukhanmun-cli
~~~~

Prebuilt binaries for macOS, Windows, and Linux (musl) are attached to each
GitHub release.


Usage
-----

~~~~ sh
# Plain text, default preset (ko-KR):
echo "漢字 北京 標識" | gukhanmun
# → 한자 베이징 표지

# With hanja in parentheses:
echo "漢字" | gukhanmun --rendering hangul-hanja-parens
# → 한자(漢字)

# North Korean orthography:
echo "來日 北京" | gukhanmun --preset ko-kp
# → 래일 북경

# HTML and Markdown via --format, or inferred from the file extension:
echo "<p>漢字</p>" | gukhanmun --format text/html
gukhanmun input.html -o output.html
gukhanmun notes.md   -o notes.md

# Print full help:
gukhanmun --help
~~~~


Options
-------

| Flag                        | Description                                                                                                |
| --------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `--preset`                  | `ko-kr` (default) or `ko-kp`                                                                               |
| `--format`/`-f`             | `text/plain`, `text/html`, or `text/markdown`; inferred from extension if omitted                          |
| `--rendering`               | `hangul-only`, `hangul-hanja-parens`, `hanja-hangul-parens`, `ruby-on-hangul`, `ruby-on-hanja`, `original` |
| `--dictionary`/`-d`         | Additional *.gukfst* or *.gukcdb* dictionary file (repeatable)                                             |
| `--loanwords`               | Enable NIKL loanword examples (already enabled for `ko-kr`)                                                |
| `--no-loanwords`            | Disable only NIKL loanword examples                                                                        |
| `--no-bundled-dictionaries` | Disable every bundled dictionary selected by the preset                                                    |
| `--output`/`-o`             | Output file path; defaults to standard output                                                              |


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.


Loanword spelling examples
--------------------------

The default `ko-kr` preset loads the bundled NIKL loanword examples before
other preset dictionaries, for example to convert `香港` to `홍콩`. Pass
`--no-loanwords` to keep only the standard dictionary. The `ko-kp` preset
leaves loanwords disabled; pass `--loanwords` to enable them explicitly.
`--loanwords` conflicts with `--no-loanwords` and
`--no-bundled-dictionaries`.
User dictionaries still take priority. Collection is a separate maintainer
operation; normal conversion needs neither a network connection nor an API key.
