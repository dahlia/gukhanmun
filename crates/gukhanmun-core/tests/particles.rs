// Gukhanmun: Particle selection and streaming regression tests.
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

use gukhanmun_core::*;

fn candidate(
    reading: &str,
    suffix: Option<&str>,
    preference: u32,
    marked: bool,
) -> ReadingCandidate {
    ReadingCandidate {
        reading: reading.into(),
        suffix_reading: suffix.map(Into::into),
        preference,
        mark: MatchMark {
            require_hanja: marked,
            require_hangul: false,
        },
    }
}
fn dictionary(default: &str, alternate: &str) -> MapDictionary {
    let mut dict = MapDictionary::new();
    dict.insert_candidates(
        "東京",
        ReadingCandidates::new(vec![
            candidate(default, None, 10, false),
            candidate(alternate, None, 0, false),
        ])
        .unwrap(),
    );
    dict
}
fn readings(input: &str, dict: &dyn HanjaDictionary) -> Vec<String> {
    process_tokens(read_plain_text(input), dict)
        .into_iter()
        .filter_map(|token| match token {
            OutputToken::Annotated(a) => Some(a.reading),
            _ => None,
        })
        .collect()
}
#[test]
fn finite_grammar_and_rieul_exception() {
    for (suffix, consonant) in [
        ("을", true),
        ("를", false),
        ("은", true),
        ("는", false),
        ("이", true),
        ("가", false),
        ("과", true),
        ("와", false),
        ("으로", true),
        ("로", false),
        ("으로는", true),
        ("으로도", true),
        ("으로만", true),
        ("로는", false),
        ("로도", false),
        ("로만", false),
        ("과는", true),
        ("과도", true),
        ("과만", true),
        ("와는", false),
        ("와도", false),
        ("와만", false),
    ] {
        for (default, alternate) in [("도쿄", "동경"), ("동경", "도쿄")] {
            let dict = dictionary(default, alternate);
            let expected = if consonant { "동경" } else { "도쿄" };
            for boundary in [
                "", " ", "\u{a0}", "\u{3000}", ".", ",", "!", "?", ":", ";", ")", "]", "}", "。",
                "、", "！", "？", "，", "：", "；", "）", "】", "」", "』",
            ] {
                assert_eq!(
                    readings(&format!("東京{suffix}{boundary}"), &dict),
                    [expected],
                    "{suffix}{boundary}"
                );
            }
        }
    }
    let dict = dictionary("길", "동경");
    assert_eq!(readings("東京로", &dict), ["길"]);
    assert_eq!(readings("東京으로", &dict), ["동경"]);
    assert_eq!(readings("東京를", &dict), ["길"]); // Neither survives.
}
#[test]
fn uncertain_context_and_custom_endings_remain_eligible() {
    let dict = dictionary("도쿄", "동경");
    for suffix in [
        "",
        "이다",
        "이며",
        "이라고",
        "이란",
        "이름",
        "가족",
        "과학",
        "로봇",
        "을만",
        "으로는가",
        "을a",
        "을1",
        "을甲",
        "을ᄀ",
        "을\u{301}",
        "을\u{200b}",
        "을\u{200d}",
        "을\u{fe0f}",
        "을🦀",
        "을\"",
    ] {
        assert_eq!(
            readings(&format!("東京{suffix}"), &dict)[0],
            "도쿄",
            "{suffix}"
        );
    }
    for unknown in ["x", "가", "각"] {
        assert_eq!(readings("東京을", &dictionary(unknown, "동경")), [unknown]);
    }
    let mut dict = MapDictionary::new();
    dict.insert_candidates(
        "東京",
        ReadingCandidates::new(vec![
            candidate("도쿄", Some("경"), 10, true),
            candidate("동경", None, 0, false),
        ])
        .unwrap(),
    );
    let tokens = process_tokens(read_plain_text("東京을"), &dict);
    assert!(
        tokens.iter().any(
            |t| matches!(t, OutputToken::Annotated(a) if a.reading == "도쿄" && a.require_hanja)
        )
    );
    assert_eq!(readings("東京을", &dictionary("경", "정")), ["경"]); // Multiple survivors.
}

fn assert_streaming(input: &str, dict: &dyn HanjaDictionary) {
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        let options = EngineOptions {
            segmentation: strategy,
            ..EngineOptions::default()
        };
        let expected = process_tokens_with_options(read_plain_text(input), dict, options);
        let annotation = |tokens: &[OutputToken<PlainScopeData>]| {
            tokens
                .iter()
                .filter_map(|t| match t {
                    OutputToken::Annotated(a) => Some(a.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let chunks = input
            .char_indices()
            .map(|(i, _)| vec![&input[..i], &input[i..]])
            .chain([
                vec![input],
                input
                    .char_indices()
                    .map(|(i, c)| &input[i..i + c.len_utf8()])
                    .collect(),
            ]);
        for chunks in chunks {
            let mut engine = Engine::with_options(dict, options);
            let mut actual = Vec::new();
            for chunk in chunks {
                actual.extend(engine.push_token(InputToken::Text(chunk.into())));
            }
            actual.extend(engine.finish());
            assert_eq!(
                annotation(&actual),
                annotation(&expected),
                "{input} {strategy:?}"
            );
            for mode in [
                RenderMode::HangulOnly,
                RenderMode::HangulHanjaParens,
                RenderMode::HanjaHangulParens,
            ] {
                assert_eq!(
                    write_plain_text(render_tokens(actual.clone(), mode)),
                    write_plain_text(render_tokens(expected.clone(), mode)),
                    "{input} {strategy:?}"
                );
            }
        }
    }
}
#[test]
fn streaming_resolves_particles_without_splitting_fallback_annotations() {
    for reverse in [false, true] {
        let mut dict = MapDictionary::new();
        dict.insert_candidates(
            "丁",
            ReadingCandidates::new(if reverse {
                vec![
                    candidate("정", None, 10, false),
                    candidate("가", None, 0, true),
                ]
            } else {
                vec![
                    candidate("가", None, 10, true),
                    candidate("정", None, 0, false),
                ]
            })
            .unwrap(),
        );
        for input in [
            "甲乙丁으로는 간다",
            "甲乙丁으로는",
            "甲乙丁으로는가아아아아아아아아아아",
            "甲乙丁으로는 아아아아아아아아아아",
            "甲乙丁로는 간다",
        ] {
            assert_streaming(input, &dict);
        }
    }
    let dict = dictionary("도쿄", "동경");
    for input in [
        "東京을",
        "東京으로는 간다",
        "東京이다아아아아아아아아아아",
        "東京을 아아아아아아아아아아",
        "東京를 東京을",
    ] {
        assert_streaming(input, &dict);
    }
}
#[derive(Clone, Debug)]
struct Boundary {
    block: bool,
    preserve: bool,
}
impl ScopeData for Boundary {
    fn is_block_boundary(&self) -> bool {
        self.block
    }
    fn is_preserve(&self) -> bool {
        self.preserve
    }
}
#[test]
fn flush_finish_and_scope_finality() {
    let dict = dictionary("도쿄", "동경");
    let mut engine = Engine::<PlainScopeData, _>::new(&dict);
    let mut tokens = engine.push_token(InputToken::Text("東京을".into()));
    tokens.extend(engine.flush());
    assert_eq!(
        write_plain_text(render_tokens(tokens, RenderMode::HangulOnly)),
        "도쿄을"
    );
    for block in [false, true] {
        for open in [false, true] {
            let scope = Scope::new(Boundary {
                block,
                preserve: false,
            });
            let mut input = Vec::new();
            if !open {
                input.push(InputToken::Open(scope.clone()));
            }
            input.push(InputToken::Text("東京을".into()));
            input.push(if open {
                InputToken::Open(scope)
            } else {
                InputToken::Close
            });
            if !block && !open {
                input.push(InputToken::Text("다".into()));
            }
            let tokens = process_tokens(input, &dict);
            let a = tokens
                .iter()
                .find_map(|t| match t {
                    OutputToken::Annotated(a) => Some(a),
                    _ => None,
                })
                .unwrap();
            assert_eq!(a.reading, if block { "동경" } else { "도쿄" });
        }
    }
    let tokens = process_tokens(
        [
            InputToken::<PlainScopeData>::Text("東京이".into()),
            InputToken::Close,
            InputToken::Text("다".into()),
        ],
        &dict,
    );
    assert_eq!(
        write_plain_text(render_tokens(tokens, RenderMode::HangulOnly)),
        "도쿄이다"
    );
    let tokens = process_tokens(
        [
            InputToken::<PlainScopeData>::Text("東京을".into()),
            InputToken::Verbatim("!".into()),
        ],
        &dict,
    );
    assert_eq!(
        write_plain_text(render_tokens(tokens, RenderMode::HangulOnly)),
        "도쿄을!"
    );
}
#[test]
fn bounded_stream_emits_early_and_keeps_four_extra_scalars() {
    let dict = dictionary("도쿄", "동경");
    for input in [
        "東京을아아아아아아아아아아아아아아아",
        "아아아아아아아아아아아아아아아아아아",
    ] {
        let mut engine = Engine::<PlainScopeData, _>::new(&dict);
        assert!(!engine.push_token(InputToken::Text(input.into())).is_empty());
        assert!(engine.buffered_chars() <= dict.max_word_chars().unwrap() + 4);
    }
}
#[test]
fn longest_mixed_keys_variants_and_whole_set_overrides() {
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        let options = EngineOptions {
            segmentation: strategy,
            ..EngineOptions::default()
        };
        let mut dict = dictionary("도쿄", "동경");
        dict.insert_candidates(
            "東京을",
            ReadingCandidates::new(vec![
                candidate("가", None, 10, false),
                candidate("경", None, 0, false),
            ])
            .unwrap(),
        );
        dict.insert_candidates(
            "乁",
            ReadingCandidates::new(vec![
                candidate("가", None, 10, false),
                candidate("경", None, 0, false),
            ])
            .unwrap(),
        );
        dict.insert_candidates(
            "藝術",
            ReadingCandidates::new(vec![
                candidate("가", None, 10, false),
                candidate("경", None, 0, false),
            ])
            .unwrap(),
        );
        for (input, expected) in [
            ("東京을", "가"),
            ("東京을을", "경"),
            ("乁을", "경"),
            ("芸術을", "경"),
        ] {
            let output = process_tokens_with_options(read_plain_text(input), &dict, options);
            assert!(
                output
                    .iter()
                    .any(|t| matches!(t,OutputToken::Annotated(a) if a.reading==expected)),
                "{input} {strategy:?}"
            );
            assert_streaming(input, &dict);
        }
        let mut user = MapDictionary::new();
        user.insert("東京", "도쿄");
        let chain: ChainDictionary<_> = [user, dictionary("도쿄", "동경")].into_iter().collect();
        assert_eq!(readings("東京을", &chain), ["도쿄"]);
        let mut user = MapDictionary::new();
        user.insert_candidates(
            "東京",
            ReadingCandidates::new(vec![
                candidate("가", None, 10, false),
                candidate("나", None, 0, false),
            ])
            .unwrap(),
        );
        let chain: ChainDictionary<_> = [user, dictionary("도쿄", "동경")].into_iter().collect();
        assert_eq!(readings("東京을", &chain), ["가"]);
    }
}
#[test]
fn homophones_query_emitted_readings_and_ignore_unused_alternatives() {
    let mut dict = dictionary("도쿄", "동경");
    dict.insert("銅鏡", "동경");
    let marked = mark_homophones_with_detection(
        process_tokens(read_plain_text("東京을"), &dict),
        &dict,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );
    assert!(
        marked
            .iter()
            .any(|t| matches!(t,OutputToken::Annotated(a) if a.reading=="동경" && a.homophone))
    );
    let marked = mark_homophones_with_detection(
        process_tokens(read_plain_text("東京를 銅鏡"), &dict),
        &dict,
        ContextWindow::PerDocument,
        HomophoneDetection::ContextLocal,
    );
    assert!(
        !marked
            .iter()
            .any(|t| matches!(t,OutputToken::Annotated(a) if a.homophone))
    );
    let marked = mark_homophones_with_detection(
        process_tokens(read_plain_text("東京을 銅鏡"), &dict),
        &dict,
        ContextWindow::PerDocument,
        HomophoneDetection::ContextLocal,
    );
    assert!(
        marked
            .iter()
            .filter(|t| matches!(t,OutputToken::Annotated(a) if a.homophone))
            .count()
            == 2
    );
}

#[test]
fn deferred_root_close_preserves_order_and_resolves_on_reuse() {
    let dict = dictionary("도쿄", "동경");
    assert_eq!(
        convert_plain_text("東京을", &dict, RenderMode::HangulOnly),
        "동경을"
    );
    let input = read_plain_text("東京을");
    let expected = process_tokens(input.clone(), &dict);
    assert!(matches!(expected.first(), Some(OutputToken::Open(_))));
    assert!(matches!(expected.last(), Some(OutputToken::Close)));
    let mut engine = Engine::new(&dict);
    let mut actual = Vec::new();
    for token in input {
        actual.extend(engine.push_token(token));
    }
    assert!(!actual.iter().any(|t| matches!(t, OutputToken::Close)));
    actual.extend(engine.finish());
    assert_eq!(actual, expected);
    for next in [
        InputToken::Text("다".into()),
        InputToken::Verbatim("!".into()),
        InputToken::Close,
        InputToken::Open(Scope::new(PlainScopeData)),
    ] {
        let mut engine = Engine::new(&dict);
        let mut output = Vec::new();
        for token in read_plain_text("東京이") {
            output.extend(engine.push_token(token));
        }
        output.extend(engine.push_token(next));
        output.extend(engine.finish());
        assert!(matches!(output.first(), Some(OutputToken::Open(_))));
        assert!(
            output
                .iter()
                .any(|t| matches!(t,OutputToken::Annotated(a) if a.reading=="도쿄"))
        );
        assert!(matches!(output.get(3), Some(OutputToken::Close)));
    }
    let mut engine = Engine::new(&dict);
    let mut output = Vec::new();
    for token in read_plain_text("東京을") {
        output.extend(engine.push_token(token));
    }
    output.extend(engine.flush());
    output.extend(engine.push_token(InputToken::Text(" 東京을".into())));
    output.extend(engine.finish());
    assert_eq!(
        write_plain_text(render_tokens(output, RenderMode::HangulOnly)),
        "도쿄을 동경을"
    );
}

struct UnknownBound(MapDictionary);
impl HanjaDictionary for UnknownBound {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        self.0.matches_at(s)
    }
    fn candidate_matches_at<'a>(
        &'a self,
        s: &'a str,
    ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
        self.0.candidate_matches_at(s)
    }
}
#[test]
fn unknown_dictionary_bound_keeps_real_context() {
    let dict = UnknownBound(dictionary("도쿄", "동경"));
    assert_eq!(dict.max_word_chars(), None);
    for input in [
        "東京을",
        "東京으로는 간다",
        "東京이다",
        "東京을아아아아아아아아아",
    ] {
        assert_streaming(input, &dict);
    }
}

#[test]
fn particle_evidence_never_resolves_ambiguous_variant_identity() {
    let mut dict = MapDictionary::new();
    dict.insert_candidates(
        "藝術",
        ReadingCandidates::new(vec![
            candidate("가", None, 10, false),
            candidate("경", None, 0, false),
        ])
        .unwrap(),
    );
    dict.insert_candidates(
        "藝术",
        ReadingCandidates::new(vec![
            candidate("나", None, 10, false),
            candidate("경", None, 0, false),
        ])
        .unwrap(),
    );
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        let tokens = process_tokens_with_options(
            read_plain_text("芸術을"),
            &dict,
            EngineOptions {
                segmentation: strategy,
                ..EngineOptions::default()
            },
        );
        assert!(
            !tokens
                .iter()
                .any(|t| matches!(t, OutputToken::Annotated(a) if a.from_dictionary))
        );
    }
}
