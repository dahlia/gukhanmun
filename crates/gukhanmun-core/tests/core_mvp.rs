use gukhanmun_core::{
    Annotation, HanjaDictionary, InputToken, MapDictionary, MatchMark, OutputToken, PlainScopeData,
    RenderMode, RenderedToken, Scope, ScopeData, convert_plain_text, process_tokens,
    read_plain_text, render_tokens, write_plain_text,
};
use proptest::prelude::*;

fn sample_dictionary() -> MapDictionary {
    let mut dict = MapDictionary::new();
    dict.insert("天地", "천지");
    dict.insert("玄黃", "현황");
    dict.insert("漢字", "한자");
    dict
}

#[test]
fn converts_plain_text_to_hangul_only() {
    let output = convert_plain_text(
        "天地玄黃과 漢字",
        &sample_dictionary(),
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "천지현황과 한자");
}

#[test]
fn converts_plain_text_to_hangul_hanja_parens() {
    let output = convert_plain_text(
        "天地玄黃과 漢字",
        &sample_dictionary(),
        RenderMode::HangulHanjaParens,
    );

    assert_eq!(output, "천지(天地)현황(玄黃)과 한자(漢字)");
}

#[test]
fn hangul_only_keeps_required_or_homophonous_hanja_in_parens() {
    let mut dict = MapDictionary::new();
    dict.insert_marked(
        "漢字",
        "한자",
        MatchMark {
            require_hanja: true,
            require_hangul: false,
        },
    );
    dict.insert("翰字", "한자");

    let output = convert_plain_text("漢字와 翰字", &dict, RenderMode::HangulOnly);

    assert_eq!(output, "한자(漢字)와 한자(翰字)");
}

#[test]
fn map_dictionary_returns_every_match_at_the_cursor() {
    let mut dict = MapDictionary::new();
    dict.insert("行事", "행사");
    dict.insert("行事場", "행사장");

    let matches: Vec<_> = dict.matches_at("行事場入口").collect();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].byte_len, "行事".len());
    assert_eq!(matches[0].reading, "행사");
    assert_eq!(matches[1].byte_len, "行事場".len());
    assert_eq!(matches[1].reading, "행사장");
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestScopeData {
    preserve: bool,
}

impl ScopeData for TestScopeData {
    fn is_preserve(&self) -> bool {
        self.preserve
    }
}

#[test]
fn engine_preserves_text_inside_preserve_scope() {
    let tokens = vec![
        InputToken::Open(Scope::new(TestScopeData { preserve: true })),
        InputToken::Text("漢字".into()),
        InputToken::Close,
    ];

    let output = process_tokens(tokens, &sample_dictionary());

    assert_eq!(
        output,
        vec![
            OutputToken::Open(Scope::new(TestScopeData { preserve: true })),
            OutputToken::Text("漢字".into()),
            OutputToken::Close,
        ]
    );
}

#[test]
fn unknown_hanja_is_preserved_as_text() {
    let output = convert_plain_text("未知와 漢字", &sample_dictionary(), RenderMode::HangulOnly);

    assert_eq!(output, "未知와 한자");
}

#[test]
fn renderer_removes_annotations_from_the_stream() {
    let rendered = render_tokens(
        vec![OutputToken::<PlainScopeData>::Annotated(Annotation {
            hanja: "漢字".into(),
            reading: "한자".into(),
            homophone: false,
            require_hanja: false,
            require_hangul: false,
            first_in_context: true,
            from_dictionary: true,
        })],
        RenderMode::HangulOnly,
    );

    assert_eq!(rendered, vec![RenderedToken::Text("한자".into())]);
}

proptest! {
    #[test]
    fn non_hanja_plain_text_is_unchanged(input in "[\\PC&&[^一-龥]]*") {
        let dict = MapDictionary::new();

        let output = convert_plain_text(&input, &dict, RenderMode::HangulOnly);

        prop_assert_eq!(output, input);
    }

    #[test]
    fn plain_reader_writer_roundtrips(input in ".*") {
        let tokens = read_plain_text(&input);
        let rendered = tokens.into_iter().map(|token| match token {
            InputToken::Open(scope) => RenderedToken::Open(scope),
            InputToken::Close => RenderedToken::Close,
            InputToken::Text(text) => RenderedToken::Text(text),
            InputToken::Verbatim(text) => RenderedToken::Verbatim(text),
        });

        prop_assert_eq!(write_plain_text(rendered), input);
    }
}
