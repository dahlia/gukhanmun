---
title: "Streaming API"
description: |-
  Processing large documents incrementally with Gukhanmun's iterator API.
---

Streaming API
=============

All `convert_*_to_string` methods buffer the full output before returning.
The iterator variants let you process the output token by token, which is
useful for large documents or when you need to write to a sink incrementally.


Text iterator
-------------

~~~~ rust
use gukhanmun::{Builder, Preset, write_plain_text};

let converter = Builder::with_preset(Preset::KoKr).build()?;

let output = write_plain_text(converter.convert_text_iter("漢字를 한글로"));
~~~~

`convert_text_iter` returns `impl Iterator<Item = RenderedToken>`.  Hand the
iterator to `write_plain_text` rather than concatenating it by hand; the token
type distinguishes source text from annotation text, and a writer is what turns
that distinction into safe output.


HTML and Markdown iterators
---------------------------

~~~~ rust
use gukhanmun::{MarkdownVariant, html, markdown};

// HTML
let html = html::write_html_fragment(converter.convert_html_fragment_iter("<p>漢字</p>")?);

// Markdown
let source = markdown::write_markdown(
    converter.convert_markdown_iter("# 漢字", MarkdownVariant::Gfm),
)?;
~~~~

The adapters' writers own the escaping rules for their format.  In particular,
dictionary readings end up in `RenderedToken::Annotated` tokens, and
`write_html_fragment` escapes those as character data so a reading can never
become markup.


Format-agnostic token pipeline
------------------------------

`convert_tokens` accepts any iterator of `InputToken` and returns an iterator
of `RenderedToken`.  This is the lowest-level entry point and lets you supply
pre-tokenised input from a custom reader:

~~~~ rust
use gukhanmun::InputToken;

let tokens: Vec<InputToken<&str>> = /* your tokeniser output */;
for rendered in converter.convert_tokens(tokens.into_iter()) {
    // Serialize with the writer for your output format.
}
~~~~

A custom writer must serialize `RenderedToken::Annotated` as literal text:
`Text` is source data that is already valid for the format, while `Annotated`
is generated from a dictionary or other annotation source and may contain
arbitrary characters.  Passing `Annotated` through unescaped reintroduces
GHSA-j87h-8h88-wm6g.


Token types
-----------

| Type            | Description                                                                                    |
| --------------- | ---------------------------------------------------------------------------------------------- |
| `InputToken`    | A unit of input: hanja run, hangul text, mixed text, block boundary, or section boundary       |
| `OutputToken`   | A unit after dictionary lookup and annotation: annotated hanja, plain text, or boundary marker |
| `RenderedToken` | A unit of output text after rendering: source text, generated annotation text, or a boundary   |


Inspecting phrase decisions
---------------------------

Use the low-level `Engine` when you need to inspect source phrase decisions.
Enable reporting explicitly, drain completed decisions with
`take_resolution_report()`, and call `finish_with_report()` to obtain decisions
first established by EOF:

~~~~ rust
use gukhanmun::{Engine, InputToken, PlainScopeData};

fn main() {
    let dictionary = gukhanmun::stdict::ko_kr();
    let mut engine = Engine::<PlainScopeData, _>::new(dictionary)
        .with_resolution_reporting();
    let mut tokens = engine.push_token(InputToken::Text("무조건 ".into()));
    tokens.extend(engine.push_token(InputToken::Text("降服".into())));
    let early_report = engine.take_resolution_report();
    let (tail, final_report) = engine.finish_with_report();
    tokens.extend(tail);
    for event in early_report.events.iter().chain(&final_report.events) {
        println!("{:?}: {:?}", event.outcome, event.chosen.form);
    }
}
~~~~

A `ResolutionEvent` records the source-owned rule identifiers, the remaining
complete choices and the choice retained at that resolution step. Its outcome
is `Selected`, `Unresolved`, `Conflict`, `Limit` or `PolicyDefault`. The report
precedes positional realization and explicit gloss processing; a valid source
gloss can revalidate the final annotation afterward.

Event byte offsets refer to contiguous convertible IR text, not positions in the
original HTML or Markdown bytes. Offsets restart at structural/preserved
boundaries and explicit flushes.

An unconstrained source pool produces
`PolicyDefault` when its particle-compatible choices are empty or semantically
distinct. This records the existing policy selection without reclassifying
curated preferences. A semantically unique unconstrained choice needs no event.
Reporting is off by default; drain enabled reports periodically to avoid
accumulating them for a whole document.

A `Limit` event leaves `remaining` empty because the resolver did not enumerate
that set. `MAX_PHRASE_CANDIDATES` limits the additional resolver work to 256
ranked choices per edge, including policy-default reporting. The resolver
keeps the legacy choice when a pool exceeds that limit and withholds the
entire affected rule/window. It never truncates the stored source pool. Pool
construction and legacy policy selection happen outside this additional-work
limit; it is not a bound on the whole engine.

Ordinary adjacent `Text` chunks share phrase context. A scope open/close,
preserved/verbatim transition or explicit `flush()` ends it. Inline markup,
preserved/verbatim transitions and explicit flushes do not themselves prove a
lexical right boundary. A rule needs a textual witness within its flow, an
adapter-reported block boundary or actual EOF. Complete rules within that flow
can still apply; rules cannot bridge the boundary. `finish()`
and `finish_with_report()` prove EOF, whereas `flush()` does not supply EOF
particle evidence. Calling `flush()` in the middle of a phrase can therefore
forfeit a later constraint.

Source phrase rules retain a finite additional tail and committed left context,
including literal hangul before the first hanja chunk. That allowance depends
on the maximum admitted phrase surface and its boundary/particle witnesses.
Without a known word maximum, the engine can still emit earlier
whitespace-delimited prefixes once their paths and phrase context are settled.
An unfinished word can remain buffered until a delimiter or EOF.

This is not a universal memory bound: unresolved lattice frontiers, dictionaries
without a known maximum, unfinished numerals, fallback annotation grouping and
middleware can retain more input. `buffered_chars()` counts pending text;
committed phrase context is stored separately.
