// Gukhanmun: umbrella library that wires the engine and adapters together.
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

//! Custom-dictionary and user-directive composition tests.

use gukhanmun::{Builder, ContextWindow, DirectiveAction, HomophoneDetection, MapDictionary};

#[test]
fn earlier_dictionaries_win_over_later_ones() {
    let mut high = MapDictionary::new();
    high.insert("學校", "교실");
    let mut low = MapDictionary::new();
    low.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_dictionaries()
        .push_dictionary(high)
        .push_dictionary(low)
        .build()
        .expect("builder");
    assert_eq!(
        converter.convert_text_to_string("學校").expect("convert"),
        "교실"
    );
}

#[test]
fn context_local_detection_emits_hanja_for_in_text_collision() {
    let mut user = MapDictionary::new();
    user.insert("家長", "가장");
    user.insert("假裝", "가장");
    let converter = Builder::new()
        .no_bundled_dictionaries()
        .push_dictionary(user)
        .homophone_window(ContextWindow::PerDocument)
        .build()
        .expect("builder");
    // Both homophones share 가장 and both appear, so the default context-local
    // strategy glosses each occurrence.
    assert_eq!(
        converter
            .convert_text_to_string("家長과 假裝")
            .expect("convert"),
        "가장(家長)과 가장(假裝)"
    );
}

#[test]
fn context_local_detection_leaves_isolated_word_unglossed() {
    let mut user = MapDictionary::new();
    user.insert("家長", "가장");
    user.insert("假裝", "가장");
    let converter = Builder::new()
        .no_bundled_dictionaries()
        .push_dictionary(user)
        .homophone_window(ContextWindow::PerDocument)
        .build()
        .expect("builder");
    // 假裝 (가장) never appears, so 家長 is left as plain hangul by default.
    assert_eq!(
        converter.convert_text_to_string("家長").expect("convert"),
        "가장"
    );
}

#[test]
fn dictionary_wide_detection_emits_hanja_without_in_text_collision() {
    let mut user = MapDictionary::new();
    user.insert("家長", "가장");
    user.insert("假裝", "가장");
    let converter = Builder::new()
        .no_bundled_dictionaries()
        .push_dictionary(user)
        .homophone_window(ContextWindow::PerDocument)
        .homophone_detection(HomophoneDetection::DictionaryWide)
        .build()
        .expect("builder");
    // The opt-in dictionary-wide strategy glosses 家長 because 假裝 shares its
    // reading in the dictionary, even though it is absent from the text.
    assert_eq!(
        converter.convert_text_to_string("家長").expect("convert"),
        "가장(家長)"
    );
}

#[test]
fn user_directive_skip_annotation_collapses_to_plain_hangul() {
    let mut user = MapDictionary::new();
    user.insert_marked(
        "學校",
        "학교",
        gukhanmun::MatchMark {
            require_hanja: true,
            require_hangul: false,
        },
    );
    let converter = Builder::new()
        .no_bundled_dictionaries()
        .push_dictionary(user)
        .directive("學校", DirectiveAction::SkipAnnotation)
        .build()
        .expect("builder");
    assert_eq!(
        converter.convert_text_to_string("學校").expect("convert"),
        "학교"
    );
}

#[test]
fn candidate_sets_survive_builder_boxes_and_user_override() {
    use gukhanmun::{HanjaDictionary, MatchMark, ReadingCandidate, ReadingCandidates};
    let mut dictionary = MapDictionary::new();
    dictionary.insert_candidates(
        "東京",
        ReadingCandidates::new(vec![
            ReadingCandidate {
                reading: "동경".into(),
                suffix_reading: None,
                mark: MatchMark::default(),
                preference: 0,
            },
            ReadingCandidate {
                reading: "도쿄".into(),
                suffix_reading: None,
                mark: MatchMark::default(),
                preference: 10,
            },
        ])
        .unwrap(),
    );
    let converter = Builder::new()
        .no_bundled_dictionaries()
        .push_dictionary(dictionary.clone())
        .build()
        .unwrap();
    assert_eq!(
        converter
            .dictionary()
            .candidate_matches_at("東京")
            .next()
            .unwrap()
            .candidates
            .as_slice()
            .len(),
        2
    );
    assert_eq!(
        converter.convert_text_to_string("東京을").unwrap(),
        "동경을"
    );
    assert_eq!(
        converter.convert_text_to_string("東京를").unwrap(),
        "도쿄를"
    );
    let mut user = MapDictionary::new();
    user.insert("東京", "동경");
    let converter = Builder::new()
        .no_bundled_dictionaries()
        .push_dictionary(user)
        .push_dictionary(dictionary)
        .build()
        .unwrap();
    assert_eq!(
        converter.convert_text_to_string("東京을").unwrap(),
        "동경을"
    );
    assert_eq!(
        converter
            .dictionary()
            .candidate_entries()
            .unwrap()
            .next()
            .unwrap()
            .candidates
            .as_slice()
            .len(),
        1
    );
}

#[test]
fn bundled_particles_and_format_boundaries() {
    use gukhanmun::SegmentationStrategy;
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        // Isolate the standard dictionary candidate set from loanword overrides.
        let converter = Builder::new()
            .no_bundled_loanwords()
            .segmentation(strategy)
            .homophone_window(ContextWindow::Off)
            .build()
            .unwrap();
        for (input, expected) in [
            ("東京을", "동경을"),
            ("東京를", "도쿄를"),
            ("東京이다", "동경이다"),
            ("七分을", "칠분을"),
            ("一切가", "일체가"),
            ("一切로", "일절로"),
        ] {
            assert_eq!(
                converter.convert_text_to_string(input).unwrap(),
                expected,
                "{input}"
            );
        }
        // Use a controlled 東京-only dictionary: bundled 東京이 is itself a
        // longer mixed-script key, so it cannot isolate copula evidence.
        let mut controlled = MapDictionary::new();
        controlled.insert_candidates(
            "東京",
            gukhanmun::ReadingCandidates::new(vec![
                gukhanmun::ReadingCandidate {
                    reading: "도쿄".into(),
                    suffix_reading: None,
                    preference: 10,
                    mark: gukhanmun::MatchMark::default(),
                },
                gukhanmun::ReadingCandidate {
                    reading: "동경".into(),
                    suffix_reading: None,
                    preference: 0,
                    mark: gukhanmun::MatchMark::default(),
                },
            ])
            .unwrap(),
        );
        let converter = Builder::new()
            .no_bundled_dictionaries()
            .push_dictionary(controlled)
            .segmentation(strategy)
            .homophone_window(ContextWindow::Off)
            .build()
            .unwrap();
        #[cfg(feature = "html")]
        for (input, expected) in [
            ("東京이<b>다</b>", "도쿄이<b>다</b>"),
            ("<p>東京이</p>", "<p>동경이</p>"),
            ("東京<b>을</b>", "도쿄<b>을</b>"),
            ("<b>東京이</b>다", "<b>도쿄이</b>다"),
            ("<b>東京을</b>", "<b>동경을</b>"),
            ("東京을<br>", "도쿄을<br>"),
            ("東京을&nbsp;", "도쿄을&nbsp;"),
            ("東京을 <br>", "동경을 <br>"),
            ("東京을<code>x</code>", "도쿄을<code>x</code>"),
        ] {
            assert_eq!(
                converter.convert_html_fragment_to_string(input).unwrap(),
                expected,
                "{input}"
            );
        }
        #[cfg(feature = "markdown")]
        for (input, expected) in [
            ("*東京이*다", "*도쿄이*다"),
            ("[東京을](u)", "[도쿄을](u)"),
            ("東京을\n간다", "도쿄을\n간다"),
            ("東京을  \n간다", "도쿄을  \n간다"),
        ] {
            assert_eq!(
                converter
                    .convert_markdown_to_string(
                        input,
                        gukhanmun::markdown::MarkdownVariant::CommonMark
                    )
                    .unwrap()
                    .trim(),
                expected,
                "{input}"
            );
        }
    }
}
