<picture>
  <source srcset="logo-square-white.svg" media="(prefers-color-scheme: dark)">
  <img src="logo-square.svg" width="75" height="75">
</picture>

Gukhanmun
=========

[![crates.io][crates.io badge]][crates.io]
[![JSR][JSR badge]][JSR]
[![npm][npm badge]][npm]
[![GitHub Actions][GitHub Actions badge]][GitHub Actions]
[![License: GPL-3.0-only][GPL badge]][GPL]
[![GitHub Sponsors][GitHub Sponsors badge]][GitHub Sponsors]

*Also available: [韓國語](README.ko-Kore.md) (Korean).*

Gukhanmun converts Korean text written in mixed script (國漢文混用體, hanja
characters interleaved with hangul) into hangul-only text. It is the successor
to [Seonbi], narrowed to the hanja-to-hangul conversion pipeline and extended
along several axes: streaming I/O, pluggable dictionaries, lattice-based
segmentation, and a wider range of output formats. The library is implemented
in Rust and exposed as a Rust library, a command-line tool, and (planned)
WebAssembly and Node-API bindings.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun
[JSR badge]: https://jsr.io/badges/@gukhanmun/types
[JSR]: https://jsr.io/@gukhanmun
[npm badge]: https://img.shields.io/npm/v/@gukhanmun/types?logo=npm
[npm]: https://www.npmjs.com/package/@gukhanmun/types
[GitHub Actions badge]: https://github.com/dahlia/gukhanmun/actions/workflows/main.yaml/badge.svg
[GitHub Actions]: https://github.com/dahlia/gukhanmun/actions/workflows/main.yaml
[GPL badge]: https://img.shields.io/github/license/dahlia/gukhanmun
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html
[GitHub Sponsors badge]: https://img.shields.io/github/sponsors/dahlia?logo=githubsponsors
[GitHub Sponsors]: https://github.com/sponsors/dahlia
[Seonbi]: https://github.com/dahlia/seonbi


Features
--------

 -  Lattice segmentation finds the best split rather than greedily taking the
    longest match. 行事場所 segments as 行事 + 場所, not 行事場 + 所.
 -  Pluggable dictionaries: in-memory map, mmap-friendly FST files, or CDB
    files, composable via `ChainDictionary`.
 -  The bundled South Korean Standard Dictionary (標準國語大辭典) ships as a
    compiled FST, so there is nothing extra to download.
 -  Format adapters for plain text, HTML fragments, and Markdown. The engine is
    format-neutral; adapters handle parsing and serialization.
 -  Five rendering modes: hangul-only, hangul(hanja) parentheses,
    hanja(hangul) parentheses, ruby markup, and original mixed script with
    selective glossing.
 -  Streaming-first: the engine buffers only within a single hanja conversion
    span, not the whole document.
 -  Initial sound law (頭音法則) for South Korean orthography, applied to
    fallback readings. Dictionary entries encode the correct reading already.
 -  The core crate (`gukhanmun-core`) is `no_std` + `alloc`, suitable for
    embedded targets.


Installation
------------

### Command-line tool

#### Via mise

If you use [mise], install a prebuilt binary with a single command:

~~~~ sh
mise use -g "github:dahlia/gukhanmun[asset_pattern=gukhanmun-{{ version }}-*.{% if os() == 'windows' %}zip{% else %}tar.bz2{% endif %}]"
~~~~

The `-g` flag installs it globally.  Omit it to activate the tool only in the
current project directory.

#### From crates.io

If you have a Rust toolchain installed, install from crates.io:

~~~~ sh
cargo install gukhanmun-cli gukhanmun-mkdict
~~~~

This compiles the binaries and places them in *~/.cargo/bin/*.  Make sure that
directory is on your `PATH`.

#### Prebuilt binaries

Prebuilt binaries for Linux (x86\_64, aarch64), macOS (x86\_64, aarch64), and
Windows (x86\_64) are attached to each release on GitHub:

<https://github.com/dahlia/gukhanmun/releases>

Download the archive for your platform, extract it, and place the `gukhanmun`
binary somewhere on your `PATH`.

[mise]: https://mise.jdx.dev/

### Rust library

Add to *Cargo.toml*:

~~~~ toml
[dependencies]
gukhanmun-core = "0.1"

# Optional format adapters:
gukhanmun-html     = "0.1"
gukhanmun-markdown = "0.1"

# Optional dictionary backends:
gukhanmun-fst  = "0.1"
gukhanmun-cdb  = "0.1"

# Optional bundled Standard Korean Language Dictionary:
gukhanmun-stdict = "0.1"
~~~~


Quick start
-----------

### Command line

The `ko-kr` preset is active by default. It loads the bundled Standard Korean
Language Dictionary and applies the initial sound law.

~~~~ sh
echo "漢字 北京 標識" | gukhanmun
# → 한자 베이징 표지

echo "漢字" | gukhanmun --rendering hangul-hanja-parens
# → 한자(漢字)

echo "來日 北京" | gukhanmun --preset ko-kp
# → 래일 북경

# HTML and Markdown formats are supported via --format (-f).
# The format is also inferred automatically from the input file extension
# (.html/.htm → text/html, .md/.markdown → text/markdown):
echo "<p>漢字</p>" | gukhanmun --format text/html
# → <p>한자</p>

echo "# 漢字" | gukhanmun --format text/markdown
# → # 한자

gukhanmun input.html -o output.html   # format inferred from extension
gukhanmun notes.md -o notes.md        # format inferred from extension

gukhanmun --help
~~~~

### Plain text (Rust)

~~~~ rust
use gukhanmun_core::{MapDictionary, RenderMode, convert_plain_text};

let mut dict = MapDictionary::new();
dict.insert("漢字", "한자");
dict.insert("北京", "베이징");

let output = convert_plain_text("漢字 北京", &dict, RenderMode::HangulOnly);
assert_eq!(output, "한자 베이징");
~~~~

### HTML fragment (Rust)

~~~~ rust
use gukhanmun_core::{MapDictionary, RenderMode};
use gukhanmun_html::convert_html_fragment;

let mut dict = MapDictionary::new();
dict.insert("漢字", "한자");

let output = convert_html_fragment(
    "<p class=\"intro\">漢字</p>",
    &dict,
    RenderMode::HangulOnly,
);
assert_eq!(output, "<p class=\"intro\">한자</p>");
// Preserved tags pass through unchanged:
// <code>漢字</code>, <pre>, <script>, <style>, <textarea>, <kbd>
~~~~

### Markdown (Rust)

~~~~ rust
use gukhanmun_core::{MapDictionary, RenderMode};
use gukhanmun_markdown::{MarkdownVariant, convert_markdown};

let mut dict = MapDictionary::new();
dict.insert("漢字", "한자");
dict.insert("北京", "베이징");

let output = convert_markdown(
    "# 漢字\n\n- 北京 and **漢字**\n",
    &dict,
    RenderMode::HangulOnly,
    MarkdownVariant::CommonMark,
).unwrap();
// → "# 한자\n\n- 베이징 and **한자**\n" (semantically equivalent)
~~~~


Rendering modes
---------------

The renderer is decoupled from the engine and middlewares. The mode is chosen
per conversion call.

| Mode                                          | Rust enum variant               | Output for 漢字                                      |
| --------------------------------------------- | ------------------------------- | ---------------------------------------------------- |
| Hangul only                                   | `RenderMode::HangulOnly`        | 한자                                                 |
| Hangul with hanja in parentheses              | `RenderMode::HangulHanjaParens` | 한자(漢字)                                           |
| Hanja with hangul in parentheses              | `RenderMode::HanjaHangulParens` | 漢字(한자)                                           |
| Ruby markup                                   | `RenderMode::Ruby`              | `<ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>` |
| Original mixed script with selective glossing | `RenderMode::Original`          | 漢字 (glossed only when `require_hangul` is set)     |

`HangulOnly` adds hanja in parentheses automatically when the dictionary flags
the word as having a homophone or as requiring disambiguation.


Presets
-------

| Option                   | `ko-kr` (default)                   | `ko-kp`     |
| ------------------------ | ----------------------------------- | ----------- |
| Bundled dictionary       | Standard Korean Language Dictionary | none        |
| Initial sound law        | enabled                             | disabled    |
| Homophone disambiguation | per-block                           | off         |
| Rendering                | hangul-only                         | hangul-only |

`ko-kp` follows North Korean orthographic conventions: Sino-Korean words are
written in hangul without the initial sound law (래일, 류행, 녀자). No bundled
dictionary is included because the South Korean Standard Dictionary's readings
are incorrect for `ko-KP`.


Crates
------

The project is a Cargo workspace. All crates share the same version.

| Crate                           | Description                                                                                                |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| [`gukhanmun-core`][cr-core]     | Format-neutral IR, engine, dictionary trait, lattice segmenter, fallback phoneticizer. `no_std` + `alloc`. |
| [`gukhanmun-html`][cr-html]     | HTML fragment reader and writer; `HtmlScopeData` with `lang` inheritance and preserved-tag handling.       |
| [`gukhanmun-markdown`][cr-md]   | Markdown adapter over `pulldown-cmark`; inline HTML is re-scanned for `lang` attributes.                   |
| [`gukhanmun-fst`][cr-fst]       | FST-backed `HanjaDictionary` implementation for mmap-friendly on-disk dictionaries.                        |
| [`gukhanmun-cdb`][cr-cdb]       | CDB-trie `HanjaDictionary` implementation; trivially auditable on-disk format.                             |
| [`gukhanmun-stdict`][cr-stdict] | The bundled South Korean Standard Dictionary as an embedded FST byte array.                                |
| [`gukhanmun-mkdict`][cr-mkdict] | CLI tool to build FST or CDB dictionary files from TSV, CSV, or JSON Lines input.                          |
| [`gukhanmun-cli`][cr-cli]       | The `gukhanmun` command-line binary.                                                                       |

[cr-core]: https://crates.io/crates/gukhanmun-core
[cr-html]: https://crates.io/crates/gukhanmun-html
[cr-md]: https://crates.io/crates/gukhanmun-markdown
[cr-fst]: https://crates.io/crates/gukhanmun-fst
[cr-cdb]: https://crates.io/crates/gukhanmun-cdb
[cr-stdict]: https://crates.io/crates/gukhanmun-stdict
[cr-mkdict]: https://crates.io/crates/gukhanmun-mkdict
[cr-cli]: https://crates.io/crates/gukhanmun-cli


npm/JSR packages
----------------

The project also publishes five JavaScript packages, all sharing the same
version as the Rust crates.

| Package                 | JSR                              | npm                              | Description                                                                         |
| ----------------------- | -------------------------------- | -------------------------------- | ----------------------------------------------------------------------------------- |
| `@gukhanmun/types`      | [JSR][jsr:@gukhanmun/types]      | [npm][npm:@gukhanmun/types]      | TypeScript type declarations shared by the WASM and NAPI packages. No runtime code. |
| `@gukhanmun/wasm`       | [JSR][jsr:@gukhanmun/wasm]       | [npm][npm:@gukhanmun/wasm]       | WebAssembly build. Runs in browsers, Deno, Node.js, and Bun.                        |
| `@gukhanmun/napi`       |                                  | [npm][npm:@gukhanmun/napi]       | Native Node.js addon via napi-rs. Faster than WASM for server-side use.             |
| `@gukhanmun/stdict-fst` | [JSR][jsr:@gukhanmun/stdict-fst] | [npm][npm:@gukhanmun/stdict-fst] | Bundled Standard Korean Language Dictionary in FST format.                          |
| `@gukhanmun/stdict-cdb` | [JSR][jsr:@gukhanmun/stdict-cdb] | [npm][npm:@gukhanmun/stdict-cdb] | Bundled Standard Korean Language Dictionary in CDB format.                          |

[jsr:@gukhanmun/types]: https://jsr.io/@gukhanmun/types
[npm:@gukhanmun/types]: https://www.npmjs.com/package/@gukhanmun/types
[jsr:@gukhanmun/wasm]: https://jsr.io/@gukhanmun/wasm
[npm:@gukhanmun/wasm]: https://www.npmjs.com/package/@gukhanmun/wasm
[npm:@gukhanmun/napi]: https://www.npmjs.com/package/@gukhanmun/napi
[jsr:@gukhanmun/stdict-fst]: https://jsr.io/@gukhanmun/stdict-fst
[npm:@gukhanmun/stdict-fst]: https://www.npmjs.com/package/@gukhanmun/stdict-fst
[jsr:@gukhanmun/stdict-cdb]: https://jsr.io/@gukhanmun/stdict-cdb
[npm:@gukhanmun/stdict-cdb]: https://www.npmjs.com/package/@gukhanmun/stdict-cdb


Design documentation
--------------------

[*DESIGN.md*](./DESIGN.en.md) covers the full architecture: intermediate
representation, lattice segmentation algorithm, dictionary trait design,
middleware system, and format adapter internals.


License
-------

Distributed under GPL 3.0. See [*LICENSE*](./LICENSE).
