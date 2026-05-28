---
title: "Directives"
description: |-
  Per-hanja annotation overrides in the Gukhanmun Rust library.
---

Directives
==========

Directives override the dictionary's annotation marks for specific hanja.


Literal directive
-----------------

~~~~ rust
use gukhanmun::DirectiveAction;

builder
    .directive("漢", DirectiveAction::RequireHanja)
    .directive("字", DirectiveAction::RequireHanja)
    .directive("東", DirectiveAction::RequireHangul)
    .directive("中", DirectiveAction::SkipAnnotation);
~~~~

| Action           | Effect                                                  |
| ---------------- | ------------------------------------------------------- |
| `RequireHanja`   | Always include the hanja in the output                  |
| `RequireHangul`  | Always include the hangul reading (for `Original` mode) |
| `SkipAnnotation` | Suppress the annotation entirely                        |


Predicate directive
-------------------

For pattern-based rules supply a closure:

~~~~ rust
builder.directive_predicate(
    |hanja: &str| hanja.starts_with('東'),
    DirectiveAction::RequireHanja,
);
~~~~

Predicates are evaluated at conversion time.  Multiple predicates are OR-ed:
the first matching predicate's action wins.


Replacing all directives at once
--------------------------------

`UserDirectives` collects a set of directives and can be applied in bulk:

~~~~ rust
use gukhanmun::{UserDirectives, DirectiveAction};

let mut directives = UserDirectives::new();
directives.add_literal("漢", DirectiveAction::RequireHanja);
directives.add_literal("字", DirectiveAction::RequireHanja);

builder.directives(directives);
~~~~

Calling `builder.directives(d)` replaces all previously registered directives.
