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

use gukhanmun_core::{
    HanjaDictionary, MapDictionary, Match, OriginalGloss, RenderMode, RenderOptions, RenderedToken,
    RubyBase,
};
use gukhanmun_markdown::{
    MarkdownError, MarkdownScopeData, MarkdownVariant, convert_markdown, read_markdown,
    read_markdown_iter, write_markdown,
};
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

struct ContextOnlyDictionary(MapDictionary);

impl HanjaDictionary for ContextOnlyDictionary {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        self.0.matches_at(s)
    }

    fn max_word_chars(&self) -> Option<usize> {
        self.0.max_word_chars()
    }
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
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert_eq!(events(&output), events("# 한자\n\n- 베이징 and **한자**\n"));
}

#[test]
fn adjacent_markdown_text_events_are_merged_before_conversion() {
    let output = convert_markdown(
        "漢&#23383;\n",
        &dictionary(),
        RenderMode::HangulHanjaParens,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert_eq!(events(&output), events("한자(漢字)\n"));
}

#[test]
fn code_span_and_code_block_are_not_converted() {
    let output = convert_markdown(
        "`漢字`\n\n```text\n北京\n```\n\n漢字\n",
        &dictionary(),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
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
        MarkdownVariant::CommonMark,
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
        MarkdownVariant::CommonMark,
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
        MarkdownVariant::CommonMark,
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
        MarkdownVariant::CommonMark,
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
        MarkdownVariant::CommonMark,
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
        MarkdownVariant::CommonMark,
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
        MarkdownVariant::CommonMark,
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
        &ContextOnlyDictionary(dictionary()),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events("포고하다\n\n포고하다\n\n- 포고하다(布告하다) 포고하다(佈告하다)\n")
    );
}

#[test]
fn markdown_scope_data_exposes_effective_flags() {
    let tokens = read_markdown(
        r#"<span lang="ja"><span lang=ko>漢字</span></span>"#,
        MarkdownVariant::CommonMark,
    );
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

#[test]
fn markdown_reader_iterator_matches_vec_reader() {
    let input = "# 漢字\n\n<q lang=ja>北京</q>\n";

    assert_eq!(
        read_markdown_iter(input, MarkdownVariant::CommonMark).collect::<Vec<_>>(),
        read_markdown(input, MarkdownVariant::CommonMark)
    );
}

#[test]
fn markdown_error_preserves_serialization_source() {
    let error = MarkdownError::from(pulldown_cmark_to_cmark::Error::UnexpectedEvent);

    assert!(std::error::Error::source(&error).is_some());
}

proptest! {
    #[test]
    fn simple_hangul_markdown_roundtrips_semantically(
        heading in "[A-Za-z0-9가-힣 ]{0,24}",
        paragraph in "[A-Za-z0-9가-힣 .,!?]{0,48}",
        item in "[A-Za-z0-9가-힣 .,!?]{0,32}",
    ) {
        let input = format!("# {heading}\n\n{paragraph}\n\n- {item}\n");
        let rendered = read_markdown(&input, MarkdownVariant::CommonMark)
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

#[test]
fn gfm_table_converts_hanja_in_cells() {
    let output = convert_markdown(
        "| 頭 | 尾 |\n|---|---|\n| 東 | 西 |\n",
        &dictionary(),
        RenderMode::HangulOnly,
        MarkdownVariant::Gfm,
    )
    .unwrap();

    assert!(
        !output.contains("\\|"),
        "table pipes must not be escaped: {output}"
    );
    assert!(
        output.contains("두"),
        "header cell should be converted: {output}"
    );
    assert!(
        output.contains("동"),
        "body cell should be converted: {output}"
    );
}

#[test]
fn ruby_on_hangul_emits_inline_html_in_paragraph() {
    let output = convert_markdown(
        "漢字 만세\n",
        &dictionary(),
        RenderMode::Ruby(RubyBase::OnHangul),
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events("<ruby>한자<rt>漢字</rt></ruby> 만세\n")
    );
}

#[test]
fn ruby_on_hanja_emits_inline_html_with_hanja_base() {
    let output = convert_markdown(
        "漢字\n",
        &dictionary(),
        RenderMode::Ruby(RubyBase::OnHanja),
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert_eq!(events(&output), events("<ruby>漢字<rt>한자</rt></ruby>\n"));
}

#[test]
fn ruby_mode_does_not_touch_code_span_or_block() {
    let output = convert_markdown(
        "`漢字`\n\n```text\n北京\n```\n\n漢字\n",
        &dictionary(),
        RenderMode::Ruby(RubyBase::OnHangul),
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events("`漢字`\n\n```text\n北京\n```\n\n<ruby>한자<rt>漢字</rt></ruby>\n")
    );
}

#[test]
fn ruby_inside_inline_html_text_only_element_falls_back_to_parens() {
    let output = convert_markdown(
        "Paragraph with <option>漢字</option> inline and 漢字.\n",
        &dictionary(),
        RenderMode::Ruby(RubyBase::OnHangul),
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events(
            "Paragraph with <option>한자(漢字)</option> inline and <ruby>한자<rt>漢字</rt></ruby>.\n"
        )
    );
}

#[test]
fn ruby_inside_emphasis_within_text_only_inline_html_falls_back_to_parens() {
    // Emphasis adds a nested container scope; without ancestor-aware
    // policy the renderer would see only the emphasis (allows markup) and
    // emit ruby inside <option>, which is invalid HTML.
    let output = convert_markdown(
        "Paragraph <option>**漢字**</option> and 漢字.\n",
        &dictionary(),
        RenderMode::Ruby(RubyBase::OnHangul),
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events("Paragraph <option>**한자(漢字)**</option> and <ruby>한자<rt>漢字</rt></ruby>.\n")
    );
}

#[test]
fn ruby_writer_escapes_hostile_dictionary_readings() {
    let mut dict = MapDictionary::new();
    dict.insert("漢字", "<script>alert(1)</script>");

    let output = convert_markdown(
        "漢字\n",
        &dict,
        RenderMode::Ruby(RubyBase::OnHangul),
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert_eq!(
        events(&output),
        events("<ruby>&lt;script&gt;alert(1)&lt;/script&gt;<rt>漢字</rt></ruby>\n")
    );
}

#[test]
#[tracing_test::traced_test]
fn inline_html_close_tag_without_open_emits_debug_event() {
    let _tokens = read_markdown("foo </span> bar", MarkdownVariant::CommonMark);
    assert!(logs_contain("unmatched inline HTML close tag"));
}

#[test]
fn original_with_ruby_gloss_renders_required_hangul_as_inline_html() {
    let mut dict = MapDictionary::new();
    dict.insert_marked(
        "漢字",
        "한자",
        gukhanmun_core::MatchMark {
            require_hanja: false,
            require_hangul: true,
        },
    );
    let options = RenderOptions {
        mode: RenderMode::Original,
        original_gloss: OriginalGloss::Ruby,
    };

    let output = convert_markdown("漢字\n", &dict, options, MarkdownVariant::CommonMark).unwrap();

    assert_eq!(events(&output), events("<ruby>漢字<rt>한자</rt></ruby>\n"));
}
