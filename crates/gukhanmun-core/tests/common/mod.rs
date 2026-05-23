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

//! Shared `proptest` strategies used by the core crate's property test
//! binaries.
//!
//! The strategies are deliberately compact — narrow enough to produce
//! interesting inputs for the engine (mixing hanja, hangul, and connective
//! punctuation) without exploring the full Unicode space, which makes
//! shrinking tractable and counter-examples readable.

use proptest::prelude::*;

/// Generates a string of hangul syllables and spaces (no hanja, no ASCII
/// letters).  Use this as the canonical "should be a no-op" input for
/// engine / renderer identity properties: any dictionary that does not
/// contain hangul-only keys must leave the text untouched.
pub fn arb_hangul_only_string() -> impl Strategy<Value = String> {
    "[가-힣 ]{0,32}".prop_map(String::from)
}

/// Generates an `Vec<String>` of arbitrary text chunks whose concatenation
/// is a mixed-script string.  Designed for chunked-streaming equivalence
/// properties: the test feeds the chunks into a stateful engine and
/// compares the result with a single one-shot conversion of `concat`.
///
/// Every chunk is itself a valid UTF-8 string with no partial-codepoint
/// splits, mirroring how the umbrella streaming entry points buffer until
/// codepoint boundaries.
pub fn arb_mixed_script_chunks() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec("[가-힣A-Za-z .,!?]{0,8}|漢字|天地|汽|車|길|色|깔|論", 0..16)
        .prop_map(|chunks| chunks.into_iter().map(String::from).collect())
}
