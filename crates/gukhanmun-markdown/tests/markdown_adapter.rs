// Gukhanmun: Markdown adapter for Gukhanmun.
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

use gukhanmun_core::{MapDictionary, RenderMode, RenderedToken};
use gukhanmun_markdown::{MarkdownScopeData, convert_markdown, read_markdown, write_markdown};
use proptest::prelude::*;
use pulldown_cmark::{Event, Parser};

fn dictionary() -> MapDictionary {
    let mut dict = MapDictionary::new();
    dict.insert("漢字", "한자");
    dict.insert("北京", "베이징");
    dict.insert("布告하다", "포고하다");
    dict.insert("佈告하다", "포고하다");
    dict
}

fn events(markdown: &str) -> Vec<Event<'static>> {
    Parser::new(markdown).map(Event::into_static).collect()
}

#[test]
fn converts_markdown_text_in_blocks_and_inlines() {
    let output = convert_markdown(
        "# 漢字\n\n- 北京 and **漢字**\n",
        &dictionary(),
        RenderMode::HangulOnly,
    )
    .unwrap();

    assert_eq!(events(&output), events("# 한자\n\n- 베이징 and **한자**\n"));
}

#[test]
fn adjacent_markdown_text_events_are_merged_before_conversion() {
    let output =
        convert_markdown("漢&#23383;\n", &dictionary(), RenderMode::HangulHanjaParens).unwrap();

    assert_eq!(events(&output), events("한자(漢字)\n"));
}

#[test]
fn code_span_and_code_block_are_not_converted() {
    let output = convert_markdown(
        "`漢字`\n\n```text\n北京\n```\n\n漢字\n",
        &dictionary(),
        RenderMode::HangulOnly,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events("`漢字`\n\n```text\n北京\n```\n\n한자\n")
    );
}

#[test]
fn inline_html_lang_scope_preserves_non_korean_text() {
    let output = convert_markdown(
        r#"<q lang="ja">漢字</q> 漢字"#,
        &dictionary(),
        RenderMode::HangulOnly,
    )
    .unwrap();

    assert_eq!(events(&output), events(r#"<q lang="ja">漢字</q> 한자"#));
}

#[test]
fn inline_html_lang_scope_preserves_markdown_child_scopes() {
    let output = convert_markdown(
        r#"<span lang="ja">**漢字** [北京](https://example.com)</span> 漢字"#,
        &dictionary(),
        RenderMode::HangulOnly,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events(r#"<span lang="ja">**漢字** [北京](https://example.com)</span> 한자"#)
    );
}

#[test]
fn ancestor_inline_html_close_recovers_scope_stack() {
    let output = convert_markdown(
        "<span lang=ja><b>漢字</span> 漢字",
        &dictionary(),
        RenderMode::HangulOnly,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events("<span lang=ja><b>漢字</b></span> 한자")
    );
}

#[test]
fn inline_html_close_inside_markdown_scope_updates_preserve_policy() {
    let output = convert_markdown(
        r#"<span lang="ja">**漢字</span> 北京**"#,
        &dictionary(),
        RenderMode::HangulOnly,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events(r#"<span lang="ja">**漢字**</span> **베이징**"#)
    );
}

#[test]
fn unclosed_inline_html_scope_ends_before_markdown_block_end() {
    let output = convert_markdown(
        "<span lang=\"ja\">漢字\n\n漢字\n",
        &dictionary(),
        RenderMode::HangulOnly,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events("<span lang=\"ja\">漢字</span>\n\n한자\n")
    );
}

#[test]
fn html_blocks_are_preserved_as_raw_markdown_html() {
    let output = convert_markdown(
        "<div>\n漢字\n</div>\n\n漢字\n",
        &dictionary(),
        RenderMode::HangulOnly,
    )
    .unwrap();

    assert_eq!(events(&output), events("<div>\n漢字\n</div>\n\n한자\n"));
}

#[test]
fn nested_inline_html_korean_lang_overrides_non_korean_ancestor() {
    let output = convert_markdown(
        r#"<span lang="ja">漢字 <span lang=ko>漢字</span></span> 漢字"#,
        &dictionary(),
        RenderMode::HangulOnly,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events(r#"<span lang="ja">漢字 <span lang=ko>한자</span></span> 한자"#)
    );
}

#[test]
fn block_scopes_reset_homophone_marking() {
    let output = convert_markdown(
        "布告하다\n\n佈告하다\n\n- 布告하다 佈告하다\n",
        &dictionary(),
        RenderMode::HangulOnly,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events("포고하다\n\n포고하다\n\n- 포고하다(布告하다) 포고하다(佈告하다)\n")
    );
}

#[test]
fn markdown_scope_data_exposes_effective_flags() {
    let tokens = read_markdown(r#"<span lang="ja"><span lang=ko>漢字</span></span>"#);
    let scopes = tokens
        .into_iter()
        .filter_map(|token| match token {
            gukhanmun_core::InputToken::Open(scope) => Some(scope.into_data()),
            _ => None,
        })
        .collect::<Vec<MarkdownScopeData>>();

    assert!(scopes.iter().any(MarkdownScopeData::is_preserve));
    assert!(scopes.iter().any(|scope| !scope.is_preserve()));
}

#[test]
fn writer_preserves_rendered_verbatim_as_raw_markdown() {
    let output = write_markdown([RenderedToken::<MarkdownScopeData>::Verbatim(
        "<span>漢字</span>".into(),
    )])
    .unwrap();

    assert_eq!(output, "<span>漢字</span>");
}

proptest! {
    #[test]
    fn simple_hangul_markdown_roundtrips_semantically(
        heading in "[A-Za-z0-9가-힣 ]{0,24}",
        paragraph in "[A-Za-z0-9가-힣 .,!?]{0,48}",
        item in "[A-Za-z0-9가-힣 .,!?]{0,32}",
    ) {
        let input = format!("# {heading}\n\n{paragraph}\n\n- {item}\n");
        let rendered = read_markdown(&input)
            .into_iter()
            .map(|token| match token {
                gukhanmun_core::InputToken::Open(scope) => gukhanmun_core::RenderedToken::Open(scope),
                gukhanmun_core::InputToken::Close => gukhanmun_core::RenderedToken::Close,
                gukhanmun_core::InputToken::Text(text) => gukhanmun_core::RenderedToken::Text(text),
                gukhanmun_core::InputToken::Verbatim(text) => gukhanmun_core::RenderedToken::Verbatim(text),
            });

        let output = write_markdown(rendered).unwrap();
        prop_assert_eq!(events(&output), events(&input));
    }
}
