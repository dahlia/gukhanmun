// Gukhanmun: Bundled alias recognition and identity regression tests.
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

//! Bundled alias recognition tests.

use gukhanmun::{Builder, ContextWindow, DirectiveAction, RenderMode};

#[test]
fn bundled_alias_renders_as_one_annotation() {
    let converter = Builder::new()
        .rendering(RenderMode::HangulHanjaParens)
        .build()
        .expect("builder");
    assert_eq!(
        converter
            .convert_text_to_string("記念館 聯絡處 顓門家")
            .expect("convert"),
        "기념관(記念館) 연락처(聯絡處) 전문가(顓門家)"
    );
}

#[test]
fn alias_repairs_the_parenthetical_collapser() {
    let converter = Builder::new().build().expect("builder");
    assert_eq!(
        converter
            .convert_text_to_string("記念館(기념관)")
            .expect("convert"),
        "기념관(記念館)"
    );
}

#[test]
fn sibling_spellings_do_not_trigger_homophone_glosses() {
    let converter = Builder::new()
        .homophone_window(ContextWindow::PerDocument)
        .build()
        .expect("builder");
    assert_eq!(
        converter
            .convert_text_to_string("紀念館 記念館")
            .expect("convert"),
        "기념관 기념관"
    );
    assert_eq!(
        converter
            .convert_text_to_string("連絡 聯絡")
            .expect("convert"),
        "연락 연락"
    );
}

#[test]
fn real_homophones_still_get_glossed() {
    let converter = Builder::new()
        .homophone_window(ContextWindow::PerDocument)
        .build()
        .expect("builder");
    assert_eq!(
        converter
            .convert_text_to_string("刻本 脚本")
            .expect("convert"),
        "각본(刻本) 각본(脚本)"
    );
}

#[test]
fn canonical_directive_reaches_a_reviewed_alias() {
    let converter = Builder::new()
        .directive("紀念館", DirectiveAction::RequireHanja)
        .build()
        .expect("builder");
    assert_eq!(
        converter
            .convert_text_to_string("紀念館 記念館")
            .expect("convert"),
        "기념관(紀念館) 기념관(記念館)"
    );
}
