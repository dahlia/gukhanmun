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
    HanjaDictionary, InputToken, MapDictionary, Match, OriginalGloss, RenderMode, RenderOptions,
    RenderedToken, RubyBase,
};
use gukhanmun_markdown::{
    MarkdownError, MarkdownScopeData, MarkdownVariant, convert_markdown, read_markdown,
    read_markdown_iter, write_markdown, write_markdown_to_fmt,
};
use proptest::prelude::*;
use pulldown_cmark::{Event, Options, Parser, Tag};

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

fn markdown_options(variant: MarkdownVariant) -> Options {
    match variant {
        MarkdownVariant::CommonMark => Options::empty(),
        MarkdownVariant::Gfm => {
            Options::ENABLE_TABLES
                | Options::ENABLE_FOOTNOTES
                | Options::ENABLE_STRIKETHROUGH
                | Options::ENABLE_TASKLISTS
                | Options::ENABLE_GFM
        }
    }
}

fn events(markdown: &str) -> Vec<Event<'static>> {
    Parser::new(markdown).map(Event::into_static).collect()
}

/// Parses `markdown` and merges adjacent text events, so character references
/// that `pulldown-cmark` decodes into several text pieces compare equal to the
/// original single run.
fn normalized_events(markdown: &str, variant: MarkdownVariant) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut pending_text = String::new();
    for event in Parser::new_ext(markdown, markdown_options(variant)) {
        match event {
            Event::Text(text) => pending_text.push_str(&text),
            other => {
                if !pending_text.is_empty() {
                    normalized.push(format!("text:{pending_text:?}"));
                    pending_text.clear();
                }
                normalized.push(format!("{other:?}"));
            }
        }
    }
    if !pending_text.is_empty() {
        normalized.push(format!("text:{pending_text:?}"));
    }
    normalized
}

/// Replaces every text run in `template` with `payload` and writes the result.
///
/// The template keeps the reader-provided structure (paragraph, heading, table
/// cell), so the writer sees a realistic scope stack.
fn write_with_payload(template: &str, variant: MarkdownVariant, payload: &str) -> String {
    let tokens = read_markdown(template, variant)
        .into_iter()
        .map(|token| match token {
            InputToken::Open(scope) => RenderedToken::Open(scope),
            InputToken::Close => RenderedToken::Close,
            InputToken::Text(_) => RenderedToken::Text(payload.to_owned()),
            InputToken::Verbatim(text) => RenderedToken::Verbatim(text),
        })
        .collect::<Vec<_>>();
    write_markdown(tokens).unwrap()
}

/// Concatenates the literal text of `markdown`, turning soft and hard breaks
/// into newlines.  Used to assert that no markup re-formed.
fn text_content(markdown: &str, variant: MarkdownVariant) -> String {
    let mut content = String::new();
    for event in Parser::new_ext(markdown, markdown_options(variant)) {
        match event {
            Event::Text(text) | Event::Code(text) | Event::InlineHtml(text) | Event::Html(text) => {
                content.push_str(&text);
            }
            Event::SoftBreak | Event::HardBreak => content.push('\n'),
            _ => {}
        }
    }
    content
}

fn assert_no_markup(markdown: &str, variant: MarkdownVariant) {
    for event in Parser::new_ext(markdown, markdown_options(variant)) {
        match event {
            Event::Start(tag) => assert!(
                matches!(tag, pulldown_cmark::Tag::Paragraph),
                "unexpected start tag in output: {tag:?}\n{markdown}"
            ),
            Event::End(tag) => assert!(
                matches!(tag, pulldown_cmark::TagEnd::Paragraph),
                "unexpected end tag in output: {tag:?}\n{markdown}"
            ),
            Event::Html(_) | Event::InlineHtml(_) | Event::Code(_) => {
                panic!("unexpected inline markup in output: {markdown}")
            }
            _ => {}
        }
    }
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
fn fmt_writer_matches_string_writer() {
    let rendered = vec![RenderedToken::<MarkdownScopeData>::Verbatim(
        "<span>漢字</span>".into(),
    )];
    let buffered = write_markdown(rendered.clone()).unwrap();
    let mut streamed = String::new();

    write_markdown_to_fmt(rendered, &mut streamed).unwrap();

    assert_eq!(streamed, buffered);
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
        events("<ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby> 만세\n")
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

    assert_eq!(
        events(&output),
        events("<ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>\n")
    );
}

#[test]
fn ruby_inside_shortcut_reference_link_preserves_definition_label() {
    let mut dict = dictionary();
    dict.insert("大韓民國", "대한민국");
    dict.insert("憲法", "헌법");

    let output = convert_markdown(
        "[大韓民國憲法]\n\n[大韓民國憲法]: https://example.com/law\n",
        &dict,
        RenderMode::Ruby(RubyBase::OnHanja),
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert!(
        output.contains("[<ruby>大韓民國<rp>(</rp><rt>대한민국</rt><rp>)</rp></ruby><ruby>憲法<rp>(</rp><rt>헌법</rt><rp>)</rp></ruby>][大韓民國憲法]"),
        "converted shortcut link should keep a non-empty reference label: {output}"
    );
    assert!(
        output.contains("[大韓民國憲法]: https://example.com/law"),
        "reference definition label should be preserved: {output}"
    );
    assert!(
        !output.contains("[]:"),
        "writer must not emit an empty reference definition: {output}"
    );
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
        events(
            "`漢字`\n\n```text\n北京\n```\n\n<ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>\n"
        )
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
            "Paragraph with <option>한자(漢字)</option> inline and <ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>.\n"
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
        events(
            "Paragraph <option>**한자(漢字)**</option> and <ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>.\n"
        )
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
        events("<ruby>&lt;script>alert(1)&lt;/script><rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>\n")
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

    assert_eq!(
        events(&output),
        events("<ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>\n")
    );
}

#[test]
fn inline_html_lang_ja_preserves_hanja_same_as_html_adapter() {
    // Both the Markdown and HTML adapters should agree: text inside a
    // `lang="ja"` inline element is not Korean, so hanja is not converted.
    let dict = dictionary();
    // Use real inline HTML inside a Markdown paragraph so pulldown-cmark
    // emits Event::InlineHtml, exercising classify_inline_html integration.
    let input_md = "x <span lang=\"ja\">漢字</span> 漢字\n";
    let md_out = convert_markdown(
        input_md,
        &dict,
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    // The hanja inside lang=ja must pass through unchanged.
    assert!(
        md_out.contains("漢字"),
        "Markdown adapter must preserve hanja inside lang=ja scope, got: {md_out:?}"
    );
    // The hanja outside lang=ja must be converted.
    assert!(
        md_out.contains("한자"),
        "Markdown adapter must convert hanja outside lang=ja scope, got: {md_out:?}"
    );
}

#[test]
fn inline_html_code_element_preserves_hanja_same_as_html_adapter() {
    // Both adapters must agree: text inside `<code>` is verbatim and not
    // converted, while hanja outside `<code>` in the same paragraph is.
    let dict = dictionary();
    let input_md = "<code>漢字</code> 漢字\n";
    let md_out = convert_markdown(
        input_md,
        &dict,
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    // The hanja inside <code> must survive unchanged.
    assert!(
        md_out.contains("漢字"),
        "Markdown adapter must preserve hanja inside code element, got: {md_out:?}"
    );
    // The hanja outside <code> must be converted.
    assert!(
        md_out.contains("한자"),
        "Markdown adapter must convert hanja outside code element, got: {md_out:?}"
    );
}

fn write_reader_tokens(input: &str, variant: MarkdownVariant) -> String {
    let tokens = read_markdown(input, variant)
        .into_iter()
        .map(|token| match token {
            InputToken::Open(scope) => RenderedToken::Open(scope),
            InputToken::Close => RenderedToken::Close,
            InputToken::Text(text) => RenderedToken::Text(text),
            InputToken::Verbatim(text) => RenderedToken::Verbatim(text),
        })
        .collect::<Vec<_>>();
    write_markdown(tokens).unwrap()
}

fn escape_ascii_punctuation(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() * 2);
    for ch in text.chars() {
        if ch.is_ascii_punctuation() {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

#[test]
fn escaped_source_text_roundtrips_literally() {
    let input = "a \\<img src=x onerror=alert(1)> b\n\n\
                 c &lt;script&gt;alert(1)&lt;/script&gt; d\n\n\
                 e \\[x\\](y) f\n";
    let empty = MapDictionary::new();
    for variant in [MarkdownVariant::CommonMark, MarkdownVariant::Gfm] {
        let output = convert_markdown(input, &empty, RenderMode::HangulOnly, variant).unwrap();
        assert_eq!(
            normalized_events(&output, variant),
            normalized_events(input, variant),
            "output did not round-trip ({variant:?}): {output:?}"
        );
        assert_no_markup(&output, variant);
    }
    let output = convert_markdown(
        input,
        &empty,
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert!(!output.contains("<img"), "raw image leaked: {output:?}");
    assert!(!output.contains("<script"), "raw script leaked: {output:?}");
}

#[test]
fn markdown_writer_escapes_line_starts() {
    let paragraph = "x\n";
    for (payload, expected) in [
        ("# x", "&#35; x"),
        ("- x", "&#45; x"),
        ("+ x", "&#43; x"),
        ("= x", "&#61; x"),
        ("> x", "&#62; x"),
        (": x", "&#58; x"),
        ("| --- |", "&#124; --- |"),
        ("1. x", "1\\. x"),
        ("0) x", "0\\) x"),
        ("10. x", "10\\. x"),
        ("3.14", "3.14"),
        ("1234567890. x", "1234567890. x"),
    ] {
        let output = write_with_payload(paragraph, MarkdownVariant::CommonMark, payload);
        assert_eq!(
            output.trim_end_matches('\n'),
            expected,
            "unexpected encoding for {payload:?}"
        );
        assert_no_markup(&output, MarkdownVariant::CommonMark);
        assert_eq!(text_content(&output, MarkdownVariant::CommonMark), payload);
    }

    let output = write_with_payload(paragraph, MarkdownVariant::CommonMark, "a\n- b\n> c\n1. d");
    assert_no_markup(&output, MarkdownVariant::CommonMark);
    assert_eq!(
        text_content(&output, MarkdownVariant::CommonMark),
        "a\n- b\n> c\n1. d"
    );
}

#[test]
fn markdown_writer_escapes_interior_markup() {
    let payload = "<img src=x onerror=alert(1)> & [x](y) `c` *e* _s_ ~d~ | p \\ end";
    let output = write_with_payload("x\n", MarkdownVariant::CommonMark, payload);
    assert_no_markup(&output, MarkdownVariant::CommonMark);
    assert_eq!(text_content(&output, MarkdownVariant::CommonMark), payload);
}

#[test]
fn markdown_writer_preserves_trailing_bang_before_link() {
    let input = "**a**\\![x](y)\n";
    let output = convert_markdown(
        input,
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert_eq!(
        normalized_events(&output, MarkdownVariant::CommonMark),
        normalized_events(input, MarkdownVariant::CommonMark)
    );
    assert!(
        !output.contains("!["),
        "literal bang and link merged into an image: {output:?}"
    );
}

#[test]
fn markdown_writer_escapes_heading_line_starts() {
    let heading = "# x\n";
    assert_eq!(
        write_with_payload(heading, MarkdownVariant::CommonMark, "1. 서론"),
        "# 1\\. 서론"
    );
    assert_eq!(
        write_with_payload(heading, MarkdownVariant::CommonMark, "- 항목"),
        "# &#45; 항목"
    );
    assert_eq!(
        write_with_payload(heading, MarkdownVariant::CommonMark, "foo # "),
        "# foo &#35;&#32;"
    );

    // Text that ends a run only because of inline markup must not look like a
    // trailing heading space.
    for input in [
        "# 서론 **강조** 끝\n",
        "# 서론 [링크](https://example.com) 끝\n",
        "# 漢字 學校\n",
    ] {
        let output = convert_markdown(
            input,
            &dictionary_preserving_hanja(),
            RenderMode::HangulOnly,
            MarkdownVariant::CommonMark,
        )
        .unwrap();
        assert!(
            !output.contains("&#32;"),
            "ordinary heading space was encoded: {output:?}"
        );
    }
}

fn dictionary_preserving_hanja() -> MapDictionary {
    let mut dict = MapDictionary::new();
    dict.insert("漢字", "한자");
    dict.insert("學校", "학교");
    dict
}

#[test]
fn markdown_writer_does_not_churn_ordinary_paragraphs() {
    let empty = MapDictionary::new();
    for input in [
        "**굵게** - 설명 + 더하기 = 합\n",
        "[링크](https://example.com) 1. 참조\n",
        "3.14는 원주율이다.\n",
    ] {
        let output = convert_markdown(
            input,
            &empty,
            RenderMode::HangulOnly,
            MarkdownVariant::CommonMark,
        )
        .unwrap();
        assert_eq!(
            output,
            input.trim_end_matches('\n'),
            "ordinary paragraph churned"
        );
    }
}

#[test]
fn markdown_writer_preserves_autolinks_and_code_blocks() {
    let input = "Visit <https://example.com/a_b?x=1&y=2> for more.\n\n\
                 ```\n& < > * _ [ ] |\n```\n";
    let output = convert_markdown(
        input,
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert_eq!(
        normalized_events(&output, MarkdownVariant::CommonMark),
        normalized_events(input, MarkdownVariant::CommonMark)
    );
}

#[test]
fn gfm_table_cell_pipes_roundtrip_literally() {
    let input = "| a | b |\n|---|---|\n| c \\| d | e |\n";
    let output = convert_markdown(
        input,
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        MarkdownVariant::Gfm,
    )
    .unwrap();
    assert_eq!(
        normalized_events(&output, MarkdownVariant::Gfm),
        normalized_events(input, MarkdownVariant::Gfm)
    );
    assert!(
        !output.contains("\\\\|"),
        "table pipe was double-escaped: {output:?}"
    );
}

#[test]
fn markdown_conversion_is_idempotent_for_escaped_source() {
    let input = "a \\<img src=x onerror=alert(1)> b\n\n\
                 c &lt;script&gt;alert(1)&lt;/script&gt; d\n\n\
                 e \\[x\\](y) f\n";
    let empty = MapDictionary::new();
    let first = convert_markdown(
        input,
        &empty,
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    let second = convert_markdown(
        &first,
        &empty,
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert_eq!(first, second);
}

#[test]
fn inline_script_text_is_escaped_not_reemitted_raw() {
    let input = "x <script>a &lt;/script&gt;&lt;img src=x onerror=alert(1)&gt;</script> y\n";
    let output = convert_markdown(
        input,
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert!(
        !output.contains("</script><img"),
        "decoded script content broke out of the element: {output:?}"
    );
    assert!(
        output.contains("&lt;/script"),
        "decoded closing tag was not escaped: {output:?}"
    );
    assert!(
        output.contains("&lt;img"),
        "decoded image tag was not escaped: {output:?}"
    );
}

#[test]
fn line_feed_text_cannot_terminate_a_heading() {
    let input = "# a&#10;b\n";
    let output = convert_markdown(
        input,
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert!(
        output.contains("&#10;"),
        "literal line feed was not encoded: {output:?}"
    );
    assert_eq!(
        normalized_events(&output, MarkdownVariant::CommonMark),
        normalized_events(input, MarkdownVariant::CommonMark)
    );
}

#[test]
fn carriage_return_text_cannot_start_a_block() {
    let input = "a&#13;# x\n";
    let output = convert_markdown(
        input,
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert!(
        output.contains("&#13;"),
        "literal carriage return was not encoded: {output:?}"
    );
    assert_eq!(
        normalized_events(&output, MarkdownVariant::CommonMark),
        normalized_events(input, MarkdownVariant::CommonMark)
    );
}

#[test]
fn gfm_table_cell_backslash_does_not_merge_cells() {
    let input = "| a | b |\n|---|---|\n| x&#92; | y |\n";
    let output = convert_markdown(
        input,
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        MarkdownVariant::Gfm,
    )
    .unwrap();
    assert!(
        output.contains("&#92;"),
        "table backslash was not encoded: {output:?}"
    );
    assert_eq!(
        normalized_events(&output, MarkdownVariant::Gfm),
        normalized_events(input, MarkdownVariant::Gfm)
    );
}

#[test]
fn markdown_writer_does_not_encode_interior_double_spaces() {
    let empty = MapDictionary::new();
    for input in ["문장이다.  text\n", "문장이다.  **강조**\n"] {
        let output = convert_markdown(
            input,
            &empty,
            RenderMode::HangulOnly,
            MarkdownVariant::CommonMark,
        )
        .unwrap();
        assert!(
            !output.contains("&#32;"),
            "interior double space was encoded: {output:?}"
        );
        assert_eq!(
            normalized_events(&output, MarkdownVariant::CommonMark),
            normalized_events(input, MarkdownVariant::CommonMark)
        );
    }
}

#[test]
fn markdown_writer_preserves_heading_interior_hashes() {
    let empty = MapDictionary::new();
    for input in ["### C# text\n", "### C# **입문**\n"] {
        let output = convert_markdown(
            input,
            &empty,
            RenderMode::HangulOnly,
            MarkdownVariant::CommonMark,
        )
        .unwrap();
        assert!(
            !output.contains("&#35;"),
            "interior heading hash was encoded: {output:?}"
        );
        assert_eq!(
            normalized_events(&output, MarkdownVariant::CommonMark),
            normalized_events(input, MarkdownVariant::CommonMark)
        );
    }
}

#[test]
fn markdown_writer_preserves_space_before_inline_html_in_heading() {
    let empty = MapDictionary::new();
    let input = "# <span lang=\"ja\">漢字 </span>끝\n";
    let output = convert_markdown(
        input,
        &empty,
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert!(
        !output.contains("&#32;"),
        "space inside inline HTML was encoded: {output:?}"
    );
    assert_eq!(
        normalized_events(&output, MarkdownVariant::CommonMark),
        normalized_events(input, MarkdownVariant::CommonMark)
    );
}

#[test]
fn markdown_writer_escapes_marker_spanning_annotation_runs() {
    let mut dict = MapDictionary::new();
    dict.insert("一", "1");
    let output = convert_markdown(
        "一. 서론\n",
        &dict,
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert_no_markup(&output, MarkdownVariant::CommonMark);
    // The numeral is an annotation, so its boundaries carry U+200B. That keeps
    // the following `.` from completing an ordered-list marker; the character
    // is invisible and does not change the rendered text.
    assert_eq!(
        text_content(&output, MarkdownVariant::CommonMark).replace('\u{200b}', ""),
        "1. 서론"
    );
}

#[test]
fn trailing_entity_space_before_soft_break_is_preserved() {
    let empty = MapDictionary::new();
    for (input, expected) in [("a&#32;\nb\n", "a&#32;"), ("a&#9;\nb\n", "a&#9;")] {
        let output = convert_markdown(
            input,
            &empty,
            RenderMode::HangulOnly,
            MarkdownVariant::CommonMark,
        )
        .unwrap();
        assert!(
            output.contains(expected),
            "trailing whitespace was not encoded: {output:?}"
        );
        assert_eq!(
            normalized_events(&output, MarkdownVariant::CommonMark),
            normalized_events(input, MarkdownVariant::CommonMark)
        );
    }
}

proptest! {
    #[test]
    fn escaped_punctuation_markdown_roundtrips_literally(
        raw in "[!-~가-힣 ]{0,48}",
    ) {
        let raw = raw.trim().to_owned();
        if raw.is_empty() {
            return Ok(());
        }
        let input = escape_ascii_punctuation(&raw);
        let output = write_reader_tokens(&input, MarkdownVariant::CommonMark);
        assert_no_markup(&output, MarkdownVariant::CommonMark);
        prop_assert_eq!(
            text_content(&output, MarkdownVariant::CommonMark),
            raw
        );
    }
}

// ── GHSA-j87h-8h88-wm6g: annotation text must stay literal ───────────────────

fn hostile_reading(reading: &str) -> MapDictionary {
    let mut dict = MapDictionary::new();
    dict.insert("漢字", reading);
    dict
}

/// Parses `markdown` and fails if annotation-derived text formed any inline or
/// block construct. Ruby markup from the adapter is the only inline HTML that
/// may appear.
fn assert_annotation_renders_as_text(markdown: &str, expected_text: &str) {
    let parsed: Vec<Event<'_>> = Parser::new(markdown).collect();
    for event in &parsed {
        match event {
            Event::Start(Tag::Link { .. })
            | Event::Start(Tag::Image { .. })
            | Event::Start(Tag::Emphasis)
            | Event::Start(Tag::Strong)
            | Event::Start(Tag::Strikethrough)
            | Event::Start(Tag::CodeBlock(_))
            | Event::Start(Tag::Heading { .. })
            | Event::Start(Tag::List(_))
            | Event::Code(_)
            | Event::Html(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_) => {
                panic!("annotation text formed markup in {markdown:?}: {event:?}");
            }
            Event::InlineHtml(html) => {
                assert!(
                    html.starts_with("<ruby>")
                        || html.starts_with("</ruby>")
                        || html.starts_with("<rp>")
                        || html.starts_with("<rt>")
                        || html.starts_with("</rt>")
                        || html.starts_with("</rp>"),
                    "unexpected inline HTML in {markdown:?}: {html:?}"
                );
            }
            _ => {}
        }
    }
    let text: String = parsed
        .iter()
        .filter_map(|event| match event {
            Event::Text(text) => Some(text.as_ref()),
            _ => None,
        })
        .collect();
    assert!(
        text.contains(expected_text),
        "decoded text {text:?} does not contain {expected_text:?} (markdown: {markdown:?})"
    );
}

#[test]
fn markdown_readings_render_as_literal_text() {
    for reading in [
        "<script>alert(1)</script>",
        "safe <img src=x onerror=alert(1)>",
        "[x](javascript:alert(1))",
        "*emphasis*",
        "`code`",
        "~~strike~~",
        "# heading",
        "1. item",
        "a|b",
        "\" onmouseover=\"x",
        "{onclick=x}",
        "$x$",
        "a\nb",
    ] {
        let output = convert_markdown(
            "漢字\n",
            &hostile_reading(reading),
            RenderMode::HangulOnly,
            MarkdownVariant::CommonMark,
        )
        .unwrap();
        let expected = reading.split('\n').next().unwrap();
        assert_annotation_renders_as_text(&output, expected);
    }
}

#[test]
fn markdown_annotation_cannot_complete_a_source_tag() {
    let output = convert_markdown(
        "foo <漢字>\n",
        &hostile_reading("img src=x onerror=alert(1)"),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    // The all-ASCII reading is isolated on both sides, so the stray `<` cannot
    // be completed into a tag.
    assert_annotation_renders_as_text(&output, "img src=x onerror=alert(1)");
}

#[test]
fn markdown_empty_reading_cannot_complete_a_source_tag() {
    let output = convert_markdown(
        "foo <漢字img src=x onerror=alert(1)>\n",
        &hostile_reading(""),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    // The zero width space keeps the stray `<` from starting a tag, so the
    // decoded text is `<` + U+200B + the rest.
    assert_annotation_renders_as_text(&output, "img src=x onerror=alert(1)>");
}

#[test]
fn markdown_ruby_escapes_hostile_markdown_in_readings() {
    let output = convert_markdown(
        "漢字\n",
        &hostile_reading("[x](javascript:alert(1))"),
        RenderMode::Ruby(RubyBase::OnHangul),
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert_annotation_renders_as_text(&output, "[x](javascript:alert(1))");
}

#[test]
fn markdown_link_label_keeps_the_original_link_only() {
    let output = convert_markdown(
        "[漢字](https://example.com)\n",
        &hostile_reading("x](javascript:alert(1))[y"),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    let links: Vec<_> = Parser::new(&output)
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(links, ["https://example.com"]);
}

#[test]
fn markdown_readings_cannot_form_gfm_autolinks() {
    let reading =
        "한자 https://evil.example/ and www.evil.example and WWW.evil.example and a@b.example";
    let output = convert_markdown(
        "漢字\n",
        &hostile_reading(reading),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    // Some GFM parsers decode character references before looking for
    // autolinks, so the encoder also inserts U+200B after every
    // autolink-triggering character. Assert both halves of that contract.
    assert!(
        !output.contains("://"),
        "autolink scheme survived: {output:?}"
    );
    assert!(!output.contains("www."), "www host survived: {output:?}");
    assert!(!output.contains("a@b"), "e-mail survived: {output:?}");
    assert!(
        output.contains("&#8203;"),
        "boundary was not inserted: {output:?}"
    );

    let decoded: String = Parser::new(&output)
        .filter_map(|event| match event {
            Event::Text(text) => Some(text.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(decoded.replace('\u{200b}', ""), reading);
}

#[test]
fn markdown_self_closing_raw_text_elements_do_not_close_outside_foreign_content() {
    let output = convert_markdown(
        "Text <script/>漢字</script>\n",
        &dictionary(),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert!(
        output.contains("漢字"),
        "script content must stay preserved: {output:?}"
    );
}

#[test]
fn markdown_self_closing_raw_text_inside_foreign_content_stays_preserved() {
    // Same conservative choice as the HTML adapter: a raw-text tag with a
    // trailing slash stays open, which preserves text instead of converting it.
    let output = convert_markdown(
        "<svg><style/>漢字</svg>\n",
        &dictionary(),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();

    assert!(
        output.contains("漢字"),
        "text after <style/> must stay preserved: {output:?}"
    );
}

#[test]
fn markdown_annotation_boundaries_cannot_complete_an_autolink() {
    // A reading that is itself an autolink prefix must not be completed by the
    // source text that follows it.
    let output = convert_markdown(
        "漢字.evil.example\n",
        &hostile_reading("www"),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert!(
        output.contains("&#8203;"),
        "trailing boundary separator missing: {output:?}"
    );

    let scheme = convert_markdown(
        "漢字://evil.example\n",
        &hostile_reading("http"),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert!(
        scheme.contains("&#8203;"),
        "trailing boundary separator missing: {scheme:?}"
    );

    // A reading that starts with autolink punctuation must not continue a
    // scheme from the source text before it.
    let continuation = convert_markdown(
        "http漢字evil.example\n",
        &hostile_reading("://"),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert!(
        continuation.contains("&#8203;"),
        "leading boundary separator missing: {continuation:?}"
    );

    // An all-ASCII reading can be a fragment of a trigger on both sides.
    for (reading, input) in [
        ("s", "http漢字://evil.example\n"),
        ("ttp", "h漢字://evil.example\n"),
        ("w.", "ww漢字evil.example\n"),
    ] {
        let output = convert_markdown(
            input,
            &hostile_reading(reading),
            RenderMode::HangulOnly,
            MarkdownVariant::CommonMark,
        )
        .unwrap();
        assert!(
            output.contains("&#8203;"),
            "fragment boundary was not isolated: {output:?}"
        );
    }
}

#[test]
fn markdown_annotation_boundaries_cannot_complete_an_email_autolink() {
    // Source supplies `local@`; the reading supplies the domain.
    let domain = convert_markdown(
        "abc@漢字\n",
        &hostile_reading("evil.example"),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert!(
        domain.contains("&#8203;"),
        "email domain boundary was not isolated: {domain:?}"
    );

    // The reading supplies the local part; source supplies `@domain`.
    let local = convert_markdown(
        "漢字@evil.example\n",
        &hostile_reading("한글 abc"),
        RenderMode::HangulOnly,
        MarkdownVariant::CommonMark,
    )
    .unwrap();
    assert!(
        local.contains("&#8203;"),
        "email local-part boundary was not isolated: {local:?}"
    );
}

#[test]
fn markdown_html_elements_inside_foreign_content_restore_html_raw_text_rules() {
    let mut dict = MapDictionary::new();
    dict.insert("漢字", "가=1,alert(1)//");

    for input in [
        "<svg><b><script/>漢字</script></b></svg>\n",
        "<svg><foreignObject><script/>漢字</script></foreignObject></svg>\n",
        "<math><mi><script/>漢字</script></mi></math>\n",
        "<svg><br><script/>漢字</script></svg>\n",
        "<svg><b></b><script/>漢字</script></svg>\n",
        "<svg><img src=x><style/>漢字</style></svg>\n",
    ] {
        let output = convert_markdown(
            input,
            &dict,
            RenderMode::HangulOnly,
            MarkdownVariant::CommonMark,
        )
        .unwrap();
        assert!(
            !output.contains("가=1"),
            "reading escaped into script context for {input}: {output}"
        );
    }
}
