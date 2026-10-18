---
title: Punctuation profiles
description: Normalize commas, periods, and explicit citation delimiters independently of hanja rendering.
---

Punctuation profiles
====================

Punctuation normalization is opt-in and works even when the input contains no
hanja. It runs after reading selection and annotation directives, before
rendering. Language presets, dictionaries, and rendering modes do not select a
punctuation profile implicitly.

| Profile                | Comma/period | Single/double delimiters | Following U+0020        |
| ---------------------- | ------------ | ------------------------ | ----------------------- |
| `preserve` (default)   | unchanged    | unchanged                | unchanged               |
| `modern`               | `,` `.`      | `〈〉` `《》`            | contextual single space |
| `traditional`          | `、` `。`    | `「」` `『』`            | removed                 |
| `traditional-vertical` | `︑` `︒`    | `﹁﹂` `﹃﹄`            | removed                 |

The `traditional-vertical` profile emits explicit presentation characters. It
does not configure CSS writing mode or font support.


Selecting a profile
-------------------

~~~~ rust
use gukhanmun::{Builder, PunctuationProfile};

let converter = Builder::new()
    .no_bundled_dictionaries()
    .punctuation(PunctuationProfile::modern())
    .build()?;
assert_eq!(converter.convert_text_to_string("첫째、둘째。")?, "첫째, 둘째.");
~~~~

Both JavaScript bindings accept the same `punctuation` option:

~~~~ typescript
import { load } from "@gukhanmun/wasm"; // or @gukhanmun/napi
const converter = await load({ punctuation: "traditional" });
converter.convert("첫째, 둘째."); // "첫째、둘째。"
~~~~

The CLI accepts a preset name, for example `--punctuation modern`. For a custom
JSON configuration, use `--punctuation-profile profile.json` instead. These
flags are mutually exclusive.


Custom profiles
---------------

A custom configuration inherits `base`, defaulting to `preserve`. Omitted roles
inherit from `base`. An explicit `null` preserves that role. JavaScript
`undefined` properties are omitted before crossing the binding boundary.

~~~~ json
{
  "base": "modern",
  "period": null,
  "comma": { "mark": "、", "after": "remove" },
  "single": { "open": "︿", "close": "﹀" }
}
~~~~

Pass this object as the JavaScript `punctuation` option, or save it to a file
and pass that file to `--punctuation-profile`. Rust
callers can modify the public fields of `PunctuationProfile` using `StopStyle`
and `DelimiterStyle` constructors. With the optional `serde` feature,
`PunctuationConfig` parses the same configuration and `resolve()` validates it.
`builder.build()` validates directly constructed profiles too.

Each mark must be one Unicode scalar, including supplementary-plane characters.
Opening and closing delimiters must have Unicode 17.0 categories Ps/Pi and
Pe/Pf respectively. Stops accept punctuation and symbol categories except Ps,
Pi, Pe, and Pf. Roles must be distinct and must not collide with another role's
recognized input. Whitespace, letters, numbers, controls, `<`, `>`, `&`, and
U+FFFD are rejected. Unknown presets, fields, and space policies are errors.
Custom marks outside the fixed input alphabet are not recognized as new input
styles on later conversions.

The `after` policy is `preserve`, `remove`, or `single`. Only ordinary U+0020
spaces change. Newlines, tabs, nonbreaking spaces, and ideographic spaces remain
unchanged. `single` inserts one space before ordinary content and opening
delimiters; it trims spaces before closing delimiters, block/line boundaries,
and EOF. Ambiguous symbols and preservation barriers keep existing spaces
without adding one. Lookahead crosses transparent inline scopes and places the
space outside their shared boundary. If lookahead would cross more than 256
structural events for one stop, the `single` policy keeps existing spaces
without inserting one.

JavaScript's `converter.options.punctuation` exposes a copied, deeply frozen
resolved profile with all four roles present; a preserved role is `null`.


Recognized input and exclusions
-------------------------------

All output profiles recognize the same comma/period forms, including ASCII,
fullwidth, ideographic, and vertical forms. Delimiter inputs include `〈〉`,
`《》`, `「」`, `『』`, their vertical forms, and U+2329/U+232A. Mathematical
U+27E8/U+27E9 are excluded. Delimiters are mapped individually; unmatched pairs
are not repaired. ASCII substitutes such as `<>` and ordinary quotation marks
are not inferred. Dashes and ellipses are outside this feature.

Conservative guards protect ASCII and fullwidth stops between digits, runs of
two or more period forms, and ASCII/fullwidth periods next to Latin letters or
digits. ASCII/fullwidth commas between alphanumerics, or between an
alphanumeric and spaces followed by a Latin letter, also remain unchanged. Thus
`Hello, world`, `1,000`, `3.14`, and `example.com` survive, while
`IMF, 세계은행` can be normalized. Case-insensitive `http://`, `https://`,
`mailto:`, and `www.` runs are protected until Unicode whitespace or a
structural/annotation boundary. These rules deliberately favor preserving
technical text over converting every stop. Latin guards include Unicode
Latin-script letters and retain their base across combining marks. In `single`
spacing, combining marks, variation selectors, and format controls keep existing
spaces without causing insertion, so a new space cannot detach them from a stop.


HTML and Markdown
-----------------

HTML recognizes WHATWG named, decimal, and hexadecimal character references in
text, including standard semicolonless forms and numeric replacement rules.
Untouched references keep their original spelling. Eligible encoded marks and
spaces normalize just like literal characters. Decoding happens once:
`&amp;period;` remains literal text, and `&lt;script&gt;` never becomes markup.
`&lang;`/`&rang;` are mathematical delimiters and remain outside normalization.
Attributes, code, foreign-language scopes, and other preserved regions are not
normalized. Nested `lang="ko"` scopes can re-enable normalization according to
the reader's effective language policy. Ruby and unknown inline elements form
spacing barriers. Known block, list, and table elements trim contextual spaces;
this punctuation boundary classification does not change annotation windows or
inspect CSS. Newly
generated HTML `;`, `#`, `!`, `?`, and `/` use numeric references to avoid
recombining adjacent source text into a different character reference or markup.

Entity decoding is used only by this middleware. Encoded hanja and punctuation
are not passed back into the reading engine, so equivalent entity spellings do
not guarantee identical reading selection. Streaming keeps unfinished references
across text chunks. Numeric references may have arbitrarily many leading zeros;
processing is linear but their raw spelling can require unbounded buffering.

Markdown text is already decoded by its reader and is not decoded again. Raw
HTML and images remain protected. With normalization enabled, the writer uses
numeric references for ordinary ASCII punctuation and trailing spaces before a
soft break, preventing accidental lists, headings, links, or hard breaks. It
uses local `<em>`, `<strong>`, and `<del>` tags to preserve emphasis regardless
of delimiter flanking after transformation. Changed autolink labels become
ordinary links that keep the original destination. Markdown source
spelling can therefore change while rendered structure remains intact.

Escaping can disable a hosting site's implicit bare-URL, mention, and
issue-reference linking. Use explicit Markdown links or CommonMark autolinks
when their destinations must be retained. These host-specific extensions are
outside the parser's supported GFM subset.


Low-level pipelines
-------------------

`PunctuationNormalizer` accepts `OutputToken` values after annotation policy
and before `Renderer`. Call upstream `finish()` methods and drain their output
first, then call the normalizer's `finish()`. An I/O flush is not EOF.
`Annotated` payloads and `Verbatim` regions are never rewritten.

`Converter::convert_tokens()` uses literal text. For HTML tokens, use
`convert_tokens_with_codec(tokens, HtmlPunctuationTextCodec::new())` and reader
options from `converter.html_reader_options()`; these preserve unmatched end
tags as opaque markup. Rootless fragments still need the codec. Markdown
callers serializing `convert_markdown_iter()` must use
`write_markdown_with_options(tokens, converter.markdown_write_options())`.
The buffered facade methods select these options automatically.
