// Gukhanmun: Compatibility coverage for borrowed dictionaries and local callbacks.
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

use std::cell::Cell;
use std::rc::Rc;

use gukhanmun::{
    Builder, ContextWindow, DirectiveAction, HanjaDictionary, MapDictionary, UserDirectives,
};
use gukhanmun_core::Match;

struct LocalDictionary(Rc<MapDictionary>);

impl HanjaDictionary for LocalDictionary {
    fn matches_at<'a>(&'a self, input: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        self.0.matches_at(input)
    }
}

fn dictionary() -> LocalDictionary {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("學校", "학교");
    dictionary.insert("漢字", "한자");
    LocalDictionary(Rc::new(dictionary))
}

#[test]
fn original_builder_accepts_owned_and_borrowed_rc_dictionaries() {
    let dictionary = dictionary();
    let borrowed = Builder::new()
        .no_bundled_dictionaries()
        .homophone_window(ContextWindow::Off)
        .push_dictionary(&dictionary)
        .build()
        .expect("borrowed dictionary");
    assert_eq!(
        borrowed.convert_text_to_string("學校 漢字").unwrap(),
        "학교 한자"
    );
    drop(borrowed);

    let owned = Builder::new()
        .no_bundled_dictionaries()
        .homophone_window(ContextWindow::Off)
        .push_dictionary(dictionary)
        .build()
        .expect("owned Rc dictionary");
    assert_eq!(
        owned.convert_text_to_string("學校 漢字").unwrap(),
        "학교 한자"
    );
}

#[test]
fn borrowed_local_directive_runs_after_first_occurrence_filter_on_each_document() {
    let target = String::from("學校");
    let target = &target;
    let calls = Rc::new(Cell::new(0));
    let predicate_calls = Rc::clone(&calls);
    let converter = Builder::new()
        .no_bundled_dictionaries()
        .homophone_window(ContextWindow::Off)
        .first_occurrence_window(ContextWindow::PerDocument)
        .push_dictionary(dictionary())
        .directive_predicate(
            move |annotation| {
                predicate_calls.set(predicate_calls.get() + 1);
                annotation.hanja == *target
            },
            DirectiveAction::RequireHanja,
        )
        .build()
        .expect("borrowed and Rc directive captures");

    for document in 1..=2 {
        assert_eq!(
            converter.convert_text_to_string("學校 學校").unwrap(),
            "학교(學校) 학교(學校)"
        );
        assert_eq!(calls.get(), document * 2);
    }
}

#[test]
fn wholesale_directives_replace_prior_local_rules_and_allow_later_literals() {
    let target = String::from("學校");
    let target = &target;
    let replaced_calls = Rc::new(Cell::new(0));
    let old_calls = Rc::clone(&replaced_calls);
    let replacement_calls = Rc::new(Cell::new(0));
    let active_calls = Rc::clone(&replacement_calls);
    let mut replacement = UserDirectives::new();
    replacement.add_predicate(
        move |annotation| {
            active_calls.set(active_calls.get() + 1);
            annotation.hanja == *target
        },
        DirectiveAction::RequireHanja,
    );

    let converter = Builder::new()
        .no_bundled_dictionaries()
        .homophone_window(ContextWindow::Off)
        .push_dictionary(dictionary())
        .directive("學校", DirectiveAction::SkipAnnotation)
        .directive_predicate(
            move |_| {
                old_calls.set(old_calls.get() + 1);
                true
            },
            DirectiveAction::SkipAnnotation,
        )
        .directives(replacement)
        .directive("漢字", DirectiveAction::RequireHanja)
        .build()
        .expect("replacement local directives");

    assert_eq!(
        converter.convert_text_to_string("學校 漢字").unwrap(),
        "학교(學校) 한자(漢字)"
    );
    assert_eq!(replaced_calls.get(), 0);
    assert_eq!(replacement_calls.get(), 2);
}

#[cfg(feature = "html")]
#[test]
fn html_predicate_keeps_borrowed_rc_captures_and_replaces_previous_predicate() {
    let attributes = String::from("class=\"local\"");
    let attributes = &attributes;
    let replaced_calls = Rc::new(Cell::new(0));
    let old_calls = Rc::clone(&replaced_calls);
    let replacement_calls = Rc::new(Cell::new(0));
    let active_calls = Rc::clone(&replacement_calls);
    let converter = Builder::new()
        .no_bundled_dictionaries()
        .homophone_window(ContextWindow::Off)
        .push_dictionary(dictionary())
        .html_preserve_when(move |_| {
            old_calls.set(old_calls.get() + 1);
            true
        })
        .html_preserve_when(move |info| {
            active_calls.set(active_calls.get() + 1);
            info.raw_attributes.contains(attributes.as_str())
        })
        .build()
        .expect("replacement borrowed and Rc HTML predicate");

    let input = "<p class=\"old\">學校</p><p class=\"local\"><b>學校</b></p><p>漢字</p>";
    let expected = "<p class=\"old\">학교</p><p class=\"local\"><b>學校</b></p><p>한자</p>";
    for document in 1..=2 {
        assert_eq!(
            converter.convert_html_fragment_to_string(input).unwrap(),
            expected
        );
        assert_eq!(replaced_calls.get(), 0);
        // The preserved parent passes preservation to its child without
        // evaluating the predicate again for that child.
        assert_eq!(replacement_calls.get(), document * 3);
    }
}
