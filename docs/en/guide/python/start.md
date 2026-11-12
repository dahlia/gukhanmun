---
title: Python quick start
description: Build the Gukhanmun Python package locally and convert text, HTML and Markdown with typed options and explicit dictionaries.
---

Python quick start
==================

`Converter(...)` creates a reusable Python converter backed by Rust. Configure
it once, then pass each document to `convert()`. The Python package requires
Python 3.11 or later and has no required third-party Python runtime
dependencies.


Build and install a local wheel
-------------------------------

From the repository root, install the configured development tools and build
the package:

~~~~ sh
mise install
mise run py-build
~~~~

The wheel is written under *target/python/wheels/*. Install that wheel into
your project's Python environment, replacing the example path with its actual
filename:

~~~~ sh
python -m pip install /path/to/gukhanmun.whl
~~~~

The command above installs a locally built artifact. These instructions do not
require a published PyPI release. See [*Supported platforms*](./platforms.md)
for the platforms that the release wheels cover and for building from the
source distribution. To check the package from the checkout, run:

~~~~ sh
mise run py-test
mise run py-typecheck
~~~~

The tasks test an installed wheel outside the checkout. `py-typecheck` runs
mypy, Pyright, basedpyright, ty and Pyrefly against the same consumer examples.


Convert a document
------------------

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[
    MapDictionarySource(entries=[DictionaryEntry(hanja="漢字", reading="한자")]),
])

assert converter.convert("漢字를 한글로") == "한자를 한글로"
assert converter.convert("<p>漢字</p>", format=InputFormat.HTML) == "<p>한자</p>"
~~~~

Plain text is the default format. Use `InputFormat.MARKDOWN` for CommonMark,
or `MarkdownOptions(gfm=True)` for GitHub Flavored Markdown:

~~~~ python
from gukhanmun import MarkdownOptions

result = converter.convert("# 漢字", format=MarkdownOptions(gfm=True))
~~~~

Pass enum members to options. Convert a string from a configuration file
explicitly, for example `Preset("ko-kr")`. Passing `preset="ko-kr"` directly
to `Converter()` raises `TypeError`.

Neither preset loads official dictionaries automatically. With no explicit
sources, conversion uses the built-in character fallback. Supply runtime
entries or compiled FST/CDB data when word readings are needed, or install and
add the [official dictionaries](./dictionaries.md).


Load a compiled dictionary
--------------------------

~~~~ python
from pathlib import Path
from gukhanmun import Converter, DictionaryFormat, FileDictionarySource

converter = Converter(dictionaries=[
    FileDictionarySource(Path("custom.gukfst"), format=DictionaryFormat.FST),
])
~~~~

`FileDictionarySource()` also accepts `bytes`, `bytearray` and `memoryview`.
Mutable buffers are copied. Paths are read during converter construction;
conversion calls do no dictionary I/O. A string is always a filesystem path,
even when it resembles a URL.

For a remote dictionary, use an explicit `UrlDictionarySource()`:

~~~~ python
from gukhanmun import UrlDictionarySource

source = UrlDictionarySource(
    "https://example.com/custom.gukfst",
    format=DictionaryFormat.FST,
    timeout=30.0,
    max_bytes=64 * 1024 * 1024,
    total_timeout=30.0,
)
# Converter(dictionaries=[source]) fetches this example resource.
~~~~

Only HTTP(S) URLs are accepted. Redirects must keep the initial URL's scheme,
hostname and effective port; a different origin raises
`INVALID_INPUT` before the target is requested. Configure the final dictionary
URL directly when it is hosted on another origin. `timeout` is a positive,
finite socket-operation timeout in seconds. `max_bytes` limits the dictionary
body, and `total_timeout` limits HTTP response reading across redirects,
including headers and chunk framing. Cancellation during synchronous DNS lookup
or connection setup can be delayed beyond `total_timeout`. Exceeding either
bound raises `DICTIONARY_LOAD` with an `IO` cause. The loader does not use
proxy environment variables.

Map sources default to `SourceRole.OVERRIDE`. Binary sources use an explicit
role when supplied, otherwise their embedded evidence role, otherwise
`OVERRIDE`. Source filenames never determine roles. See the
[API reference](../../api/python.md) for source cooperation policies.


Change rendering and punctuation
--------------------------------

~~~~ python
from gukhanmun import (
    Converter, CustomPunctuationProfile, PunctuationPreset, RenderMode,
)

converter = Converter(
    rendering=RenderMode.HANGUL_HANJA_PARENS,
    punctuation=CustomPunctuationProfile(
        base=PunctuationPreset.MODERN,
        period=None,
    ),
)
~~~~

An omitted custom punctuation field has the value `UNSET`, which inherits the
base profile. `None` preserves that punctuation role unchanged. A `StopStyle`
or `DelimiterStyle` supplies an explicit output style. In this example, commas
inherit the modern style while periods retain their input form.

`converter.options` is a frozen `ResolvedOptions` value. Its punctuation
profile and nested styles are also frozen. A different configuration needs a
new converter.


Convert chunks
--------------

~~~~ python
converter = Converter()
stream = converter.stream()
assert stream.push("漢") == ""
assert stream.push("字") == ""
assert stream.finish() == converter.convert("漢字")

assert "".join(converter.convert_iter(["漢", "字"])) == converter.convert("漢字")
~~~~

The current stream implementation buffers all chunks and returns output only
from `finish()`.
Chunk boundaries do not change the result. A stream retains its converter and
has one consumer. After successful finalization or any push/finalization
failure, further calls raise `RuntimeError`. `convert_iter()` propagates
upstream iterator errors and finalizes only after normal exhaustion.


Handle errors and share a converter
-----------------------------------

~~~~ python
from gukhanmun import (
    Converter, DictionaryFormat, ErrorCode, FileDictionarySource, GukhanmunError,
)

try:
    Converter(dictionaries=[
        FileDictionarySource(b"invalid binary", format=DictionaryFormat.FST),
    ])
except GukhanmunError as error:
    assert error.code is ErrorCode.DICTIONARY_LOAD
    print(error.message)
    for cause in error.chain:
        print(cause.code, cause.message)
~~~~

`error.chain` is an immutable tuple, ordered deepest/root cause first. Wrong
argument types raise `TypeError`; invalid option values raise a coded
`GukhanmunError`. Lone Unicode surrogates in source text raise
`UnicodeEncodeError`. Invalid Unicode in constructor text options, such as
entries, directives, HTML rules, punctuation and URLs, raises `GukhanmunError`
with `ErrorCode.INVALID_INPUT`. A file path the operating system cannot encode
raises `GukhanmunError` with `ErrorCode.DICTIONARY_LOAD`.

The same converter can serve concurrent threads. Native conversion releases
the GIL and keeps each document's parser and annotation state independent.
Python signals are checked after native work completes; a long conversion
may delay `KeyboardInterrupt` until then. Streams themselves are mutable and
must each have a single consumer.
