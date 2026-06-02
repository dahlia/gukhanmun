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

#[cfg(any(feature = "opendict", feature = "stdict"))]
use gukhanmun::HanjaDictionary;
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
    // Default detection is context-local: a homophone is glossed only when a
    // different-hanja reading-mate actually co-occurs within the context
    // window.  `天地` and `天池` both read 천지, so both are glossed here.
    let converter = Builder::new().build().expect("default builder");
    let output = converter
        .convert_text_to_string("天地와 天池")
        .expect("convert");
    assert_eq!(output, "천지(天地)와 천지(天池)");
}

#[cfg(feature = "stdict")]
#[test]
fn default_ko_kr_leaves_standalone_homophone_unglossed() {
    // `天地` has dictionary-wide reading-mates (`天池`, `淺智`, ...) but none
    // of them occur in this input, so context-local detection leaves it as
    // plain hangul.
    let converter = Builder::new().build().expect("default builder");
    let output = converter.convert_text_to_string("天地").expect("convert");
    assert_eq!(output, "천지");
}

#[cfg(feature = "stdict")]
#[test]
fn dictionary_wide_glosses_standalone_homophone() {
    use gukhanmun::HomophoneDetection;

    // Dictionary-wide detection glosses any reading shared by another hanja
    // form anywhere in the bundled dictionary, even with no in-text collision.
    let converter = Builder::new()
        .homophone_detection(HomophoneDetection::DictionaryWide)
        .build()
        .expect("default builder");
    let output = converter.convert_text_to_string("天地").expect("convert");
    assert_eq!(output, "천지(天地)");
}

#[cfg(feature = "stdict")]
#[test]
fn default_ko_kr_leaves_uncollided_word_unglossed() {
    // `言語` (언어) is glossed by no curated require-hanja rule, and no other
    // 언어-reading hanja form occurs in this input, so the default
    // context-local detection leaves it as plain hangul.
    let converter = Builder::new().build().expect("default builder");
    assert_eq!(
        converter.convert_text_to_string("言語").expect("convert"),
        "언어"
    );
}

#[cfg(feature = "stdict")]
#[test]
fn bundled_initial_sound_law_follows_position() {
    // Hangul-only rendering with homophone marking disabled keeps the
    // assertions focused on the reading itself.
    let converter = Builder::new()
        .rendering(RenderMode::HangulOnly)
        .homophone_window(ContextWindow::Off)
        .build()
        .expect("default builder");
    let convert = |input| converter.convert_text_to_string(input).expect("convert");

    // Regression: the bundled dictionary stores the word-initial reading
    // (`年` → `연`), but after a number `年` keeps its original sound.
    assert_eq!(convert("1998年"), "1998년");
    assert_eq!(convert("年"), "연");

    // Multi-syllable suffix overrides recorded by the dictionary.
    assert_eq!(convert("1990年代"), "1990년대");
    assert_eq!(convert("2024年度"), "2024년도");
    assert_eq!(convert("年代"), "연대");

    // Single hanja resolved from the bundled unihan readings, both positions.
    assert_eq!(convert("理"), "이");
    assert_eq!(convert("5理"), "5리");

    // `曆` keeps its original sound outside word-initial position even though
    // the dictionary has no standalone word-initial head word for it.
    assert_eq!(convert("佛曆"), "불력");
    assert_eq!(convert("曆"), "역");

    // Compounds the dictionary already reads correctly must not be perturbed by
    // the single-hanja or multi-syllable rules.
    assert_eq!(convert("理論"), "이론");
    assert_eq!(convert("論理"), "논리");
}

#[cfg(feature = "stdict")]
#[test]
fn ko_kp_keeps_original_sound_everywhere() {
    let converter = Builder::with_preset(Preset::KoKp)
        .bundled_stdict()
        .rendering(RenderMode::HangulOnly)
        .build()
        .expect("ko-kp builder");
    // With initial sound law disabled the original sound is used in every
    // position, word-initial included.
    assert_eq!(
        converter.convert_text_to_string("年").expect("convert"),
        "년"
    );
    assert_eq!(
        converter.convert_text_to_string("1998年").expect("convert"),
        "1998년"
    );
}

#[test]
fn user_dictionary_overrides_fallback() {
    let mut user = MapDictionary::new();
    user.insert("外字", "외자");
    let converter = Builder::new()
        .no_bundled_dictionaries()
        .push_dictionary(user)
        .build()
        .expect("builder");
    let output = converter.convert_text_to_string("外字").expect("convert");
    assert_eq!(output, "외자");
}

#[test]
#[cfg(feature = "opendict")]
fn ko_kp_skips_initial_sound_law_and_uses_north_korean_opendict() {
    let converter = Builder::with_preset(Preset::KoKp)
        .build()
        .expect("ko-kp builder");
    let output = converter
        .convert_text_to_string("歷史 來日 勞動")
        .expect("convert");
    assert_eq!(output, "력사 래일 로동");
}

#[test]
#[cfg(not(feature = "opendict"))]
fn ko_kp_requires_opendict_for_bundled_north_korean_dictionary() {
    let error = match Builder::with_preset(Preset::KoKp).build() {
        Ok(_) => panic!("ko-kp builder without opendict should fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("`opendict` feature is disabled"));
}

#[cfg(feature = "opendict")]
#[test]
fn ko_kp_dictionary_chain_includes_north_korean_opendict_by_default() {
    let converter = Builder::with_preset(Preset::KoKp)
        .build()
        .expect("ko-kp builder");
    assert!(
        converter
            .dictionary()
            .entries()
            .unwrap()
            .any(|record| { record.hanja == "歷史" && record.reading == "력사" })
    );

    let converter = Builder::with_preset(Preset::KoKp)
        .no_bundled_dictionaries()
        .build()
        .expect("ko-kp builder");
    assert!(converter.dictionary().entries().unwrap().next().is_none());
}

#[cfg(feature = "opendict")]
#[test]
fn no_bundled_opendict_disables_ko_kp_bundled_dictionary() {
    let converter = Builder::with_preset(Preset::KoKp)
        .no_bundled_opendict()
        .build()
        .expect("ko-kp builder");
    assert!(converter.dictionary().entries().unwrap().next().is_none());

    let output = converter.convert_text_to_string("來日").expect("convert");
    assert_eq!(output, "래일");
}

#[cfg(all(feature = "opendict", feature = "stdict"))]
#[test]
fn no_bundled_opendict_leaves_stdict_enabled() {
    let converter = Builder::with_preset(Preset::KoKp)
        .bundled_stdict()
        .no_bundled_opendict()
        .build()
        .expect("ko-kp builder");
    assert!(
        converter
            .dictionary()
            .entries()
            .unwrap()
            .any(|record| { record.hanja == "歷史" && record.reading == "역사" })
    );
    assert_eq!(
        converter.convert_text_to_string("歷史").expect("convert"),
        "역사"
    );
}

#[test]
fn ko_kr_initial_sound_law_applies_in_fallback() {
    let converter = Builder::with_preset(Preset::KoKr)
        .no_bundled_dictionaries()
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
        .no_bundled_dictionaries()
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
    // rest—there should be no panic and no requirement to drive the upstream
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
    // fully drained. The engine needs some lookahead—but it must not have
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
