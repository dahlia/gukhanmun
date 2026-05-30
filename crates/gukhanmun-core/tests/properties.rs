// Gukhanmun: Core IR, engine, dictionary traits, and fallback logic for Gukhanmun.
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

//! Property-based regressions for the core engine that use the shared
//! `proptest` strategies from `tests/common`.
//!
//! These complement the case-driven assertions in `core_mvp.rs` by giving
//! the engine and renderer randomised hangul / hanja / mixed-script inputs
//! and asserting structural invariants—currently the "hangul-only input
//! is a no-op" and "chunked streaming equals one-shot" contracts.  New
//! property tests should consume the strategies in `common::*` so the
//! generator surface stays consistent across the test suite.

mod common;

use gukhanmun_core::{
    Engine, HanjaDictionary, InputToken, MapDictionary, PlainScopeData, RenderMode,
    convert_plain_text, render_tokens, write_plain_text,
};
use proptest::prelude::*;

fn chunk_by_sizes(input: &str, sizes: &[usize]) -> Vec<String> {
    let chars = input.chars().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut index = 0;

    for size in sizes {
        if index >= chars.len() {
            break;
        }
        let end = (index + size).min(chars.len());
        chunks.push(chars[index..end].iter().collect());
        index = end;
    }

    if index < chars.len() {
        chunks.push(chars[index..].iter().collect());
    }

    chunks
}

proptest! {
    /// Hangul-only input with an empty dictionary must round-trip
    /// byte-for-byte: the engine has no hanja to annotate, and the
    /// renderer's `HangulOnly` mode leaves plain text alone.
    #[test]
    fn hangul_only_input_is_noop(input in common::arb_hangul_only_string()) {
        let dict = MapDictionary::new();
        let output = convert_plain_text(&input, &dict, RenderMode::HangulOnly);
        prop_assert_eq!(output, input);
    }

    /// Feeding the engine its input in arbitrary code-point chunks must
    /// yield the same conversion as a single-shot call.  This pins the
    /// streaming contract from the design doc: chunked I/O is the default
    /// and must not change observable output.
    #[test]
    fn chunked_streaming_matches_one_shot(
        chunks in common::arb_mixed_script_chunks(),
    ) {
        let dict = common::mixed_script_dictionary();
        let combined: String = chunks.iter().map(String::as_str).collect();

        let mut engine = Engine::<PlainScopeData, _>::new(&dict);
        let mut output_tokens = Vec::new();
        for chunk in chunks {
            output_tokens.extend(engine.push_token(InputToken::Text(chunk)));
        }
        output_tokens.extend(engine.finish());
        let chunked = write_plain_text(render_tokens(output_tokens, RenderMode::HangulOnly));

        let one_shot = convert_plain_text(&combined, &dict, RenderMode::HangulOnly);
        prop_assert_eq!(chunked, one_shot);
    }

    /// Mixed-script dictionary entries must survive arbitrary chunk splits
    /// inside the key.  This is the case that requires the engine's tail
    /// buffer: `汽車길` and `色깔論` should not devolve into partial fallback
    /// merely because a reader delivered `汽`, `車`, and `길` separately.
    #[test]
    fn mixed_script_dictionary_key_survives_chunk_boundaries(
        key in prop::sample::select(vec!["汽車길", "祭祀날", "洗手대야", "火김", "色깔論", "天地"]),
        chunk_sizes in prop::collection::vec(1usize..4, 0..8),
    ) {
        let dict = common::mixed_script_dictionary();
        let input = format!("앞 {key} 뒤");
        let chunks = chunk_by_sizes(&input, &chunk_sizes);

        let mut engine = Engine::<PlainScopeData, _>::new(&dict);
        let mut output_tokens = Vec::new();
        for chunk in chunks {
            output_tokens.extend(engine.push_token(InputToken::Text(chunk)));
        }
        output_tokens.extend(engine.finish());
        let chunked = write_plain_text(render_tokens(output_tokens, RenderMode::HangulHanjaParens));

        let one_shot = convert_plain_text(&input, &dict, RenderMode::HangulHanjaParens);
        prop_assert_eq!(chunked, one_shot);
    }

    /// For dictionary-backed mixed-script prefixes, the pending text tail stays
    /// within the advertised dictionary maximum.  Long fallback-only hanja
    /// runs are intentionally excluded: those may wait for a boundary so
    /// render modes that expose source spans remain one-shot equivalent.
    #[test]
    fn dictionary_tail_buffer_stays_within_max_word_chars(
        key in prop::sample::select(vec!["汽車길", "祭祀날", "洗手대야", "火김", "色깔論", "天地"]),
    ) {
        let dict = common::mixed_script_dictionary();
        let bound = dict.max_word_chars().expect("test dictionary has a bound");
        let mut engine = Engine::<PlainScopeData, _>::new(&dict);

        for ch in format!("{key} ").chars() {
            let _ = engine.push_token(InputToken::Text(ch.to_string()));
            prop_assert!(
                engine.buffered_chars() <= bound,
                "buffered {} chars with dictionary bound {bound}",
                engine.buffered_chars(),
            );
        }
    }
}
