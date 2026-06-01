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

//! Shared `proptest` strategies and snapshot helpers used by the core
//! crate's test binaries.
//!
//! The strategies are deliberately compact—narrow enough to produce
//! interesting inputs for the engine (mixing hanja, hangul, and connective
//! punctuation) without exploring the full Unicode space, which makes
//! shrinking tractable and counter-examples readable.
//!
//! The snapshot helpers project the engine's [`OutputToken`] stream onto
//! a stable JSON shape owned by the test layer.  That shape is what the
//! `tests/snapshots/` `.snap` files record, which means an internal field
//! rename in `gukhanmun-core` is free until the test layer chooses to
//! reflect it in the projection.

// Each test binary in `tests/` recompiles this module independently and
// reports dead-code warnings for any helper it does not happen to use.
// All exports here are part of a shared test toolbox, so silence the
// warning at the module level rather than annotating each item.
#![allow(dead_code)]

use gukhanmun_core::{Annotation, MapDictionary, OutputToken, PlainScopeData, Scope, ScopeData};
use proptest::prelude::*;
use serde_json::{Value, json};

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
}

/// Dictionary entries that exercise mixed-script keys and overlapping
/// hanja-only matches in streaming properties.
pub fn mixed_script_dictionary() -> MapDictionary {
    let mut dict = MapDictionary::new();
    dict.insert("汽車길", "기찻길");
    dict.insert("祭祀날", "제삿날");
    dict.insert("洗手대야", "세숫대야");
    dict.insert("火김", "홧김");
    dict.insert("色깔論", "색깔론");
    dict.insert("汽車", "기차");
    dict.insert("天地", "천지");
    dict
}

/// Projects an [`OutputToken`] stream onto a stable JSON value for use
/// with `insta::assert_json_snapshot!`.
///
/// The projection is owned by the test layer rather than derived from
/// `#[derive(Serialize)]` on the public types, so a rename of an internal
/// field in `gukhanmun-core` does not silently churn every recorded
/// `.snap` file—the projection has to be updated deliberately.  The
/// output is an array; each element is one of:
///
/// * `{"open": <scope>}` where `<scope>` is the projection from
///   [`scope_to_value`].
/// * `"close"`.
/// * `{"text": "..."}` or `{"verbatim": "..."}` for passthrough text.
/// * `{"annotated": {…}}` for converted hanja, carrying both the
///   pre-rendering metadata and the seven policy flags.
pub fn tokens_to_snapshot_value<S>(tokens: &[OutputToken<S>]) -> Value
where
    S: ScopeData + SnapshotScope,
{
    Value::Array(tokens.iter().map(token_to_value).collect())
}

fn token_to_value<S: ScopeData + SnapshotScope>(token: &OutputToken<S>) -> Value {
    match token {
        OutputToken::Open(scope) => json!({ "open": scope_to_value(scope) }),
        OutputToken::Close => Value::String("close".to_owned()),
        OutputToken::Text(text) => json!({ "text": text }),
        OutputToken::Verbatim(text) => json!({ "verbatim": text }),
        OutputToken::Annotated(ann) => json!({ "annotated": annotation_to_value(ann) }),
    }
}

fn annotation_to_value(ann: &Annotation) -> Value {
    json!({
        "hanja": ann.hanja,
        "reading": ann.reading,
        "homophone": ann.homophone,
        "require_hanja": ann.require_hanja,
        "require_hangul": ann.require_hangul,
        "first_in_context": ann.first_in_context,
        "skip_annotation": ann.skip_annotation,
        "from_dictionary": ann.from_dictionary,
        "from_source_gloss": ann.from_source_gloss,
    })
}

fn scope_to_value<S: ScopeData + SnapshotScope>(scope: &Scope<S>) -> Value {
    json!({
        "preserve": scope.data().is_preserve(),
        "allows_inline_markup": scope.data().allows_inline_markup(),
        "block_boundary": scope.data().is_block_boundary(),
        "section_boundary": scope.data().is_section_boundary(),
        "extra": scope.data().snapshot_extra(),
    })
}

/// Lets each scope type contribute its adapter-specific fields to the
/// snapshot shape without coupling `tokens_to_snapshot_value` to a
/// particular [`ScopeData`] implementation.  The default returns
/// [`Value::Null`], so [`PlainScopeData`] needs no extra columns in the
/// recorded snapshots.
pub trait SnapshotScope {
    /// Returns extra adapter-specific scope state for the snapshot.
    fn snapshot_extra(&self) -> Value {
        Value::Null
    }
}

impl SnapshotScope for PlainScopeData {}
