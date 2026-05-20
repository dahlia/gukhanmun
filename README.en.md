Gukhanmun
=========

*Also available: [韓國語](README.ko-Kore.md) (Korean).*

Gukhanmun converts Korean text written in mixed script (國漢文混用體, hanja
characters interleaved with hangul) into hangul-only text. It is the successor
to [Seonbi], narrowed to the hanja-to-hangul conversion pipeline and extended
along several axes: streaming I/O, pluggable dictionaries, lattice-based
segmentation, and a wider range of output formats. The library is implemented
in Rust and exposed as a Rust library, a command-line tool, and (planned)
WebAssembly and Node-API bindings.

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

~~~~ sh
cargo install gukhanmun-cli
~~~~

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
use gukhanmun_markdown::convert_markdown;

let mut dict = MapDictionary::new();
dict.insert("漢字", "한자");
dict.insert("北京", "베이징");

let output = convert_markdown(
    "# 漢字\n\n- 北京 and **漢字**\n",
    &dict,
    RenderMode::HangulOnly,
).unwrap();
// → "# 한자\n\n- 베이징 and **한자**\n" (semantically equivalent)
~~~~


Rendering modes
---------------

The renderer is decoupled from the engine and middlewares. The mode is chosen
per conversion call.

| Mode                                          | Rust enum variant               | Output for 漢字                                  |
| --------------------------------------------- | ------------------------------- | ------------------------------------------------ |
| Hangul only                                   | `RenderMode::HangulOnly`        | 한자                                             |
| Hangul with hanja in parentheses              | `RenderMode::HangulHanjaParens` | 한자(漢字)                                       |
| Hanja with hangul in parentheses              | `RenderMode::HanjaHangulParens` | 漢字(한자)                                       |
| Ruby markup                                   | `RenderMode::Ruby`              | `<ruby>한자<rt>漢字</rt></ruby>`                 |
| Original mixed script with selective glossing | `RenderMode::Original`          | 漢字 (glossed only when `require_hangul` is set) |

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

| Crate                | Description                                                                                                |
| -------------------- | ---------------------------------------------------------------------------------------------------------- |
| `gukhanmun-core`     | Format-neutral IR, engine, dictionary trait, lattice segmenter, fallback phoneticizer. `no_std` + `alloc`. |
| `gukhanmun-html`     | HTML fragment reader and writer; `HtmlScopeData` with `lang` inheritance and preserved-tag handling.       |
| `gukhanmun-markdown` | Markdown adapter over `pulldown-cmark`; inline HTML is re-scanned for `lang` attributes.                   |
| `gukhanmun-fst`      | FST-backed `HanjaDictionary` implementation for mmap-friendly on-disk dictionaries.                        |
| `gukhanmun-cdb`      | CDB-trie `HanjaDictionary` implementation; trivially auditable on-disk format.                             |
| `gukhanmun-stdict`   | The bundled South Korean Standard Dictionary as an embedded FST byte array.                                |
| `gukhanmun-mkdict`   | CLI tool to build FST or CDB dictionary files from TSV, CSV, or JSON Lines input.                          |
| `gukhanmun-cli`      | The `gukhanmun` command-line binary.                                                                       |


Design documentation
--------------------

[*DESIGN.md*](./DESIGN.en.md) covers the full architecture: intermediate
representation, lattice segmentation algorithm, dictionary trait design,
middleware system, and format adapter internals.


License
-------

Distributed under GPL 3.0. See [*LICENSE*](./LICENSE).
