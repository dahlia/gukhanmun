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

//! Plain-text conversion tests for the umbrella `Builder` / `Converter`.

use std::cell::Cell;

use gukhanmun::{
    Builder, ContextWindow, DirectiveAction, InputToken, MapDictionary, NumeralStrategy,
    PlainScopeData, Preset, RenderMode, RenderedToken, write_plain_text,
};

#[cfg(feature = "stdict")]
#[test]
fn default_ko_kr_converts_bundled_word() {
    let converter = Builder::new().build().expect("default builder");
    let output = converter.convert_text_to_string("學校").expect("convert");
    assert_eq!(output, "학교");
}

#[cfg(feature = "stdict")]
#[test]
fn default_ko_kr_marks_homonyms_with_hanja() {
    let converter = Builder::new().build().expect("default builder");
    let output = converter.convert_text_to_string("天地").expect("convert");
    assert_eq!(output, "천지(天地)");
}

#[test]
fn user_dictionary_overrides_fallback() {
    let mut user = MapDictionary::new();
    user.insert("外字", "외자");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .build()
        .expect("builder");
    let output = converter.convert_text_to_string("外字").expect("convert");
    assert_eq!(output, "외자");
}

#[test]
fn ko_kp_skips_initial_sound_law_and_bundled() {
    let converter = Builder::with_preset(Preset::KoKp)
        .build()
        .expect("ko-kp builder");
    // 來日 → 래일 in KP (no initial sound law) using single-char fallback.
    let output = converter.convert_text_to_string("來日").expect("convert");
    assert_eq!(output, "래일");
}

#[test]
fn ko_kr_initial_sound_law_applies_in_fallback() {
    let converter = Builder::with_preset(Preset::KoKr)
        .no_bundled_stdict()
        .build()
        .expect("ko-kr builder");
    let output = converter.convert_text_to_string("來日").expect("convert");
    assert_eq!(output, "내일");
}

#[test]
fn rendering_override_emits_hanja_first() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .rendering(RenderMode::HanjaHangulParens)
        .build()
        .expect("builder");
    let output = converter.convert_text_to_string("學校").expect("convert");
    assert_eq!(output, "學校(학교)");
}

#[test]
fn numeral_strategy_smart_converts_year() {
    let converter = Builder::new()
        .no_bundled_stdict()
        .numerals(NumeralStrategy::Smart)
        .build()
        .expect("builder");
    let output = converter
        .convert_text_to_string("二〇一六年")
        .expect("convert");
    assert_eq!(output, "2016년");

    let output = converter.convert_text_to_string("三時").expect("convert");
    assert_eq!(output, "3시");
}

#[test]
fn user_directive_can_force_hanja() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .directive("學校", DirectiveAction::RequireHanja)
        .build()
        .expect("builder");
    let output = converter.convert_text_to_string("學校").expect("convert");
    assert_eq!(output, "학교(學校)");
}

#[test]
fn streaming_iter_matches_buffered_string() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    user.insert("大韓", "대한");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .build()
        .expect("builder");

    let buffered = converter
        .convert_text_to_string("大韓의 學校")
        .expect("buffered");
    let streamed: Vec<RenderedToken<_>> = converter.convert_text_iter("大韓의 學校").collect();
    assert_eq!(write_plain_text(streamed), buffered);
}

#[test]
fn streaming_iter_is_lazy_for_unconsumed_tokens() {
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .build()
        .expect("builder");
    // Pull a handful of tokens then drop the iterator without consuming the
    // rest — there should be no panic and no requirement to drive the upstream
    // reader to completion before yielding the first token.
    let mut iter = converter.convert_text_iter("學校 學校 學校 學校 學校");
    let _first_two: Vec<_> = iter.by_ref().take(2).collect();
    drop(iter);
}

#[test]
fn streaming_iter_does_not_drain_upstream_ahead_of_demand() {
    // Build a converter with an `Off` context window so middlewares cannot
    // force document-wide buffering.
    let mut user = MapDictionary::new();
    user.insert("學校", "학교");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .first_occurrence_window(ContextWindow::Off)
        .build()
        .expect("builder");

    // A side-channel-counting input iterator: each `next()` increments
    // `consumed`. Each input chunk is `學校 ` (the trailing space is the
    // boundary the engine flushes on).
    let consumed = Cell::new(0usize);
    let total = 50usize;
    let upstream = (0..total).map(|_| {
        consumed.set(consumed.get() + 1);
        InputToken::<PlainScopeData>::Text("學校 ".into())
    });

    let mut output = converter.convert_tokens(upstream);

    // After pulling the first output token, the upstream must not have been
    // fully drained. The engine needs some lookahead — but it must not have
    // walked the entire 50-token input just to yield the first rendered
    // token.
    let _first = output.next().expect("at least one output token");
    let after_first = consumed.get();
    assert!(
        after_first < total,
        "first output should not require draining the entire upstream \
         (consumed {after_first} of {total})"
    );
}
