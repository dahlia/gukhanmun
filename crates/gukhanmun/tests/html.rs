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

//! HTML adapter integration tests (gated on the `html` feature).

#![cfg(feature = "html")]

use gukhanmun::{Builder, ContextWindow, MapDictionary, RenderMode, RubyBase};

#[test]
fn html_buffered_converts_text_inside_paragraph() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .build()
        .expect("builder");
    let output = converter
        .convert_html_fragment_to_string("<p>學校</p>")
        .expect("html");
    assert_eq!(output, "<p>학교</p>");
}

#[test]
fn html_streaming_iter_matches_buffered() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    user.insert("大韓", "대한");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .build()
        .expect("builder");

    let input = "<p>大韓</p><p>學校</p>";
    let buffered = converter
        .convert_html_fragment_to_string(input)
        .expect("buf");
    let streamed: Vec<_> = converter
        .convert_html_fragment_iter(input)
        .expect("html iter")
        .collect();
    let streamed_string = gukhanmun::html::write_html_fragment(streamed);
    assert_eq!(streamed_string, buffered);
}

#[test]
fn html_preserve_when_skips_marked_elements() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .html_preserve_when(|info| info.raw_attributes.contains("class=\"no-convert\""))
        .build()
        .expect("builder");

    let output = converter
        .convert_html_fragment_to_string("<p>學校</p><span class=\"no-convert\">學校</span>")
        .expect("html");
    assert!(output.contains("<p>학교</p>"));
    assert!(output.contains("<span class=\"no-convert\">學校</span>"));
}

#[test]
fn html_ruby_rendering_emits_ruby_tags() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .rendering(RenderMode::Ruby(RubyBase::OnHangul))
        .build()
        .expect("builder");

    let output = converter
        .convert_html_fragment_to_string("<p>學校</p>")
        .expect("html");
    assert!(output.contains("<ruby>학교<rp>(</rp><rt>學校</rt><rp>)</rp></ruby>"));
}
