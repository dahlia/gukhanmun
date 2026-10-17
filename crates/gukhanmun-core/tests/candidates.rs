// Gukhanmun: Candidate dictionary and compatibility regression tests.
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
    hanja: bool,
) -> ReadingCandidate {
    ReadingCandidate {
        reading: reading.into(),
        suffix_reading: suffix.map(Into::into),
        preference,
        mark: MatchMark {
            require_hanja: hanja,
            require_hangul: false,
        },
    }
}
fn tokyo() -> ReadingCandidates {
    ReadingCandidates::new(vec![
        candidate("동경", None, 10, false),
        candidate("도쿄", None, 20, true),
    ])
    .unwrap()
}
#[test]
fn sets_merge_only_identical_positional_readings_and_normalize_preferences() {
    let mut a = candidate("연대", Some("년대"), 10, true);
    a.mark.require_hangul = true;
    let inputs = vec![
        a,
        candidate("연대", Some("년대"), 15, false),
        candidate("연대", None, 20, false),
    ];
    let set = ReadingCandidates::new(inputs.clone()).unwrap();
    let reversed = ReadingCandidates::new(inputs.into_iter().rev().collect()).unwrap();
    assert_eq!(set, reversed);
    assert_eq!(set.as_slice().len(), 2);
    assert_eq!(select_default_reading(&set).preference, 5);
    assert!(!select_default_reading(&set).mark.require_hanja);
    assert!(set.as_slice()[1].mark.require_hanja && set.as_slice()[1].mark.require_hangul);
    assert_eq!(
        ReadingCandidates::decode_payload(&set.encode_payload().unwrap()).unwrap(),
        set
    );
    assert!(ReadingCandidates::new(vec![]).is_err());
    assert!(ReadingCandidates::new(vec![candidate("", None, 0, false)]).is_err());
    assert!(ReadingCandidates::new(vec![candidate("연대", Some(""), 0, false)]).is_err());
}
#[test]
fn codec_rejects_truncation_trailing_bytes_unknown_marks_and_noncanonical_order() {
    let bytes = tokyo().encode_payload().unwrap();
    for len in 0..bytes.len() {
        assert!(ReadingCandidates::decode_payload(&bytes[..len]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(ReadingCandidates::decode_payload(&trailing).is_err());
    let mut mark = bytes.clone();
    mark[6] = 4;
    assert!(ReadingCandidates::decode_payload(&mark).is_err());
    let mut order = bytes.clone();
    order[2..6].copy_from_slice(&0u32.to_le_bytes());
    let offset = 2 + 9 + "도쿄".len();
    order[offset..offset + 4].copy_from_slice(&1u32.to_le_bytes());
    assert!(ReadingCandidates::decode_payload(&order).is_err());
    let mut not_normalized = bytes;
    not_normalized[2..6].copy_from_slice(&11u32.to_le_bytes());
    // The second preference begins after the first candidate's nine-byte header and UTF-8 reading.
    let second = 2 + 9 + "도쿄".len();
    not_normalized[second..second + 4].copy_from_slice(&1u32.to_le_bytes());
    assert!(ReadingCandidates::decode_payload(&not_normalized).is_err());
    assert!(ReadingCandidates::decode_payload(&[0, 0]).is_err());
}
#[test]
fn precedence_forwarding_variants_and_selected_marks_agree() {
    let mut bundled = MapDictionary::new();
    bundled.insert_candidates("東京", tokyo());
    let boxed: Box<dyn HanjaDictionary> = Box::new(bundled);
    let reference = &boxed;
    assert_eq!(
        HanjaDictionary::candidate_matches_at(&reference, "東京")
            .next()
            .unwrap()
            .candidates,
        tokyo()
    );
    assert_eq!(
        boxed
            .candidate_entries()
            .unwrap()
            .next()
            .unwrap()
            .candidates,
        tokyo()
    );
    assert_eq!(
        convert_plain_text("東京", &boxed, RenderMode::HangulOnly),
        "도쿄(東京)"
    );
    let mut user = MapDictionary::new();
    user.insert("東京", "동경");
    let chain: ChainDictionary<Box<dyn HanjaDictionary>> =
        [Box::new(user) as Box<dyn HanjaDictionary>, boxed]
            .into_iter()
            .collect();
    assert_eq!(
        chain
            .candidate_matches_at("東京")
            .next()
            .unwrap()
            .candidates
            .as_slice()
            .len(),
        1
    );
    assert_eq!(
        chain
            .candidate_entries()
            .unwrap()
            .next()
            .unwrap()
            .candidates
            .as_slice()
            .len(),
        1
    );
    assert_eq!(
        convert_plain_text("東京", &chain, RenderMode::HangulOnly),
        "동경"
    );
    let mut variants = MapDictionary::new();
    variants.insert_candidates("藝術", tokyo());
    let output = process_tokens(read_plain_text("芸術"), &variants);
    let annotation = output
        .iter()
        .find_map(|t| {
            if let OutputToken::Annotated(a) = t {
                Some(a)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(annotation.reading, "도쿄");
    assert_eq!(annotation.dictionary_hanja.as_deref(), Some("藝術"));
}
#[test]
fn homophones_use_only_default_readings_and_include_their_suffixes() {
    let mut dict = MapDictionary::new();
    dict.insert_candidates("東京", tokyo());
    dict.insert("銅鏡", "동경");
    assert!(!dict.has_homophone("銅鏡", "동경"));
    dict.insert_with_suffix("年代", "연대", "년대");
    dict.insert("年臺", "년대");
    assert!(dict.has_homophone("年臺", "년대"));
    let tokens = process_tokens(read_plain_text("1990年代"), &dict);
    let marked = mark_homophones_with_detection(
        tokens,
        &dict,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );
    assert!(
        marked
            .iter()
            .any(|t| matches!(t,OutputToken::Annotated(a) if a.reading=="년대" && a.homophone))
    );
}
#[test]
fn candidate_streaming_matches_one_shot_at_every_character_split() {
    let mut dict = MapDictionary::new();
    dict.insert_candidates("東京", tokyo());
    dict.insert_candidates(
        "率",
        ReadingCandidates::new(vec![
            candidate("률", None, 20, false),
            candidate("율", None, 10, true),
        ])
        .unwrap(),
    );
    dict.insert_with_suffix("年代", "연대", "년대");
    for strategy in [SegmentationStrategy::Lattice, SegmentationStrategy::Eager] {
        let options = EngineOptions {
            segmentation: strategy,
            ..EngineOptions::default()
        };
        for input in ["東京을 東京를", "1990年代 東京", "비率 年率"] {
            let expected = process_tokens_with_options(
                [InputToken::<PlainScopeData>::Text(input.into())],
                &dict,
                options,
            );
            for split in input.char_indices().map(|(i, _)| i).chain([input.len()]) {
                let mut engine = Engine::<PlainScopeData, _>::with_options(&dict, options);
                let mut actual = engine.push_token(InputToken::Text(input[..split].into()));
                actual.extend(engine.push_token(InputToken::Text(input[split..].into())));
                actual.extend(engine.finish());
                let annotations = |tokens: &[OutputToken<PlainScopeData>]| {
                    tokens
                        .iter()
                        .filter_map(|t| {
                            if let OutputToken::Annotated(a) = t {
                                Some(a.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    annotations(&actual),
                    annotations(&expected),
                    "{input} split {split} {strategy:?}"
                );
                assert_eq!(
                    write_plain_text(render_tokens(actual, RenderMode::HangulHanjaParens)),
                    write_plain_text(render_tokens(
                        expected.clone(),
                        RenderMode::HangulHanjaParens
                    ))
                );
            }
        }
    }
    assert_eq!(
        convert_plain_text("비率", &dict, RenderMode::HangulHanjaParens),
        "비율(率)"
    );
}
struct Legacy;
impl HanjaDictionary for Legacy {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        Box::new(
            s.starts_with("漢字")
                .then(|| Match {
                    byte_len: "漢字".len(),
                    reading: "".into(),
                    suffix_reading: Some("".into()),
                    mark: MatchMark {
                        require_hanja: true,
                        require_hangul: false,
                    },
                })
                .into_iter(),
        )
    }
    fn matches_at_spellings(&self, spellings: &[&str]) -> Vec<(usize, Match)> {
        spellings
            .iter()
            .enumerate()
            .flat_map(|(i, s)| self.matches_at(s).map(move |m| (i, m)))
            .collect()
    }
}
#[test]
fn legacy_empty_values_are_lifted_losslessly_through_map_custom_box_and_chain() {
    let mut map = MapDictionary::new();
    map.insert_with_suffix("漢字", "한자", "");
    assert_eq!(
        map.candidate_matches_at("漢字")
            .next()
            .unwrap()
            .select_default()
            .suffix_reading
            .as_deref(),
        Some("")
    );
    assert_eq!(
        convert_plain_text("1漢字", &map, RenderMode::HangulOnly),
        "1"
    );
    map.insert("漢字", "");
    assert_eq!(convert_plain_text("漢字", &map, RenderMode::HangulOnly), "");
    let dict: Box<dyn HanjaDictionary> = Box::new(Legacy);
    let chain: ChainDictionary<_> = [dict].into_iter().collect();
    let matched = chain
        .candidate_matches_at("漢字")
        .next()
        .unwrap()
        .select_default();
    assert_eq!(matched.reading, "");
    assert_eq!(matched.suffix_reading.as_deref(), Some(""));
    assert!(matched.mark.require_hanja);
    assert_eq!(
        chain.candidate_matches_at_spellings(&["漢字"])[0]
            .1
            .select_default(),
        matched
    );
    assert_eq!(
        convert_plain_text("漢字", &chain, RenderMode::HangulOnly),
        "(漢字)"
    );
}
