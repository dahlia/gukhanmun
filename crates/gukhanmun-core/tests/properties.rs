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
//! and asserting structural invariants — currently the "hangul-only input
//! is a no-op" and "chunked streaming equals one-shot" contracts.  New
//! property tests should consume the strategies in `common::*` so the
//! generator surface stays consistent across the test suite.

mod common;

use gukhanmun_core::{
    Engine, InputToken, MapDictionary, PlainScopeData, RenderMode, convert_plain_text,
    render_tokens, write_plain_text,
};
use proptest::prelude::*;

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
        let dict = MapDictionary::new();
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
}
