---
title: "Rendering modes"
description: |-
  Controlling how Gukhanmun presents hanja and hangul readings in Rust.
---

Rendering modes
===============

Annotating only hanja runs
--------------------------

Both `Builder` and `SharedBuilder` accept
`.annotation_granularity(AnnotationGranularity::HanjaRun)` independently of
`.rendering()`. With hangul-first parentheses, this writes `汽車길` as
`기찻(汽車)길`. In ruby mode, `半달形` puts separate glosses on `半` and `形`.
The dictionary still supplies the reading for the whole word. If the unchanged
hangul does not identify a unique split, the renderer keeps the whole-word
gloss. `AnnotationGranularity::Word` is the default.

Low-level callers can use `render_tokens_iter_with_granularity()` or
`ExpandingRenderer` to receive multiple rendered tokens per annotation.

`builder.rendering(mode)` sets how hanja and their hangul readings appear in
the output.


`RenderMode` variants
---------------------

~~~~ rust
use gukhanmun::RenderMode;

builder.rendering(RenderMode::HangulOnly);          // "한자" (default)
builder.rendering(RenderMode::HangulHanjaParens);   // "한자(漢字)"
builder.rendering(RenderMode::HanjaHangulParens);   // "漢字(한자)"
builder.rendering(RenderMode::RubyOnHangul);        // <ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>
builder.rendering(RenderMode::RubyOnHanja);         // <ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>
builder.rendering(RenderMode::Original);            // keep hanja, gloss where needed
~~~~

`RubyOnHangul` and `RubyOnHanja` produce `<ruby>` markup; they are most
useful with HTML or Markdown output.  In plain-text mode they fall back to
parentheses.  The annotation is wrapped in `<rp>` (ruby parenthesis)
elements so that browsers without `<ruby>` support still render the reading
as a parenthesized gloss (`한자(漢字)`) rather than running it into the base
text.


`Original` mode with a gloss style
----------------------------------

`RenderMode::Original` keeps hanja in place and adds a gloss only for
homophones that need disambiguation.  Use `RenderOptions` to also set the
gloss style:

~~~~ rust
use gukhanmun::{RenderOptions, RenderMode, OriginalGloss};

builder.rendering(RenderOptions {
    mode: RenderMode::Original,
    original_gloss: OriginalGloss::Parens,  // or OriginalGloss::Ruby
    ..RenderOptions::default()
});
~~~~


Hanja variant sets
------------------

Variant recognition is always enabled. `Builder::hanja_variant_set`
independently chooses the spelling used whenever the renderer emits hanja:

~~~~ rust
use gukhanmun::HanjaVariantSet;

builder.hanja_variant_set(HanjaVariantSet::Shinjitai);
~~~~

The available profiles are `AsDictionary` (default), `Shinjitai`, `Kanxi`,
`Simplified`, and `Asahimoji`.
