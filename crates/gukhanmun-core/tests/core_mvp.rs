use gukhanmun_core::{
    Annotation, HanjaDictionary, InputToken, MapDictionary, MatchMark, OutputToken, PlainScopeData,
    RenderMode, RenderedToken, Scope, ScopeData, convert_plain_text, process_tokens,
    read_plain_text, render_tokens, write_plain_text,
};
use proptest::prelude::*;
use std::cell::Cell;

fn sample_dictionary() -> MapDictionary {
    let mut dict = MapDictionary::new();
    dict.insert("天地", "천지");
    dict.insert("玄黃", "현황");
    dict.insert("漢字", "한자");
    dict
}

fn segmentation_dictionary() -> MapDictionary {
    let mut dict = MapDictionary::new();
    dict.insert("行事", "행사");
    dict.insert("行事場", "행사장");
    dict.insert("場所", "장소");
    dict.insert("入口", "입구");
    dict
}

fn mixed_script_dictionary() -> MapDictionary {
    let mut dict = MapDictionary::new();
    dict.insert("汽車길", "기찻길");
    dict.insert("祭祀날", "제삿날");
    dict.insert("洗手대야", "세숫대야");
    dict.insert("火김", "홧김");
    dict.insert("色깔論", "색깔론");
    dict.insert("汽車", "기차");
    dict.insert("天地", "천지");
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

#[test]
fn lattice_keeps_valid_long_match_when_it_covers_the_run() {
    let output = convert_plain_text(
        "行事場入口",
        &segmentation_dictionary(),
        RenderMode::HangulHanjaParens,
    );

    assert_eq!(output, "행사장(行事場)입구(入口)");
}

#[test]
fn lattice_prefers_two_dictionary_words_over_longer_prefix_plus_fallback() {
    let output = convert_plain_text(
        "行事場所",
        &segmentation_dictionary(),
        RenderMode::HangulHanjaParens,
    );

    assert_eq!(output, "행사(行事)장소(場所)");
}

#[test]
fn lattice_prefers_whole_dictionary_word_over_component_split() {
    let mut dict = MapDictionary::new();
    dict.insert("天", "천");
    dict.insert("地", "지");
    dict.insert("天地", "천지");

    let output = convert_plain_text("天地", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "천지(天地)");
}

#[test]
fn lattice_consumes_mixed_script_dictionary_entries_as_one_annotation() {
    let cases = [
        ("汽車길", "기찻길(汽車길)"),
        ("祭祀날", "제삿날(祭祀날)"),
        ("洗手대야", "세숫대야(洗手대야)"),
        ("火김", "홧김(火김)"),
        ("色깔論", "색깔론(色깔論)"),
    ];

    for (input, expected) in cases {
        let output = convert_plain_text(
            input,
            &mixed_script_dictionary(),
            RenderMode::HangulHanjaParens,
        );

        assert_eq!(output, expected);
    }
}

#[test]
fn mixed_script_annotation_keeps_the_full_source_spelling() {
    let output = process_tokens(
        vec![InputToken::<PlainScopeData>::Text("色깔論".into())],
        &mixed_script_dictionary(),
    );

    assert_eq!(
        output,
        vec![OutputToken::Annotated(Annotation {
            hanja: "色깔論".into(),
            reading: "색깔론".into(),
            homophone: false,
            require_hanja: false,
            require_hangul: false,
            first_in_context: true,
            from_dictionary: true,
        })]
    );
}

#[test]
fn mixed_script_match_beats_shorter_hanja_prefix_match() {
    let output = convert_plain_text(
        "汽車길",
        &mixed_script_dictionary(),
        RenderMode::HangulHanjaParens,
    );

    assert_eq!(output, "기찻길(汽車길)");
}

#[test]
fn hangul_only_dictionary_entry_is_not_a_conversion_candidate() {
    let mut dict = MapDictionary::new();
    dict.insert("길", "road");

    let output = convert_plain_text("길", &dict, RenderMode::HangulOnly);

    assert_eq!(output, "길");
}

#[test]
fn non_hanja_text_does_not_query_the_dictionary() {
    let dict = CountingDictionary::new(vec![("天地", "천지"), ("汽車길", "기찻길")]);

    let output = convert_plain_text(
        "이 문장은 한글과 ASCII only text만 있습니다.",
        &dict,
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "이 문장은 한글과 ASCII only text만 있습니다.");
    assert_eq!(dict.lookup_count(), 0);
}

#[test]
fn whitespace_bounds_dictionary_lookup_windows() {
    let dict = CountingDictionary::new(vec![("漢字", "한자")]);

    let output = convert_plain_text("가나다 漢字", &dict, RenderMode::HangulOnly);

    assert_eq!(output, "가나다 한자");
    assert_eq!(dict.lookup_count(), 2);
}

#[test]
fn mixed_script_lookup_does_not_cross_text_tokens() {
    let tokens = vec![
        InputToken::<PlainScopeData>::Text("汽車".into()),
        InputToken::Text("길".into()),
    ];
    let output = render_tokens(
        process_tokens(tokens, &mixed_script_dictionary()),
        RenderMode::HangulHanjaParens,
    );

    assert_eq!(write_plain_text(output), "기차(汽車)길");
}

#[test]
fn mixed_script_lookup_does_not_cross_verbatim_tokens() {
    let tokens = vec![
        InputToken::<PlainScopeData>::Text("汽車".into()),
        InputToken::Verbatim("길".into()),
    ];
    let output = render_tokens(
        process_tokens(tokens, &mixed_script_dictionary()),
        RenderMode::HangulHanjaParens,
    );

    assert_eq!(write_plain_text(output), "기차(汽車)길");
}

#[test]
fn whitespace_bounded_lattice_skips_non_hanja_spans_without_max_word_length() {
    let dict = CountingDictionary::without_max_word_chars(vec![("漢字", "한자")]);

    let output = convert_plain_text("가나다라마바사 漢字", &dict, RenderMode::HangulOnly);

    assert_eq!(output, "가나다라마바사 한자");
    assert_eq!(dict.lookup_count(), 2);
}

#[test]
fn dictionary_without_max_word_length_can_match_mixed_script_entries() {
    let dict = CountingDictionary::without_max_word_chars(vec![("汽車길", "기찻길")]);

    let output = convert_plain_text("汽車길", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "기찻길(汽車길)");
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
fn lattice_mixes_dictionary_segments_with_fallback_text() {
    let mut dict = MapDictionary::new();
    dict.insert("天地", "천지");
    dict.insert("漢字", "한자");

    let output = convert_plain_text("天地未知漢字", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "천지(天地)未知한자(漢字)");
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

    #[test]
    fn lattice_result_does_not_depend_on_clear_winner_match_order(reversed in any::<bool>()) {
        let entries = [
            ("行事", "행사"),
            ("行事場", "행사장"),
            ("場所", "장소"),
        ];
        let dict = OrderedDictionary::new(if reversed {
            entries.into_iter().rev().collect()
        } else {
            entries.into_iter().collect()
        });

        let output = convert_plain_text("行事場所", &dict, RenderMode::HangulHanjaParens);

        prop_assert_eq!(output, "행사(行事)장소(場所)");
    }

    #[test]
    fn mixed_script_dictionary_match_covers_the_generated_key(
        prefix in "[가-힣]{0,2}",
        middle in "[一-龥]{1,2}",
        suffix in "[가-힣一-龥]{0,2}",
        reading in "[가-힣]{1,5}",
    ) {
        let key = format!("{prefix}{middle}{suffix}");
        let mut dict = MapDictionary::new();
        dict.insert(&key, &reading);

        let output = process_tokens(
            vec![InputToken::<PlainScopeData>::Text(key.clone())],
            &dict,
        );

        prop_assert_eq!(
            output,
            vec![OutputToken::Annotated(Annotation {
                hanja: key,
                reading,
                homophone: false,
                require_hanja: false,
                require_hangul: false,
                first_in_context: true,
                from_dictionary: true,
            })],
        );
    }
}

#[derive(Clone, Debug)]
struct OrderedDictionary {
    entries: Vec<(&'static str, &'static str)>,
}

impl OrderedDictionary {
    fn new(entries: Vec<(&'static str, &'static str)>) -> Self {
        Self { entries }
    }
}

#[derive(Clone, Debug)]
struct CountingDictionary {
    entries: Vec<(&'static str, &'static str)>,
    lookup_count: Cell<usize>,
    max_word_chars: Option<usize>,
}

impl CountingDictionary {
    fn new(entries: Vec<(&'static str, &'static str)>) -> Self {
        let max_word_chars = entries.iter().map(|(hanja, _)| hanja.chars().count()).max();
        Self {
            entries,
            lookup_count: Cell::new(0),
            max_word_chars,
        }
    }

    fn without_max_word_chars(entries: Vec<(&'static str, &'static str)>) -> Self {
        Self {
            entries,
            lookup_count: Cell::new(0),
            max_word_chars: None,
        }
    }

    fn lookup_count(&self) -> usize {
        self.lookup_count.get()
    }
}

impl HanjaDictionary for CountingDictionary {
    fn matches_at<'a>(
        &'a self,
        s: &'a str,
    ) -> Box<dyn Iterator<Item = gukhanmun_core::Match> + 'a> {
        self.lookup_count.set(self.lookup_count.get() + 1);
        Box::new(
            self.entries
                .iter()
                .copied()
                .filter(move |(hanja, _)| s.starts_with(hanja))
                .map(|(hanja, reading)| gukhanmun_core::Match {
                    byte_len: hanja.len(),
                    reading: reading.into(),
                    mark: MatchMark::default(),
                }),
        )
    }

    fn max_word_chars(&self) -> Option<usize> {
        self.max_word_chars
    }
}

impl HanjaDictionary for OrderedDictionary {
    fn matches_at<'a>(
        &'a self,
        s: &'a str,
    ) -> Box<dyn Iterator<Item = gukhanmun_core::Match> + 'a> {
        Box::new(
            self.entries
                .iter()
                .copied()
                .filter(move |(hanja, _)| s.starts_with(hanja))
                .map(|(hanja, reading)| gukhanmun_core::Match {
                    byte_len: hanja.len(),
                    reading: reading.into(),
                    mark: MatchMark::default(),
                }),
        )
    }

    fn max_word_chars(&self) -> Option<usize> {
        self.entries
            .iter()
            .map(|(hanja, _)| hanja.chars().count())
            .max()
    }
}
