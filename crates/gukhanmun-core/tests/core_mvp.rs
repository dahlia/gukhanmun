use gukhanmun_core::{
    Annotation, ContextWindow, EngineOptions, HanjaDictionary, InputToken, MapDictionary,
    MatchMark, NumeralStrategy, OutputToken, PlainScopeData, RenderMode, RenderedToken, Scope,
    ScopeData, UserDirectives, apply_user_directives, convert_plain_text,
    convert_plain_text_with_options, filter_first_occurrences, mark_homophones, process_tokens,
    read_plain_text, render_tokens, write_plain_text,
};
use proptest::prelude::*;
use std::cell::Cell;

#[test]
fn hanja_detection_covers_known_cjk_ranges() {
    let cases = [
        '⼀',        // CJK radicals / ideographic description range used by Seonbi.
        '〇',        // Ideographic number zero.
        '㐀',        // CJK Unified Ideographs Extension A.
        '一',        // CJK Unified Ideographs.
        '豈',        // CJK Compatibility Ideographs.
        '\u{20000}', // CJK Unified Ideographs Extension B.
        '\u{2A6DF}', // CJK Unified Ideographs Extension B end.
        '\u{2A700}', // CJK Unified Ideographs Extension C.
        '\u{2B73F}', // CJK Unified Ideographs Extension C end.
        '\u{2B740}', // CJK Unified Ideographs Extension D.
        '\u{2B81F}', // CJK Unified Ideographs Extension D end.
        '\u{2B820}', // CJK Unified Ideographs Extension E.
        '\u{2CEAF}', // CJK Unified Ideographs Extension E end.
        '\u{2CEB0}', // CJK Unified Ideographs Extension F.
        '\u{2EBEF}', // CJK Unified Ideographs Extension F end.
        '\u{2EBF0}', // CJK Unified Ideographs Extension I.
        '\u{2EE5F}', // CJK Unified Ideographs Extension I end.
        '\u{2F800}', // CJK Compatibility Ideographs Supplement.
        '\u{30000}', // CJK Unified Ideographs Extension G.
        '\u{3134F}', // CJK Unified Ideographs Extension G end.
        '\u{31350}', // CJK Unified Ideographs Extension H.
        '\u{323AF}', // CJK Unified Ideographs Extension H end.
        '\u{323B0}', // CJK Unified Ideographs Extension J.
        '\u{3347F}', // CJK Unified Ideographs Extension J end.
    ];

    for ch in cases {
        assert!(
            gukhanmun_core::is_hanja(ch),
            "{ch} should be treated as hanja"
        );
    }
}

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

fn annotation(hanja: &str, reading: &str) -> Annotation {
    Annotation {
        hanja: hanja.into(),
        reading: reading.into(),
        homophone: false,
        require_hanja: false,
        require_hangul: false,
        first_in_context: true,
        from_dictionary: true,
    }
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
fn renders_hanja_hangul_parens() {
    let output = convert_plain_text(
        "天地玄黃과 漢字",
        &sample_dictionary(),
        RenderMode::HanjaHangulParens,
    );

    assert_eq!(output, "天地(천지)玄黃(현황)과 漢字(한자)");
}

#[test]
fn original_renderer_keeps_hanja_unless_hangul_is_required() {
    let tokens = vec![
        OutputToken::<PlainScopeData>::Annotated(annotation("天地", "천지")),
        OutputToken::Text(" ".into()),
        OutputToken::Annotated(Annotation {
            require_hangul: true,
            ..annotation("漢字", "한자")
        }),
    ];

    let rendered = render_tokens(tokens, RenderMode::Original);

    assert_eq!(
        rendered,
        vec![
            RenderedToken::Text("天地".into()),
            RenderedToken::Text(" ".into()),
            RenderedToken::Text("漢字(한자)".into()),
        ]
    );
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
fn engine_leaves_homophone_policy_to_middleware() {
    let mut dict = MapDictionary::new();
    dict.insert("漢字", "한자");
    dict.insert("翰字", "한자");

    let output = process_tokens(
        vec![InputToken::<PlainScopeData>::Text("漢字".into())],
        &dict,
    );

    assert_eq!(
        output,
        vec![OutputToken::Annotated(annotation("漢字", "한자"))]
    );
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
    block_boundary: bool,
}

impl ScopeData for TestScopeData {
    fn is_preserve(&self) -> bool {
        self.preserve
    }

    fn is_block_boundary(&self) -> bool {
        self.block_boundary
    }
}

#[test]
fn engine_preserves_text_inside_preserve_scope() {
    let tokens = vec![
        InputToken::Open(Scope::new(TestScopeData {
            preserve: true,
            block_boundary: false,
        })),
        InputToken::Text("漢字".into()),
        InputToken::Close,
    ];

    let output = process_tokens(tokens, &sample_dictionary());

    assert_eq!(
        output,
        vec![
            OutputToken::Open(Scope::new(TestScopeData {
                preserve: true,
                block_boundary: false,
            })),
            OutputToken::Text("漢字".into()),
            OutputToken::Close,
        ]
    );
}

#[test]
fn hanja_without_fallback_reading_is_preserved_as_text() {
    let output = convert_plain_text("龥와 漢字", &sample_dictionary(), RenderMode::HangulOnly);

    assert_eq!(output, "龥와 한자");
}

#[test]
fn fallback_only_hanja_is_phoneticized_with_initial_sound_law() {
    let output = convert_plain_text(
        "未知 來日 未來 良質 力量",
        &MapDictionary::new(),
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "미지 내일 미래 양질 역량");
}

#[test]
fn mixed_script_fallback_keeps_context_after_plain_text() {
    let cases = [("가羅", "가라"), ("가來", "가래"), ("色깔論", "色깔론")];

    for (input, expected) in cases {
        let output = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

        assert_eq!(output, expected);
    }
}

#[test]
fn unmapped_hanja_inside_fallback_run_does_not_reset_word_start() {
    let output = convert_plain_text("新羅", &MapDictionary::new(), RenderMode::HangulOnly);

    assert_eq!(output, "新라");
}

#[test]
fn fallback_initial_sound_law_can_be_disabled() {
    let options = EngineOptions {
        initial_sound_law: false,
        numeral_strategy: NumeralStrategy::HangulPhonetic,
    };
    let output = convert_plain_text_with_options(
        "來日 良質 力量",
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        options,
    );

    assert_eq!(output, "래일 량질 력량");
}

#[test]
fn fallback_numerals_honor_initial_sound_law_option() {
    let no_law = EngineOptions {
        initial_sound_law: false,
        numeral_strategy: NumeralStrategy::HangulPhonetic,
    };

    let default_cases = [
        ("六", "육"),
        ("一六年", "일륙년"),
        ("六六年", "육륙년"),
        ("一九六六年", "일구륙륙년"),
        ("第六", "제육"),
        ("六〇年", "육공년"),
        ("陸〇年", "육공년"),
    ];
    for (input, expected) in default_cases {
        let output = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

        assert_eq!(output, expected);
    }

    let no_law_cases = [
        ("六", "륙"),
        ("第六", "제륙"),
        ("六〇年", "륙공년"),
        ("陸〇年", "륙공년"),
    ];
    for (input, expected) in no_law_cases {
        let output = convert_plain_text_with_options(
            input,
            &MapDictionary::new(),
            RenderMode::HangulOnly,
            no_law,
        );

        assert_eq!(output, expected);
    }
}

#[test]
fn fallback_after_punctuation_starts_a_new_word() {
    let cases = [("(來日)", "(내일)"), ("「良質」", "「양질」")];

    for (input, expected) in cases {
        let output = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

        assert_eq!(output, expected);
    }
}

#[test]
fn dictionary_readings_are_not_rewritten_by_fallback_initial_sound_law() {
    let mut dict = MapDictionary::new();
    dict.insert("來日", "래일");

    let output = convert_plain_text("來日", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "래일(來日)");
}

#[test]
fn fallback_combines_with_dictionary_segments() {
    let mut dict = MapDictionary::new();
    dict.insert("標識", "표지");
    dict.insert("毛澤東", "마오쩌둥");

    let output = convert_plain_text(
        "安全標識 毛澤東語錄 毛澤東理論",
        &dict,
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "안전표지 마오쩌둥어록 마오쩌둥이론");
}

#[test]
fn fallback_keeps_context_after_prefix_dictionary_segments() {
    let cases = [
        ("力", "역", "力量", "역량"),
        ("法", "법", "法律", "법률"),
        ("新", "신", "新羅", "신라"),
        ("新法", "신법", "新法律", "신법률"),
    ];

    for (hanja, reading, input, expected) in cases {
        let mut dict = MapDictionary::new();
        dict.insert(hanja, reading);

        let output = convert_plain_text(input, &dict, RenderMode::HangulOnly);

        assert_eq!(output, expected);
    }
}

#[test]
fn fallback_keeps_context_after_mixed_script_dictionary_prefixes() {
    let mut dict = MapDictionary::new();
    dict.insert("色깔", "색깔");
    dict.insert("毛澤東", "마오쩌둥");

    let cases = [("色깔論", "색깔론"), ("毛澤東理論", "마오쩌둥이론")];

    for (input, expected) in cases {
        let output = convert_plain_text(input, &dict, RenderMode::HangulOnly);

        assert_eq!(output, expected);
    }
}

#[test]
fn fallback_applies_yeol_yul_rule() {
    let cases = [
        ("法律", "법률"),
        ("一列", "일렬"),
        ("十二列", "십이열"),
        ("十二律", "십이율"),
        ("羅列", "나열"),
        ("序列", "서열"),
        ("規律", "규율"),
        ("自律", "자율"),
        ("前列", "전열"),
        ("韻律", "운율"),
        ("分列", "분열"),
        ("旋律", "선율"),
    ];

    for (input, expected) in cases {
        let output = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

        assert_eq!(output, expected);
    }
}

#[test]
fn fallback_phoneticizes_hanja_numerals() {
    let cases = [
        ("千九百八十六年", "천구백팔십육년"),
        ("二〇一六年", "이공일륙년"),
        ("第六共和國", "제육공화국"),
        ("拾萬圓", "십만원"),
        ("參佰拾圓", "삼백십원"),
        ("仟參佰圓", "천삼백원"),
        ("〇", "공"),
        ("零", "영"),
    ];

    for (input, expected) in cases {
        let output = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

        assert_eq!(output, expected);
    }
}

#[test]
fn hangul_phonetic_numerals_remain_renderable_annotations() {
    let cases = [
        ("二〇一六年", "이공일륙(二〇一六)년(年)"),
        ("第六共和國", "제육(第六)공화국(共和國)"),
    ];

    for (input, expected) in cases {
        let output =
            convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulHanjaParens);

        assert_eq!(output, expected);
    }
}

#[test]
fn lattice_mixes_dictionary_segments_with_fallback_text() {
    let mut dict = MapDictionary::new();
    dict.insert("天地", "천지");
    dict.insert("漢字", "한자");

    let output = convert_plain_text("天地未知漢字", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "천지(天地)미지(未知)한자(漢字)");
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

#[test]
fn homophone_marker_uses_forms_that_appear_in_the_same_context() {
    let tokens = vec![
        OutputToken::<PlainScopeData>::Annotated(annotation("漢字", "한자")),
        OutputToken::Text("와 ".into()),
        OutputToken::Annotated(annotation("翰字", "한자")),
        OutputToken::Text("와 ".into()),
        OutputToken::Annotated(annotation("天地", "천지")),
    ];

    let marked = mark_homophones(tokens, ContextWindow::PerDocument);

    assert_eq!(
        marked,
        vec![
            OutputToken::Annotated(Annotation {
                homophone: true,
                ..annotation("漢字", "한자")
            }),
            OutputToken::Text("와 ".into()),
            OutputToken::Annotated(Annotation {
                homophone: true,
                ..annotation("翰字", "한자")
            }),
            OutputToken::Text("와 ".into()),
            OutputToken::Annotated(annotation("天地", "천지")),
        ]
    );
}

#[test]
fn homophone_marker_resets_at_block_boundaries() {
    let block = TestScopeData {
        preserve: false,
        block_boundary: true,
    };
    let tokens = vec![
        OutputToken::Open(Scope::new(block.clone())),
        OutputToken::Annotated(annotation("漢字", "한자")),
        OutputToken::Close,
        OutputToken::Open(Scope::new(block)),
        OutputToken::Annotated(annotation("翰字", "한자")),
        OutputToken::Close,
    ];

    let marked = mark_homophones(tokens.clone(), ContextWindow::PerBlock);

    assert_eq!(marked, tokens);
}

#[test]
fn homophone_marker_resets_at_nested_block_boundaries() {
    let block = TestScopeData {
        preserve: false,
        block_boundary: true,
    };
    let tokens = vec![
        OutputToken::Open(Scope::new(block.clone())),
        OutputToken::Open(Scope::new(block.clone())),
        OutputToken::Annotated(annotation("漢字", "한자")),
        OutputToken::Close,
        OutputToken::Open(Scope::new(block)),
        OutputToken::Annotated(annotation("翰字", "한자")),
        OutputToken::Close,
        OutputToken::Close,
    ];

    let marked = mark_homophones(tokens.clone(), ContextWindow::PerBlock);

    assert_eq!(marked, tokens);
}

#[test]
fn first_occurrence_filter_clears_repeated_form_requirements() {
    let tokens = vec![
        OutputToken::<PlainScopeData>::Annotated(Annotation {
            require_hanja: true,
            require_hangul: true,
            ..annotation("漢字", "한자")
        }),
        OutputToken::Text(" ".into()),
        OutputToken::Annotated(Annotation {
            require_hanja: true,
            require_hangul: true,
            ..annotation("翰字", "한자")
        }),
        OutputToken::Text(" ".into()),
        OutputToken::Annotated(Annotation {
            require_hanja: true,
            require_hangul: true,
            ..annotation("漢字", "한자")
        }),
    ];

    let filtered = filter_first_occurrences(tokens, ContextWindow::PerDocument);

    assert_eq!(
        filtered,
        vec![
            OutputToken::Annotated(Annotation {
                require_hanja: true,
                require_hangul: true,
                first_in_context: true,
                ..annotation("漢字", "한자")
            }),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(Annotation {
                require_hanja: true,
                require_hangul: true,
                first_in_context: true,
                ..annotation("翰字", "한자")
            }),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(Annotation {
                require_hanja: false,
                require_hangul: false,
                first_in_context: false,
                ..annotation("漢字", "한자")
            }),
        ]
    );
}

#[test]
fn first_occurrence_filter_resets_at_nested_block_boundaries() {
    let block = TestScopeData {
        preserve: false,
        block_boundary: true,
    };
    let required = || Annotation {
        require_hanja: true,
        require_hangul: true,
        ..annotation("漢字", "한자")
    };
    let tokens = vec![
        OutputToken::Open(Scope::new(block.clone())),
        OutputToken::Open(Scope::new(block.clone())),
        OutputToken::Annotated(required()),
        OutputToken::Close,
        OutputToken::Open(Scope::new(block)),
        OutputToken::Annotated(required()),
        OutputToken::Close,
        OutputToken::Close,
    ];

    let filtered = filter_first_occurrences(tokens.clone(), ContextWindow::PerBlock);

    assert_eq!(filtered, tokens);
}

#[test]
fn user_directives_mark_literal_hanja_forms_without_rendering_them() {
    let mut directives = UserDirectives::new();
    directives.require_hanja("漢字");
    directives.require_hangul("天地");

    let tokens = vec![
        OutputToken::<PlainScopeData>::Annotated(annotation("漢字", "한자")),
        OutputToken::Text(" ".into()),
        OutputToken::Annotated(annotation("天地", "천지")),
    ];

    let directed = apply_user_directives(tokens, &directives);

    assert_eq!(
        directed,
        vec![
            OutputToken::Annotated(Annotation {
                require_hanja: true,
                ..annotation("漢字", "한자")
            }),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(Annotation {
                require_hangul: true,
                ..annotation("天地", "천지")
            }),
        ]
    );
}

proptest! {
    #[test]
    fn non_hanja_plain_text_is_unchanged(input in "[A-Za-z0-9가-힣 .,!?()「」]*") {
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

    #[test]
    fn known_fallback_hanja_are_removed_from_hangul_only_output(input in "[未知來日良質力量安全語錄理論法律一列羅序規律自前韻分旋千九百八十六年第共和國拾萬圓參佰仟]{1,12}") {
        let output = convert_plain_text(&input, &MapDictionary::new(), RenderMode::HangulOnly);

        prop_assert!(!output.chars().any(gukhanmun_core::is_hanja));
    }

    #[test]
    fn renderer_never_emits_unrendered_annotations(
        hanja in "[一-龥]{1,4}",
        reading in "[가-힣]{1,6}",
        require_hanja in any::<bool>(),
        require_hangul in any::<bool>(),
        homophone in any::<bool>(),
        mode in 0u8..4,
    ) {
        let mode = match mode {
            0 => RenderMode::HangulOnly,
            1 => RenderMode::HangulHanjaParens,
            2 => RenderMode::HanjaHangulParens,
            _ => RenderMode::Original,
        };
        let rendered = render_tokens(
            vec![OutputToken::<PlainScopeData>::Annotated(Annotation {
                hanja,
                reading,
                homophone,
                require_hanja,
                require_hangul,
                first_in_context: true,
                from_dictionary: true,
            })],
            mode,
        );

        prop_assert!(rendered.into_iter().all(|token| matches!(
            token,
            RenderedToken::Open(_)
                | RenderedToken::Close
                | RenderedToken::Text(_)
                | RenderedToken::Verbatim(_)
        )));
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
