// Gukhanmun: Punctuation profile and format serialization regressions.
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

use gukhanmun::{
    Builder, InputToken, OutputToken, PlainScopeData, PunctuationNormalizer,
    PunctuationProfile as P, RenderOptions, SpaceAfter, StopStyle, render_tokens_iter,
    write_plain_text,
};
fn converter(profile: P) -> gukhanmun::Converter<'static> {
    Builder::new()
        .no_bundled_dictionaries()
        .punctuation(profile)
        .build()
        .unwrap()
}
#[test]
fn profiles_and_universal_inputs() {
    let input =
        "첫째， 둘째、셋째︑ 끝． 다음。또︒ 〈가〉「나」︿다﹀﹁라﹂ 《마》『바』︽사︾﹃아﹄";
    let modern =
        "첫째, 둘째, 셋째, 끝. 다음. 또. 〈가〉〈나〉〈다〉〈라〉 《마》《바》《사》《아》";
    let c = converter(P::modern());
    assert_eq!(c.convert_text_to_string(input).unwrap(), modern);
    for p in [
        P::preserve(),
        P::modern(),
        P::traditional(),
        P::traditional_vertical(),
    ] {
        let c = converter(p);
        let once = c.convert_text_to_string(input).unwrap();
        assert_eq!(c.convert_text_to_string(&once).unwrap(), once);
    }
    assert_eq!(
        converter(P::traditional())
            .convert_text_to_string("가, 나. 〈다〉 《라》")
            .unwrap(),
        "가、나。「다」 『라』"
    );
}
#[test]
fn technical_guards_and_space_context() {
    let cases = [
        (
            "1,000 １，０００ 3.14 v1.2 example.com test@example.com",
            "1,000 １，０００ 3.14 v1.2 example.com test@example.com",
        ),
        (
            "Hello, world. IMF, 세계은행 IMF,OECD",
            "Hello, world. IMF、세계은행 IMF,OECD",
        ),
        ("가... 나。．。 다", "가... 나。．。 다"),
        (
            "HTTPS://예。시/가、나 www.예。시 mailto:가、나",
            "HTTPS://예。시/가、나 www.예。시 mailto:가、나",
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(
            converter(P::traditional())
                .convert_text_to_string(input)
                .unwrap(),
            expected,
            "{input}"
        );
    }
    let c = converter(P::modern());
    for (input, expected) in [
        ("가。   나", "가. 나"),
        ("가。 \n나", "가.\n나"),
        ("가。  ", "가."),
        ("가。〉", "가.〉"),
        ("가。〈나〉", "가. 〈나〉"),
        ("가。  ’나", "가.  ’나"),
        ("가。’나", "가.’나"),
        ("가。\u{a0}나", "가.\u{a0}나"),
    ] {
        assert_eq!(
            c.convert_text_to_string(input).unwrap(),
            expected,
            "{input}"
        );
    }
}
#[test]
fn chunk_invariance_all_utf8_boundaries() {
    let input = "한글。  다음、다음 1,000 Hello, world HTTPS://가。나 ︒ 끝";
    for p in [P::modern(), P::traditional(), P::traditional_vertical()] {
        let expected = converter(p).convert_text_to_string(input).unwrap();
        for split in input.char_indices().map(|(i, _)| i) {
            let tokens = [
                InputToken::<PlainScopeData>::Text(input[..split].into()),
                InputToken::Text(input[split..].into()),
            ];
            let got = write_plain_text(converter(p).convert_tokens(tokens));
            assert_eq!(got, expected, "{split}");
        }
        let mut n = PunctuationNormalizer::<PlainScopeData>::new(p);
        let mut out = Vec::new();
        for c in input.chars() {
            out.extend(n.push_token(OutputToken::Text(c.to_string())));
        }
        out.extend(n.finish());
        assert_eq!(
            write_plain_text(render_tokens_iter(out, RenderOptions::default())),
            expected
        );
    }
}
#[test]
fn custom_validation() {
    let mut p = P::modern();
    p.period = Some(StopStyle::new(';', SpaceAfter::Remove));
    assert!(p.validate().is_ok());
    for c in ['x', ' ', '<', '&', '\u{fffd}', '〈', ','] {
        p.period = Some(StopStyle::new(c, SpaceAfter::Remove));
        assert!(p.validate().is_err(), "{c}");
        assert!(
            Builder::new()
                .no_bundled_dictionaries()
                .punctuation(p)
                .build()
                .is_err()
        );
    }
}
#[cfg(feature = "html")]
#[test]
fn html_entities_and_structural_spaces() {
    let c = converter(P::modern());
    for (input, expected) in [
        ("가&#12290;&#32;&#x20;나&period;다", "가. 나. 다"),
        (
            "&lt;script&gt; &amp;period; &lang;가&rang; &fjlig;",
            "&lt;script&gt; &amp;period; &lang;가&rang; &fjlig;",
        ),
        (
            "<em>가。</em><strong>나</strong>",
            "<em>가.</em> <strong>나</strong>",
        ),
        ("<em>가。 </em>나", "<em>가.</em> 나"),
        ("가。 <code>。</code>나", "가. <code>。</code>나"),
        ("<p>가。 <br>나。 </p>", "<p>가.<br>나.</p>"),
        ("가&#46;com 1&#44;000", "가&#46;com 1&#44;000"),
        ("앞</span data-x=。>뒤。다음", "앞</span data-x=。>뒤. 다음"),
    ] {
        assert_eq!(
            c.convert_html_fragment_to_string(input).unwrap(),
            expected,
            "{input}"
        );
    }
    let mut p = P::preserve();
    p.period = Some(StopStyle::new(';', SpaceAfter::Remove));
    let c = converter(p);
    for (input, expected) in [
        ("&#65。", "&#65&#59;"),
        ("&notin。", "&notin&#59;"),
        ("&period。", "&period&#59;"),
    ] {
        assert_eq!(c.convert_html_fragment_to_string(input).unwrap(), expected);
    }
    let mut p = P::preserve();
    p.period = Some(StopStyle::new('#', SpaceAfter::Remove));
    assert_eq!(
        converter(p)
            .convert_html_fragment_to_string("&。32;")
            .unwrap(),
        "&&#35;32;"
    );
}
#[cfg(feature = "html")]
#[test]
fn html_codec_chunks() {
    use gukhanmun::html::{HtmlFragmentReader, HtmlFragmentWriter, HtmlPunctuationTextCodec};
    let input =
        "<p>한글&#000000012290;  다음&period;글 &NotEqualTilde; &notin; &#x80; &amp;period;</p>";
    let c = converter(P::modern());
    let expected = c.convert_html_fragment_to_string(input).unwrap();
    for split in input.char_indices().map(|(i, _)| i) {
        let mut r = HtmlFragmentReader::with_options(c.html_reader_options());
        let mut tokens = r.push_str(&input[..split]);
        tokens.extend(r.push_str(&input[split..]));
        tokens.extend(r.finish());
        let tokens = tokens.into_iter().map(Result::unwrap);
        let mut w = HtmlFragmentWriter::new(Vec::new());
        for t in c.convert_tokens_with_codec(tokens, HtmlPunctuationTextCodec::new()) {
            w.write_token(t).unwrap();
        }
        let out = w.finish().unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), expected, "{split}");
    }
}
#[cfg(feature = "markdown")]
#[test]
fn markdown_protects_structure() {
    use gukhanmun::markdown::MarkdownVariant;
    let c = converter(P::modern());
    let output = c
        .convert_markdown_to_string(
            "*끝。* 다음\n\n`가。` ![설명。](img.png)\n\n<urn:하나、둘>",
            MarkdownVariant::CommonMark,
        )
        .unwrap();
    assert!(output.contains("<em>끝&#46;</em> 다음"), "{output}");
    assert!(output.contains("`가。`"));
    assert!(output.contains("설명。"));
    assert!(output.contains("(urn:하나、둘)"), "{output}");
    assert!(output.contains("하나&#44; 둘"), "{output}");
    let mut p = P::preserve();
    p.period = Some(StopStyle::new('-', SpaceAfter::Single));
    let c = converter(p);
    let output = c
        .convert_markdown_to_string("。 항목\n다음", MarkdownVariant::CommonMark)
        .unwrap();
    assert!(output.starts_with("&#45; 항목"), "{output}");
}

#[test]
fn shared_json_profile_resolution() {
    use gukhanmun_core::PunctuationConfig;
    let config: PunctuationConfig =
        serde_json::from_str(r#"{"base":"modern","period":null}"#).unwrap();
    let p = config.resolve().unwrap();
    assert_eq!(p.period, None);
    assert_eq!(p.comma, P::modern().comma);
    for json in [
        r#""unknown""#,
        r#"{"base":"modern","typo":true}"#,
        r#"{"period":{"mark":"xx","after":"remove"}}"#,
        r#"{"period":{"mark":"\ud800","after":"remove"}}"#,
        r#"{"period":{"mark":";","after":"unknown"}}"#,
        r#"{"single":{"open":"〉","close":"〈"}}"#,
    ] {
        let result = serde_json::from_str::<PunctuationConfig>(json).map(|c| c.resolve());
        assert!(!matches!(result, Ok(Ok(_))), "{json}");
    }
}
#[cfg(feature = "markdown")]
#[test]
fn markdown_rendered_semantics() {
    use gukhanmun::markdown::MarkdownVariant;
    fn html(s: &str) -> String {
        let mut out = String::new();
        pulldown_cmark::html::push_html(
            &mut out,
            pulldown_cmark::Parser::new_ext(s, pulldown_cmark::Options::ENABLE_STRIKETHROUGH),
        );
        out
    }
    for (input, expected) in [
        ("*끝。* 다음", "<p><em>끝。</em>다음</p>\n"),
        ("**앞、** 뒤", "<p><strong>앞、</strong>뒤</p>\n"),
        ("~~앞。~~ 뒤", "<p><del>앞。</del>뒤</p>\n"),
        (
            "[앞。](dest \"title\") 뒤",
            "<p><a href=\"dest\" title=\"title\">앞。</a>뒤</p>\n",
        ),
    ] {
        let output = converter(P::traditional())
            .convert_markdown_to_string(input, MarkdownVariant::Gfm)
            .unwrap();
        assert_eq!(html(&output), expected, "{output}");
    }
    let c = converter(P::modern());
    let output = c
        .convert_markdown_to_string("가。  \n나", MarkdownVariant::CommonMark)
        .unwrap();
    assert!(html(&output).contains("<br />"), "{output}");
    let mut p = P::modern();
    p.period = Some(StopStyle::new('.', SpaceAfter::Preserve));
    let output = converter(p)
        .convert_markdown_to_string("가。 \n나", MarkdownVariant::CommonMark)
        .unwrap();
    assert!(!html(&output).contains("<br"), "{output}");
}

#[test]
fn punctuation_uses_annotations_as_content_without_rewriting_readings() {
    let mut d = gukhanmun::MapDictionary::new();
    d.insert("學校", "학교");
    for profile in [P::modern(), P::traditional()] {
        let c = Builder::new()
            .no_bundled_dictionaries()
            .push_dictionary(&d)
            .punctuation(profile)
            .build()
            .unwrap();
        assert_eq!(
            c.convert_text_to_string("앞。學校").unwrap(),
            if profile == P::modern() {
                "앞. 학교"
            } else {
                "앞。학교"
            }
        );
    }
}
#[cfg(feature = "html")]
#[test]
fn html_reference_replacement_and_long_numeric_chunks() {
    use gukhanmun::{PunctuationTextCodec, html::HtmlPunctuationTextCodec};
    let mut codec = HtmlPunctuationTextCodec::new();
    let units = codec.decode(
        "&#0; &#xD800; &#x110000; &#x80; &NotEqualTilde; &fjlig; &unknown;",
        true,
    );
    let decoded: String = units.iter().map(|u| u.decoded.as_str()).collect();
    assert_eq!(decoded, "� � � € ≂\u{338} fj &unknown;");
    let input = format!("&#{}12290;", "0".repeat(20_000));
    let mut codec = HtmlPunctuationTextCodec::new();
    let mut units = Vec::new();
    for c in input.chars() {
        units.extend(codec.decode(&c.to_string(), false));
    }
    units.extend(codec.decode("", true));
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].raw, input);
    assert_eq!(units[0].decoded, "。");
}
#[cfg(feature = "html")]
#[test]
fn space_lookahead_cap_and_chunk_count_are_independent() {
    let c = converter(P::modern());
    let within = format!("가。{}나", "<span></span>".repeat(100));
    let beyond = format!("가。{}나", "<span></span>".repeat(129));
    assert_eq!(
        c.convert_html_fragment_to_string(&within).unwrap(),
        format!("가. {}나", "<span></span>".repeat(100))
    );
    assert_eq!(
        c.convert_html_fragment_to_string(&beyond).unwrap(),
        format!("가.{}나", "<span></span>".repeat(129))
    );
    assert_eq!(
        c.convert_html_fragment_to_string("가。<span> </span>’나")
            .unwrap(),
        "가.<span> </span>’나"
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_emphasis_can_begin_with_hard_break() {
    let output = converter(P::modern())
        .convert_markdown_to_string(
            "**\\\n끝** 다음",
            gukhanmun::markdown::MarkdownVariant::CommonMark,
        )
        .unwrap();
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, pulldown_cmark::Parser::new(&output));
    assert_eq!(
        html, "<p><strong><br />\n끝</strong> 다음</p>\n",
        "{output}"
    );
}

#[cfg(feature = "html")]
#[test]
fn preserved_block_boundaries_trim_without_rewriting_contents() {
    let c = converter(P::modern());
    assert_eq!(
        c.convert_html_fragment_to_string("가。 <pre>나。</pre>")
            .unwrap(),
        "가.<pre>나。</pre>"
    );
    assert_eq!(
        c.convert_html_fragment_to_string("가。 <br lang=en>나")
            .unwrap(),
        "가.<br lang=en>나"
    );
    let input = format!("가。{}나", "<span></span>".repeat(128));
    assert_eq!(
        c.convert_html_fragment_to_string(&input).unwrap(),
        format!("가. {}나", "<span></span>".repeat(128))
    );
}
#[cfg(feature = "markdown")]
#[test]
fn markdown_inline_html_is_transparent_for_spaces() {
    let output = converter(P::modern())
        .convert_markdown_to_string(
            "앞 <EM>가。</EM>나",
            gukhanmun::markdown::MarkdownVariant::CommonMark,
        )
        .unwrap();
    assert!(output.contains("</EM> 나"), "{output}");
}
#[test]
fn long_space_runs_keep_streaming_and_preserve_source_positions() {
    let mut n = PunctuationNormalizer::<PlainScopeData>::new(P::modern());
    let mut out = n.push_token(OutputToken::Text("가。".into()));
    for _ in 0..10_000 {
        out.extend(n.push_token(OutputToken::Text(" ".repeat(32))));
    }
    out.extend(n.push_token(OutputToken::Text("’나".into())));
    out.extend(n.finish());
    assert_eq!(
        write_plain_text(render_tokens_iter(out, RenderOptions::default())),
        format!("가.{}’나", " ".repeat(320_000))
    );
}

#[cfg(feature = "html")]
#[test]
fn generated_marks_cannot_complete_markup_after_recovery() {
    for (mark, entity) in [('!', "&#33;"), ('?', "&#63;"), ('/', "&#47;")] {
        let mut p = P::preserve();
        p.period = Some(StopStyle::new(mark, SpaceAfter::Remove));
        let c = Builder::new()
            .no_bundled_dictionaries()
            .punctuation(p)
            .recovery(gukhanmun::Recovery::Lenient)
            .build()
            .unwrap();
        for tail in ["--hello-->VISIBLE", "div>hello", "xml version=\"1.0\">next"] {
            assert_eq!(
                c.convert_html_fragment_to_string(&format!("<。{tail}"))
                    .unwrap(),
                format!("<{entity}{tail}")
            );
        }
    }
}
#[cfg(feature = "html")]
#[test]
fn adapter_effective_language_flags_are_authoritative() {
    let c = converter(P::modern());
    assert_eq!(
        c.convert_html_fragment_to_string("<div lang=en><span lang=ko>韓國。文化</span></div>")
            .unwrap(),
        "<div lang=en><span lang=ko>한국. 문화</span></div>"
    );
    assert_eq!(
        c.convert_html_fragment_to_string(
            "<span lang=en>keep<span lang=ko>가。  </span> tail。</span>"
        )
        .unwrap(),
        "<span lang=en>keep<span lang=ko>가.  </span> tail。</span>"
    );
    assert_eq!(
        c.convert_html_fragment_to_string("<code>가。<span lang=ko>나。다</span></code>")
            .unwrap(),
        "<code>가。<span lang=ko>나。다</span></code>"
    );
}
#[cfg(feature = "markdown")]
#[test]
fn markdown_language_overrides_and_image_subtree_preservation() {
    let c = converter(P::modern());
    let output = c
        .convert_markdown_to_string(
            "앞 <span lang=en>keep<span lang=ko>가。  </span> tail。</span>",
            gukhanmun::markdown::MarkdownVariant::CommonMark,
        )
        .unwrap();
    assert!(output.contains("가&#46;  </span> tail。"), "{output}");
    let output = c
        .convert_markdown_to_string(
            "![*가。나*](image.png)",
            gukhanmun::markdown::MarkdownVariant::CommonMark,
        )
        .unwrap();
    assert!(output.contains("가。나"), "{output}");
}

#[cfg(feature = "markdown")]
#[test]
fn protected_markdown_writer_retains_inline_preservation_policy() {
    use gukhanmun::markdown::MarkdownVariant;
    let modern = converter(P::modern());
    let preserve = converter(P::preserve());
    for source in [
        "앞 <script>alert(1);</script> 뒤",
        "앞 <script>let x=2**3**2;</script> 뒤",
        "앞 <style>.x { color: red; }</style> 뒤",
        "앞 <code>a&b</code> 뒤",
        "앞 <span lang=en>English, text.</span> 뒤",
    ] {
        assert_eq!(
            modern
                .convert_markdown_to_string(source, MarkdownVariant::CommonMark)
                .unwrap(),
            preserve
                .convert_markdown_to_string(source, MarkdownVariant::CommonMark)
                .unwrap(),
            "{source}"
        );
    }
    let source = "앞 <span lang=en>keep *text* <span lang=ko>가。나</span> end.</span>";
    let output = modern
        .convert_markdown_to_string(source, MarkdownVariant::CommonMark)
        .unwrap();
    assert!(output.contains("가&#46; 나"), "{output}");
    assert!(output.contains("end.</span>"), "{output}");
}

#[cfg(feature = "markdown")]
#[test]
fn decoded_line_endings_stay_text_inside_markdown_emphasis() {
    for (entity, line) in [("&#10;", "\n"), ("&#13;", "\r")] {
        let input = format!("**{entity}[글](dest)** 다음");
        let output = converter(P::modern())
            .convert_markdown_to_string(&input, gukhanmun::markdown::MarkdownVariant::CommonMark)
            .unwrap();
        let mut html = String::new();
        pulldown_cmark::html::push_html(&mut html, pulldown_cmark::Parser::new(&output));
        assert_eq!(
            html,
            format!("<p><strong>{line}<a href=\"dest\">글</a></strong> 다음</p>\n"),
            "{output}"
        );
    }
}

#[cfg(feature = "markdown")]
#[test]
fn protected_writer_keeps_blockquote_heading_structure() {
    let source = converter(P::modern())
        .convert_markdown_to_string(
            "> # *가。* 다음",
            gukhanmun::markdown::MarkdownVariant::CommonMark,
        )
        .unwrap();
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, pulldown_cmark::Parser::new(&source));
    assert_eq!(
        html,
        "<blockquote>\n<h1><em>가.</em> 다음</h1>\n</blockquote>\n"
    );
}
