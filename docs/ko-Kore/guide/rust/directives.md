---
title: 指示
description: |-
  Gukhanmun Rust 라이브러리에서의 漢字別 倂記 再定義.
---

指示
====

指示는 特定 漢字에 對해 辭典의 倂記 標識를 덮어씁니다.


리터럴 指示
-----------

~~~~ rust
use gukhanmun::DirectiveAction;

builder
    .directive("漢", DirectiveAction::RequireHanja)
    .directive("字", DirectiveAction::RequireHanja)
    .directive("東", DirectiveAction::RequireHangul)
    .directive("中", DirectiveAction::SkipAnnotation);
~~~~

| 行動             | 效果                                     |
| ---------------- | ---------------------------------------- |
| `RequireHanja`   | 出力에 漢字를 恒常 包含                  |
| `RequireHangul`  | 한글 讀音을 恒常 包含(`Original` 모드用) |
| `SkipAnnotation` | 倂記를 全的으로 抑制                     |


述語 指示
---------

패턴 基盤 規則을 爲해서는 클로저를 提供합니다:

~~~~ rust
builder.directive_predicate(
    |hanja: &str| hanja.starts_with('東'),
    DirectiveAction::RequireHanja,
);
~~~~

述語는 變換 時點에 評價됩니다.  여러 述語는 OR로 結合됩니다: 처음 一致하는
述語의 行動이 採擇됩니다.


모든 指示를 한꺼번에 交替
-------------------------

`UserDirectives`는 指示 集合을 모아 一括 適用할 수 있습니다:

~~~~ rust
use gukhanmun::{UserDirectives, DirectiveAction};

let mut directives = UserDirectives::new();
directives.add_literal("漢", DirectiveAction::RequireHanja);
directives.add_literal("字", DirectiveAction::RequireHanja);

builder.directives(directives);
~~~~

`builder.directives(d)`를 呼出하면 以前에 登錄된 모든 指示가 交替됩니다.
