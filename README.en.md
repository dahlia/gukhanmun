<picture>
  <source srcset="logo-square-white.svg" media="(prefers-color-scheme: dark)">
  <img src="logo-square.svg" width="75" height="75">
</picture>

Gukhanmun
=========

[![crates.io][crates.io badge]][crates.io]
[![JSR][JSR badge]][JSR]
[![npm][npm badge]][npm]
[![PyPI][PyPI badge]][PyPI]
[![GitHub Actions][GitHub Actions badge]][GitHub Actions]
[![License: GPL-3.0-only][GPL badge]][GPL]
[![GitHub Sponsors][GitHub Sponsors badge]][GitHub Sponsors]
[![Open Collective][Open Collective badge]][Open Collective]
[![Funded by FOSS for All Microgrants 2026][FOSS for All Microgrants 2026 badge]][FOSS for All Microgrants 2026]

*Also available: [韓國語](README.ko-Kore.md) (Korean).*

Gukhanmun converts Korean text written in mixed script (國漢文混用體, hanja
characters interleaved with hangul) into hangul-only text. It is the successor
to [Seonbi], narrowed to the hanja-to-hangul conversion pipeline and extended
along several axes: streaming I/O, pluggable dictionaries, lattice-based
segmentation, and a wider range of output formats. The library is implemented
in Rust and exposed as a Rust library, a command-line tool, a WebAssembly
package, a native Node-API addon, and a Python package.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun
[JSR badge]: https://jsr.io/badges/@gukhanmun/types
[JSR]: https://jsr.io/@gukhanmun
[npm badge]: https://img.shields.io/npm/v/@gukhanmun/types?logo=npm
[npm]: https://www.npmjs.com/package/@gukhanmun/types
[PyPI badge]: https://img.shields.io/pypi/v/gukhanmun?logo=pypi
[PyPI]: https://pypi.org/project/gukhanmun/
[GitHub Actions badge]: https://github.com/dahlia/gukhanmun/actions/workflows/main.yaml/badge.svg
[GitHub Actions]: https://github.com/dahlia/gukhanmun/actions/workflows/main.yaml
[GPL badge]: https://img.shields.io/github/license/dahlia/gukhanmun
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html
[GitHub Sponsors badge]: https://img.shields.io/github/sponsors/dahlia?logo=githubsponsors
[GitHub Sponsors]: https://github.com/sponsors/dahlia
[Open Collective badge]: https://img.shields.io/badge/Open%20Collective-Support-7FADF2?logo=opencollective
[Open Collective]: https://opencollective.com/gukhanmun
[FOSS for All Microgrants 2026 badge]: https://fossforall.org/badges/microgrants-2026.svg
[FOSS for All Microgrants 2026]: https://fossforall.org/en/docs/microgrants/2026/
[Seonbi]: https://github.com/dahlia/seonbi


Features
--------

 -  Lattice segmentation finds the best split rather than greedily taking the
    longest match. 行事場所 segments as 行事 + 場所, not 行事場 + 所.
 -  Pluggable dictionaries: in-memory map, mmap-friendly FST files, or CDB
    files, composable via `ChainDictionary`.
 -  The *Standard Korean Language Dictionary* (標準國語大辭典) and
    *Open Korean Dictionary* (우리말샘) ship as compiled FST/CDB data. Rust
    bundles them; JavaScript and Python provide separate, opt-in data packages.
 -  Format adapters for plain text, HTML fragments, and Markdown. The engine is
    format-neutral; adapters handle parsing and serialization.
 -  Six rendering modes: hangul-only, hangul(hanja) parentheses,
    hanja(hangul) parentheses, ruby on hangul, ruby on hanja, and original
    mixed script with selective glossing.
 -  The engine supports incremental conversion. Context-dependent annotation
    may buffer more input; the Python chunked API buffers the whole document
    until finalization.
 -  Initial sound law (頭音法則) for South Korean orthography, applied to
    fallback readings. Dictionary entries encode the correct reading already.
 -  The core crate (`gukhanmun-core`) is `no_std` + `alloc`, suitable for
    embedded targets.
 -  JavaScript and TypeScript bindings ship in two flavours: a WebAssembly
    package that runs in browsers, Deno, Node.js, Bun, and edge runtimes,
    and a native Node-API addon for higher server-side throughput.
 -  Python bindings provide typed, immutable configuration and reusable
    converters. Native conversion releases the global interpreter lock (GIL),
    and a configured converter can be shared across threads.


Installation
------------

### Command-line tool

#### Via mise

If you use [mise], install a prebuilt binary with a single command:

~~~~ sh
mise use -g aqua:dahlia/gukhanmun
~~~~

The `-g` flag installs it globally.  Omit it to activate the tool only in the
current project directory.

#### Via Windows Package Manager (`winget`)

On Windows, install via `winget`:

~~~~ pwsh
winget install HongMinhee.Gukhanmun
~~~~

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
and `gukhanmun-mkdict` executables somewhere on your `PATH`.

[mise]: https://mise.jdx.dev/

### Rust library

Add to *Cargo.toml*:

~~~~ sh
cargo add gukhanmun-core
~~~~

Optionally add format adapters and dictionary backends:

~~~~ sh
cargo add gukhanmun-html gukhanmun-markdown
cargo add gukhanmun-stdict gukhanmun-opendict
cargo add gukhanmun-fst gukhanmun-cdb
~~~~

### JavaScript/TypeScript library

Install the WebAssembly package for most JavaScript environments:

~~~~ sh
npm  add       @gukhanmun/wasm @gukhanmun/stdict-fst
pnpm add       @gukhanmun/wasm @gukhanmun/stdict-fst
yarn add       @gukhanmun/wasm @gukhanmun/stdict-fst
bun  add       @gukhanmun/wasm @gukhanmun/stdict-fst
deno add --jsr @gukhanmun/wasm @gukhanmun/stdict-fst
~~~~

If you need better server-side performance and don't mind a native dependency,
install the Node-API package instead:

~~~~ sh
npm  add     @gukhanmun/napi     @gukhanmun/stdict-fst
pnpm add     @gukhanmun/napi     @gukhanmun/stdict-fst
yarn add     @gukhanmun/napi     @gukhanmun/stdict-fst
bun  add     @gukhanmun/napi     @gukhanmun/stdict-fst
deno add npm:@gukhanmun/napi jsr:@gukhanmun/stdict-fst
~~~~

### Python library

Requires Python 3.11 or later. Use pip in your virtual environment, or add the
package to a uv-managed project:

~~~~ sh
python -m pip install gukhanmun
# Or, in a uv project:
uv add gukhanmun
~~~~

The base package has no required third-party Python dependencies and includes
no official dictionary data. To install the optional *Standard Korean Language
Dictionary*, use `python -m pip install 'gukhanmun[stdict]'` or
`uv add 'gukhanmun[stdict]'`, then explicitly add `stdict()` from
`gukhanmun.dictionaries` to a converter. Installing an extra does not activate
it automatically.

See the [Python installation guide] for supported wheels, source builds, and
prerelease installation, and the [Python quick start] for conversion examples.

[Python installation guide]: https://gukhanmun.org/guide/python/install
[Python quick start]: https://gukhanmun.org/guide/python/start


Quick start
-----------

~~~~ sh
echo "漢字 北京 標識" | gukhanmun
# → 한자 베이징 표지

echo "漢字" | gukhanmun --rendering hangul-hanja-parens
# → 한자(漢字)

echo "<p>漢字</p>" | gukhanmun --format text/html
# → <p>한자</p>
~~~~

~~~~ rust
use gukhanmun_core::{MapDictionary, RenderMode, convert_plain_text};

let mut dict = MapDictionary::new();
dict.insert("漢字", "한자");

let output = convert_plain_text("漢字", &dict, RenderMode::HangulOnly);
assert_eq!(output, "한자");
~~~~

The Python example supplies its own reading and needs no optional data:

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[
    MapDictionarySource(entries=[DictionaryEntry(hanja="漢字", reading="한자")]),
])

assert converter.convert("漢字를 한글로") == "한자를 한글로"
assert converter.convert("<p>漢字</p>", format=InputFormat.HTML) == "<p>한자</p>"
~~~~

For the full guide, including HTML/Markdown adapters, rendering modes, presets,
and the JavaScript and Python APIs, visit <https://gukhanmun.org/>.


Crates
------

The project is a Cargo workspace. All crates share the same version.

| Crate                                                           | Description                                                                                                |
| --------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| [`gukhanmun-core`][cr-core]                                     | Format-neutral IR, engine, dictionary trait, lattice segmenter, fallback phoneticizer. `no_std` + `alloc`. |
| [`gukhanmun-html`][cr-html]                                     | HTML fragment reader and writer; `HtmlScopeData` with `lang` inheritance and preserved-tag handling.       |
| [`gukhanmun-markdown`][cr-md]                                   | Markdown adapter over `pulldown-cmark`; inline HTML is re-scanned for `lang` attributes.                   |
| [`gukhanmun-fst`][cr-fst]                                       | FST-backed `HanjaDictionary` implementation for mmap-friendly on-disk dictionaries.                        |
| [`gukhanmun-cdb`][cr-cdb]                                       | CDB-trie `HanjaDictionary` implementation; trivially auditable on-disk format.                             |
| [`gukhanmun-stdict`][cr-stdict]                                 | The bundled *Standard Korean Language Dictionary* as an embedded FST byte array.                           |
| [`gukhanmun-opendict`][cr-opendict]                             | The bundled *Open Korean Dictionary* (우리말샘) data.                                                      |
| [`gukhanmun-loanwords`](./crates/gukhanmun-loanwords/README.md) | NIKL loanword spelling examples with explicit reading choices.                                             |
| [`gukhanmun-dict-extract`][cr-dict-extract]                     | Shared dictionary dump extraction helpers.                                                                 |
| [`gukhanmun-mkdict`][cr-mkdict]                                 | CLI tool to build FST or CDB dictionary files from TSV, CSV, or JSON Lines input.                          |
| [`gukhanmun-cli`][cr-cli]                                       | The `gukhanmun` command-line binary.                                                                       |

[cr-core]: https://crates.io/crates/gukhanmun-core
[cr-html]: https://crates.io/crates/gukhanmun-html
[cr-md]: https://crates.io/crates/gukhanmun-markdown
[cr-fst]: https://crates.io/crates/gukhanmun-fst
[cr-cdb]: https://crates.io/crates/gukhanmun-cdb
[cr-stdict]: https://crates.io/crates/gukhanmun-stdict
[cr-opendict]: https://crates.io/crates/gukhanmun-opendict
[cr-dict-extract]: https://crates.io/crates/gukhanmun-dict-extract
[cr-mkdict]: https://crates.io/crates/gukhanmun-mkdict
[cr-cli]: https://crates.io/crates/gukhanmun-cli


npm/JSR packages
----------------

The project also publishes nine JavaScript packages, all sharing the same
version as the Rust crates.

| Package                    | JSR                                 | npm                                 | Description                                                                         |
| -------------------------- | ----------------------------------- | ----------------------------------- | ----------------------------------------------------------------------------------- |
| `@gukhanmun/types`         | [JSR][jsr:@gukhanmun/types]         | [npm][npm:@gukhanmun/types]         | TypeScript type declarations shared by the WASM and NAPI packages. No runtime code. |
| `@gukhanmun/wasm`          | [JSR][jsr:@gukhanmun/wasm]          | [npm][npm:@gukhanmun/wasm]          | WebAssembly build. Runs in browsers, Deno, Node.js, and Bun.                        |
| `@gukhanmun/napi`          |                                     | [npm][npm:@gukhanmun/napi]          | Native Node.js addon via napi-rs. Faster than WASM for server-side use.             |
| `@gukhanmun/stdict-fst`    | [JSR][jsr:@gukhanmun/stdict-fst]    | [npm][npm:@gukhanmun/stdict-fst]    | Bundled *Standard Korean Language Dictionary* in FST format.                        |
| `@gukhanmun/stdict-cdb`    | [JSR][jsr:@gukhanmun/stdict-cdb]    | [npm][npm:@gukhanmun/stdict-cdb]    | Bundled *Standard Korean Language Dictionary* in CDB format.                        |
| `@gukhanmun/opendict-fst`  | [JSR][jsr:@gukhanmun/opendict-fst]  | [npm][npm:@gukhanmun/opendict-fst]  | Bundled *Open Korean Dictionary* categories in FST format.                          |
| `@gukhanmun/opendict-cdb`  | [JSR][jsr:@gukhanmun/opendict-cdb]  | [npm][npm:@gukhanmun/opendict-cdb]  | Bundled *Open Korean Dictionary* categories in CDB format.                          |
| `@gukhanmun/loanwords-fst` | [JSR][jsr:@gukhanmun/loanwords-fst] | [npm][npm:@gukhanmun/loanwords-fst] | Bundled NIKL loanword spelling examples in FST format.                              |
| `@gukhanmun/loanwords-cdb` | [JSR][jsr:@gukhanmun/loanwords-cdb] | [npm][npm:@gukhanmun/loanwords-cdb] | Bundled NIKL loanword spelling examples in CDB format.                              |

[jsr:@gukhanmun/types]: https://jsr.io/@gukhanmun/types
[npm:@gukhanmun/types]: https://www.npmjs.com/package/@gukhanmun/types
[jsr:@gukhanmun/wasm]: https://jsr.io/@gukhanmun/wasm
[npm:@gukhanmun/wasm]: https://www.npmjs.com/package/@gukhanmun/wasm
[npm:@gukhanmun/napi]: https://www.npmjs.com/package/@gukhanmun/napi
[jsr:@gukhanmun/stdict-fst]: https://jsr.io/@gukhanmun/stdict-fst
[npm:@gukhanmun/stdict-fst]: https://www.npmjs.com/package/@gukhanmun/stdict-fst
[jsr:@gukhanmun/stdict-cdb]: https://jsr.io/@gukhanmun/stdict-cdb
[npm:@gukhanmun/stdict-cdb]: https://www.npmjs.com/package/@gukhanmun/stdict-cdb
[jsr:@gukhanmun/opendict-fst]: https://jsr.io/@gukhanmun/opendict-fst
[npm:@gukhanmun/opendict-fst]: https://www.npmjs.com/package/@gukhanmun/opendict-fst
[jsr:@gukhanmun/opendict-cdb]: https://jsr.io/@gukhanmun/opendict-cdb
[npm:@gukhanmun/opendict-cdb]: https://www.npmjs.com/package/@gukhanmun/opendict-cdb
[jsr:@gukhanmun/loanwords-fst]: https://jsr.io/@gukhanmun/loanwords-fst
[npm:@gukhanmun/loanwords-fst]: https://www.npmjs.com/package/@gukhanmun/loanwords-fst
[jsr:@gukhanmun/loanwords-cdb]: https://jsr.io/@gukhanmun/loanwords-cdb
[npm:@gukhanmun/loanwords-cdb]: https://www.npmjs.com/package/@gukhanmun/loanwords-cdb


PyPI packages
-------------

All seven Python distributions share one version derived from the workspace,
using PEP 440 spelling. The six optional data distributions have
platform-independent wheels and no dependency on the native package.

| Distribution                | Description                                                          |
| --------------------------- | -------------------------------------------------------------------- |
| [`gukhanmun`][PyPI]         | Native converter, typed Python API, and optional dictionary loaders. |
| [`gukhanmun-stdict-fst`]    | *Standard Korean Language Dictionary* data, FST format.              |
| [`gukhanmun-stdict-cdb`]    | *Standard Korean Language Dictionary* data, CDB format.              |
| [`gukhanmun-opendict-fst`]  | Four *Open Korean Dictionary* categories, FST format.                |
| [`gukhanmun-opendict-cdb`]  | Four *Open Korean Dictionary* categories, CDB format.                |
| [`gukhanmun-loanwords-fst`] | NIKL loanword spelling examples, FST format.                         |
| [`gukhanmun-loanwords-cdb`] | NIKL loanword spelling examples, CDB format.                         |

Dictionary data retains its own license and attribution requirements, separate
from the library's GPL license. See [*Official dictionaries*] and the *LICENSE*
and *ATTRIBUTION.md* files in each data distribution.

[`gukhanmun-stdict-fst`]: https://pypi.org/project/gukhanmun-stdict-fst/
[`gukhanmun-stdict-cdb`]: https://pypi.org/project/gukhanmun-stdict-cdb/
[`gukhanmun-opendict-fst`]: https://pypi.org/project/gukhanmun-opendict-fst/
[`gukhanmun-opendict-cdb`]: https://pypi.org/project/gukhanmun-opendict-cdb/
[`gukhanmun-loanwords-fst`]: https://pypi.org/project/gukhanmun-loanwords-fst/
[`gukhanmun-loanwords-cdb`]: https://pypi.org/project/gukhanmun-loanwords-cdb/
[*Official dictionaries*]: https://gukhanmun.org/guide/python/dictionaries


Design documentation
--------------------

[*DESIGN.md*](./DESIGN.en.md) covers the full architecture: intermediate
representation, lattice segmentation algorithm, dictionary trait design,
middleware system, and format adapter internals.


Support the project
-------------------

You can support Gukhanmun's ongoing development and maintenance through
[GitHub Sponsors] or [Open Collective].


License
-------

Distributed under GPL 3.0. See [*LICENSE*](./LICENSE).
