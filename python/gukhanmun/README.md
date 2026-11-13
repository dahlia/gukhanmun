Gukhanmun for Python
====================

Gukhanmun converts Korean text containing hanja into hangul. Its Python API
uses a Rust engine and supports plain text, HTML and Markdown. Create a
converter once and reuse it for multiple documents.


Installation
------------

Requires Python 3.11 or later:

~~~~ sh
python -m pip install gukhanmun
~~~~

To request development releases:

~~~~ sh
python -m pip install --upgrade --pre gukhanmun
~~~~

A fresh install prefers a compatible stable release. With pip 26.0 or later,
the installer can also select a development release when no stable candidate
satisfies the requirement. Without `--upgrade`, pip keeps an installed version
that already satisfies the request.

Available wheels and data packages depend on the published version. See
[*Supported platforms*] for wheel
targets and source-build requirements. Building from source needs Rust and a
platform C toolchain.

[*Supported platforms*]: https://gukhanmun.org/guide/python/platforms


Convert a document
------------------

This example supplies its own reading for one word and needs no optional data:

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[
    MapDictionarySource(entries=[DictionaryEntry(hanja="漢字", reading="한자")]),
])

assert converter.convert("漢字를 한글로") == "한자를 한글로"
assert converter.convert("<p>漢字</p>", format=InputFormat.HTML) == "<p>한자</p>"
markdown = converter.convert("# 漢字", format=InputFormat.MARKDOWN)
~~~~

Plain text is the default. Without explicit dictionary sources, conversion
uses built-in character readings. Installing a data package does not
automatically add a dictionary to a converter.

The optional `stdict`, `opendict` and `loanwords` extras select separate
official data packages pinned to the core package's exact version. Add their
sources explicitly with the functions in `gukhanmun.dictionaries`. For example,
import `stdict` from that module and use `Converter(dictionaries=[stdict()])`.
See [*Official dictionaries*] for installation, available formats and data
licenses.

The [Python guide] and
[API reference] cover rendering options,
compiled dictionaries and errors.

[*Official dictionaries*]: https://gukhanmun.org/guide/python/dictionaries
[Python guide]: https://gukhanmun.org/guide/python/start
[API reference]: https://gukhanmun.org/api/python


License
-------

GNU General Public License version 3 only (`GPL-3.0-only`). See the
[license text].

[license text]: https://github.com/dahlia/gukhanmun/blob/main/LICENSE
