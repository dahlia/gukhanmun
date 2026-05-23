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

//! Markdown adapter integration tests (gated on the `markdown` feature).

#![cfg(feature = "markdown")]

use gukhanmun::markdown::MarkdownVariant;
use gukhanmun::{Builder, ContextWindow, MapDictionary};

#[test]
fn markdown_commonmark_buffered_converts_paragraph() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .build()
        .expect("builder");
    let output = converter
        .convert_markdown_to_string("學校", MarkdownVariant::CommonMark)
        .expect("md");
    assert!(output.contains("학교"));
    assert!(!output.contains("學校"));
}

#[test]
fn markdown_preserves_inline_code_verbatim() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .build()
        .expect("builder");
    let output = converter
        .convert_markdown_to_string("text with `學校` code", MarkdownVariant::CommonMark)
        .expect("md");
    assert!(output.contains("`學校`"));
}

#[test]
fn markdown_gfm_table_round_trips() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .build()
        .expect("builder");
    let input = "| Word |\n|------|\n| 學校 |";
    let output = converter
        .convert_markdown_to_string(input, MarkdownVariant::Gfm)
        .expect("md");
    assert!(output.contains("학교"));
    assert!(output.contains("|"));
}

#[test]
fn markdown_streaming_iter_matches_buffered() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .build()
        .expect("builder");
    let input = "first 學校.\n\nsecond 學校.";
    let buffered = converter
        .convert_markdown_to_string(input, MarkdownVariant::CommonMark)
        .expect("buf");
    let streamed: Vec<_> = converter
        .convert_markdown_iter(input, MarkdownVariant::CommonMark)
        .collect();
    let streamed_string = gukhanmun::markdown::write_markdown(streamed).expect("write");
    assert_eq!(streamed_string, buffered);
}
