// Gukhanmun: Core IR, engine, dictionary traits, and fallback logic for Gukhanmun.
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
    Annotation, ChainDictionary, ContextWindow, DirectiveAction, Engine, EngineOptions,
    Error as CoreError, HanjaDictionary, HanjaVariantSet, HomophoneDetection, InputToken,
    MapDictionary, Match, MatchMark, NumeralStrategy, OriginalGloss, OutputToken, PlainScopeData,
    RecoverableInputError, Recovery, RenderMode, RenderOptions, RenderedToken, RubyBase, Scope,
    ScopeData, SegmentationStrategy, UnihanCharDict, UserDirectives, apply_user_directives,
    convert_plain_text, convert_plain_text_with_options, filter_first_occurrences, mark_homophones,
    mark_homophones_with_detection, process_fallible_tokens, process_tokens, process_tokens_iter,
    process_tokens_with_options, read_plain_text, recover_input_tokens, render_tokens,
    render_tokens_iter, write_plain_text,
};
use proptest::prelude::*;

/// Builds an [`Annotation`] from `Default` plus field assignments, with an
/// optional trailing `..base` to start from another annotation.  `Annotation`
/// is `#[non_exhaustive]`, so this separate test crate cannot use a struct
/// literal; the macro keeps the call sites reading like one.
macro_rules! annotated {
    { $($field:ident : $value:expr,)* ..$base:expr } => {{
        let mut annotation = $base;
        $(annotation.$field = $value;)*
        annotation
    }};
    { $($field:ident : $value:expr),* $(,)? } => {{
        let mut annotation = Annotation::default();
        $(annotation.$field = $value;)*
        annotation
    }};
}
use std::cell::Cell;

#[test]
fn annotation_canonical_hanja_prefers_the_dictionary_spelling() {
    let source_only = annotated! { hanja: "芸術".into() };
    let dictionary_backed = annotated! {
        hanja: "芸術".into(),
        dictionary_hanja: Some("藝術".into()),
    };

    assert_eq!(source_only.canonical_hanja(), "芸術");
    assert_eq!(dictionary_backed.canonical_hanja(), "藝術");
}

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
    annotated! {
        hanja: hanja.into(),
        dictionary_hanja: Some(hanja.into()),
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

fn eager_options() -> EngineOptions {
    EngineOptions {
        segmentation: SegmentationStrategy::Eager,
        ..EngineOptions::default()
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
fn recognizes_unicode_joyo_simplified_asahi_and_compatibility_variants() {
    let mut dict = MapDictionary::new();
    dict.insert("沖繩縣", "오키나와현");
    dict.insert("辨護", "변호");
    dict.insert("藝", "운");
    dict.insert("藝術", "예술");
    dict.insert("總統", "총통");
    dict.insert("俠客", "협객");
    dict.insert("神", "신");

    let output = convert_plain_text(
        "沖縄県 弁護 芸 芸術 总统 侠客 神",
        &dict,
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "오키나와현 변호 운 예술 총통 협객 신");
}

#[test]
fn ambiguous_variant_readings_fall_back_instead_of_guessing() {
    let mut dict = MapDictionary::new();
    dict.insert("辨", "변");
    dict.insert("瓣", "판");

    let tokens = process_tokens(vec![InputToken::<PlainScopeData>::Text("弁".into())], &dict);
    let annotation = match &tokens[0] {
        OutputToken::Annotated(annotation) => annotation,
        other => panic!("expected annotation, got {other:?}"),
    };
    assert_eq!(annotation.reading, "변");
    assert!(!annotation.from_dictionary);
    assert_eq!(annotation.dictionary_hanja, None);
}

#[test]
fn ambiguous_variant_dictionary_identities_fall_back_even_with_the_same_reading() {
    let mut dict = MapDictionary::new();
    dict.insert("發", "발");
    dict.insert("髮", "발");

    let tokens = process_tokens(vec![InputToken::<PlainScopeData>::Text("发".into())], &dict);
    assert_eq!(tokens, vec![OutputToken::Text("发".into())]);
}

#[test]
fn ambiguous_variant_match_metadata_falls_back_instead_of_guessing() {
    struct AmbiguousMetadataDictionary;

    impl HanjaDictionary for AmbiguousMetadataDictionary {
        fn matches_at<'a>(&'a self, source: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            let matches = if source.starts_with("發") {
                vec![
                    Match {
                        byte_len: "發".len(),
                        reading: "발".into(),
                        suffix_reading: None,
                        mark: MatchMark::default(),
                    },
                    Match {
                        byte_len: "發".len(),
                        reading: "발".into(),
                        suffix_reading: None,
                        mark: MatchMark {
                            require_hanja: true,
                            require_hangul: false,
                        },
                    },
                ]
            } else {
                Vec::new()
            };
            Box::new(matches.into_iter())
        }
    }

    let tokens = process_tokens(
        vec![InputToken::<PlainScopeData>::Text("发".into())],
        &AmbiguousMetadataDictionary,
    );
    assert_eq!(tokens, vec![OutputToken::Text("发".into())]);
}

#[test]
fn invalid_custom_variant_candidate_indices_are_ignored() {
    struct InvalidCandidateDictionary;

    impl HanjaDictionary for InvalidCandidateDictionary {
        fn matches_at<'a>(&'a self, _: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            Box::new(core::iter::empty())
        }

        fn matches_at_spellings(&self, _: &[&str]) -> Vec<(usize, Match)> {
            vec![(
                usize::MAX,
                Match {
                    byte_len: "藝".len(),
                    reading: "예".into(),
                    suffix_reading: None,
                    mark: MatchMark::default(),
                },
            )]
        }
    }

    let tokens = process_tokens(
        vec![InputToken::<PlainScopeData>::Text("芸".into())],
        &InvalidCandidateDictionary,
    );

    assert!(tokens.iter().all(|token| {
        !matches!(token, OutputToken::Annotated(annotation) if annotation.from_dictionary)
    }));
}

#[test]
fn custom_dictionary_exact_results_override_variant_results() {
    struct ExactAndVariantDictionary;

    impl HanjaDictionary for ExactAndVariantDictionary {
        fn matches_at<'a>(&'a self, _: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            Box::new(core::iter::empty())
        }

        fn matches_at_spellings(&self, spellings: &[&str]) -> Vec<(usize, Match)> {
            let variant_index = spellings
                .iter()
                .position(|spelling| *spelling == "藝")
                .expect("recognition includes the traditional form");
            vec![
                (
                    0,
                    Match {
                        byte_len: "芸".len(),
                        reading: "운".into(),
                        suffix_reading: None,
                        mark: MatchMark::default(),
                    },
                ),
                (
                    variant_index,
                    Match {
                        byte_len: "藝".len(),
                        reading: "예".into(),
                        suffix_reading: None,
                        mark: MatchMark::default(),
                    },
                ),
            ]
        }
    }

    let tokens = process_tokens(
        vec![InputToken::<PlainScopeData>::Text("芸".into())],
        &ExactAndVariantDictionary,
    );
    let annotation = match &tokens[0] {
        OutputToken::Annotated(annotation) => annotation,
        other => panic!("expected annotation, got {other:?}"),
    };

    assert_eq!(annotation.reading, "운");
    assert_eq!(annotation.dictionary_hanja.as_deref(), Some("芸"));
}

#[test]
fn variant_recognition_does_not_cross_senses_joined_by_a_simplified_form() {
    for unrelated_hanja in ["干", "幹"] {
        let mut dict = MapDictionary::new();
        dict.insert(unrelated_hanja, "간");

        let tokens = process_tokens(vec![InputToken::<PlainScopeData>::Text("乾".into())], &dict);
        let annotation = match &tokens[0] {
            OutputToken::Annotated(annotation) => annotation,
            other => panic!("expected annotation, got {other:?}"),
        };
        assert_eq!(annotation.reading, "건");
        assert!(!annotation.from_dictionary);
        assert_eq!(annotation.dictionary_hanja, None);
    }
}

#[test]
fn variant_recognition_handles_long_unknown_runs_without_rebuilding_choices() {
    let source = "漢".repeat(10_000);
    let output = convert_plain_text(&source, &MapDictionary::new(), RenderMode::HangulOnly);

    assert_eq!(output, "한".repeat(10_000));
}

#[test]
fn variant_sets_render_from_the_canonical_dictionary_spelling() {
    let mut dict = MapDictionary::new();
    dict.insert("藝術", "예술");
    let tokens = process_tokens(
        vec![InputToken::<PlainScopeData>::Text("芸術".into())],
        &dict,
    );
    let annotation = match &tokens[0] {
        OutputToken::Annotated(annotation) => annotation,
        other => panic!("expected annotation, got {other:?}"),
    };
    assert_eq!(annotation.hanja, "芸術");
    assert_eq!(annotation.dictionary_hanja.as_deref(), Some("藝術"));

    let rendered = render_tokens(
        tokens,
        RenderOptions {
            mode: RenderMode::Original,
            hanja_variant_set: HanjaVariantSet::Shinjitai,
            ..RenderOptions::default()
        },
    );
    assert_eq!(rendered, vec![RenderedToken::Text("芸術".into())]);
}

#[test]
fn every_variant_set_applies_to_hanja_in_parentheses_and_original_output() {
    let annotation = annotated! {
        hanja: "芸術".into(),
        dictionary_hanja: Some("藝術".into()),
        reading: "예술".into(),
        from_dictionary: true,
    };
    let cases = [
        (HanjaVariantSet::AsDictionary, "藝術"),
        (HanjaVariantSet::Shinjitai, "芸術"),
        (HanjaVariantSet::Kanxi, "藝術"),
        (HanjaVariantSet::Simplified, "艺术"),
        (HanjaVariantSet::Asahimoji, "芸術"),
    ];
    for (variant_set, expected) in cases {
        let parens = render_tokens(
            vec![OutputToken::<PlainScopeData>::Annotated(annotation.clone())],
            RenderOptions {
                mode: RenderMode::HangulHanjaParens,
                hanja_variant_set: variant_set,
                ..RenderOptions::default()
            },
        );
        assert_eq!(
            parens,
            vec![RenderedToken::Text(format!("예술({expected})"))]
        );
        let original = render_tokens(
            vec![OutputToken::<PlainScopeData>::Annotated(annotation.clone())],
            RenderOptions {
                mode: RenderMode::Original,
                hanja_variant_set: variant_set,
                ..RenderOptions::default()
            },
        );
        assert_eq!(original, vec![RenderedToken::Text(expected.into())]);
    }
}

#[test]
fn variant_sets_normalize_transitive_dictionary_spellings() {
    let cases = [
        ("艺术", HanjaVariantSet::Shinjitai, "芸術"),
        ("芸術", HanjaVariantSet::Simplified, "艺术"),
    ];
    for (dictionary_hanja, variant_set, expected) in cases {
        let annotation = annotated! {
            hanja: dictionary_hanja.into(),
            dictionary_hanja: Some(dictionary_hanja.into()),
            reading: "예술".into(),
            from_dictionary: true,
        };
        let rendered = render_tokens(
            vec![OutputToken::<PlainScopeData>::Annotated(annotation)],
            RenderOptions {
                mode: RenderMode::Original,
                hanja_variant_set: variant_set,
                ..RenderOptions::default()
            },
        );
        assert_eq!(rendered, vec![RenderedToken::Text(expected.into())]);
    }
}

#[test]
fn variant_sets_prefer_direct_mappings_and_preserve_existing_targets() {
    let cases = [
        ("並", HanjaVariantSet::Simplified, "并"),
        ("苧", HanjaVariantSet::Simplified, "苎"),
        ("叙", HanjaVariantSet::Simplified, "叙"),
        ("卷", HanjaVariantSet::Shinjitai, "巻"),
        ("卷", HanjaVariantSet::Asahimoji, "巻"),
        ("侠桧涛祷", HanjaVariantSet::Asahimoji, "侠桧涛祷"),
        ("値", HanjaVariantSet::Kanxi, "値"),
        ("值", HanjaVariantSet::Kanxi, "値"),
        ("叱", HanjaVariantSet::Kanxi, "叱"),
        ("𠮟", HanjaVariantSet::Kanxi, "叱"),
        ("漢字", HanjaVariantSet::Kanxi, "漢字"),
        ("難民", HanjaVariantSet::Kanxi, "難民"),
    ];
    for (dictionary_hanja, variant_set, expected) in cases {
        let annotation = annotated! {
            hanja: dictionary_hanja.into(),
            dictionary_hanja: Some(dictionary_hanja.into()),
            reading: "읽기".into(),
            from_dictionary: true,
        };
        let rendered = render_tokens(
            vec![OutputToken::<PlainScopeData>::Annotated(annotation)],
            RenderOptions {
                mode: RenderMode::Original,
                hanja_variant_set: variant_set,
                ..RenderOptions::default()
            },
        );
        assert_eq!(rendered, vec![RenderedToken::Text(expected.into())]);
    }
}

#[test]
fn variant_sets_do_not_cross_senses_joined_by_a_shared_simplified_form() {
    let cases = [
        (HanjaVariantSet::Shinjitai, "發"),
        (HanjaVariantSet::Kanxi, "發"),
        (HanjaVariantSet::Asahimoji, "發"),
    ];
    for (variant_set, expected) in cases {
        let annotation = annotated! {
            hanja: "發".into(),
            dictionary_hanja: Some("發".into()),
            reading: "발".into(),
            from_dictionary: true,
        };
        let rendered = render_tokens(
            vec![OutputToken::<PlainScopeData>::Annotated(annotation)],
            RenderOptions {
                mode: RenderMode::Original,
                hanja_variant_set: variant_set,
                ..RenderOptions::default()
            },
        );
        assert_eq!(rendered, vec![RenderedToken::Text(expected.into())]);
    }
}

#[test]
fn asahi_variant_set_includes_the_verified_extra_pairs() {
    let annotation = annotated! {
        hanja: "彙".into(),
        dictionary_hanja: Some("彙".into()),
        reading: "휘".into(),
        from_dictionary: true,
    };
    let rendered = render_tokens(
        vec![OutputToken::<PlainScopeData>::Annotated(annotation)],
        RenderOptions {
            mode: RenderMode::Original,
            hanja_variant_set: HanjaVariantSet::Asahimoji,
            ..RenderOptions::default()
        },
    );
    assert_eq!(rendered, vec![RenderedToken::Text("彚".into())]);
}

#[test]
fn convert_plain_text_collapses_redundant_parens_like_the_default() {
    let mut dict = MapDictionary::new();
    dict.insert("庫間", "곳간");
    let output = convert_plain_text("庫間(곳간)", &dict, RenderMode::HangulOnly);
    assert_eq!(output, "곳간(庫間)");
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
        OutputToken::Annotated(annotated! {
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
fn unihan_char_dictionary_returns_single_canonical_character_matches() {
    let matches: Vec<_> = UnihanCharDict.matches_at("龍馬").collect();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].byte_len, "龍".len());
    assert_eq!(matches[0].reading, "룡");
    assert_eq!(matches[0].mark, MatchMark::default());
    assert_eq!(UnihanCharDict.max_word_chars(), Some(1));
    assert!(UnihanCharDict.has_homophone("漢", "한"));
    assert!(!UnihanCharDict.has_homophone("龍馬", "룡마"));
    assert!(UnihanCharDict.matches_at("가龍").next().is_none());
}

#[test]
fn unihan_char_dictionary_folds_compatibility_ideographs() {
    let compatibility = UnihanCharDict.matches_at("豈").next().unwrap();
    let unified = UnihanCharDict.matches_at("豈").next().unwrap();

    assert_eq!(compatibility.byte_len, "豈".len());
    assert_eq!(compatibility.reading, unified.reading);
}

#[test]
fn unihan_char_dictionary_matches_fallback_canonical_reading_without_initial_sound_law() {
    let no_law = EngineOptions {
        initial_sound_law: false,
        numeral_strategy: NumeralStrategy::HangulPhonetic,
        ..EngineOptions::default()
    };
    let cases = ["龍", "馬", "漢", "字", "\u{349A}"];

    for input in cases {
        let fallback = convert_plain_text_with_options(
            input,
            &MapDictionary::new(),
            RenderMode::HangulOnly,
            no_law,
        );
        let matches: Vec<_> = UnihanCharDict.matches_at(input).collect();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reading, fallback);
    }
}

#[test]
fn single_hanja_dictionary_reading_follows_initial_sound_law_by_position() {
    // The dictionary stores the word-initial form; the engine recovers the
    // original sound outside word-initial position from the unihan readings.
    let mut dict = MapDictionary::new();
    dict.insert("年", "연");

    assert_eq!(
        convert_plain_text("年", &dict, RenderMode::HangulOnly),
        "연"
    );
    assert_eq!(
        convert_plain_text("1998年", &dict, RenderMode::HangulOnly),
        "1998년"
    );

    // With initial sound law disabled the original sound is used everywhere.
    let no_law = EngineOptions {
        initial_sound_law: false,
        ..EngineOptions::default()
    };
    assert_eq!(
        convert_plain_text_with_options("年", &dict, RenderMode::HangulOnly, no_law),
        "년"
    );
}

#[test]
fn variant_dictionary_reading_follows_initial_sound_law_by_position() {
    let mut dict = MapDictionary::new();
    dict.insert("勞", "노");

    assert_eq!(
        convert_plain_text("1998勞", &dict, RenderMode::HangulOnly),
        "1998로"
    );
    assert_eq!(
        convert_plain_text("1998劳", &dict, RenderMode::HangulOnly),
        "1998로"
    );
}

#[test]
fn single_hanja_dictionary_reading_honors_yeol_yul_rule() {
    // `률`/`렬` keep the `율`/`열` form after a vowel or `ㄴ` coda even outside
    // word-initial position, just as fallback does.
    let mut dict = MapDictionary::new();
    dict.insert("比", "비");
    dict.insert("學", "학");
    dict.insert("率", "률");
    dict.insert("列", "렬");

    // `比` ends in a vowel, so `率`/`列` take the `율`/`열` form.
    assert_eq!(
        convert_plain_text("比率", &dict, RenderMode::HangulOnly),
        "비율"
    );
    assert_eq!(
        convert_plain_text("比列", &dict, RenderMode::HangulOnly),
        "비열"
    );
    // `學` ends in a `ㄱ` coda, so the original `률` is kept.
    assert_eq!(
        convert_plain_text("學率", &dict, RenderMode::HangulOnly),
        "학률"
    );
}

#[test]
fn multi_syllable_suffix_reading_follows_position() {
    // A dictionary that records a distinct suffix reading (as the Standard
    // Korean Language Dictionary does for `年代`).
    let mut dict = MapDictionary::new();
    dict.insert_with_suffix("年代", "연대", "년대");
    dict.insert("理論", "이론");

    assert_eq!(
        convert_plain_text("年代", &dict, RenderMode::HangulOnly),
        "연대"
    );
    assert_eq!(
        convert_plain_text("1990年代", &dict, RenderMode::HangulOnly),
        "1990년대"
    );
    // A multi-syllable entry without a suffix reading is left untouched, even
    // though its leading hanja undergoes initial sound law.
    assert_eq!(
        convert_plain_text("理論", &dict, RenderMode::HangulOnly),
        "이론"
    );
}

#[test]
fn chain_dictionary_keeps_first_match_for_duplicate_lengths() {
    let mut high = MapDictionary::new();
    high.insert("漢字", "사용자");
    high.insert("漢", "한");
    let mut low = MapDictionary::new();
    low.insert("漢字", "표준");
    low.insert("漢字語", "한자어");
    let chain = ChainDictionary::from_iter([high, low]);

    let matches: Vec<_> = chain.matches_at("漢字語").collect();

    assert_eq!(matches.len(), 3);
    assert_eq!(
        matches
            .iter()
            .map(|matched| matched.reading.as_str())
            .collect::<Vec<_>>(),
        vec!["한", "사용자", "한자어"]
    );
}

#[test]
fn chain_dictionary_allows_lattice_to_choose_lower_priority_longer_match() {
    let mut high = MapDictionary::new();
    high.insert("行事", "행사");
    let mut low = MapDictionary::new();
    low.insert("行事場", "행사장");
    let chain = ChainDictionary::from_iter([high, low]);

    let output = convert_plain_text("行事場", &chain, RenderMode::HangulHanjaParens);

    assert_eq!(output, "행사장(行事場)");
}

#[test]
fn chain_dictionary_priority_applies_before_variant_spelling_preference() {
    let mut high = MapDictionary::new();
    high.insert("藝術", "예술");
    let mut low = MapDictionary::new();
    low.insert("芸術", "기예");
    let chain = ChainDictionary::from_iter([high, low]);

    assert_eq!(
        convert_plain_text("芸術", &chain, RenderMode::HangulOnly),
        "예술"
    );
}

#[test]
fn chain_dictionary_reports_homophones_from_any_dictionary() {
    let mut first = MapDictionary::new();
    first.insert("天地", "천지");
    let mut second = MapDictionary::new();
    second.insert("漢字", "한자");
    second.insert("翰字", "한자");
    let chain = ChainDictionary::from_iter([first, second]);

    assert!(chain.has_homophone("漢字", "한자"));
    assert!(!chain.has_homophone("天地", "천지"));
}

#[test]
fn chain_dictionary_homophones_respect_higher_priority_overrides() {
    let mut high = MapDictionary::new();
    high.insert("翰字", "하자");
    let mut low = MapDictionary::new();
    low.insert("漢字", "한자");
    low.insert("翰字", "한자");
    let chain = ChainDictionary::from_iter([high, low]);

    assert!(!chain.has_homophone("漢字", "한자"));
    assert!(!chain.has_homophone("翰字", "하자"));
}

#[test]
fn chain_dictionary_homophones_fall_back_for_lookup_only_dictionaries() {
    struct LookupOnlyHomophoneDictionary;

    impl HanjaDictionary for LookupOnlyHomophoneDictionary {
        fn matches_at<'a>(&'a self, _s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            Box::new(std::iter::empty())
        }

        fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
            hanja == "漢字" && reading == "한자"
        }
    }

    let mut enumerable = MapDictionary::new();
    enumerable.insert("天地", "천지");
    let chain = ChainDictionary::from_iter([
        Box::new(enumerable) as Box<dyn HanjaDictionary>,
        Box::new(LookupOnlyHomophoneDictionary),
    ]);

    assert!(chain.has_homophone("漢字", "한자"));
    assert!(!chain.has_homophone("天地", "천지"));
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
fn eager_takes_longest_prefix_even_when_lattice_covers_more_dictionary_text() {
    let output = convert_plain_text_with_options(
        "行事場所",
        &segmentation_dictionary(),
        RenderMode::HangulHanjaParens,
        eager_options(),
    );

    assert_eq!(output, "행사장(行事場)소(所)");
}

#[test]
fn eager_keeps_valid_long_match_when_it_covers_the_run() {
    let output = convert_plain_text_with_options(
        "行事場入口",
        &segmentation_dictionary(),
        RenderMode::HangulHanjaParens,
        eager_options(),
    );

    assert_eq!(output, "행사장(行事場)입구(入口)");
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
fn eager_consumes_mixed_script_dictionary_entries_as_one_annotation() {
    let output = convert_plain_text_with_options(
        "汽車길",
        &mixed_script_dictionary(),
        RenderMode::HangulHanjaParens,
        eager_options(),
    );

    assert_eq!(output, "기찻길(汽車길)");
}

#[test]
fn mixed_script_annotation_keeps_the_full_source_spelling() {
    let output = process_tokens(
        vec![InputToken::<PlainScopeData>::Text("色깔論".into())],
        &mixed_script_dictionary(),
    );

    assert_eq!(
        output,
        vec![OutputToken::Annotated(annotated! {
            hanja: "色깔論".into(),
            dictionary_hanja: Some("色깔論".into()),
            reading: "색깔론".into(),
            homophone: false,
            require_hanja: false,
            require_hangul: false,
            first_in_context: true,
            skip_annotation: false,
            from_dictionary: true,
            from_source_gloss: false,
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
    let baseline = CountingDictionary::new(vec![("漢字", "한자")]);

    let output = convert_plain_text("가나다 漢字", &dict, RenderMode::HangulOnly);

    assert_eq!(output, "가나다 한자");
    let _ = convert_plain_text("漢字", &baseline, RenderMode::HangulOnly);
    assert_eq!(dict.lookup_count(), baseline.lookup_count());
}

#[test]
fn mixed_script_lookup_can_cross_text_chunk_boundaries() {
    let tokens = vec![
        InputToken::<PlainScopeData>::Text("汽車".into()),
        InputToken::Text("길".into()),
    ];
    let output = render_tokens(
        process_tokens(tokens, &mixed_script_dictionary()),
        RenderMode::HangulHanjaParens,
    );

    assert_eq!(write_plain_text(output), "기찻길(汽車길)");
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
fn verbatim_boundaries_reset_fallback_context() {
    let tokens = vec![
        InputToken::<PlainScopeData>::Text("각".into()),
        InputToken::Verbatim("`x`".into()),
        InputToken::Text("律".into()),
    ];
    let output = render_tokens(
        process_tokens(tokens, &MapDictionary::new()),
        RenderMode::HangulOnly,
    );

    assert_eq!(write_plain_text(output), "각`x`율");
}

#[test]
fn preserved_scope_boundaries_reset_fallback_context() {
    let tokens = vec![
        InputToken::Text("각".into()),
        InputToken::Open(Scope::new(TestScopeData {
            preserve: true,
            block_boundary: false,
        })),
        InputToken::Text("x".into()),
        InputToken::Close,
        InputToken::Text("律".into()),
    ];
    let output = render_tokens(
        process_tokens(tokens, &MapDictionary::new()),
        RenderMode::HangulOnly,
    );

    assert_eq!(write_plain_text(output), "각x율");
}

#[test]
fn block_boundaries_reset_fallback_context() {
    let block = TestScopeData {
        preserve: false,
        block_boundary: true,
    };
    let tokens = vec![
        InputToken::Open(Scope::new(block.clone())),
        InputToken::Text("各".into()),
        InputToken::Close,
        InputToken::Open(Scope::new(block)),
        InputToken::Text("律".into()),
        InputToken::Close,
    ];
    let output = render_tokens(
        process_tokens(tokens, &MapDictionary::new()),
        RenderMode::HangulOnly,
    );

    assert_eq!(write_plain_text(output), "각율");
}

#[test]
fn streaming_engine_matches_one_shot_across_mixed_script_chunks() {
    let dict = mixed_script_dictionary();
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);
    let mut output = Vec::new();

    output.extend(engine.push_token(InputToken::Text("汽".into())));
    assert!(output.is_empty());
    output.extend(engine.push_token(InputToken::Text("車".into())));
    output.extend(engine.push_token(InputToken::Text("길".into())));
    output.extend(engine.finish());

    let rendered = render_tokens(output, RenderMode::HangulHanjaParens);

    assert_eq!(write_plain_text(rendered), "기찻길(汽車길)");
}

#[test]
fn streaming_engine_preserves_non_hanja_prefix_for_mixed_script_match() {
    let mut dict = MapDictionary::new();
    dict.insert("가羅", "가라");
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);
    let mut output = Vec::new();

    output.extend(engine.push_token(InputToken::Text("가".into())));
    output.extend(engine.push_token(InputToken::Text("羅".into())));
    output.extend(engine.finish());
    let chunked = write_plain_text(render_tokens(output, RenderMode::HangulHanjaParens));

    assert_eq!(
        chunked,
        convert_plain_text("가羅", &dict, RenderMode::HangulHanjaParens)
    );
    assert_eq!(chunked, "가라(가羅)");
}

#[test]
fn streaming_engine_flushes_at_structural_boundaries() {
    let dict = mixed_script_dictionary();
    let mut engine = Engine::new(&dict);
    let mut output = Vec::new();

    output.extend(engine.push_token(InputToken::<PlainScopeData>::Text("汽車".into())));
    output.extend(engine.push_token(InputToken::Verbatim("길".into())));
    output.extend(engine.finish());

    let rendered = render_tokens(output, RenderMode::HangulHanjaParens);

    assert_eq!(write_plain_text(rendered), "기차(汽車)길");
}

#[test]
fn streaming_engine_preserves_long_unknown_dictionary_match() {
    let dict = CountingDictionary::without_max_word_chars(vec![(
        "龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜",
        "긴항목",
    )]);
    let input = "龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜龜";
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);
    let mut output = Vec::new();

    for ch in input.chars() {
        output.extend(engine.push_token(InputToken::Text(ch.to_string())));
    }
    output.extend(engine.finish());
    let chunked = write_plain_text(render_tokens(output, RenderMode::HangulOnly));

    assert_eq!(input.chars().count(), 36);
    assert_eq!(
        chunked,
        convert_plain_text(input, &dict, RenderMode::HangulOnly)
    );
    assert_eq!(chunked, "긴항목");
}

#[test]
fn streaming_engine_flushes_unknown_dictionary_at_whitespace_boundaries() {
    let dict = CountingDictionary::without_max_word_chars(Vec::new());
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);

    let output = engine.push_token(InputToken::Text("北\n".into()));
    let chunked = write_plain_text(render_tokens(output, RenderMode::HangulOnly));

    assert_eq!(chunked, "북\n");
    assert_eq!(engine.buffered_chars(), 0);
}

#[test]
fn streaming_engine_waits_for_overlapping_tail_matches() {
    let mut dict = MapDictionary::new();
    dict.insert("乙丙丁", "왼쪽");
    dict.insert("丙丁戊", "오른쪽");

    let input = "甲乙丙丁戊";
    let one_shot = convert_plain_text(input, &dict, RenderMode::HangulOnly);
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);
    let mut output = Vec::new();
    for ch in input.chars() {
        output.extend(engine.push_token(InputToken::Text(ch.to_string())));
    }
    output.extend(engine.finish());
    let chunked = write_plain_text(render_tokens(output, RenderMode::HangulOnly));

    assert_eq!(one_shot, "갑을오른쪽");
    assert_eq!(chunked, one_shot);
}

#[test]
fn streaming_engine_does_not_split_long_fallback_numeral_runs() {
    let input = "六".repeat(40);
    let dict = MapDictionary::new();
    let one_shot = convert_plain_text(&input, &dict, RenderMode::HangulOnly);
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);
    let mut output = Vec::new();

    for ch in input.chars() {
        output.extend(engine.push_token(InputToken::Text(ch.to_string())));
    }
    output.extend(engine.finish());
    let chunked = write_plain_text(render_tokens(output, RenderMode::HangulOnly));

    assert_eq!(one_shot, format!("육{}", "륙".repeat(39)));
    assert_eq!(chunked, one_shot);
}

#[test]
fn streaming_engine_does_not_split_long_fallback_annotation_runs() {
    let input = "天地玄黃";
    let mut dict = MapDictionary::new();
    dict.insert("甲乙丙", "갑을병");
    let one_shot = convert_plain_text(input, &dict, RenderMode::HangulHanjaParens);
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);
    let mut output = Vec::new();

    for ch in input.chars() {
        output.extend(engine.push_token(InputToken::Text(ch.to_string())));
    }
    output.extend(engine.finish());
    let chunked = write_plain_text(render_tokens(output, RenderMode::HangulHanjaParens));

    assert_eq!(one_shot, "천지현황(天地玄黃)");
    assert_eq!(chunked, one_shot);
}

#[test]
fn streaming_engine_keeps_tail_fallback_run_for_one_char_dictionary() {
    let input = "天地玄";
    let mut dict = MapDictionary::new();
    dict.insert("甲", "갑");
    let one_shot = convert_plain_text(input, &dict, RenderMode::HangulHanjaParens);
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);
    let mut output = Vec::new();

    for ch in input.chars() {
        output.extend(engine.push_token(InputToken::Text(ch.to_string())));
    }
    output.extend(engine.finish());
    let chunked = write_plain_text(render_tokens(output, RenderMode::HangulHanjaParens));

    assert_eq!(one_shot, "천지현(天地玄)");
    assert_eq!(chunked, one_shot);
}

#[test]
fn streaming_engine_does_not_resegment_buffered_fallback_run_quadratically() {
    let input = "天".repeat(64);
    let dict = CountingDictionary::new(vec![("甲", "갑")]);
    let mut expected_dict = MapDictionary::new();
    expected_dict.insert("甲", "갑");
    let one_shot = convert_plain_text(&input, &expected_dict, RenderMode::HangulHanjaParens);
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);
    let mut output = Vec::new();

    for ch in input.chars() {
        output.extend(engine.push_token(InputToken::Text(ch.to_string())));
    }
    output.extend(engine.finish());
    let chunked = write_plain_text(render_tokens(output, RenderMode::HangulHanjaParens));

    assert_eq!(chunked, one_shot);
    assert!(
        dict.lookup_count() <= input.chars().count() * 2 + 1,
        "lookup count should stay linear, got {}",
        dict.lookup_count()
    );
}

#[test]
fn streaming_engine_fallback_fast_path_still_detects_tail_dictionary_match() {
    let input = "玄玄玄天地";
    let mut dict = MapDictionary::new();
    dict.insert("天地", "천지");
    let one_shot = convert_plain_text(input, &dict, RenderMode::HangulHanjaParens);
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);
    let mut output = Vec::new();

    for ch in input.chars() {
        output.extend(engine.push_token(InputToken::Text(ch.to_string())));
    }
    output.extend(engine.finish());
    let chunked = write_plain_text(render_tokens(output, RenderMode::HangulHanjaParens));

    assert_eq!(one_shot, "현현현(玄玄玄)천지(天地)");
    assert_eq!(chunked, one_shot);
}

proptest! {
    #[test]
    fn chunked_plain_text_matches_one_shot(
        chunks in prop::collection::vec("[가-힣A-Za-z .,!?]{0,8}|漢字|天地|汽|車|길|色|깔|論", 0..32)
    ) {
        let dict = mixed_script_dictionary();
        let input = chunks.concat();
        let mut engine = Engine::<PlainScopeData, _>::new(&dict);
        let mut output = Vec::new();

        for chunk in chunks {
            output.extend(engine.push_token(InputToken::Text(chunk)));
        }
        output.extend(engine.finish());

        let chunked = write_plain_text(render_tokens(output, RenderMode::HangulOnly));
        let one_shot = convert_plain_text(&input, &dict, RenderMode::HangulOnly);

        prop_assert_eq!(chunked, one_shot);
    }
}

#[test]
fn whitespace_bounded_lattice_skips_non_hanja_spans_without_max_word_length() {
    let dict = CountingDictionary::without_max_word_chars(vec![("漢字", "한자")]);
    let baseline = CountingDictionary::without_max_word_chars(vec![("漢字", "한자")]);

    let output = convert_plain_text("가나다라마바사 漢字", &dict, RenderMode::HangulOnly);

    assert_eq!(output, "가나다라마바사 한자");
    let _ = convert_plain_text("漢字", &baseline, RenderMode::HangulOnly);
    assert_eq!(dict.lookup_count(), baseline.lookup_count());
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestSectionScopeData {
    section_boundary: bool,
}

impl ScopeData for TestSectionScopeData {
    fn is_preserve(&self) -> bool {
        false
    }

    fn is_section_boundary(&self) -> bool {
        self.section_boundary
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
fn engine_uses_the_current_scope_preserve_flag() {
    let tokens = vec![
        InputToken::Open(Scope::new(TestScopeData {
            preserve: true,
            block_boundary: false,
        })),
        InputToken::Open(Scope::new(TestScopeData {
            preserve: false,
            block_boundary: false,
        })),
        InputToken::Text("漢字".into()),
        InputToken::Close,
        InputToken::Close,
    ];

    let output = process_tokens(tokens, &sample_dictionary());

    assert!(output.contains(&OutputToken::Annotated(annotation("漢字", "한자"))));
}

#[test]
fn strict_recovery_returns_reader_error_and_stops() {
    let tokens = vec![
        Ok(InputToken::<PlainScopeData>::Text("漢字".into())),
        Err(RecoverableInputError::new(
            "<broken".into(),
            CoreError::Internal("reader failed"),
        )),
        Ok(InputToken::Text("天地".into())),
    ];

    let error = process_fallible_tokens(tokens, &sample_dictionary(), Recovery::Strict)
        .expect_err("strict recovery must return the reader error");

    assert!(matches!(error, CoreError::Internal("reader failed")));
}

#[test]
fn lenient_recovery_resets_fallback_context_after_bad_region() {
    let tokens = vec![
        Ok(InputToken::<PlainScopeData>::Text("각".into())),
        Err(RecoverableInputError::new(
            "<x>".into(),
            CoreError::Internal("reader failed"),
        )),
        Ok(InputToken::Text("律".into())),
    ];

    let output = process_fallible_tokens(tokens, &MapDictionary::new(), Recovery::Lenient)
        .expect("lenient recovery should keep processing after recoverable regions");
    let rendered = render_tokens(output, RenderMode::HangulOnly);

    assert_eq!(write_plain_text(rendered), "각<x>율");
}

#[test]
fn recover_input_tokens_strict_returns_first_error() {
    let tokens = vec![
        Ok(InputToken::<PlainScopeData>::Text("漢字".into())),
        Err(RecoverableInputError::new(
            "<broken".into(),
            CoreError::Internal("reader failed"),
        )),
    ];

    let error = recover_input_tokens(tokens, Recovery::Strict)
        .expect_err("strict recovery must return the first reader error");

    assert!(matches!(error, CoreError::Internal("reader failed")));
}

#[test]
fn recover_input_tokens_lenient_preserves_region_as_verbatim() {
    let tokens = vec![
        Ok(InputToken::<PlainScopeData>::Text("漢字".into())),
        Err(RecoverableInputError::new(
            "<broken".into(),
            CoreError::Internal("reader failed"),
        )),
        Ok(InputToken::Text("天地".into())),
    ];

    let recovered = recover_input_tokens(tokens, Recovery::Lenient)
        .expect("lenient recovery never fails for recoverable input errors");

    assert_eq!(
        recovered,
        vec![
            InputToken::Text("漢字".into()),
            InputToken::Verbatim("<broken".into()),
            InputToken::Text("天地".into()),
        ]
    );
}

proptest! {
    #[test]
    fn lenient_recovery_preserves_bad_regions_and_continues(
        prefix in "[A-Za-z가-힣 ]{0,24}",
        recovered in "[<>/A-Za-z0-9 ]{1,16}",
        suffix in "[A-Za-z가-힣 ]{0,24}",
    ) {
        let tokens = vec![
            Ok(InputToken::<PlainScopeData>::Text(prefix.clone())),
            Err(RecoverableInputError::new(
                recovered.clone(),
                CoreError::Internal("reader failed"),
            )),
            Ok(InputToken::Text(format!("漢字{suffix}"))),
        ];

        let output = process_fallible_tokens(tokens, &sample_dictionary(), Recovery::Lenient)
            .expect("lenient recovery should not fail for recoverable input errors");
        let rendered = render_tokens(output, RenderMode::HangulOnly);

        prop_assert_eq!(
            write_plain_text(rendered),
            format!("{prefix}{recovered}한자{suffix}")
        );
    }
}

#[test]
fn hanja_without_fallback_reading_is_preserved_as_text() {
    let output = convert_plain_text(
        "\u{9FFF}와 漢字",
        &sample_dictionary(),
        RenderMode::HangulOnly,
    );

    assert_eq!(output, "\u{9FFF}와 한자");
}

#[test]
fn fallback_phoneticizes_unihan_khangul_samples() {
    let cases = [
        ("學問", "학문"),
        ("山川", "산천"),
        ("龍馬", "용마"),
        ("\u{349A}", "온"),
    ];

    for (input, expected) in cases {
        let output = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

        assert_eq!(output, expected);
    }
}

#[test]
fn fallback_uses_pre_initial_sound_law_khangul_reading_as_canonical() {
    let default = convert_plain_text("龍", &MapDictionary::new(), RenderMode::HangulOnly);
    assert_eq!(default, "용");

    let no_law = EngineOptions {
        initial_sound_law: false,
        numeral_strategy: NumeralStrategy::HangulPhonetic,
        ..EngineOptions::default()
    };
    let output = convert_plain_text_with_options(
        "龍",
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        no_law,
    );

    assert_eq!(output, "룡");
}

#[test]
fn fallback_keeps_pre_initial_readings_inside_words() {
    let cases = [("古老", "고로"), ("가老", "가로")];

    for (input, expected) in cases {
        let output = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

        assert_eq!(output, expected);
    }

    let no_law = EngineOptions {
        initial_sound_law: false,
        numeral_strategy: NumeralStrategy::HangulPhonetic,
        ..EngineOptions::default()
    };
    let output = convert_plain_text_with_options(
        "老",
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        no_law,
    );

    assert_eq!(output, "로");
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
    let cases = [("가羅", "가라"), ("가來", "가래"), ("色깔論", "색깔론")];

    for (input, expected) in cases {
        let output = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

        assert_eq!(output, expected);
    }
}

#[test]
fn unmapped_hanja_inside_fallback_run_does_not_reset_word_start() {
    let input = format!("\u{9FFF}{}", "羅");
    let output = convert_plain_text(&input, &MapDictionary::new(), RenderMode::HangulOnly);

    assert_eq!(output, format!("\u{9FFF}{}", "라"));
}

#[test]
fn fallback_initial_sound_law_can_be_disabled() {
    let options = EngineOptions {
        initial_sound_law: false,
        numeral_strategy: NumeralStrategy::HangulPhonetic,
        ..EngineOptions::default()
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
        ..EngineOptions::default()
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
fn fallback_keeps_context_after_alternate_dictionary_readings() {
    let mut dict = MapDictionary::new();
    dict.insert("音樂", "음악");

    let output = convert_plain_text("音樂律", &dict, RenderMode::HangulOnly);

    assert_eq!(output, "음악률");
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
fn positional_arabic_numerals_convert_digit_only_runs() {
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::PositionalArabic,
        ..EngineOptions::default()
    };
    let cases = [
        ("二〇一六年", "2016년"),
        ("貳〇壹陸年", "2016년"),
        ("十一月", "십일월"),
    ];

    for (input, expected) in cases {
        let output = convert_plain_text_with_options(
            input,
            &MapDictionary::new(),
            RenderMode::HangulOnly,
            options,
        );

        assert_eq!(output, expected);
    }
}

#[test]
fn positional_arabic_numerals_override_dictionary_calendar_entries() {
    let mut dict = MapDictionary::new();
    // Do not add standalone 年/月/日 entries here.  Calendar dictionary words
    // must still normalize when the unit suffix is only present inside the
    // whole dictionary entry.
    dict.insert("二", "이");
    dict.insert("六", "육");
    dict.insert("六月", "유월");
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::PositionalArabic,
        ..EngineOptions::default()
    };

    let output = convert_plain_text_with_options(
        "二〇二六年 六月 二〇日",
        &dict,
        RenderMode::HangulOnly,
        options,
    );

    assert_eq!(output, "2026년 6월 20일");
}

#[test]
fn positional_arabic_does_not_split_additive_numerals() {
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::PositionalArabic,
        ..EngineOptions::default()
    };
    let cases = ["二十", "一百二十三", "第六"];

    for input in cases {
        let output = convert_plain_text_with_options(
            input,
            &MapDictionary::new(),
            RenderMode::HangulOnly,
            options,
        );
        let fallback = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

        assert_eq!(output, fallback);
    }
}

#[test]
fn additive_arabic_numerals_parse_place_markers() {
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::AdditiveArabic,
        ..EngineOptions::default()
    };
    let cases = [
        ("二〇一六年", "이공일륙년"),
        ("十一月", "11월"),
        ("一千二百三十四", "1234"),
        ("參佰拾圓", "310원"),
        ("拾萬圓", "100000원"),
    ];

    for (input, expected) in cases {
        let output = convert_plain_text_with_options(
            input,
            &MapDictionary::new(),
            RenderMode::HangulOnly,
            options,
        );

        assert_eq!(output, expected);
    }
}

#[test]
fn additive_arabic_numerals_override_dictionary_calendar_entries() {
    let mut dict = MapDictionary::new();
    // Standalone 月 is intentionally absent; the Arabic edge must be able to
    // replace the whole calendar entry.
    dict.insert("十月", "시월");
    dict.insert("十一月", "십일월");
    dict.insert("十二月", "십이월");
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::AdditiveArabic,
        ..EngineOptions::default()
    };

    let output = convert_plain_text_with_options(
        "十月 十一月 十二月",
        &dict,
        RenderMode::HangulOnly,
        options,
    );

    assert_eq!(output, "10월 11월 12월");
}

#[test]
fn variant_dictionary_match_with_wider_utf8_key_does_not_panic_in_numeral_checks() {
    let mut dict = MapDictionary::new();
    dict.insert("𱁶年", "특별");
    for segmentation in [SegmentationStrategy::Lattice, SegmentationStrategy::Eager] {
        let options = EngineOptions {
            segmentation,
            numeral_strategy: NumeralStrategy::AdditiveArabic,
            ..EngineOptions::default()
        };
        let output =
            convert_plain_text_with_options("千年", &dict, RenderMode::HangulOnly, options);
        assert_eq!(output, "1000년");
    }
}

#[test]
fn smart_numerals_choose_arabic_only_for_structured_numbers() {
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::Smart,
        ..EngineOptions::default()
    };
    let cases = [
        // Existing cases (must remain unchanged)
        ("二〇一六年", "2016년"),
        ("十一月", "11월"),
        ("一千二百三十四", "1234"),
        ("六", "육"),
        ("陸", "육"),
        ("北京", "북경"),
        ("京", "경"),
        ("萬", "만"),
        ("百濟", "백제"),
        ("千里", "천리"),
        ("十長生", "십장생"),
        // Short digit run + unit hanja → Arabic
        ("三時", "3시"),
        ("五日", "5일"),
        ("七年", "7년"),
        ("十月", "10월"),
        ("百年", "100년"),
        ("十一日", "11일"),
        ("六月", "6월"),
        ("六年", "6년"),
        // Short digit run without unit → hangul phonetic
        ("三", "삼"),
        ("一二三", "일이삼"),
    ];

    for (input, expected) in cases {
        let output = convert_plain_text_with_options(
            input,
            &MapDictionary::new(),
            RenderMode::HangulOnly,
            options,
        );

        assert_eq!(output, expected);
    }
}

#[test]
fn smart_numerals_override_dictionary_calendar_entries() {
    let mut dict = MapDictionary::new();
    // These entries model lexicalized calendar readings without separate unit
    // entries, which used to let the dictionary path beat numeric
    // normalization.
    dict.insert("二", "이");
    dict.insert("六", "육");
    dict.insert("六月", "유월");
    dict.insert("十月", "시월");
    dict.insert("十一月", "십일월");
    dict.insert("十二月", "십이월");
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::Smart,
        ..EngineOptions::default()
    };

    let output = convert_plain_text_with_options(
        "二〇二六年 六月 二〇日 十月 十一月 十二月",
        &dict,
        RenderMode::HangulOnly,
        options,
    );

    assert_eq!(output, "2026년 6월 20일 10월 11월 12월");
}

#[test]
fn smart_numerals_do_not_split_non_numeric_dictionary_words() {
    let mut dict = MapDictionary::new();
    dict.insert("北", "북");
    dict.insert("京", "경");
    dict.insert("北京", "베이징");
    dict.insert("一分錢", "일푼전");
    dict.insert("分錢", "분전");
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::Smart,
        ..EngineOptions::default()
    };

    let output =
        convert_plain_text_with_options("北京 一分錢", &dict, RenderMode::HangulOnly, options);

    assert_eq!(output, "베이징 일푼전");
}

#[test]
fn hangul_phonetic_numerals_keep_dictionary_calendar_entries() {
    let mut dict = MapDictionary::new();
    dict.insert("六月", "유월");
    dict.insert("十月", "시월");
    dict.insert("十一月", "십일월");

    let output = convert_plain_text("六月 十月 十一月", &dict, RenderMode::HangulOnly);

    assert_eq!(output, "유월 시월 십일월");
}

#[test]
fn dictionary_matches_preserve_context_after_arabic_numeral_fallback_equivalents() {
    let mut dict = MapDictionary::new();
    dict.insert("二〇一六", "2016");
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::PositionalArabic,
        ..EngineOptions::default()
    };

    let output =
        convert_plain_text_with_options("二〇一六年", &dict, RenderMode::HangulOnly, options);

    assert_eq!(output, "2016년");
}

#[test]
fn arabic_numerals_emit_plain_text_not_annotations() {
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::Smart,
        ..EngineOptions::default()
    };
    let output = process_tokens_with_options(
        [InputToken::Text("十一月".into())],
        &MapDictionary::new(),
        options,
    );

    assert_eq!(
        output,
        vec![
            OutputToken::<PlainScopeData>::Text("11".into()),
            OutputToken::Annotated(annotated! {
                hanja: "月".into(),
                reading: "월".into(),
                homophone: false,
                require_hanja: false,
                require_hangul: false,
                first_in_context: true,
                skip_annotation: false,
                from_dictionary: false,
                from_source_gloss: false,
            }),
        ]
    );
}

#[test]
fn additive_arabic_overflow_falls_back_to_hangul_phonetic() {
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::AdditiveArabic,
        ..EngineOptions::default()
    };
    let input = "九千澗九千澗";
    let output = convert_plain_text_with_options(
        input,
        &MapDictionary::new(),
        RenderMode::HangulOnly,
        options,
    );
    let fallback = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

    assert_eq!(output, fallback);
}

#[test]
fn additive_arabic_rejects_explicit_zero_before_large_units() {
    let options = EngineOptions {
        numeral_strategy: NumeralStrategy::AdditiveArabic,
        ..EngineOptions::default()
    };
    let cases = ["零萬", "一億零萬"];

    for input in cases {
        let output = convert_plain_text_with_options(
            input,
            &MapDictionary::new(),
            RenderMode::HangulOnly,
            options,
        );
        let fallback = convert_plain_text(input, &MapDictionary::new(), RenderMode::HangulOnly);

        assert_eq!(output, fallback);
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
fn trivial_single_char_dictionary_merges_with_fallback() {
    let mut dict = MapDictionary::new();
    dict.insert("洪", "홍");
    dict.insert("民", "민");

    let output = convert_plain_text("洪民憙 部長", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "홍민희(洪民憙) 부장(部長)");
}

#[test]
fn pure_trivial_dictionary_run_merges_into_one_annotation() {
    let mut dict = MapDictionary::new();
    dict.insert("洪", "홍");
    dict.insert("民", "민");

    let output = convert_plain_text("洪民", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "홍민(洪民)");
}

#[test]
fn merged_dictionary_identity_does_not_leak_across_fallback_numeral_boundaries() {
    let mut dict = MapDictionary::new();
    dict.insert("馬", "마");
    let tokens = process_tokens(
        vec![InputToken::<PlainScopeData>::Text("學一馬".into())],
        &dict,
    );
    let last = tokens
        .iter()
        .rev()
        .find_map(|token| match token {
            OutputToken::Annotated(annotation) => Some(annotation),
            _ => None,
        })
        .expect("input produces annotations");
    assert_eq!(last.hanja, "馬");
    assert_eq!(last.dictionary_hanja.as_deref(), Some("馬"));
}

#[test]
fn trivial_dictionary_does_not_merge_across_dictionary_boundary() {
    let mut dict = MapDictionary::new();
    dict.insert("學校", "학교");
    dict.insert("民", "민");
    dict.insert("大學", "대학");
    dict.insert("洪", "홍");

    let output = convert_plain_text("學校民大學", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "학교(學校)민(民)대학(大學)");
}

#[test]
fn trivial_dictionary_respects_initial_sound_law() {
    let mut dict = MapDictionary::new();
    dict.insert("力", "력");

    let output = convert_plain_text("力", &dict, RenderMode::HangulOnly);

    assert_eq!(output, "역");
}

#[test]
fn trivial_dictionary_no_unihan_reading_stays_dictionary() {
    let mut dict = MapDictionary::new();
    dict.insert("洪", "홍");
    dict.insert("民", "민");

    let tokens = process_tokens(read_plain_text("洪 民"), &dict);

    let annotations: Vec<_> = tokens
        .iter()
        .filter_map(|t| match t {
            OutputToken::Annotated(a) => Some(a),
            _ => None,
        })
        .collect();

    assert_eq!(annotations.len(), 2);

    for a in &annotations {
        assert!(
            a.from_dictionary,
            "trivial-dictionary annotations should carry from_dictionary"
        );
    }
}

#[test]
fn trivial_dictionary_from_dictionary_is_true_on_merged_annotation() {
    let mut dict = MapDictionary::new();
    dict.insert("洪", "홍");
    dict.insert("民", "민");

    let tokens = process_tokens(read_plain_text("洪民憙"), &dict);

    let annotations: Vec<_> = tokens
        .iter()
        .filter_map(|t| match t {
            OutputToken::Annotated(a) => Some(a),
            _ => None,
        })
        .collect();

    assert_eq!(annotations.len(), 1);
    assert!(
        annotations[0].from_dictionary,
        "merged trivial+fallback annotation should carry from_dictionary: true"
    );
    assert_eq!(annotations[0].hanja, "洪民憙");
    assert_eq!(annotations[0].reading, "홍민희");
}

#[test]
fn trivial_dictionary_homophone_marking_works_with_merged_annotation() {
    let mut dict = MapDictionary::new();
    dict.insert("天", "천");
    dict.insert("地", "지");
    dict.insert("天池", "천지");

    let tokens = process_tokens(read_plain_text("天地"), &dict);

    let marked = mark_homophones_with_detection(
        tokens,
        &dict,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );

    let annotations: Vec<_> = marked
        .iter()
        .filter_map(|t| match t {
            OutputToken::Annotated(a) => Some(a),
            _ => None,
        })
        .collect();

    assert_eq!(annotations.len(), 1);
    assert!(
        annotations[0].homophone,
        "merged trivial annotation '天地→천지' should be flagged as homophone because '天池→천지' exists in dictionary"
    );
}

#[test]
fn streaming_engine_merges_trivial_dictionary_across_chunks() {
    let mut dict = MapDictionary::new();
    dict.insert("洪", "홍");
    dict.insert("民", "민");

    let input = "洪民憙";
    let tokens: Vec<InputToken<PlainScopeData>> = input
        .chars()
        .map(|ch| InputToken::Text(ch.to_string()))
        .collect();

    let one_shot = process_tokens(tokens.clone(), &dict);

    for split in 1..4 {
        let mut engine = Engine::<PlainScopeData, _>::new(&dict);
        let mut streaming = Vec::new();
        for chunk in tokens.as_slice().chunks(split) {
            for token in chunk.iter().cloned() {
                streaming.extend(engine.push_token(token));
            }
        }
        streaming.extend(engine.finish());

        assert_eq!(
            streaming, one_shot,
            "chunk size {split} must match one-shot"
        );
    }
}

#[test]
fn trivial_dictionary_preserves_fallback_numeral_boundary() {
    let mut dict = MapDictionary::new();
    dict.insert("共", "공");
    dict.insert("和", "화");
    dict.insert("國", "국");

    let output = convert_plain_text("第六共和國", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "제육(第六)공화국(共和國)");
}

#[test]
fn trivial_dictionary_homophone_merges_into_fallback_run() {
    let mut dict = MapDictionary::new();
    dict.insert("洪", "홍");
    dict.insert("紅", "홍");

    let output = convert_plain_text("洪憙", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "홍희(洪憙)");
}

#[test]
fn trivial_dictionary_splits_pure_trivial_homophone_run() {
    let mut dict = MapDictionary::new();
    dict.insert("洪", "홍");
    dict.insert("紅", "홍");

    let output = convert_plain_text("洪紅", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "홍(洪)홍(紅)");
}

#[test]
fn merged_annotation_does_not_override_homophone_from_engine() {
    let mut dict = MapDictionary::new();
    dict.insert("洪", "홍");
    dict.insert("紅", "홍");

    let tokens = process_tokens(read_plain_text("洪憙"), &dict);

    let annotations: Vec<_> = tokens
        .iter()
        .filter_map(|t| match t {
            OutputToken::Annotated(a) => Some(a),
            _ => None,
        })
        .collect();

    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].hanja, "洪憙");
    assert!(
        !annotations[0].homophone,
        "engine does not set homophone; homophone marking is middleware-only"
    );
    assert!(
        annotations[0].from_dictionary,
        "merged annotation carries from_dictionary for middleware to use"
    );
}

#[test]
fn trivial_dictionary_merges_homophones_across_fallback_boundary() {
    let mut dict = MapDictionary::new();
    dict.insert("洪", "홍");
    dict.insert("紅", "홍");

    let output = convert_plain_text("洪憙紅", &dict, RenderMode::HangulHanjaParens);

    assert_eq!(output, "홍희홍(洪憙紅)");
}

#[test]
fn renderer_removes_annotations_from_the_stream() {
    let rendered = render_tokens(
        vec![OutputToken::<PlainScopeData>::Annotated(annotated! {
            hanja: "漢字".into(),
            reading: "한자".into(),
            homophone: false,
            require_hanja: false,
            require_hangul: false,
            first_in_context: true,
            skip_annotation: false,
            from_dictionary: true,
            from_source_gloss: false,
        })],
        RenderMode::HangulOnly,
    );

    assert_eq!(rendered, vec![RenderedToken::Text("한자".into())]);
}

#[test]
fn token_iterator_apis_match_vec_convenience_apis() {
    let tokens = read_plain_text("天地와 漢字");

    let iter_output = process_tokens_iter(tokens.clone(), &sample_dictionary()).collect::<Vec<_>>();
    let vec_output = process_tokens(tokens, &sample_dictionary());
    assert_eq!(iter_output, vec_output);

    let iter_rendered =
        render_tokens_iter(vec_output.clone(), RenderMode::HangulHanjaParens).collect::<Vec<_>>();
    let vec_rendered = render_tokens(vec_output, RenderMode::HangulHanjaParens);
    assert_eq!(iter_rendered, vec_rendered);
}

#[test]
fn render_tokens_iter_is_lazy() {
    let consumed = Cell::new(0);
    let tokens = (0..3).map(|index| {
        consumed.set(consumed.get() + 1);
        OutputToken::<PlainScopeData>::Text(index.to_string())
    });

    let mut rendered = render_tokens_iter(tokens, RenderMode::HangulOnly);

    assert_eq!(consumed.get(), 0);
    assert_eq!(rendered.next(), Some(RenderedToken::Text("0".into())));
    assert_eq!(consumed.get(), 1);
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

    let marked = mark_homophones(tokens, &MapDictionary::new(), ContextWindow::PerDocument);

    assert_eq!(
        marked,
        vec![
            OutputToken::Annotated(annotated! {
                homophone: true,
                ..annotation("漢字", "한자")
            }),
            OutputToken::Text("와 ".into()),
            OutputToken::Annotated(annotated! {
                homophone: true,
                ..annotation("翰字", "한자")
            }),
            OutputToken::Text("와 ".into()),
            OutputToken::Annotated(annotation("天地", "천지")),
        ]
    );
}

#[test]
fn dictionary_wide_detection_uses_dictionary_homophones_even_when_other_form_is_absent() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("漢字", "한자");
    dictionary.insert("翰字", "한자");
    dictionary.insert("天地", "천지");
    let tokens = vec![
        OutputToken::<PlainScopeData>::Annotated(annotation("漢字", "한자")),
        OutputToken::Text("와 ".into()),
        OutputToken::Annotated(annotation("天地", "천지")),
    ];

    let marked = mark_homophones_with_detection(
        tokens,
        &dictionary,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );

    assert_eq!(
        marked,
        vec![
            OutputToken::Annotated(annotated! {
                homophone: true,
                ..annotation("漢字", "한자")
            }),
            OutputToken::Text("와 ".into()),
            OutputToken::Annotated(annotation("天地", "천지")),
        ]
    );
}

#[test]
fn context_local_detection_ignores_dictionary_homophones_when_other_form_is_absent() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("漢字", "한자");
    dictionary.insert("翰字", "한자");
    dictionary.insert("天地", "천지");
    let tokens = vec![
        OutputToken::<PlainScopeData>::Annotated(annotation("漢字", "한자")),
        OutputToken::Text("와 ".into()),
        OutputToken::Annotated(annotation("天地", "천지")),
    ];

    // The default strategy only marks readings that collide within the window.
    // 翰字 (한자) exists in the dictionary but never appears here, so 漢字 is left
    // unglossed.
    let marked = mark_homophones(tokens.clone(), &dictionary, ContextWindow::PerDocument);

    assert_eq!(marked, tokens);
}

#[test]
fn context_local_detection_does_not_build_dictionary_index() {
    struct EntriesPanicDictionary;

    impl HanjaDictionary for EntriesPanicDictionary {
        fn matches_at<'a>(&'a self, _s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            Box::new(std::iter::empty())
        }

        fn entries<'a>(
            &'a self,
        ) -> Option<Box<dyn Iterator<Item = gukhanmun_core::DictionaryRecord> + 'a>> {
            panic!("entries should not be called for context-local detection");
        }
    }

    let tokens = vec![OutputToken::<PlainScopeData>::Annotated(annotation(
        "漢字", "한자",
    ))];

    // Default detection never consults the dictionary index, even with an
    // active window.
    let marked = mark_homophones(
        tokens.clone(),
        &EntriesPanicDictionary,
        ContextWindow::PerDocument,
    );

    assert_eq!(marked, tokens);
}

#[test]
fn homophone_marker_respects_chain_dictionary_homophone_overrides() {
    let mut high = MapDictionary::new();
    high.insert("翰字", "하자");
    let mut low = MapDictionary::new();
    low.insert("漢字", "한자");
    low.insert("翰字", "한자");
    let dictionary = ChainDictionary::from_iter([high, low]);
    let tokens = vec![OutputToken::<PlainScopeData>::Annotated(annotation(
        "漢字", "한자",
    ))];

    let marked = mark_homophones_with_detection(
        tokens.clone(),
        &dictionary,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );

    assert_eq!(marked, tokens);
}

#[test]
fn homophone_marker_falls_back_to_lookup_only_dictionary_homophones() {
    struct LookupOnlyHomophoneDictionary;

    impl HanjaDictionary for LookupOnlyHomophoneDictionary {
        fn matches_at<'a>(&'a self, _s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            Box::new(std::iter::empty())
        }

        fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
            hanja == "漢字" && reading == "한자"
        }
    }

    let tokens = vec![OutputToken::<PlainScopeData>::Annotated(annotation(
        "漢字", "한자",
    ))];

    let marked = mark_homophones_with_detection(
        tokens,
        &LookupOnlyHomophoneDictionary,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );

    assert_eq!(
        marked,
        vec![OutputToken::Annotated(annotated! {
            homophone: true,
            ..annotation("漢字", "한자")
        })]
    );
}

#[test]
fn homophone_marker_preserves_mixed_chain_lookup_fallbacks() {
    struct LookupOnlyHomophoneDictionary;

    impl HanjaDictionary for LookupOnlyHomophoneDictionary {
        fn matches_at<'a>(&'a self, _s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            Box::new(std::iter::empty())
        }

        fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
            hanja == "漢字" && reading == "한자"
        }
    }

    let mut enumerable = MapDictionary::new();
    enumerable.insert("天地", "천지");
    let dictionary = ChainDictionary::from_iter([
        Box::new(enumerable) as Box<dyn HanjaDictionary>,
        Box::new(LookupOnlyHomophoneDictionary),
    ]);
    let tokens = vec![OutputToken::<PlainScopeData>::Annotated(annotation(
        "漢字", "한자",
    ))];

    let marked = mark_homophones_with_detection(
        tokens,
        &dictionary,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );

    assert_eq!(
        marked,
        vec![OutputToken::Annotated(annotated! {
            homophone: true,
            ..annotation("漢字", "한자")
        })]
    );
}

#[test]
fn homophone_marker_off_ignores_dictionary_homophones() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("漢字", "한자");
    dictionary.insert("翰字", "한자");
    let tokens = vec![OutputToken::<PlainScopeData>::Annotated(annotation(
        "漢字", "한자",
    ))];

    let marked = mark_homophones(tokens.clone(), &dictionary, ContextWindow::Off);

    assert_eq!(marked, tokens);
}

#[test]
fn homophone_marker_off_does_not_build_dictionary_index() {
    struct EntriesPanicDictionary;

    impl HanjaDictionary for EntriesPanicDictionary {
        fn matches_at<'a>(&'a self, _s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            Box::new(std::iter::empty())
        }

        fn entries<'a>(
            &'a self,
        ) -> Option<Box<dyn Iterator<Item = gukhanmun_core::DictionaryRecord> + 'a>> {
            panic!("entries should not be called when homophone marking is off");
        }
    }

    let tokens = vec![OutputToken::<PlainScopeData>::Annotated(annotation(
        "漢字", "한자",
    ))];

    let marked = mark_homophones(tokens.clone(), &EntriesPanicDictionary, ContextWindow::Off);

    assert_eq!(marked, tokens);
}

#[test]
fn homophone_marker_ignores_fallback_annotations_even_when_reading_is_ambiguous() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("漢", "한");
    dictionary.insert("翰", "한");
    let tokens = vec![OutputToken::<PlainScopeData>::Annotated(annotated! {
        from_dictionary: false,
        ..annotation("漢", "한")
    })];

    let marked = mark_homophones_with_detection(
        tokens.clone(),
        &dictionary,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );

    assert_eq!(marked, tokens);
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

    let marked = mark_homophones(
        tokens.clone(),
        &MapDictionary::new(),
        ContextWindow::PerBlock,
    );

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

    let marked = mark_homophones(
        tokens.clone(),
        &MapDictionary::new(),
        ContextWindow::PerBlock,
    );

    assert_eq!(marked, tokens);
}

#[test]
fn homophone_marker_keeps_heading_and_body_in_same_section() {
    let heading = TestSectionScopeData {
        section_boundary: true,
    };
    let tokens = vec![
        OutputToken::Open(Scope::new(heading)),
        OutputToken::Annotated(annotation("漢字", "한자")),
        OutputToken::Close,
        OutputToken::Text("\n".into()),
        OutputToken::Annotated(annotation("翰字", "한자")),
    ];

    let marked = mark_homophones(tokens, &MapDictionary::new(), ContextWindow::PerSection);

    assert_eq!(
        marked,
        vec![
            OutputToken::Open(Scope::new(TestSectionScopeData {
                section_boundary: true
            })),
            OutputToken::Annotated(annotated! {
                homophone: true,
                ..annotation("漢字", "한자")
            }),
            OutputToken::Close,
            OutputToken::Text("\n".into()),
            OutputToken::Annotated(annotated! {
                homophone: true,
                ..annotation("翰字", "한자")
            }),
        ]
    );
}

#[test]
fn first_occurrence_filter_clears_repeated_form_requirements() {
    let tokens = vec![
        OutputToken::<PlainScopeData>::Annotated(annotated! {
            require_hanja: true,
            require_hangul: true,
            ..annotation("漢字", "한자")
        }),
        OutputToken::Text(" ".into()),
        OutputToken::Annotated(annotated! {
            require_hanja: true,
            require_hangul: true,
            ..annotation("翰字", "한자")
        }),
        OutputToken::Text(" ".into()),
        OutputToken::Annotated(annotated! {
            require_hanja: true,
            require_hangul: true,
            ..annotation("漢字", "한자")
        }),
    ];

    let filtered = filter_first_occurrences(tokens, ContextWindow::PerDocument);

    assert_eq!(
        filtered,
        vec![
            OutputToken::Annotated(annotated! {
                require_hanja: true,
                require_hangul: true,
                first_in_context: true,
                ..annotation("漢字", "한자")
            }),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(annotated! {
                require_hanja: true,
                require_hangul: true,
                first_in_context: true,
                ..annotation("翰字", "한자")
            }),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(annotated! {
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
    let required = || {
        annotated! {
            require_hanja: true,
            require_hangul: true,
            ..annotation("漢字", "한자")
        }
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
fn first_occurrence_filter_keeps_heading_and_body_in_same_section() {
    let heading = TestSectionScopeData {
        section_boundary: true,
    };
    let required = || {
        annotated! {
            require_hanja: true,
            require_hangul: true,
            ..annotation("漢字", "한자")
        }
    };
    let tokens = vec![
        OutputToken::Open(Scope::new(heading)),
        OutputToken::Annotated(required()),
        OutputToken::Close,
        OutputToken::Text("\n".into()),
        OutputToken::Annotated(required()),
    ];

    let filtered = filter_first_occurrences(tokens, ContextWindow::PerSection);

    assert_eq!(
        filtered,
        vec![
            OutputToken::Open(Scope::new(TestSectionScopeData {
                section_boundary: true
            })),
            OutputToken::Annotated(annotated! {
                require_hanja: true,
                require_hangul: true,
                first_in_context: true,
                ..annotation("漢字", "한자")
            }),
            OutputToken::Close,
            OutputToken::Text("\n".into()),
            OutputToken::Annotated(annotated! {
                require_hanja: false,
                require_hangul: false,
                first_in_context: false,
                ..annotation("漢字", "한자")
            }),
        ]
    );
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
            OutputToken::Annotated(annotated! {
                require_hanja: true,
                ..annotation("漢字", "한자")
            }),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(annotated! {
                require_hangul: true,
                ..annotation("天地", "천지")
            }),
        ]
    );
}

#[test]
fn user_directives_apply_closure_predicates() {
    let mut directives = UserDirectives::new();
    directives.add_predicate(
        |annotation| annotation.reading == "한자",
        DirectiveAction::RequireHanja,
    );
    let tokens = vec![
        OutputToken::<PlainScopeData>::Annotated(annotation("漢字", "한자")),
        OutputToken::Text(" ".into()),
        OutputToken::Annotated(annotation("天地", "천지")),
    ];

    let directed = apply_user_directives(tokens, &directives);

    assert_eq!(
        directed,
        vec![
            OutputToken::Annotated(annotated! {
                require_hanja: true,
                ..annotation("漢字", "한자")
            }),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(annotation("天地", "천지")),
        ]
    );
}

#[test]
fn user_directives_skip_annotations_without_rendering_them() {
    let mut directives = UserDirectives::new();
    directives.skip_annotation("漢字");
    let tokens = vec![OutputToken::<PlainScopeData>::Annotated(annotation(
        "漢字", "한자",
    ))];

    let directed = apply_user_directives(tokens, &directives);

    assert_eq!(
        directed,
        vec![OutputToken::Annotated(annotated! {
            skip_annotation: true,
            ..annotation("漢字", "한자")
        })]
    );
}

#[test]
fn skip_annotation_collapses_to_primary_plain_text_for_each_render_mode() {
    let token = || {
        vec![OutputToken::<PlainScopeData>::Annotated(annotated! {
            require_hanja: true,
            require_hangul: true,
            homophone: true,
            skip_annotation: true,
            ..annotation("漢字", "한자")
        })]
    };

    assert_eq!(
        render_tokens(token(), RenderMode::HangulOnly),
        vec![RenderedToken::Text("한자".into())]
    );
    assert_eq!(
        render_tokens(token(), RenderMode::HangulHanjaParens),
        vec![RenderedToken::Text("한자".into())]
    );
    assert_eq!(
        render_tokens(token(), RenderMode::HanjaHangulParens),
        vec![RenderedToken::Text("漢字".into())]
    );
    assert_eq!(
        render_tokens(token(), RenderMode::Original),
        vec![RenderedToken::Text("漢字".into())]
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
            vec![OutputToken::Annotated(annotated! {
                hanja: key,
                dictionary_hanja: Some(format!("{prefix}{middle}{suffix}")),
                reading: reading,
                homophone: false,
                require_hanja: false,
                require_hangul: false,
                first_in_context: true,
                skip_annotation: false,
                from_dictionary: true,
                from_source_gloss: false,
            })],
        );
    }

    #[test]
    fn known_fallback_hanja_are_removed_from_hangul_only_output(input in "[未知來日良質力量安全語錄理論法律一列羅序規律自前韻分旋千九百八十六年第共和國拾萬圓參佰仟學問山川龍馬㒚]{1,12}") {
        let output = convert_plain_text(&input, &MapDictionary::new(), RenderMode::HangulOnly);

        prop_assert!(!output.chars().any(gukhanmun_core::is_hanja));
    }

    #[test]
    fn chain_dictionary_max_word_chars_matches_component_policy(
        values in prop::collection::vec(prop::option::of(0usize..20), 0..8),
    ) {
        let dictionaries = values
            .iter()
            .copied()
            .map(MaxOnlyDictionary::new)
            .collect::<Vec<_>>();
        let chain = ChainDictionary::from_iter(dictionaries);
        let expected = if values.iter().all(Option::is_some) {
            values.into_iter().max().flatten()
        } else {
            None
        };

        prop_assert_eq!(chain.max_word_chars(), expected);
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
            vec![OutputToken::<PlainScopeData>::Annotated(annotated! {
                hanja: hanja,
                reading: reading,
                homophone: homophone,
                require_hanja: require_hanja,
                require_hangul: require_hangul,
                first_in_context: true,
                skip_annotation: false,
                from_dictionary: true,
                from_source_gloss: false,
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
                    suffix_reading: None,
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
                    suffix_reading: None,
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

#[derive(Clone, Debug)]
struct MaxOnlyDictionary {
    max_word_chars: Option<usize>,
}

impl MaxOnlyDictionary {
    fn new(max_word_chars: Option<usize>) -> Self {
        Self { max_word_chars }
    }
}

impl HanjaDictionary for MaxOnlyDictionary {
    fn matches_at<'a>(
        &'a self,
        _s: &'a str,
    ) -> Box<dyn Iterator<Item = gukhanmun_core::Match> + 'a> {
        Box::new(std::iter::empty())
    }

    fn max_word_chars(&self) -> Option<usize> {
        self.max_word_chars
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MarkupTestScopeData {
    allows_inline_markup: bool,
    preserve: bool,
}

impl MarkupTestScopeData {
    fn inline() -> Self {
        Self {
            allows_inline_markup: true,
            preserve: false,
        }
    }

    fn no_inline() -> Self {
        Self {
            allows_inline_markup: false,
            preserve: false,
        }
    }
}

impl ScopeData for MarkupTestScopeData {
    fn is_preserve(&self) -> bool {
        self.preserve
    }

    fn allows_inline_markup(&self) -> bool {
        self.allows_inline_markup
    }
}

fn ruby_annotation() -> Annotation {
    annotation("漢字", "한자")
}

#[test]
fn render_options_default_uses_hangul_only_and_parens_gloss() {
    let options = RenderOptions::default();

    assert_eq!(options.mode, RenderMode::HangulOnly);
    assert_eq!(options.original_gloss, OriginalGloss::Parens);
}

#[test]
fn render_options_from_render_mode_preserves_mode_with_parens_gloss() {
    let options: RenderOptions = RenderMode::HangulHanjaParens.into();

    assert_eq!(options.mode, RenderMode::HangulHanjaParens);
    assert_eq!(options.original_gloss, OriginalGloss::Parens);
}

#[test]
fn plain_scope_data_disallows_inline_markup() {
    let scope = PlainScopeData;

    assert!(!scope.allows_inline_markup());
}

#[test]
fn ruby_on_hangul_emits_inline_markup_in_allowing_scope() {
    let tokens = vec![
        OutputToken::Open(Scope::new(MarkupTestScopeData::inline())),
        OutputToken::Annotated(ruby_annotation()),
        OutputToken::Close,
    ];

    let rendered = render_tokens(tokens, RenderMode::Ruby(RubyBase::OnHangul));

    assert_eq!(
        rendered,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Ruby {
                base: "한자".into(),
                rt: "漢字".into()
            },
            RenderedToken::Close,
        ]
    );
}

#[test]
fn ruby_on_hanja_emits_inline_markup_in_allowing_scope() {
    let tokens = vec![
        OutputToken::Open(Scope::new(MarkupTestScopeData::inline())),
        OutputToken::Annotated(ruby_annotation()),
        OutputToken::Close,
    ];

    let rendered = render_tokens(tokens, RenderMode::Ruby(RubyBase::OnHanja));

    assert_eq!(
        rendered,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Ruby {
                base: "漢字".into(),
                rt: "한자".into()
            },
            RenderedToken::Close,
        ]
    );
}

#[test]
fn ruby_falls_back_to_parens_when_inline_markup_disallowed() {
    let tokens = vec![
        OutputToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
        OutputToken::Annotated(ruby_annotation()),
        OutputToken::Close,
    ];

    let rendered_on_hangul = render_tokens(tokens.clone(), RenderMode::Ruby(RubyBase::OnHangul));
    assert_eq!(
        rendered_on_hangul,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
            RenderedToken::Text("한자(漢字)".into()),
            RenderedToken::Close,
        ]
    );

    let rendered_on_hanja = render_tokens(tokens, RenderMode::Ruby(RubyBase::OnHanja));
    assert_eq!(
        rendered_on_hanja,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
            RenderedToken::Text("漢字(한자)".into()),
            RenderedToken::Close,
        ]
    );
}

#[test]
fn ruby_at_top_level_without_open_scope_emits_ruby_for_markup_capable_adapters() {
    // No scope means the renderer cannot prove anything about the active
    // adapter, so it defaults to "inline markup allowed".  Plain text is
    // covered by the explicit PlainScopeData test below; HTML and Markdown
    // adapters that emit text outside any scope (e.g. bare fragment input)
    // therefore receive a structured Ruby token rather than a parens
    // fallback, and the adapter's writer decides how to serialize it.
    let tokens: Vec<OutputToken<MarkupTestScopeData>> =
        vec![OutputToken::Annotated(ruby_annotation())];

    let rendered = render_tokens(tokens, RenderMode::Ruby(RubyBase::OnHangul));

    assert_eq!(
        rendered,
        vec![RenderedToken::Ruby {
            base: "한자".into(),
            rt: "漢字".into(),
        }]
    );
}

#[test]
fn ruby_falls_back_when_any_ancestor_disallows_inline_markup() {
    // An inner scope that allows inline markup must not re-enable ruby output
    // once an outer scope has disallowed it. This protects against adapters
    // like Markdown where an emphasis container inside an HTML text-only
    // element would otherwise look like it permits markup at the current
    // cursor.
    let tokens = vec![
        OutputToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
        OutputToken::Open(Scope::new(MarkupTestScopeData::inline())),
        OutputToken::Annotated(ruby_annotation()),
        OutputToken::Close,
        OutputToken::Close,
    ];

    let rendered = render_tokens(tokens, RenderMode::Ruby(RubyBase::OnHangul));

    assert_eq!(
        rendered,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Text("한자(漢字)".into()),
            RenderedToken::Close,
            RenderedToken::Close,
        ]
    );
}

#[test]
fn ruby_recovers_after_a_disallowing_scope_closes() {
    // Once the disallowing ancestor closes, subsequent annotations in
    // allow-inline-markup scopes recover and emit a real ruby token.
    let tokens = vec![
        OutputToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
        OutputToken::Open(Scope::new(MarkupTestScopeData::inline())),
        OutputToken::Annotated(ruby_annotation()),
        OutputToken::Close,
        OutputToken::Close,
        OutputToken::Open(Scope::new(MarkupTestScopeData::inline())),
        OutputToken::Annotated(ruby_annotation()),
        OutputToken::Close,
    ];

    let rendered = render_tokens(tokens, RenderMode::Ruby(RubyBase::OnHangul));

    assert_eq!(
        rendered,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Text("한자(漢字)".into()),
            RenderedToken::Close,
            RenderedToken::Close,
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Ruby {
                base: "한자".into(),
                rt: "漢字".into(),
            },
            RenderedToken::Close,
        ]
    );
}

#[test]
fn renderer_pops_scope_on_close_so_outer_scope_governs_next_annotation() {
    let tokens = vec![
        OutputToken::Open(Scope::new(MarkupTestScopeData::inline())),
        OutputToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
        OutputToken::Annotated(ruby_annotation()),
        OutputToken::Close,
        OutputToken::Annotated(ruby_annotation()),
        OutputToken::Close,
    ];

    let rendered = render_tokens(tokens, RenderMode::Ruby(RubyBase::OnHangul));

    assert_eq!(
        rendered,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
            RenderedToken::Text("한자(漢字)".into()),
            RenderedToken::Close,
            RenderedToken::Ruby {
                base: "한자".into(),
                rt: "漢字".into()
            },
            RenderedToken::Close,
        ]
    );
}

#[test]
fn extra_close_tokens_do_not_panic() {
    let tokens: Vec<OutputToken<MarkupTestScopeData>> = vec![
        OutputToken::Close,
        OutputToken::Open(Scope::new(MarkupTestScopeData::inline())),
        OutputToken::Annotated(ruby_annotation()),
        OutputToken::Close,
        OutputToken::Close,
    ];

    let rendered = render_tokens(tokens, RenderMode::Ruby(RubyBase::OnHangul));

    assert_eq!(
        rendered,
        vec![
            RenderedToken::Close,
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Ruby {
                base: "한자".into(),
                rt: "漢字".into()
            },
            RenderedToken::Close,
            RenderedToken::Close,
        ]
    );
}

#[test]
fn ruby_uses_parens_in_plain_text_pipeline() {
    let output = convert_plain_text(
        "天地玄黃과 漢字",
        &sample_dictionary(),
        RenderMode::Ruby(RubyBase::OnHangul),
    );

    assert_eq!(output, "천지(天地)현황(玄黃)과 한자(漢字)");
}

#[test]
fn ruby_skip_annotation_collapses_to_primary_form() {
    let annotation = annotated! {
        skip_annotation: true,
        ..ruby_annotation()
    };
    let tokens = vec![
        OutputToken::Open(Scope::new(MarkupTestScopeData::inline())),
        OutputToken::Annotated(annotation),
        OutputToken::Close,
    ];

    let rendered_on_hangul = render_tokens(tokens.clone(), RenderMode::Ruby(RubyBase::OnHangul));
    assert_eq!(
        rendered_on_hangul,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Text("한자".into()),
            RenderedToken::Close,
        ]
    );

    let rendered_on_hanja = render_tokens(tokens, RenderMode::Ruby(RubyBase::OnHanja));
    assert_eq!(
        rendered_on_hanja,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Text("漢字".into()),
            RenderedToken::Close,
        ]
    );
}

#[test]
fn original_with_ruby_gloss_emits_inline_markup_for_required_hangul() {
    let tokens = vec![
        OutputToken::Open(Scope::new(MarkupTestScopeData::inline())),
        OutputToken::Annotated(annotated! {
            require_hangul: true,
            ..ruby_annotation()
        }),
        OutputToken::Annotated(ruby_annotation()),
        OutputToken::Close,
    ];

    let options = RenderOptions {
        mode: RenderMode::Original,
        original_gloss: OriginalGloss::Ruby,
        ..RenderOptions::default()
    };
    let rendered = render_tokens(tokens, options);

    assert_eq!(
        rendered,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Ruby {
                base: "漢字".into(),
                rt: "한자".into()
            },
            RenderedToken::Text("漢字".into()),
            RenderedToken::Close,
        ]
    );
}

#[test]
fn original_with_ruby_gloss_falls_back_to_parens_in_disallowing_scope() {
    let tokens = vec![
        OutputToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
        OutputToken::Annotated(annotated! {
            require_hangul: true,
            ..ruby_annotation()
        }),
        OutputToken::Close,
    ];

    let options = RenderOptions {
        mode: RenderMode::Original,
        original_gloss: OriginalGloss::Ruby,
        ..RenderOptions::default()
    };
    let rendered = render_tokens(tokens, options);

    assert_eq!(
        rendered,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::no_inline())),
            RenderedToken::Text("漢字(한자)".into()),
            RenderedToken::Close,
        ]
    );
}

#[test]
fn original_with_parens_gloss_keeps_existing_behavior() {
    let tokens = vec![
        OutputToken::Open(Scope::new(MarkupTestScopeData::inline())),
        OutputToken::Annotated(annotated! {
            require_hangul: true,
            ..ruby_annotation()
        }),
        OutputToken::Close,
    ];

    let options = RenderOptions {
        mode: RenderMode::Original,
        original_gloss: OriginalGloss::Parens,
        ..RenderOptions::default()
    };
    let rendered = render_tokens(tokens, options);

    assert_eq!(
        rendered,
        vec![
            RenderedToken::Open(Scope::new(MarkupTestScopeData::inline())),
            RenderedToken::Text("漢字(한자)".into()),
            RenderedToken::Close,
        ]
    );
}
