---
title: Python API
description: Reference for Gukhanmun Python converters, immutable options, dictionary sources, punctuation profiles, streams and coded errors.
---

Python API
==========

Import public types from `gukhanmun`. The extension module `gukhanmun._native`
is private. Start with the [Python guide](../guide/python/start.md) for local
wheel installation and a complete conversion example.


Converter
---------

`Converter(...)` accepts keyword-only configuration and returns an immutable,
reusable converter. Accepted sequences are copied into owned immutable
snapshots. Dictionary bytes and files are loaded at construction time.

| Keyword                     | Type                                            | Default                             |
| --------------------------- | ----------------------------------------------- | ----------------------------------- |
| `preset`                    | `Preset`                                        | `Preset.KO_KR`                      |
| `rendering`                 | `RenderMode`                                    | `RenderMode.HANGUL_ONLY`            |
| `original_gloss`            | `OriginalGloss`                                 | `OriginalGloss.PARENS`              |
| `hanja_variant_set`         | `HanjaVariantSet`                               | `HanjaVariantSet.AS_DICTIONARY`     |
| `segmentation`              | `Segmentation`                                  | `Segmentation.LATTICE`              |
| `source_policy`             | `SourcePolicy`                                  | `SourcePolicy.REFERENCE_AUTHORIZED` |
| `reference_only`            | `bool`                                          | `False`                             |
| `numerals`                  | `NumeralStrategy`                               | `NumeralStrategy.HANGUL_PHONETIC`   |
| `initial_sound_law`         | `bool \| None`                                  | `None`, inherit preset              |
| `homophone_window`          | `ContextWindow \| None`                         | `None`, inherit preset              |
| `homophone_detection`       | `HomophoneDetection`                            | `HomophoneDetection.CONTEXT_LOCAL`  |
| `first_occurrence_window`   | `ContextWindow`                                 | `ContextWindow.OFF`                 |
| `collapse_redundant_parens` | `bool`                                          | `True`                              |
| `recovery`                  | `Recovery`                                      | `Recovery.STRICT`                   |
| `punctuation`               | `PunctuationPreset \| CustomPunctuationProfile` | `PunctuationPreset.PRESERVE`        |
| `dictionaries`              | Sequence of file, URL or map sources            | `()`                                |
| `directives`                | `Directives \| None`                            | `None`                              |
| `html`                      | `HtmlOptions \| None`                           | `None`                              |

`KO_KR` resolves `initial_sound_law=True` and
`homophone_window=ContextWindow.PER_BLOCK`. `KO_KP` resolves `False` and
`ContextWindow.OFF`. Neither preset loads official dictionaries. Original
gloss settings affect only `RenderMode.ORIGINAL`; variant selection occurs
after dictionary recognition.

The read-only `options` property returns `ResolvedOptions` with fourteen
fields: all keywords above except `original_gloss`, `dictionaries`,
`directives` and `html`. Values use their declared enum categories, and
`punctuation` is a `ResolvedPunctuationProfile` with no remaining `UNSET`
fields. The profile and nested records are frozen.


Conversion operations
---------------------

~~~~ text
convert(source: str, *, format: InputFormat | MarkdownOptions = InputFormat.TEXT) -> str
~~~~

`InputFormat.TEXT` converts plain text, `HTML` converts an HTML fragment, and
`MARKDOWN` converts CommonMark. `MarkdownOptions(gfm=False)` is equivalent to
`MARKDOWN`; `gfm=True` enables GitHub Flavored Markdown. Ruby rendering falls
back to parentheses where inline markup is unavailable.

~~~~ text
stream(*, format: InputFormat | MarkdownOptions = InputFormat.TEXT) -> ConversionStream
convert_iter(chunks: Iterable[str], *, format: InputFormat | MarkdownOptions = InputFormat.TEXT) -> Iterator[str]
~~~~

`ConversionStream.push(source: str) -> str` buffers the chunk and returns `""`.
`finish() -> str` converts the concatenated input once. A stream retains its
converter and is single-consumer. It becomes terminal after successful finish
or any push/finish failure; later calls raise `RuntimeError`.

`convert_iter()` validates the format when called, then consumes chunks when
its returned iterator advances. It skips empty outputs and finishes only after
normal upstream exhaustion. An upstream error or early iterator close does
not finalize a partial document. For every partition of an input, concatenated
stream/iterator output equals `convert()` on the complete input.

A converter may be shared across threads; streams have independent document
state. Native work releases the GIL. Pending Python signals are checked after
native work returns, so a long call can delay `KeyboardInterrupt`.


Option enums
------------

These are standard-library `enum.Enum` classes. Values equal the JavaScript
contract strings. Member names are uppercase, replacing hyphens with
underscores: `RenderMode.HANGUL_ONLY.value == "hangul-only"`. Raw strings and
members of the wrong enum class raise `TypeError`; explicitly construct an
enum to parse strings, for example `Preset("ko-kp")`.

| Enum                 | String values                                                                                                                                                                |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Preset`             | `ko-kr`, `ko-kp`                                                                                                                                                             |
| `RenderMode`         | `hangul-only`, `hangul-hanja-parens`, `hanja-hangul-parens`, `ruby-on-hangul`, `ruby-on-hanja`, `original`                                                                   |
| `OriginalGloss`      | `parens`, `ruby`                                                                                                                                                             |
| `HanjaVariantSet`    | `as-dictionary`, `shinjitai`, `kanxi`, `simplified`, `asahimoji`                                                                                                             |
| `Segmentation`       | `lattice`, `eager`                                                                                                                                                           |
| `SourcePolicy`       | `reference-authorized`, `reference-first`, `loanword-first`, `ordered`                                                                                                       |
| `SourceRole`         | `override`, `reference`, `loanword`                                                                                                                                          |
| `NumeralStrategy`    | `hangul-phonetic`, `positional-arabic`, `additive-arabic`, `smart`                                                                                                           |
| `ContextWindow`      | `off`, `per-block`, `per-section`, `per-document`                                                                                                                            |
| `HomophoneDetection` | `context-local`, `dictionary-wide`                                                                                                                                           |
| `Recovery`           | `strict`, `lenient`                                                                                                                                                          |
| `PunctuationPreset`  | `preserve`, `modern`, `traditional`, `traditional-vertical`                                                                                                                  |
| `SpaceAfter`         | `preserve`, `remove`, `single`                                                                                                                                               |
| `DictionaryFormat`   | `fst`, `cdb`                                                                                                                                                                 |
| `InputFormat`        | `text`, `html`, `markdown`                                                                                                                                                   |
| `ErrorCode`          | `dictionary-load`, `segmentation`, `invalid-reading`, `html-scan`, `html-malformed-attr`, `markdown`, `unsupported-content-type`, `invalid-input`, `io`, `internal`, `other` |


Dictionary sources and entries
------------------------------

`DictionaryEntry(hanja, reading, suffix_reading=None, preference=0, require_hanja=False, require_hangul=False)`
is a frozen record. `hanja` and `reading` are required and nonempty. A supplied
`suffix_reading` is also nonempty. `preference` is a non-boolean integer from 0
through 4294967295. Repeated hanja keys remain competing readings: higher
preference wins by default, with reading spelling breaking ties. A following
Korean particle can select a compatible candidate. `suffix_reading` provides
the non-word-initial form. `require_hanja` and `require_hangul` request hanja
or hangul annotations, respectively.

`MapDictionarySource(entries, role=SourceRole.OVERRIDE)` snapshots its sequence
into a tuple. An empty source is valid. Authored overrides take precedence over
cooperative reference and loanword sources.

`FileDictionarySource(data, format, role=None)` accepts `bytes`, `bytearray`,
`memoryview`, a string path or `os.PathLike[str]`. Mutable buffers and logical
memoryview contents are copied; unreadable/released buffers raise
`INVALID_INPUT`. Paths remain Python filesystem values, including valid
surrogateescape paths, and are read during converter construction.

`UrlDictionarySource(url, format, role=None, timeout=30.0)` explicitly requests
an HTTP(S) download during converter construction. Malformed URLs, invalid
ports, forbidden schemes and forbidden redirect destinations raise
`INVALID_INPUT`. The finite positive timeout applies to I/O operations, not
the entire request. Proxy environment variables are not used.

Every binary source needs `DictionaryFormat.FST` or `CDB`. Role precedence is
explicit role, embedded source-evidence role, then `OVERRIDE`. Binary source
and phrase evidence remain attached to their dictionaries.

`SourcePolicy.REFERENCE_AUTHORIZED` uses reference evidence to authorize
alternate forms. `REFERENCE_FIRST` and `LOANWORD_FIRST` choose their stated
preference. `ORDERED` selects by source registration order and ignores roles
and `reference_only`. Under cooperative policies, `reference_only=True`
excludes independent loanword choices while retaining reference-authorized
forms. Source roles alone do not create lexical identities or authorize forms.


Directives and HTML preservation
--------------------------------

`Directives(require_hanja=(), require_hangul=(), skip_annotation=())` stores
three literal-hanja tuples. Rules are registered in that order and affect
annotation rendering. Python callbacks are not accepted.

`HtmlOptions(preserve_classes=(), preserve_attributes=())` preserves an element
and its descendants whenever any rule matches. Classes are case-sensitive
ASCII-whitespace-separated tokens. Each attribute rule is an
`AttributeMatcher(name, value=None)`: `None` checks presence, while a string
checks the exact decoded value. Attribute names are compared without ASCII
case distinctions; values remain case-sensitive. The first duplicate
attribute determines the match. A rule matches only a complete attribute name,
including non-ASCII names. Values decode numeric character references and the
named references `amp`, `lt`, `gt`, `quot` and `apos`.


Punctuation profiles
--------------------

`StopStyle(mark, after)` selects one Unicode scalar and a `SpaceAfter` policy
for following U+0020 spaces. `DelimiterStyle(open, close)` selects opening and
closing punctuation scalars. Native validation checks categories and
collisions when constructing the converter; output styles do not expand the
set of recognized input punctuation.

`CustomPunctuationProfile(base=PunctuationPreset.PRESERVE, comma=UNSET, period=UNSET, single=UNSET, double=UNSET)`
has three override states:

| Value                        | Effect                                 |
| ---------------------------- | -------------------------------------- |
| `UNSET`                      | Inherit the base profile for that role |
| `None`                       | Preserve the role's input punctuation  |
| `StopStyle`/`DelimiterStyle` | Use that explicit output style         |

`UNSET` is the sole member of `UnsetType`; copying or pickling it preserves
its identity. Comma/period accept `StopStyle`, and single/double accept
`DelimiterStyle`. The resolved profile has the same four role fields, each
containing the appropriate style or `None`.


Errors
------

`GukhanmunError(code, message, chain=())` exposes `code: ErrorCode`,
`message: str` and an immutable `tuple[ErrorCause, ...]`.
`ErrorCause(code, message)` is a frozen record. Causes are ordered from the
deepest/root cause to the immediate cause.

| Failure                                              | Python result                                  |
| ---------------------------------------------------- | ---------------------------------------------- |
| Wrong argument type, enum category or required field | `TypeError`                                    |
| Invalid option, entry, punctuation or URL value      | `GukhanmunError` with `INVALID_INPUT`          |
| File/URL operational failure                         | `DICTIONARY_LOAD`, with root-first `IO` causes |
| Invalid binary dictionary                            | `DICTIONARY_LOAD`                              |
| Invalid Unicode in constructor text options          | `INVALID_INPUT`                                |
| File path the operating system cannot encode         | `DICTIONARY_LOAD`                              |
| Lone surrogate in source text or stream input        | `UnicodeEncodeError`                           |
| Unexpected native Rust panic                         | `INTERNAL`                                     |
| Python interruption                                  | Original `KeyboardInterrupt` propagates        |

All eleven error code values are retained even when a particular code is not
normally produced by this binding. Native conversion mappings match Node-API:
core errors map to `SEGMENTATION`, HTML to `HTML_SCAN`, Markdown to `MARKDOWN`,
FST/CDB to `DICTIONARY_LOAD`, I/O to `IO`, configuration to `INVALID_INPUT`,
and unclassified native errors to `INTERNAL`.


JavaScript correspondence
-------------------------

*packages/types/index.ts* defines the behavior baseline. Python uses constructor
keywords instead of the async `load()` factory, and explicit enums instead of
implicit string options.

| JavaScript contract                                                                                                | Python equivalent                                               |
| ------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| `load(options)`                                                                                                    | `Converter(**options)`                                          |
| `preset`, `rendering`, `segmentation`, `numerals`, `recovery`, `punctuation`, `dictionaries`, `directives`, `html` | Same keyword names                                              |
| `originalGloss`, `hanjaVariantSet`                                                                                 | `original_gloss`, `hanja_variant_set`                           |
| `sourcePolicy`, `referenceOnly`, `initialSoundLaw`                                                                 | `source_policy`, `reference_only`, `initial_sound_law`          |
| `homophoneWindow`, `homophoneDetection`                                                                            | `homophone_window`, `homophone_detection`                       |
| `firstOccurrenceWindow`, `collapseRedundantParens`                                                                 | `first_occurrence_window`, `collapse_redundant_parens`          |
| `convert(source, format)`                                                                                          | `convert(source, format=...)`                                   |
| `Format` strings/Markdown object                                                                                   | `InputFormat`/`MarkdownOptions`                                 |
| `TransformStream` returned by `stream()`                                                                           | `ConversionStream` with `push()`/`finish()`                     |
| `options`                                                                                                          | Frozen `ResolvedOptions`                                        |
| `DictionaryEntry.suffixReading`, `requireHanja`, `requireHangul`                                                   | `suffix_reading`, `require_hanja`, `require_hangul`             |
| File source `data`, `format`, `role`                                                                               | `FileDictionarySource`; explicit URLs use `UrlDictionarySource` |
| Map source `entries`, `role`                                                                                       | `MapDictionarySource`                                           |
| `requireHanja`, `requireHangul`, `skipAnnotation` directives                                                       | `Directives.require_hanja`, `require_hangul`, `skip_annotation` |
| `preserveClasses`, `preserveAttributes`                                                                            | `HtmlOptions.preserve_classes`, `preserve_attributes`           |
| Attribute string or `{name, value}`                                                                                | `AttributeMatcher(name, value=None)`                            |
| Custom punctuation omitted/null/style                                                                              | `UNSET`/`None`/style record                                     |
| `GukhanmunError.code`, `message`, `chain`                                                                          | Typed code, message and frozen root-first cause tuple           |

The Python convenience method `convert_iter()` adapts an iterable of string
chunks to the buffered stream contract.
