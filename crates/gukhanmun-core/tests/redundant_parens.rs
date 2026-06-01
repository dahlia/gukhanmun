// Gukhanmun: Tests for the redundant parenthetical annotation collapser.
// Copyright (C) 2026  Hong Minhee
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Behavioural tests for [`RedundantParenCollapser`], covering the two
//! adjacency patterns, the two-tier reading heuristic, reading override,
//! rejection of definition glosses and foreign transliterations, scope
//! boundaries, and streaming/buffered equivalence.

use gukhanmun_core::{
    Annotation, OutputToken, PlainScopeData, RedundantParenCollapser, RenderMode, Scope,
    collapse_redundant_parens, render_tokens, write_plain_text,
};

type Token = OutputToken<PlainScopeData>;

fn annotation(hanja: &str, reading: &str) -> Annotation {
    Annotation {
        hanja: hanja.into(),
        reading: reading.into(),
        homophone: false,
        require_hanja: false,
        require_hangul: false,
        first_in_context: true,
        skip_annotation: false,
        from_dictionary: true,
        from_source_gloss: false,
    }
}

fn ann(hanja: &str, reading: &str) -> Token {
    OutputToken::Annotated(annotation(hanja, reading))
}

fn text(value: &str) -> Token {
    OutputToken::Text(value.into())
}

fn collapse(tokens: Vec<Token>) -> Vec<Token> {
    collapse_redundant_parens(tokens, true)
}

fn render(tokens: Vec<Token>, mode: RenderMode) -> String {
    write_plain_text(render_tokens(tokens, mode))
}

fn only_annotation(tokens: &[Token]) -> &Annotation {
    assert_eq!(tokens.len(), 1, "expected a single token, got {tokens:?}");
    match &tokens[0] {
        OutputToken::Annotated(annotation) => annotation,
        other => panic!("expected an annotation, got {other:?}"),
    }
}

// ── Pattern A: hanja first ──────────────────────────────────────────────────

#[test]
fn pattern_a_collapses_and_sets_both_flags() {
    let collapsed = collapse(vec![ann("庫間", "곳간"), text("(곳간)")]);
    let annotation = only_annotation(&collapsed);
    assert_eq!(annotation.hanja, "庫間");
    assert_eq!(annotation.reading, "곳간");
    assert!(annotation.require_hanja);
    assert!(annotation.require_hangul);
}

#[test]
fn pattern_a_renders_both_scripts_in_every_mode() {
    assert_eq!(
        render(
            collapse(vec![ann("庫間", "곳간"), text("(곳간)")]),
            RenderMode::HangulOnly
        ),
        "곳간(庫間)"
    );
    assert_eq!(
        render(
            collapse(vec![ann("庫間", "곳간"), text("(곳간)")]),
            RenderMode::Original
        ),
        "庫間(곳간)"
    );
}

#[test]
fn pattern_a_preserves_text_after_the_parenthetical() {
    let collapsed = collapse(vec![ann("庫間", "곳간"), text("(곳간)에서 만나자.")]);
    assert_eq!(collapsed.len(), 2);
    assert_eq!(collapsed[1], text("에서 만나자."));
    assert_eq!(
        render(
            collapse(vec![ann("庫間", "곳간"), text("(곳간)에서 만나자.")]),
            RenderMode::HangulOnly
        ),
        "곳간(庫間)에서 만나자."
    );
}

// ── Pattern B: hangul first ─────────────────────────────────────────────────

#[test]
fn pattern_b_collapses_and_sets_both_flags() {
    let collapsed = collapse(vec![text("곳간("), ann("庫間", "곳간"), text(")")]);
    let annotation = only_annotation(&collapsed);
    assert_eq!(annotation.reading, "곳간");
    assert!(annotation.require_hanja);
    assert!(annotation.require_hangul);
    assert_eq!(
        render(
            collapse(vec![text("곳간("), ann("庫間", "곳간"), text(")")]),
            RenderMode::HangulOnly
        ),
        "곳간(庫間)"
    );
}

#[test]
fn pattern_b_preserves_surrounding_text() {
    let tokens = || {
        vec![
            text("이 "),
            text("곳간("),
            ann("庫間", "곳간"),
            text(")이다."),
        ]
    };
    let collapsed = collapse(tokens());
    assert_eq!(collapsed[0], text("이 "));
    assert_eq!(collapsed[collapsed.len() - 1], text("이다."));
    assert_eq!(
        render(collapse(tokens()), RenderMode::HangulOnly),
        "이 곳간(庫間)이다."
    );
}

#[test]
fn pattern_b_keeps_lead_in_within_one_text_token() {
    // The reading shares its text token with a preceding word; only the
    // trailing `곳간(` is consumed.
    let collapsed = collapse(vec![text("이 곳간("), ann("庫間", "곳간"), text(")")]);
    assert_eq!(collapsed[0], text("이 "));
    assert_eq!(
        render(
            collapse(vec![text("이 곳간("), ann("庫間", "곳간"), text(")")]),
            RenderMode::HangulOnly
        ),
        "이 곳간(庫間)"
    );
}

// ── Tier 2: alternative reading override ────────────────────────────────────

#[test]
fn pattern_a_overrides_reading_with_valid_alternative() {
    let collapsed = collapse(vec![ann("數字", "숫자"), text("(수자)")]);
    let annotation = only_annotation(&collapsed);
    assert_eq!(annotation.reading, "수자");
    assert!(annotation.require_hanja);
    assert!(annotation.require_hangul);
    assert_eq!(
        render(
            collapse(vec![ann("數字", "숫자"), text("(수자)")]),
            RenderMode::HangulOnly
        ),
        "수자(數字)"
    );
    assert_eq!(
        render(
            collapse(vec![ann("數字", "숫자"), text("(수자)")]),
            RenderMode::Original
        ),
        "數字(수자)"
    );
}

#[test]
fn pattern_b_overrides_reading_with_valid_alternative() {
    let collapsed = collapse(vec![text("수자("), ann("數字", "숫자"), text(")")]);
    assert_eq!(only_annotation(&collapsed).reading, "수자");
}

#[test]
fn parenthetical_matching_dictionary_reading_keeps_it() {
    let collapsed = collapse(vec![ann("議論", "의논"), text("(의논)")]);
    assert_eq!(only_annotation(&collapsed).reading, "의논");
}

#[test]
fn alternative_initial_sound_law_reading_is_accepted() {
    // 議論 reads both 의논 and 의론; the explicit 의론 overrides the default.
    let collapsed = collapse(vec![ann("議論", "의논"), text("(의론)")]);
    assert_eq!(only_annotation(&collapsed).reading, "의론");
    assert_eq!(
        render(
            collapse(vec![ann("議論", "의논"), text("(의론)")]),
            RenderMode::HangulOnly
        ),
        "의론(議論)"
    );
}

#[test]
fn fallback_reading_transliteration_collapses_when_it_matches() {
    // 蔣介石 → 장개석 is the regular per-character reading, so the matching
    // parenthetical collapses (tier 1).
    let collapsed = collapse(vec![ann("蔣介石", "장개석"), text("(장개석)")]);
    let annotation = only_annotation(&collapsed);
    assert!(annotation.require_hanja);
    assert!(annotation.require_hangul);
}

// ── Rejections (left untouched) ─────────────────────────────────────────────

#[test]
fn definition_gloss_is_left_untouched() {
    let tokens = || vec![ann("庫間", "곳간"), text("(물건을 간직하여 두는 곳)")];
    let collapsed = collapse(tokens());
    assert_eq!(collapsed.len(), 2);
    match &collapsed[0] {
        OutputToken::Annotated(annotation) => {
            assert!(!annotation.require_hanja);
            assert!(!annotation.require_hangul);
        }
        other => panic!("expected annotation, got {other:?}"),
    }
    assert_eq!(collapsed[1], text("(물건을 간직하여 두는 곳)"));
    assert_eq!(
        render(collapse(tokens()), RenderMode::HangulOnly),
        "곳간(물건을 간직하여 두는 곳)"
    );
}

#[test]
fn foreign_transliteration_is_left_untouched() {
    let jiang = collapse(vec![ann("蔣介石", "장개석"), text("(장제스)")]);
    assert_eq!(jiang, vec![ann("蔣介石", "장개석"), text("(장제스)")]);

    let mao = collapse(vec![ann("毛澤東", "모택동"), text("(마오쩌둥)")]);
    assert_eq!(mao, vec![ann("毛澤東", "모택동"), text("(마오쩌둥)")]);
}

#[test]
fn syllable_count_mismatch_is_left_untouched() {
    let collapsed = collapse(vec![ann("庫間", "곳간"), text("(곳간이)")]);
    assert_eq!(collapsed, vec![ann("庫間", "곳간"), text("(곳간이)")]);
}

// ── Split text tokens (coalescing) ──────────────────────────────────────────

#[test]
fn following_parenthetical_split_across_text_tokens_collapses() {
    // The streaming engine flushes `(곳간)` as `(곳간` then `)`.
    let collapsed = collapse(vec![ann("庫間", "곳간"), text("(곳간"), text(")")]);
    let annotation = only_annotation(&collapsed);
    assert!(annotation.require_hanja);
    assert!(annotation.require_hangul);
    assert_eq!(
        render(
            collapse(vec![ann("庫間", "곳간"), text("(곳간"), text(")")]),
            RenderMode::HangulOnly
        ),
        "곳간(庫間)"
    );
}

#[test]
fn preceding_reading_split_across_text_tokens_collapses() {
    // The hangul-first reading arrives as `곳` then `간(`.
    let collapsed = collapse(vec![
        text("곳"),
        text("간("),
        ann("庫間", "곳간"),
        text(")"),
    ]);
    assert_eq!(
        render(
            collapse(vec![
                text("곳"),
                text("간("),
                ann("庫間", "곳간"),
                text(")")
            ]),
            RenderMode::HangulOnly,
        ),
        "곳간(庫間)"
    );
    assert!(only_annotation(&collapsed).require_hanja);
}

#[test]
fn long_preceding_hangul_run_is_emitted_eagerly() {
    // A pathological space-free hangul run must not be buffered whole; only a
    // bounded matchable tail is held back.
    let mut collapser = RedundantParenCollapser::<PlainScopeData>::new(true);
    let run = "가".repeat(1000);
    let emitted = collapser.push_token(text(&run));
    let emitted_chars: usize = emitted
        .iter()
        .map(|token| match token {
            OutputToken::Text(value) => value.chars().count(),
            _ => 0,
        })
        .sum();
    assert!(
        emitted_chars >= 1000 - 64,
        "expected most of the 1000-syllable run emitted eagerly, held too much (emitted {emitted_chars})"
    );
}

#[test]
fn unclosed_parenthetical_does_not_buffer_unboundedly() {
    // A `(` that never closes before a long hangul run must not be mistaken for
    // a match, and the trailing text passes through unchanged.
    let tokens = || vec![ann("庫間", "곳간"), text("(가나다라마바사)")];
    assert_eq!(
        render(collapse(tokens()), RenderMode::HangulOnly),
        "곳간(가나다라마바사)"
    );
}

// ── Scope boundaries ────────────────────────────────────────────────────────

#[test]
fn verbatim_between_tokens_blocks_a_match() {
    let collapsed = collapse(vec![
        ann("庫間", "곳간"),
        OutputToken::Verbatim("x".into()),
        text("(곳간)"),
    ]);
    assert_eq!(
        collapsed,
        vec![
            ann("庫間", "곳간"),
            OutputToken::Verbatim("x".into()),
            text("(곳간)")
        ]
    );
}

#[test]
fn scope_boundary_between_tokens_blocks_a_match() {
    let open = || OutputToken::Open(Scope::new(PlainScopeData));
    let collapsed = collapse(vec![
        ann("庫間", "곳간"),
        open(),
        text("(곳간)"),
        OutputToken::Close,
    ]);
    assert_eq!(
        collapsed,
        vec![
            ann("庫間", "곳간"),
            open(),
            text("(곳간)"),
            OutputToken::Close
        ]
    );
}

#[test]
fn lone_annotation_is_unchanged() {
    let collapsed = collapse(vec![ann("庫間", "곳간")]);
    assert_eq!(collapsed, vec![ann("庫間", "곳간")]);
}

// ── Toggle and streaming ────────────────────────────────────────────────────

#[test]
fn disabled_collapser_is_exact_passthrough() {
    let tokens = vec![text("곳간("), ann("庫間", "곳간"), text(")")];
    assert_eq!(collapse_redundant_parens(tokens.clone(), false), tokens);
}

#[test]
fn adjacent_patterns_collapse_independently() {
    // Pattern A's leftover text (` 곳간(`) becomes pattern B's preceding text
    // for the next annotation, so both words collapse.
    let tokens = || {
        vec![
            ann("庫間", "곳간"),
            text("(곳간) 곳간("),
            ann("庫間", "곳간"),
            text(")"),
        ]
    };
    let collapsed = collapse(tokens());
    assert_eq!(collapsed.len(), 3);
    assert!(
        matches!(&collapsed[0], OutputToken::Annotated(a) if a.require_hanja && a.require_hangul)
    );
    assert_eq!(collapsed[1], text(" "));
    assert!(
        matches!(&collapsed[2], OutputToken::Annotated(a) if a.require_hanja && a.require_hangul)
    );
    assert_eq!(
        render(collapse(tokens()), RenderMode::HangulOnly),
        "곳간(庫間) 곳간(庫間)"
    );
}

#[test]
fn streaming_one_token_at_a_time_matches_the_buffered_helper() {
    let tokens = vec![
        text("이 곳간("),
        ann("庫間", "곳간"),
        text(")과 "),
        ann("數字", "숫자"),
        text("(수자)"),
    ];
    let buffered = collapse_redundant_parens(tokens.clone(), true);

    let mut collapser = RedundantParenCollapser::<PlainScopeData>::new(true);
    let mut streamed = Vec::new();
    for token in tokens {
        streamed.extend(collapser.push_token(token));
    }
    streamed.extend(collapser.finish());

    assert_eq!(streamed, buffered);
    assert_eq!(
        render(streamed, RenderMode::HangulOnly),
        "이 곳간(庫間)과 수자(數字)"
    );
}
