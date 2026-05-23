// Gukhanmun: HTML fragment adapter for Gukhanmun.
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
    ContextWindow, EngineOptions, HanjaDictionary, MapDictionary, Match, OriginalGloss, Recovery,
    RenderMode, RenderOptions, RubyBase, mark_homophones, process_tokens_iter_with_options,
    render_tokens_iter,
};
use gukhanmun_html::{
    HtmlElementInfo, HtmlError, HtmlReaderOptions, HtmlScopeData, convert_html_fragment,
    read_html_fragment, read_html_fragment_iter, read_html_fragment_with_options,
    try_convert_html_fragment, write_html_fragment,
};
use proptest::prelude::*;

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

#[test]
fn converts_hanja_inside_html_text_and_preserves_raw_attributes() {
    let output = convert_html_fragment(
        "<p class=foo data-x=\"1\">漢字</p>",
        &dictionary(),
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "<p class=foo data-x=\"1\">한자</p>");
}

#[test]
fn preserved_tags_are_not_converted() {
    let output = convert_html_fragment(
        "<p>漢字 <code>漢字</code><pre>北京</pre><textarea>漢字</textarea></p>",
        &dictionary(),
        RenderMode::HangulOnly,
    );

    assert_eq!(
        output,
        "<p>한자 <code>漢字</code><pre>北京</pre><textarea>漢字</textarea></p>"
    );
}

#[test]
fn unquoted_attribute_value_ending_with_slash_does_not_self_close_preserved_tag() {
    let output = convert_html_fragment(
        "<code data=http://x/>漢字</code><p>漢字</p>",
        &dictionary(),
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "<code data=http://x/>漢字</code><p>한자</p>");
}

#[test]
fn inherited_non_korean_lang_preserves_text_but_korean_child_overrides_it() {
    let output = convert_html_fragment(
        "<p lang=\"ja\">漢字 <span lang=ko-Hang>漢字</span></p><p>北京</p>",
        &dictionary(),
        RenderMode::HangulOnly,
    );

    assert_eq!(
        output,
        "<p lang=\"ja\">漢字 <span lang=ko-Hang>한자</span></p><p>베이징</p>"
    );
}

#[test]
fn unquoted_attribute_value_ending_with_slash_does_not_self_close_lang_scope() {
    let output = convert_html_fragment(
        "<span lang=ja/>漢字</span><p>漢字</p>",
        &dictionary(),
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "<span lang=ja/>漢字</span><p>한자</p>");
}

#[test]
fn comments_cdata_declarations_and_raw_text_are_preserved() {
    let output = convert_html_fragment(
        "<!doctype html><!-- 漢字 --><p><![CDATA[漢字]]></p><script>漢字</script><style>漢字</style>",
        &dictionary(),
        RenderMode::HangulOnly,
    );

    assert_eq!(
        output,
        "<!doctype html><!-- 漢字 --><p><![CDATA[漢字]]></p><script>漢字</script><style>漢字</style>"
    );
}

#[test]
fn raw_text_end_tag_prefix_does_not_leave_raw_text_mode() {
    let output = convert_html_fragment(
        "<script></scripted><div>漢字</script><p>漢字</p>",
        &dictionary(),
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "<script></scripted><div>漢字</script><p>한자</p>");
}

#[test]
fn void_and_self_closing_tags_do_not_gain_extra_end_tags() {
    let input = "<p>漢字<br><img src=\"a.jpg\"><em/></p>";
    let rendered = read_html_fragment(input)
        .into_iter()
        .map(|token| match token {
            gukhanmun_core::InputToken::Open(scope) => gukhanmun_core::RenderedToken::Open(scope),
            gukhanmun_core::InputToken::Close => gukhanmun_core::RenderedToken::Close,
            gukhanmun_core::InputToken::Text(text) => gukhanmun_core::RenderedToken::Text(text),
            gukhanmun_core::InputToken::Verbatim(text) => {
                gukhanmun_core::RenderedToken::Verbatim(text)
            }
        });

    assert_eq!(write_html_fragment(rendered), input);
}

#[test]
fn html_reader_iterator_matches_vec_reader() {
    let input = "<p lang=ko>漢字</p><pre>北京</pre>";

    assert_eq!(
        read_html_fragment_iter(input).collect::<Vec<_>>(),
        read_html_fragment(input)
    );
}

#[test]
fn block_scopes_reset_homophone_marking() {
    let output = convert_html_fragment(
        "<p>布告하다</p><p>佈告하다</p><div>布告하다 佈告하다</div>",
        &ContextOnlyDictionary(dictionary()),
        RenderMode::HangulOnly,
    );

    assert_eq!(
        output,
        "<p>포고하다</p><p>포고하다</p><div>포고하다(布告하다) 포고하다(佈告하다)</div>"
    );
}

#[test]
fn malformed_fragments_do_not_panic() {
    let output = convert_html_fragment(
        "<p>漢字 <1invalid> 北京 <![CDATA[漢字",
        &dictionary(),
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "<p>한자 <1invalid> 베이징 <![CDATA[한자");
}

#[test]
fn strict_recovery_reports_malformed_html() {
    let error = try_convert_html_fragment(
        "<p>漢字 <1invalid> 北京",
        &dictionary(),
        RenderMode::HangulOnly,
        Recovery::Strict,
    )
    .unwrap_err();

    assert!(matches!(error, HtmlError::MalformedTag { .. }));
}

#[test]
fn lenient_recovery_preserves_malformed_html_and_continues() {
    let output = try_convert_html_fragment(
        "<p>漢字 <1invalid> 北京",
        &dictionary(),
        RenderMode::HangulOnly,
        Recovery::Lenient,
    )
    .unwrap();

    assert_eq!(output, "<p>한자 <1invalid> 베이징");
}

#[test]
#[tracing_test::traced_test]
fn lenient_recovery_from_malformed_html_emits_warn_event() {
    use gukhanmun_html::try_read_html_fragment;
    let result = try_read_html_fragment("</>漢字", Recovery::Lenient);
    assert!(result.is_ok());
    assert!(logs_contain("recovering from malformed HTML fragment"));
}

proptest! {
    #[test]
    fn simple_valid_fragments_roundtrip_without_conversion(
        tag in "(p|div|span|em|strong)",
        attr in "(| class=foo| data-x=\"1\")",
        text in "[A-Za-z0-9가-힣 .,!?]{0,32}",
    ) {
        let input = format!("<{tag}{attr}>{text}</{tag}>");
        let rendered = read_html_fragment(&input)
            .into_iter()
            .map(|token| match token {
                gukhanmun_core::InputToken::Open(scope) => gukhanmun_core::RenderedToken::Open(scope),
                gukhanmun_core::InputToken::Close => gukhanmun_core::RenderedToken::Close,
                gukhanmun_core::InputToken::Text(text) => gukhanmun_core::RenderedToken::Text(text),
                gukhanmun_core::InputToken::Verbatim(text) => gukhanmun_core::RenderedToken::Verbatim(text),
            });

        prop_assert_eq!(write_html_fragment(rendered), input);
    }
}

#[test]
fn html_scope_data_exposes_effective_flags() {
    let tokens = read_html_fragment("<p lang=en><span lang=ko>漢字</span></p>");
    let scopes = tokens
        .into_iter()
        .filter_map(|token| match token {
            gukhanmun_core::InputToken::Open(scope) => Some(scope.into_data()),
            _ => None,
        })
        .collect::<Vec<HtmlScopeData>>();

    assert!(scopes[0].is_preserve());
    assert!(!scopes[1].is_preserve());
}

#[test]
fn ruby_on_hangul_emits_ruby_element_inside_paragraph() {
    let output = convert_html_fragment(
        "<p>漢字 만세</p>",
        &dictionary(),
        RenderMode::Ruby(RubyBase::OnHangul),
    );

    assert_eq!(output, "<p><ruby>한자<rt>漢字</rt></ruby> 만세</p>");
}

#[test]
fn ruby_on_hanja_emits_hanja_base_in_ruby_element() {
    let output = convert_html_fragment(
        "<p>漢字</p>",
        &dictionary(),
        RenderMode::Ruby(RubyBase::OnHanja),
    );

    assert_eq!(output, "<p><ruby>漢字<rt>한자</rt></ruby></p>");
}

#[test]
fn ruby_mode_leaves_preserved_tag_text_untouched() {
    let output = convert_html_fragment(
        "<p><code>漢字</code><pre>漢字</pre></p>",
        &dictionary(),
        RenderMode::Ruby(RubyBase::OnHangul),
    );

    assert_eq!(output, "<p><code>漢字</code><pre>漢字</pre></p>");
}

#[test]
fn ruby_mode_skips_non_korean_lang_scope() {
    let output = convert_html_fragment(
        "<p lang=\"ja\">漢字 <span lang=ko>漢字</span></p>",
        &dictionary(),
        RenderMode::Ruby(RubyBase::OnHangul),
    );

    assert_eq!(
        output,
        "<p lang=\"ja\">漢字 <span lang=ko><ruby>한자<rt>漢字</rt></ruby></span></p>"
    );
}

#[test]
fn ruby_emits_inline_markup_for_root_level_text() {
    let output = convert_html_fragment("漢字", &dictionary(), RenderMode::Ruby(RubyBase::OnHangul));

    assert_eq!(output, "<ruby>한자<rt>漢字</rt></ruby>");
}

#[test]
fn ruby_inside_text_only_elements_falls_back_to_parens() {
    let output = convert_html_fragment(
        "<title>漢字</title><p>漢字<option>漢字</option></p>",
        &dictionary(),
        RenderMode::Ruby(RubyBase::OnHangul),
    );

    assert_eq!(
        output,
        "<title>한자(漢字)</title><p><ruby>한자<rt>漢字</rt></ruby><option>한자(漢字)</option></p>"
    );
}

#[test]
fn ruby_writer_escapes_hostile_dictionary_readings() {
    let mut dict = MapDictionary::new();
    dict.insert("漢字", "<script>alert(1)</script>");

    let output = convert_html_fragment("<p>漢字</p>", &dict, RenderMode::Ruby(RubyBase::OnHangul));

    assert_eq!(
        output,
        "<p><ruby>&lt;script&gt;alert(1)&lt;/script&gt;<rt>漢字</rt></ruby></p>"
    );
}

#[test]
fn original_mode_with_ruby_gloss_uses_ruby_for_required_hangul_entries() {
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

    let output = convert_html_fragment("<p>漢字</p>", &dict, options);

    assert_eq!(output, "<p><ruby>漢字<rt>한자</rt></ruby></p>");
}

fn convert_with_reader_options(input: &str, options: &HtmlReaderOptions<'_>) -> String {
    let input_tokens = read_html_fragment_with_options(input, options);
    let output_tokens =
        process_tokens_iter_with_options(input_tokens, &dictionary(), EngineOptions::default());
    let output_tokens = mark_homophones(output_tokens, &dictionary(), ContextWindow::PerBlock);
    let rendered_tokens = render_tokens_iter(output_tokens, RenderMode::HangulOnly);
    write_html_fragment(rendered_tokens)
}

fn class_contains(raw_attributes: &str, needle: &str) -> bool {
    // Cheap test-only helper: locate `class=` and look for the needle inside the
    // following quoted or unquoted value.
    let mut rest = raw_attributes;
    while let Some(idx) = rest.find("class") {
        let after = &rest[idx + "class".len()..];
        let trimmed = after.trim_start();
        if let Some(after_eq) = trimmed.strip_prefix('=') {
            let value = after_eq.trim_start();
            let value = value
                .strip_prefix('"')
                .map(|v| v.split('"').next().unwrap_or(""))
                .or_else(|| {
                    value
                        .strip_prefix('\'')
                        .map(|v| v.split('\'').next().unwrap_or(""))
                })
                .unwrap_or_else(|| value.split_whitespace().next().unwrap_or(""));
            if value.split_ascii_whitespace().any(|cls| cls == needle) {
                return true;
            }
        }
        rest = &rest[idx + "class".len()..];
    }
    false
}

#[test]
fn user_predicate_marks_class_as_preserved() {
    let options = HtmlReaderOptions::new().preserve_when(|info: &HtmlElementInfo<'_>| {
        class_contains(info.raw_attributes, "no-translate")
    });

    let output = convert_with_reader_options(
        "<div class=\"no-translate\">漢字</div><div>漢字</div>",
        &options,
    );

    assert_eq!(
        output,
        "<div class=\"no-translate\">漢字</div><div>한자</div>"
    );
}

#[test]
fn user_predicate_preserve_is_inherited_by_descendants() {
    let options = HtmlReaderOptions::new()
        .preserve_when(|info| class_contains(info.raw_attributes, "no-translate"));

    let output = convert_with_reader_options(
        "<div class=\"no-translate\"><p>漢字</p><span><em>北京</em></span></div>",
        &options,
    );

    assert_eq!(
        output,
        "<div class=\"no-translate\"><p>漢字</p><span><em>北京</em></span></div>"
    );
}

#[test]
fn user_predicate_ors_with_lang_and_tag_rules() {
    let options =
        HtmlReaderOptions::new().preserve_when(|info| info.raw_attributes.contains("data-no-mt"));

    let output = convert_with_reader_options(
        "<code>漢字</code><p lang=\"ja\">漢字</p><div data-no-mt>漢字</div><p>漢字</p>",
        &options,
    );

    assert_eq!(
        output,
        "<code>漢字</code><p lang=\"ja\">漢字</p><div data-no-mt>漢字</div><p>한자</p>"
    );
}

#[test]
fn user_predicate_sees_inherited_lang() {
    let options = HtmlReaderOptions::new().preserve_when(|info| {
        // Preserve only when an ancestor or this element sets `lang`.
        info.lang.is_some() && info.tag_name == "span"
    });

    let output = convert_with_reader_options(
        "<span>漢字</span><p lang=\"ko\"><span>漢字</span></p>",
        &options,
    );

    assert_eq!(
        output,
        "<span>한자</span><p lang=\"ko\"><span>漢字</span></p>"
    );
}

#[test]
fn user_predicate_does_not_alter_raw_serialization() {
    let options = HtmlReaderOptions::new()
        .preserve_when(|info| class_contains(info.raw_attributes, "no-translate"));

    let input = "<div class=\"no-translate\" data-x=\"1\">漢字</div>";
    let output = convert_with_reader_options(input, &options);

    assert_eq!(output, input);
}

#[test]
fn user_predicate_returning_false_matches_default_reader() {
    let always_false = HtmlReaderOptions::new().preserve_when(|_| false);

    let input = "<p>漢字</p><pre>北京</pre><span lang=\"ja\">漢字</span>";

    let with_options = read_html_fragment_with_options(input, &always_false);
    let default = read_html_fragment(input);

    assert_eq!(with_options, default);
}

proptest! {
    #[test]
    fn predicate_always_false_does_not_change_tokens(
        tag in "(p|div|span|em|strong)",
        attr in "(| class=foo| data-x=\"1\")",
        text in "[A-Za-z0-9가-힣 .,!?]{0,32}",
    ) {
        let input = format!("<{tag}{attr}>{text}</{tag}>");
        let options = HtmlReaderOptions::new().preserve_when(|_| false);
        let with_options = read_html_fragment_with_options(&input, &options);
        let default = read_html_fragment(&input);

        prop_assert_eq!(with_options, default);
    }
}
