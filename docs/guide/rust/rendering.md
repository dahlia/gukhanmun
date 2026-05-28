---
title: "Rendering modes"
description: |-
  Controlling how Gukhanmun presents hanja and hangul readings in Rust.
---

Rendering modes
===============

`builder.rendering(mode)` sets how hanja and their hangul readings appear in
the output.


`RenderMode` variants
---------------------

~~~~ rust
use gukhanmun::RenderMode;

builder.rendering(RenderMode::HangulOnly);          // "한자" (default)
builder.rendering(RenderMode::HangulHanjaParens);   // "한자(漢字)"
builder.rendering(RenderMode::HanjaHangulParens);   // "漢字(한자)"
builder.rendering(RenderMode::RubyOnHangul);        // <ruby>한자<rt>漢字</rt></ruby>
builder.rendering(RenderMode::RubyOnHanja);         // <ruby>漢字<rt>한자</rt></ruby>
builder.rendering(RenderMode::Original);            // keep hanja, gloss where needed
~~~~

`RubyOnHangul` and `RubyOnHanja` produce `<ruby>` markup; they are most
useful with HTML or Markdown output.  In plain-text mode they fall back to
parentheses.


`Original` mode with a gloss style
----------------------------------

`RenderMode::Original` keeps hanja in place and adds a gloss only for
homophones that need disambiguation.  Use `RenderOptions` to also set the
gloss style:

~~~~ rust
use gukhanmun::{RenderOptions, RenderMode, OriginalGloss};

builder.rendering(RenderOptions {
    mode: RenderMode::Original,
    original_gloss: Some(OriginalGloss::Parens),  // or OriginalGloss::Ruby
});
~~~~
