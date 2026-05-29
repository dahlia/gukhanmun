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
        .no_bundled_stdict()
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
        .no_bundled_stdict()
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
        .no_bundled_stdict()
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
        .no_bundled_stdict()
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
        .no_bundled_stdict()
        .push_dictionary(user)
        .directive("學校", DirectiveAction::SkipAnnotation)
        .build()
        .expect("builder");
    assert_eq!(
        converter.convert_text_to_string("學校").expect("convert"),
        "학교"
    );
}
