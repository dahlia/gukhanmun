gukhanmun
=========

*Also available: [韓國語](DESIGN.ko-Kore.md) (Korean).*

gukhanmun is a library for converting Korean text written in mixed script
(國漢文混用體) into hangul-only text. It is the successor to [Seonbi], narrowed
in scope to the hanja conversion pipeline and broadened along several axes:
streaming I/O, pluggable dictionaries, lattice-based segmentation, and a wider
range of output formats. The project is implemented in Rust and exposed as a
Rust library, a command-line tool, WebAssembly bindings, and Node-API bindings.

[Seonbi]: https://github.com/dahlia/seonbi


Goals and non-goals
-------------------

The library converts hanja words in Korean text into their hangul readings,
optionally annotating them with the source hanja for disambiguation, ruby
markup, or stylistic reasons. It does so without disturbing structure or
content outside the text it is asked to transform; regions marked as non-Korean
or as preserved (code blocks and the like) pass through untouched. It streams
input and output where possible, with buffering bounded by the size of
contiguous conversion spans that contain hanja and by opt-in cross-context
disambiguation. It accepts pluggable dictionaries that may be in-memory,
mmap-backed, or otherwise opaque to the engine. It ships a default dictionary
derived from the South Korean *Standard Korean Language Dictionary*
(標準國語大辭典). It is usable from Rust, from Node.js via [Node-API], from
browsers and Deno via WebAssembly, and from a command line.

The library deliberately does not provide broader Korean typographic
adjustments such as smart quotes, dashes, ellipses, or citation marks; those
remain Seonbi's job. It does not provide a fully HTML5-conformant parser; the
HTML scanner is fragment-oriented and recovers from minor malformations but is
not a substitute for html5ever. It does not roundtrip Markdown byte-for-byte;
semantic preservation is the contract, and original-form preservation is best
effort. It does not translate between languages or transliterate beyond the
hanja-to-hangul mapping.

[Node-API]: https://nodejs.org/api/n-api.html


Design principles
-----------------

Three principles run through the design.

The engine is *format-neutral*. The same engine processes HTML, Markdown, and
plain text because each format is read into a single intermediate
representation and written back from it. Adapters at the boundary handle format
specifics: scanning, serialization, and the format-specific notion of what
counts as a preserved region or a block boundary. The engine never inspects
HTML tag names, Markdown event variants, or anything else that would couple it
to one format. This is what makes a single test fixture meaningful across all
three formats, and what makes adding a new format a contained piece of work.

*Responsibilities are split into independently selectable pipeline stages*. The
*Reader* parses an input format into the IR. The *Engine* finds
hanja-containing lexical spans, segments them, and emits annotations.
*Middlewares* transform the IR stream by adjusting
annotation flags. The *Renderer* turns annotations into concrete text or
markup. The *Writer* serializes the IR back to the target format. A user
replacing one stage does not perturb the others; the boundaries are explicit
values, not method calls.

*Streaming is the default*. Operations that fundamentally require buffering,
notably per-document homophone disambiguation, are opt-in modes. The default
configuration buffers only within a contiguous conversion span that contains
hanja (usually a handful of characters in typical text) and within a single
text token. This makes gukhanmun usable inside servers that process large
documents or live streams, and it makes the WebAssembly build a viable target
for in-page conversion of long articles.


Architecture
------------

The pipeline has five stages.

~~~~ mermaid
flowchart LR
    Bytes -->|Reader| InTok[InputToken stream]
    InTok -->|Engine| OutTok[OutputToken stream]
    OutTok -->|Middlewares| OutTok2[OutputToken stream]
    OutTok2 -->|Renderer| OutTok3[OutputToken stream]
    OutTok3 -->|Writer| Bytes2[Bytes]
~~~~

A reader parses bytes into a stream of input tokens. The engine reads input
tokens and emits output tokens, with the difference that output tokens may
include *annotation* tokens carrying both an original hanja form and its hangul
reading. Middlewares walk the output stream and adjust the flags on those
annotations. The renderer expands each annotation into concrete text or markup
according to the flags and the chosen rendering mode. The writer serializes the
final stream into the output format.

### Intermediate representation

The intermediate representation is a flat stream of tokens parameterized by a
*scope data* type that belongs to the adapter rather than the engine. The
engine knows the shape of a token, but the contents of the scope payload (raw
HTML attributes, the variant of a pulldown-cmark event, and so on) are opaque
to it.

The token sent into the engine is one of:

 -  `Open(Scope<S>)`: enter a structural scope, such as an HTML element or a
    Markdown block. The `Scope<S>` carries the adapter's opaque data plus three
    pre-computed flags described below.
 -  `Close`: leave the most recent scope. The engine maintains the stack
    itself; the adapter does not have to repeat which scope is closing.
 -  `Text(Cow<str>)`: a chunk of text that the engine may transform.
 -  `Verbatim(Cow<str>)`: a chunk of text that must pass through untouched,
    such as the contents of an HTML `<code>` element or a Markdown code span.
    The adapter, not the engine, decides what is verbatim.

The token coming out of the engine is one of:

 -  `Open(Scope<S>)`, `Close`, `Text(Cow<str>)`, `Verbatim(Cow<str>)`: the same
    forms, passed through.
 -  `Annotated(Annotation)`: a position in the stream where the engine
    converted a hanja word. The annotation carries the original hanja, the
    hangul reading, and flags describing why the conversion happened and what
    subsequent stages might want to do with it.

The `Annotation` carries five flags. `homophone` is set when the dictionary
indicates another hanja word shares this hangul reading, so a reader of the
rendered output cannot recover the word from hangul alone. `require_hanja` is
set when the source dictionary or a user directive demands the original hanja
be shown alongside the hangul. `require_hangul` is set in the opposite
direction: the source preserves the hanja in the output, but a hangul gloss is
required (used for the *Original* rendering mode that keeps mixed script as the
default presentation). `first_in_context` is set when this is the first
occurrence of the hanja word within the current context window, where the
window is a block, a section, or the document depending on configuration.
`from_dictionary` distinguishes a dictionary match from a
character-by-character fallback; renderers may choose to mark these differently
for debugging.

The split between `InputToken` and `OutputToken` is the most consequential
shape decision in the IR. The alternative we considered was a single `Token`
enum that includes `Annotated` from the start, with adapters emitting
`Annotated` only as a future possibility. We rejected it for two reasons.
First, it would force the input-side `Annotated` variant to remain a no-op
throughout the reader and engine, polluting every `match` in the codebase with
an unreachable arm. Second, it would obscure the contract that the engine
*produces* annotations and renderers *consume* them. Two distinct types make
the dataflow legible at a glance and let the type system enforce that an
unrendered annotation cannot reach a writer.

### Scope data

The opaque scope payload obeys a small trait:

~~~~ rust
pub trait ScopeData: Clone + 'static {
    /// Should text inside this scope be left alone?
    fn is_preserve(&self) -> bool;

    /// May the renderer insert inline markup, such as a `<ruby>` element,
    /// inside this scope?
    fn allows_inline_markup(&self) -> bool { true }

    /// Does this scope mark a boundary at which per-block middlewares reset?
    fn is_block_boundary(&self) -> bool { false }
}
~~~~

The engine treats `is_preserve()` as the single point of truth for whether to
skip a `Text` token: it walks the scope stack and asks each scope's
`is_preserve()`. The HTML adapter's `ScopeData` implementation aggregates
several concerns into that one flag: the `lang` attribute (inherited from
ancestors and compared against a Korean predicate), the tag name (against the
preserved-tag list of `pre`, `code`, `kbd`, `script`, `style`, and `textarea`),
and any user-supplied predicate over raw attributes. The engine itself does not
know what a `lang` attribute is.

This is a departure from Seonbi, which threads a parallel `LangHtmlEntity`
annotation through the pipeline. We removed it because the engine never needs
both signals separately; it only ever asks the single question, is this text
skipped? Pushing lang inheritance into the adapter (where the tag-name
preservation list already lives) keeps the engine ignorant of HTML and keeps
the IR free of fields the engine does not consume.


Engine
------

The engine has three duties: identify convertible lexical spans that contain
hanja, segment those spans into dictionary and fallback edges, and emit
annotations and fallback text accordingly. Most spans are contiguous runs of
hanja, but dictionary entries may also include Korean native or hangul
fragments around hanja, as in `汽車길` or `色깔論`. It also has a fallback
phoneticizer that maps single hanja characters to hangul and applies the
initial sound law where appropriate, and a numeral converter that translates
hanja digits to either hangul or Arabic numerals depending on configuration.

### Lattice segmentation

Segmenting a hanja-containing span into dictionary words and fallback fragments
is the engine's core decision, and the obvious algorithm is wrong.

The obvious algorithm is *eager longest-match*: scan left to right, at each
position take the longest dictionary entry that starts there, and continue from
its end. This is what Seonbi does. Its failure mode is mis-segmentation when a
longer prefix consumes characters that belong to a more natural split. Consider
a dictionary containing `行事`, `行事場`, `場所`, and `入口`. On the input
`行事場入口`, eager segmentation produces `行事場` + `入口`, which is correct.
On the input `行事場所`, eager segmentation produces `行事場` + `所`, which
leaves `所` to the character-by-character fallback even though the segmentation
`行事` + `場所` would have covered both segments with the dictionary.

The span is not limited to hanja-only text. Some entries in the Standard Korean
Language Dictionary mix native Korean and hanja, such as `汽車길` for
`기찻길`, `祭祀날` for `제삿날`, `洗手대야` for `세숫대야`, `火김` for
`홧김`, and `色깔論` for `색깔론`. A dictionary edge may therefore consume a
mixed-script prefix from the current text cursor. Fallback edges, however, are
still created only for hanja characters that no dictionary edge covers; hangul
that is not part of a dictionary match passes through as ordinary text.

The correct algorithm is dynamic programming over a lattice. For each character
position `i` in the conversion span, the engine queries the dictionary for
every match that starts at `i` and considers a single-hanja fallback edge as a
backup when the current character is hanja. A ranking function compares the
alternatives; we choose the best segmentation by Viterbi-style backtracking
from the end of the span.

The ranking function is deliberately simple. It first maximizes the number of
characters covered by dictionary matches, so `行事` + `場所` beats `行事場` +
`所` because the former leaves no fallback. Among paths with the same
dictionary coverage, it prefers fewer segments, so a whole-word match such as
`天地` beats the component split `天` + `地`. Remaining ties are kept
deterministic by preserving the first candidate that reached the same score.

The cost of lattice segmentation is bounded by the conversion-span length
times the maximum dictionary entry length. In normal Korean text, spans that
contain hanja are short: most are one to four characters, almost never more
than ten, except for rare mixed-script dictionary entries. The dictionary's
maximum entry length is on the order of ten characters. The per-span runtime is
therefore tens of dictionary lookups, which the FST and CDB backends handle in
microseconds.

An *eager* segmentation strategy remains available as an option for callers who
do not need lattice accuracy and want to reduce per-span overhead. The default
is lattice.

### Fallback phoneticizer

When no dictionary entry covers a hanja character, the fallback phoneticizer
converts the character to hangul by looking up its canonical reading in the
embedded Unihan-derived character map. The character map is built from the
Unicode `kHangul` property and embedded in *gukhanmun-core* as a perfect-hash
table; it covers approximately ten thousand hanja and adds roughly fifty
kilobytes to the binary.

For the first character of a word produced by the fallback, that is, the first
character of a fallback-only run or the first character after a dictionary
match, the initial sound law (頭音法則) optionally applies. The law converts a
small set of word-initial hangul syllables that originally began with ㄴ or ㄹ
into their South Korean orthography forms (녀 becomes 여, 려 becomes 여, 례
becomes 예, and so on). The conversion table is small (sixteen entries) and
lives in *gukhanmun-core*. The toggle for the law applies only to the fallback;
dictionary entries are assumed to encode the correct reading already (a South
Korean dictionary stores `來日` as 내일, a North Korean dictionary stores it as
래일).

Two small rules from Seonbi survive in the fallback because they are not
learnable from per-character mappings alone. First, the *ryeol*-*ryul* rule
(`列`, `律`): when these characters follow a syllable whose final jamo is ㄴ or
absent, their pronunciation glides to 열 or 율 rather than 렬 or 률 even though
the initial sound law as such does not apply. Second, the hanja-numeral rule: a
sequence of two or more hanja digits is read as a single word with the initial
sound law applied at its head and not at internal positions. Both rules are
encoded as small parsers over the character stream.

### Numeral conversion

Hanja numerals are a special case because they can be converted to three
different surface forms and the right one depends on context. The library
exposes a `NumeralStrategy` option with four variants:

| Strategy            | Example: `二〇一六年` | Example: `十一月` | Example: `一千二百三十四` |
| ------------------- | --------------------- | ----------------- | ------------------------- |
| `hangul-phonetic`   | 이공일륙년            | 십일월            | 일천이백삼십사            |
| `positional-arabic` | 2016년                | (not applicable)  | (not applicable)          |
| `additive-arabic`   | (not applicable)      | 11월              | 1234                      |
| `smart`             | 2016년                | 11월              | 1234                      |

The `hangul-phonetic` strategy is Seonbi's behavior and the default for both
the `ko-kr` and `ko-kp` presets. The `positional-arabic` strategy treats a
digit-only sequence (`〇一二三四五六七八九`, plus their variants) as positional
notation and converts it to Arabic. The `additive-arabic` strategy parses
sequences that contain place markers (`十百千萬億兆京`) using stack-based
accumulation and produces Arabic, handling the Korean convention that `一` is
elidable before `十` (`十一` means 11, not `一十一`). The `smart` strategy
looks at the surrounding context: if a unit hanja follows (`年月日時分秒號世紀`
and so on), it uses `additive-arabic`; if not, and the run is pure digits of
length four or more, it uses `positional-arabic` (matching the year
convention); otherwise it falls back to `hangul-phonetic`.

Numeral conversion runs inside the fallback path on segments that the lattice
has identified as not matching the dictionary. The output is plain text rather
than an `Annotated` token, since there is no hanja-versus-hangul ambiguity
worth preserving for the renderer to handle later.


Dictionaries
------------

A dictionary is anything that can be asked what matches start at a given text
position.

~~~~ rust
pub trait HanjaDictionary {
    /// Yield every match that starts at the beginning of `s`.
    fn matches_at<'a>(&'a self, s: &'a str)
        -> Box<dyn Iterator<Item = Match<'a>> + 'a>;

    /// Greatest entry length, in characters, that this dictionary contains.
    /// Used as the lattice termination bound.
    fn max_word_chars(&self) -> Option<usize> { None }

    /// Is there another hanja word with the same hangul reading?
    /// Used by the homophone-marker middleware.
    fn has_homophone(&self, hanja: &str, reading: &str) -> bool { false }
}
~~~~

`matches_at` is the unusual part. The natural signature for a Korean text
application would be `longest_match`, but eager longest-match is exactly the
algorithm we reject in the engine. The trait surfaces *every* match starting at
a position because the lattice segmenter needs the full set in order to score
alternatives. The input string is the text suffix at the current cursor, not
only a pre-cut hanja run; this lets dictionaries contain mixed-script keys such
as `汽車길` as long as the match itself contains at least one hanja character.

A `Match` carries the matched byte length, the hangul reading, and a
`MatchMark`. `byte_len` is the UTF-8 length of the matched dictionary key,
which may include both hanja and hangul:

~~~~ rust
pub struct Match<'a> {
    pub byte_len: usize,
    pub reading: Cow<'a, str>,
    pub mark: MatchMark,
}

pub struct MatchMark {
    /// The dictionary asserts this entry should always be shown with hanja.
    pub require_hanja: bool,
    /// The dictionary asserts this entry should always be shown with hangul.
    pub require_hangul: bool,
}
~~~~

The marks come from the source dictionary's build-time metadata. The bundled
*Standard Korean Language Dictionary*'s CDB and FST files are accompanied by a
rules file that enumerates hard-to-read characters and ambiguous readings that
should be hanja-annotated.

### Built-in implementations

`UnihanCharDict` is the per-character fallback, exposed as a `HanjaDictionary`
so that the engine's interface is uniform: every lookup goes through
`matches_at`, including the single-character base case. The implementation is a
perfect-hash table generated at build time from the Unicode `kHangul` property.

`MapDictionary` is the in-memory dictionary used for user-supplied entries and
small custom vocabularies. It is backed by an `fst::Map` rather than a hash
map: the FST gives ordered prefix iteration in microseconds and compresses
well, which matters in WebAssembly bundles.

`ChainDictionary` composes a sequence of dictionaries with a precedence policy.
A typical configuration chains a small user dictionary (highest priority), a
domain-specific dictionary, the *Standard Korean Language Dictionary*, and
`UnihanCharDict` (lowest priority).

### External backends

Two external dictionary backends ship as separate crates.

*gukhanmun-cdb* wraps a CDB file as a `HanjaDictionary`. CDB is djb's constant
database: a static on-disk hash table with $O(1)$ lookups and trivial format
documentation. The naive use of CDB as a hanja-to-hangul map fails because CDB
is a hash table without prefix iteration; there is no way to ask whether some
key starts with a given byte sequence. We work around this by encoding the
dictionary as a *trie embedded in the CDB key space*. At build time,
*gukhanmun-mkdict* enumerates every prefix of every entry and stores a record
for each, with a one-byte flag distinguishing complete words from intermediate
prefixes:

~~~~ text
key   = utf-8 bytes of a prefix
value = { is_complete: u8, mark: u8, reading_len: u16, reading: utf-8 }
~~~~

Lookup walks one character at a time from the cursor position. On a miss, no
longer match is possible from this position and the walk terminates. On a hit
with `is_complete = 1`, the match is yielded; on a hit with `is_complete = 0`,
the walk continues to look for longer matches. The cost is
$O(\\text{max\_word\_chars})$ CDB lookups per position, and each CDB lookup is
$O(1)$. The size cost is real: every prefix of every entry occupies a record,
so the bundled stdict CDB is roughly twice the size of the source TSV. The
trade-off is that CDB's simplicity (six syscalls of file format, public-domain
reference implementations) makes the backend trivially auditable.

*gukhanmun-fst* wraps an `fst::Map` as a `HanjaDictionary`. The FST (finite
state transducer) supports prefix iteration natively and compresses better than
CDB-as-trie, but its on-disk format is less universally implemented. We provide
both because users have different priorities; CDB is the choice for
code-auditability and trivial mmap support, while FST is the choice for small
WebAssembly bundles.

### Dictionary tooling

*gukhanmun-mkdict* is a separate CLI for building CDB and FST dictionary files.
It accepts TSV, CSV, and JSON Lines input, supports merging multiple input
files with a configurable conflict policy, validates the result with a
round-trip pass, and embeds build metadata (source, license, build date) in the
dictionary header.

~~~~ text
gukhanmun-mkdict [OPTIONS] -o OUTPUT <INPUT>...

INPUT FORMATS:
    tsv     hanja TAB hangul [TAB flags]
    csv     hanja,hangul[,require_hanja,require_hangul]
    jsonl   {"hanja":..., "hangul":..., "requireHanja":..., ...}

OPTIONS:
    -o, --output PATH               base output path
    -f, --format FMT                cdb|fst|both       (default: both)
        --merge STRATEGY            first-wins|last-wins|error
        --metadata KEY=VAL          embedded metadata
        --validate                  round-trip verification
        --max-key-bytes N           reject pathologically long entries
~~~~

The same binary is invoked by *gukhanmun-stdict*'s build script to produce the
bundled *Standard Korean Language Dictionary* CDB and FST files, so end-users
and the library itself share the build path. Bugs in the build path get caught
by the library's own integration tests before they reach users.


Middlewares
-----------

The renderer's input is an `OutputToken` stream where each `Annotated` token
carries flags. The flags describe what the engine knows about the annotation:
was it from the dictionary, is there a homophone, is this the first time we
have seen this word. What they do not describe is what the renderer should do
about it; that is set by middlewares.

Splitting policy (which annotations should be presented with hanja, which with
hangul only, when does “first” reset) from form (parentheses, ruby, hangul
only) gives us a cleaner pipeline than Seonbi has. In Seonbi, the rendering
function decides both what to present and how. As a result, the
homophone-disambiguating renderer must internally re-implement a
homophone-detecting pass, and there is no way to swap in a different homophone
heuristic without rewriting the renderer. In gukhanmun the middlewares are
stateful filters on the `OutputToken` stream, and the renderer is a usually
stateless translator from `Annotated` tokens to concrete text and markup.

### Built-in middlewares

`HomophoneMarker` scans the stream and sets `homophone = true` on annotations
whose hangul reading is shared by another hanja form within the configured
context window. The window is one of `per-block` (default), `per-document`, or
`off`. Per-block windows buffer only until the next scope whose
`is_block_boundary()` returns true, which is typically a paragraph or a list
item. Per-document windows buffer the whole stream and are appropriate only
when the input is small or when full accuracy matters more than latency.

`FirstOccurrenceFilter` clears `require_hanja` and `require_hangul` on
annotations after their first occurrence within a configurable context, leaving
the first occurrence as-is so the reader still encounters the gloss once. The
context is one of `per-block`, `per-section`, or `per-document`. The section
variant resets at any heading boundary, which the HTML and Markdown adapters
expose through `is_block_boundary()` on heading scopes.

`UserDirectives` applies a user-supplied set of rules. A rule is a predicate
over the hanja form plus an action: set `require_hanja`, set `require_hangul`,
or skip the annotation entirely (which collapses to plain hangul or plain hanja
text depending on the active renderer). The rule predicate may be a literal
string set, a glob, or an arbitrary closure for Rust callers; JavaScript
callers expose only the literal-set form to avoid the cost of per-token
cross-boundary calls.

### Custom middlewares

A middleware is an `impl Iterator<Item = OutputToken<S>>` taking an upstream
iterator. The trait surface is small enough to write inline. Users who want,
for example, to mark technical-term annotations from a glossary write a
middleware that holds the glossary set and updates `require_hanja` on hits.


Renderers
---------

The renderer expands `Annotated` tokens into concrete `Text`, `Open`, and
`Close` tokens according to its mode and the annotation's flags. Five renderers
ship with the library.

The `HangulOnly` renderer emits the hangul reading alone. If `require_hanja` or
`homophone` is set, it emits `한글(한자)`. If `require_hangul` is set, the
result is already hangul, so nothing changes.

The `HangulHanjaParens` renderer always emits `한글(한자)`. The
`require_hangul` flag is satisfied by the hangul half, and `require_hanja` by
the hanja half.

The `HanjaHangulParens` renderer always emits `한자(한글)`. This is useful for
academic and historical-document styles that lead with hanja.

The `Ruby` renderer emits a `<ruby>` element with a sub-mode that determines
which side is the base: `<ruby>한글<rt>한자</rt></ruby>` for `on-hangul`, and
`<ruby>한자<rt>한글</rt></ruby>` for `on-hanja`. If the current scope returns
false from `allows_inline_markup()`, the renderer falls back to parens.

The `Original` renderer emits the original hanja as plain text. Only
annotations with `require_hangul`, or those marked by a user directive, receive
a gloss; the gloss appears in either parens or ruby form depending on a
sub-option. This is the mode for “keep mixed script, gloss only the difficult
characters”, which is the style this very design document uses on its Korean
edition.

Renderers are pure functions over a single `Annotated` token plus the current
scope's `allows_inline_markup()` value. They produce a small fixed-size
sequence of output tokens (one for plain hangul or hanja, three for parens,
between five and nine for ruby). They contain no state and no buffering.

The renderer is the right place to make form decisions because the form depends
on what other tokens are in the stream at the same scope position; for example,
`<ruby>` inside `<pre>` is wrong, and the decision needs the scope stack, which
is what flows through the IR. Putting form decisions elsewhere would either
duplicate the scope tracking or require the renderer to know about other
middlewares.


Format adapters
---------------

Three adapters ship with the library. A new adapter requires only `Reader` and
`Writer` implementations that translate between the format's tokens and the IR.

### HTML

The HTML adapter implements a hand-written scanner that produces
`InputToken<HtmlScopeData>` events. The scanner is a near-direct port of
Seonbi's `Text.Seonbi.Html.Scanner` module, which has been used on real-world
Korean web content for several years. It is fragment-oriented: the input may be
a complete document, a body, or a fragment, and the scanner emits the events it
sees rather than attempting tree construction.

We considered using html5ever or lol\_html. html5ever is the reference HTML5
parser for Rust; it produces a DOM and handles every edge case in the HTML5
specification. lol\_html is Cloudflare's streaming HTML rewriter; it is
selector-driven and integrates with WebAssembly well. We chose a hand-written
scanner because the Seonbi approach has two specific virtues that fit our
model. First, it preserves *raw attribute strings* rather than parsing them,
which means the writer can serialize an unchanged scope exactly as it appeared
in the input. Second, it is small enough to fit comfortably in a WebAssembly
bundle, where bringing along html5ever would dominate the binary size. The cost
is that our scanner is not HTML5-conformant; we accept the trade-off.

The scanner recovers from minor errors. Unclosed tags pop the most recently
opened scope of the same name, or, if none, are emitted as text. Unrecognized
constructs that begin with `<` but are not valid tag, comment, or CDATA starts
are emitted as text characters. Entirely malformed input still produces a token
stream, just one with structural anomalies; the engine is robust to anomalies
(it does not assume scope matching) and the writer emits whatever scopes it
received.

`HtmlScopeData::is_preserve()` returns true if the current element is one of
`pre`, `code`, `kbd`, `script`, `style`, or `textarea`, or if the inherited
`lang` attribute is not Korean. The lang inheritance is computed inside the
adapter: each `Open` event evaluates its raw attributes for a `lang` value, and
the adapter maintains its own lang stack. This is the only place in the
codebase that knows what `lang` means, and the only place that knows the
preserved-tag list. The engine receives the consequences as `is_preserve()` and
does not reproduce either rule.

### Markdown

The Markdown adapter is a thin layer over `pulldown-cmark`. Each
`pulldown-cmark::Event` becomes one or more IR tokens. `Start(Tag)` and
`End(Tag)` become `Open` and `Close`. `Text` becomes `Text`. `Code` becomes
`Verbatim`. Inline HTML, which `pulldown-cmark` exposes as `Event::Html`, is
passed through a second pass of the HTML scanner so that constructs like
`<q lang="ja">` inside a paragraph receive proper lang handling.

Output is via `pulldown-cmark-to-cmark`, which serializes an event stream back
to Markdown. Semantic preservation is contractual: the output, when re-parsed,
produces the same logical structure. Byte-for-byte preservation is best effort:
setext headings may become ATX, link reference definitions may be inlined, soft
breaks may be regularized. Users who require byte-fidelity should process
rendered HTML rather than Markdown.

We chose `pulldown-cmark` rather than writing a Markdown parser because the
work of CommonMark conformance is large and well-handled by `pulldown-cmark`,
and because the event-stream API is a near-perfect match for our IR. The
alternative was `markdown-rs`, which is more recent and produces an AST rather
than an event stream; we preferred event streams for the streaming property.

### Plain text

The plain-text adapter wraps the entire input in a single scope and emits one
`Text` token. The output is the concatenation of `Text` tokens. Ruby rendering
is not meaningful in plain text and falls back to parens.


Distribution
------------

### Rust workspace

The Rust source is organized as a Cargo workspace with the following layout:

 -  *Cargo.toml*: workspace manifest
 -  *DESIGN.md*: symbolic link to *DESIGN.en.md*
 -  *DESIGN.en.md*, *DESIGN.ko-Kore.md*: design documentation
 -  *crates/*
     -  *gukhanmun-core/*: IR types, engine, dictionary trait, lattice
        segmenter, fallback phoneticizer, initial sound law tables, embedded
        `UnihanCharDict`. No I/O, no format-specific code, minimal
        dependencies. Suitable for `no_std` environments with `alloc`.
     -  *gukhanmun-html/*: HTML scanner and serializer. `HtmlScopeData`
        implementation.
     -  *gukhanmun-markdown/*: Markdown adapter atop `pulldown-cmark`.
     -  *gukhanmun-cdb/*: CDB-trie dictionary backend.
     -  *gukhanmun-fst/*: FST dictionary backend.
     -  *gukhanmun-stdict/*: bundled *Standard Korean Language Dictionary* as
        embedded byte arrays, with helpers that hand them to the appropriate
        backend.
     -  *gukhanmun-mkdict/*: CLI for building CDB and FST dictionaries from
        TSV, CSV, or JSONL inputs.
     -  *gukhanmun/*: the umbrella library crate. Re-exports from the others
        under feature flags, exposes the high-level `Builder` API, defines the
        umbrella `Error` enum.
     -  *gukhanmun-cli/*: the `gukhanmun` command-line binary.
     -  *gukhanmun-wasm/*: WebAssembly bindings via `wasm-bindgen`.
     -  *gukhanmun-napi/*: Node-API bindings via `napi-rs`.

The umbrella crate's feature flags compose the others. Default features enable
HTML, Markdown, and the bundled stdict. CDB and FST are individually
selectable. Disabling everything yields a Rust-API-only build with just the
engine and `UnihanCharDict`, suitable for embedded targets.

### JavaScript packages

The JavaScript side is split into a type-only package and one package per
runtime implementation:

 -  *@gukhanmun/types* (npm and JSR): TypeScript interfaces, type aliases,
    error class declarations, and the `GukhanmunFactory` interface. Contains no
    runtime code; npm emits declarations only, while JSR receives the *.ts*
    source directly. This is the canonical API contract; both implementations
    satisfy it structurally.
 -  *@gukhanmun/wasm* (npm and JSR): WebAssembly implementation. Re-exports the
    types. Loads its *.wasm* artifact via `import.meta.url` so that Deno and
    browsers can resolve it natively and Node 22+ can resolve it via the
    standard ESM loader.
 -  *@gukhanmun/napi* (npm only): Node-API implementation. Re-exports the
    types. Ships per-platform prebuilt binaries through napi-rs's
    optional-dependency packaging.

The data dictionaries ship as separate packages so that the runtime bundle
stays small:

 -  *@gukhanmun/stdict-fst* (npm and JSR): the bundled stdict as an FST file,
    exported as a `Uint8Array`.
 -  *@gukhanmun/stdict-cdb* (npm and JSR): the same dictionary as a CDB file.
 -  *@gukhanmun/stdict-min* (npm and JSR): a reduced FST containing only
    homophonous entries and ambiguous readings, for size-sensitive contexts.

The reason for the type-only package is that the canonical API contract should
live in exactly one place. If the contract lived in *@gukhanmun/wasm* and
*@gukhanmun/napi* duplicated the types or imported them from one another,
version skew between the implementations would become a maintenance burden.
With *@gukhanmun/types* as a `peerDependency` of both implementations, users
get a single source of truth and a single set of types regardless of which
runtime they pick. We considered, and rejected, two alternatives: keeping the
types in the WASM package and having NAPI depend on it directly (asymmetric,
makes NAPI subordinate to WASM); and duplicating the types in both packages
(drift between copies, no obvious home for the canonical TSDoc comments).

Option enums on the JavaScript side are string union types rather than
const-asserted objects. This keeps *@gukhanmun/types* genuinely type-only: it
emits zero bytes of runtime code, which matters for bundles and which means the
JSR package's source can be a single *.ts* file with no transpilation step. The
trade-off is that the option strings are stringly typed at runtime; both
implementations validate them at the boundary and throw a `GukhanmunError` with
code `invalid-input` on unrecognized values.

Streaming on the JavaScript side uses the platform
`TransformStream<string, string>` interface, which is available across
browsers, Deno, Node 18+, and Bun. Chunks are JavaScript strings; encoding
concerns (`TextDecoderStream`, `TextEncoderStream`) live outside the gukhanmun
stream. Within the engine, a chunk that ends in the middle of a conversion
span, or close enough to a hanja character that a mixed-script dictionary key
could still cross the boundary, causes that trailing span to be held until the
next chunk arrives; everything before that point is flushed eagerly. The
trailing-span buffer is bounded by the dictionary's `max_word_chars` plus a
small constant for the lattice's outgoing state, typically a few dozen
characters.

We chose strings rather than `Uint8Array` for the streaming type because the
engine fundamentally works on Unicode scalar values: byte-level chunking would
force the adapter to do partial-codepoint reassembly at every boundary, which
the platform's `TextDecoderStream` already does correctly. Users who have a
byte stream chain it through `TextDecoderStream` and then through the gukhanmun
transform.

### Dictionary configuration

The JavaScript dictionary configuration accepts either a file source or an
in-memory map:

~~~~ ts
export type DictionarySource = FileDictionarySource | MapDictionarySource;

export interface FileDictionarySource {
  readonly data: BufferSource | string | URL;
  readonly format: "cdb" | "fst" | "tsv";
}

export type MapDictionarySource =
  ReadonlyMap<string, Omit<DictionaryEntry, "hanja">>;
~~~~

The two variants are distinguished by `instanceof Map` at runtime, and by
structural typing at compile time. The `Map` form is convenient for small
custom vocabularies created in code; the file form is for shipped dictionaries.

### Registry matrix

| Package                 | crates.io | npm                     | JSR                |
| ----------------------- | --------- | ----------------------- | ------------------ |
| `gukhanmun-core`        | yes       | no                      | no                 |
| `gukhanmun-html`        | yes       | no                      | no                 |
| `gukhanmun-markdown`    | yes       | no                      | no                 |
| `gukhanmun-cdb`         | yes       | no                      | no                 |
| `gukhanmun-fst`         | yes       | no                      | no                 |
| `gukhanmun-stdict`      | yes       | no                      | no                 |
| `gukhanmun-mkdict`      | yes       | no                      | no                 |
| `gukhanmun`             | yes       | no                      | no                 |
| `gukhanmun-cli`         | yes       | no                      | no                 |
| `gukhanmun-wasm`        | yes       | no                      | no                 |
| `gukhanmun-napi`        | yes       | no                      | no                 |
| `@gukhanmun/types`      | no        | yes (declarations only) | yes (`.ts` source) |
| `@gukhanmun/wasm`       | no        | yes                     | yes                |
| `@gukhanmun/napi`       | no        | yes                     | no                 |
| `@gukhanmun/stdict-fst` | no        | yes                     | yes                |
| `@gukhanmun/stdict-cdb` | no        | yes                     | yes                |
| `@gukhanmun/stdict-min` | no        | yes                     | yes                |

CLI binaries ship as platform releases (Linux x86\_64 and aarch64, macOS arm64
and x86\_64, Windows x86\_64) attached to the GitHub Releases for each version.

Versioning is lockstep across all packages. Every release tag advances every
crate's version (in the Rust workspace) and every JavaScript package's version
in tandem. Some packages have no functional change at a given release; their
version still advances so that the cross-language story is unambiguous. We
chose lockstep over per-package semver because the cost of mis-coordinated
dependency ranges (a user installing *@gukhanmun/wasm@1.2* with
*@gukhanmun/types@1.3* and getting confusing type errors) outweighs the cost of
an occasional no-op version bump. The CI workflow that fires on tag push
publishes to crates.io, builds the per-platform NAPI prebuilts in parallel,
publishes to npm, and publishes to JSR. Re-running a publish on the same tag is
a no-op against the registries that reject overwriting.


Engineering policies
--------------------

### Errors

Each crate defines its own error enum via `thiserror`. The umbrella *gukhanmun*
crate aggregates them with `#[from]` so that callers can use `?` across crate
boundaries without manual conversion. The pattern:

~~~~ rust
// gukhanmun-core/src/error.rs
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("dictionary load failed: {0}")]
    DictionaryLoad(String),

    #[error("segmentation failed for {hanja:?}: {reason}")]
    Segmentation { hanja: String, reason: String },

    #[error("invalid hangul reading {reading:?} for hanja {hanja:?}")]
    InvalidReading { hanja: String, reading: String },

    #[error("internal invariant violated: {0}")]
    Internal(&'static str),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}
~~~~

The `#[non_exhaustive]` attribute lets us add new variants in minor releases
without breaking callers; downstream `match` expressions are required to have a
wildcard arm. Each variant carries enough structured data to drive both
human-readable messages and machine consumers. `std::error::Error::source()`
chains are preserved through `#[from]` and `#[source]` so that walking an error
gives a complete trace.

Library crates do not use `anyhow`. The CLI does, because the CLI's job is to
print errors to a human, not to be inspected by other code.

The stream-level recovery policy is configurable. The default is
`Recovery::Strict`: the engine propagates any reader error and stops.
`Recovery::Lenient` causes the engine to log the error via `tracing` and emit a
`Verbatim` token for the unrecognized region so that downstream tokens still
flow.

On the JavaScript side, errors are a single class with a discriminant code:

~~~~ ts
export class GukhanmunError extends Error {
  readonly code: ErrorCode;
  readonly chain: readonly { code: ErrorCode; message: string }[];
}

export type ErrorCode =
  | "dictionary-load"
  | "segmentation"
  | "invalid-reading"
  | "html-scan"
  | "html-malformed-attr"
  | "markdown"
  | "unsupported-content-type"
  | "invalid-input"
  | "io"
  | "internal"
  | "other";
~~~~

The bindings walk the Rust `source()` chain at the FFI boundary and materialize
a `chain` property on the error, so JavaScript callers can inspect causes
without needing further FFI calls.

### Logging

*gukhanmun-core* and its siblings depend on the `tracing` crate
unconditionally. Library code uses `tracing::trace!`, `tracing::debug!`,
`tracing::info!`, `tracing::warn!`, and `tracing::error!` directly. The
overhead when no subscriber is registered is one atomic load and a branch per
call site, well under any threshold worth optimizing for.

Binaries that want to compile out the calls entirely (the WebAssembly build is
the obvious case) enable `tracing`'s `release_max_level_off` feature in their
own *Cargo.toml*. That feature works at the binary level: it replaces every
`tracing::*!` invocation in every dependency with a no-op at compile time,
without requiring library code to be reconfigured.

We considered adding a library-level feature flag to make the `tracing`
dependency itself optional, with stub macros for the off path. The added
complexity (a `log` module per crate that conditionally re-exports tracing's
macros or provides stubs) is larger than the binary savings; we will revisit if
WebAssembly bundle measurements call for it.

### Testing

The test suite has four parts.

*Regression fixtures* cover specific bug shapes that have appeared in Seonbi or
in early gukhanmun development. The relevant subset of Seonbi's *test/data/*
directory ports over directly; each fixture is a pair of input and
expected-output files in HTML or Markdown, with a configuration sidecar.

*Snapshot tests* use `insta` to compare IR serializations against a stored
JSON. They are most useful for the engine and middleware crates, where the
input and output are token streams rather than text. A failed snapshot prints a
colored diff and offers an interactive accept-or-reject prompt.

*Property-based tests* use `proptest` to assert invariants over generated
inputs. The two invariants that matter most: reader-then-writer roundtrips a
token stream losslessly (modulo the documented Markdown best-effort caveats);
and engine-then-renderer applied to plain-hangul input is a no-op (the engine
should not invent annotations from text without hanja).

*Conformance tests* run the Markdown adapter against a selected subset of the
CommonMark specification examples to verify that the adapter does not break
syntax that gukhanmun is not interested in changing.

CI runs all four under stable, beta, and the MSRV (minimum supported Rust
version) of the workspace. The WASM build is also exercised for size
regressions: a fixed size budget per artifact is enforced, and a regression
that exceeds it fails the build.


Presets
-------

| Option               | `ko-kr`           | `ko-kp`           |
| -------------------- | ----------------- | ----------------- |
| `rendering`          | `hangul-only`     | `hangul-only`     |
| `disambiguation`     | `per-block`       | `off`             |
| `segmentation`       | `lattice`         | `lattice`         |
| `initialSoundLaw`    | `true`            | `false`           |
| `numerals`           | `hangul-phonetic` | `hangul-phonetic` |
| `dictionary.bundled` | `"stdict-ko-kr"`  | `false`           |
| `firstOccurrence`    | (none)            | (none)            |

The `ko-kr` preset matches the orthographic and lexical conventions of South
Korea: dictionary-driven readings, lattice segmentation, the initial sound law
applied to fallback fragments, and per-block homophone disambiguation that
emits hanja in parentheses when the reading is ambiguous within a paragraph.
The `ko-kp` preset matches the North Korean convention of writing Sino-Korean
words in hangul without the initial sound law applied (래일, 류행, 녀자), with
no bundled dictionary because the South Korean stdict's readings would be
incorrect for `ko-KP`.

The CLI exposes both as `--preset ko-kr` and `--preset ko-kp`. Individual
options remain settable to override the preset, for example
`--preset ko-kr --no-stdict` disables the bundled dictionary while keeping the
other South Korean defaults.


Initial sound law table
-----------------------

The South Korean orthography (한글 맞춤법, Clause 5, Section 52, Chapter 6)
converts a small set of word-initial hangul syllables. The table is reproduced
from Seonbi's `Text.Seonbi.Hanja` module and is the source of truth for
*gukhanmun-core*'s `initial_sound_law_table` constant.

| Original | Converted |
| -------- | --------- |
| 녀       | 여        |
| 뇨       | 요        |
| 뉴       | 유        |
| 니       | 이        |
| 랴       | 야        |
| 려       | 여        |
| 례       | 예        |
| 료       | 요        |
| 류       | 유        |
| 리       | 이        |
| 라       | 나        |
| 래       | 내        |
| 로       | 노        |
| 뢰       | 뇌        |
| 루       | 누        |
| 르       | 느        |


Hanja numeral table
-------------------

The fallback phoneticizer and the `additive-arabic` numeral strategy share a
single table of digit and place-marker hanja with their canonical values.

| Hanja                              | Value     | Notes  |
| ---------------------------------- | --------- | ------ |
| `零`, `〇`                         | 0         |        |
| `一`, `壹`, `壱`, `弌`, `夁`       | 1         |        |
| `二`, `貳`, `贰`, `弐`, `弍`, `貮` | 2         |        |
| `三`, `參`, `叁`, `参`, `弎`, `叄` | 3         |        |
| `四`, `肆`, `䦉`                   | 4         |        |
| `五`, `伍`                         | 5         |        |
| `六`, `陸`, `陆`                   | 6         |        |
| `七`, `柒`, `漆`                   | 7         |        |
| `八`, `捌`                         | 8         |        |
| `九`, `玖`                         | 9         |        |
| `十`, `拾`                         | 10        |        |
| `百`, `佰`, `陌`                   | 100       |        |
| `千`, `仟`, `阡`                   | 1000      |        |
| `萬`, `万`                         | 10000     |        |
| `億`                               | 100000000 | $10^8$ |
| `兆`                               | $10^{12}$ |        |
| `京`                               | $10^{16}$ |        |
| `垓`                               | $10^{20}$ |        |
| `秭`                               | $10^{24}$ |        |
| `穰`                               | $10^{28}$ |        |
| `溝`                               | $10^{32}$ |        |
| `澗`                               | $10^{36}$ |        |

The `additive-arabic` strategy treats place markers as multipliers and adjacent
digit hanja as multiplicands, with the Korean elision rule that bare `十`,
`百`, `千` mean 10, 100, 1000 respectively (not `一十`, `一百`, `一千`).


HTML preserved tags
-------------------

The default `HtmlScopeData::is_preserve()` returns true for the following tag
names regardless of attribute content: `pre`, `code`, `kbd`, `script`, `style`,
`textarea`.

It additionally returns true when the inherited `lang` attribute's primary tag
is anything other than `ko`, `kor`, or a subtag-prefixed Korean form (`ko-KR`,
`ko-Hang`, `ko-Kore`, `kor-KP`, and so on). The Korean predicate matches
Seonbi's `isKorean`.

Users who want to extend the list (for example, to add a project-specific
`class="no-translate"` attribute) wrap the default `HtmlScopeData` in their own
type whose `is_preserve()` ORs in the additional rule, or pass a closure to the
engine's `skip` option.


CDB-trie key scheme
-------------------

A CDB-trie database stores one record per prefix of each entry. The key is the
UTF-8 bytes of the prefix. The value layout:

| Offset | Size          | Field                                                                  |
| ------ | ------------- | ---------------------------------------------------------------------- |
| 0      | 1             | `is_complete` (1 if the prefix is itself an entry)                     |
| 1      | 1             | `mark` (bitfield: bit 0 is `require_hanja`, bit 1 is `require_hangul`) |
| 2      | 2             | `reading_len` (little-endian; 0 if `is_complete = 0`)                  |
| 4      | `reading_len` | reading (UTF-8)                                                        |

A separate well-known key, `__gukhanmun_meta__`, stores a small CBOR document
with build metadata: source, license, build date, original entry count, prefix
count, maximum entry length in characters and bytes.


FST schema
----------

The FST database stores one entry per dictionary word. The key is the UTF-8
bytes of the hanja form; the value is a 64-bit integer whose layout is:

| Bits  | Field                                                        |
| ----- | ------------------------------------------------------------ |
| 0–15  | reading length in UTF-8 bytes                                |
| 16–23 | `mark` (`require_hanja` and `require_hangul` bits as in CDB) |
| 24–63 | offset into a side reading-string table                      |

The reading-string table is a contiguous block of UTF-8 bytes following the FST
itself. The side table is necessary because the FST value type is fixed-size;
we cannot store variable-length readings inline. The `mark` byte sits in the
value rather than the side table because checking it is hot for every lookup.

A metadata header at the file start (eight bytes of magic, version, layout
offsets, and a CBOR metadata blob analogous to the CDB form) precedes the FST
bytes.


Glossary
--------

| Hangul         | Hanja          | English                                           |
| -------------- | -------------- | ------------------------------------------------- |
| 한자           | 漢字           | hanja, a Chinese character used in Korean writing |
| 한자어         | 漢字語         | Sino-Korean word                                  |
| 국한문혼용     | 國漢文混用     | mixed-script Korean writing                       |
| 두음법칙       | 頭音法則       | initial sound law                                 |
| 표준국어대사전 | 標準國語大辭典 | Standard Korean Language Dictionary               |
| 한글           | 한글           | hangul, the native Korean alphabet                |
| 한글전용       | 한글專用       | hangul-only writing                               |
| 동음이의       | 同音異義       | homophony, distinct words sharing a reading       |
