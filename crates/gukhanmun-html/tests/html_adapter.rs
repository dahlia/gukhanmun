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

use gukhanmun_core::{MapDictionary, Recovery, RenderMode};
use gukhanmun_html::{
    HtmlError, HtmlScopeData, convert_html_fragment, read_html_fragment, try_convert_html_fragment,
    write_html_fragment,
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
fn block_scopes_reset_homophone_marking() {
    let output = convert_html_fragment(
        "<p>布告하다</p><p>佈告하다</p><div>布告하다 佈告하다</div>",
        &dictionary(),
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
