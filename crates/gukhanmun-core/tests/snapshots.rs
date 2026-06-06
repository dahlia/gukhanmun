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

//! `insta` snapshot tests pinning the engine's [`OutputToken`] stream for
//! representative inputs.
//!
//! The snapshots cover canonical cases that mix dictionary matches, the
//! per-character fallback, the initial sound law, and the homophone
//! marker.  Updating a snapshot is a deliberate act: run
//! `cargo insta review` after editing engine behaviour to inspect and
//! accept (or reject) the new baseline.  See `CONTRIBUTING.md`.
//!
//! The recorded shape lives in `tests/common/mod.rs` (the
//! `tokens_to_snapshot_value` projection).  Snapshots intentionally do
//! *not* serialise the public `OutputToken` / `Annotation` types
//! directly—that way a rename inside the core crate does not silently
//! churn every `.snap` file.

mod common;

use gukhanmun_core::{
    ContextWindow, Engine, EngineOptions, HomophoneDetection, InputToken, MapDictionary,
    OutputToken, PlainScopeData, mark_homophones_with_detection, process_tokens_with_options,
};

fn run(
    input: &str,
    dict: &MapDictionary,
    options: EngineOptions,
) -> Vec<OutputToken<PlainScopeData>> {
    let tokens = vec![InputToken::<PlainScopeData>::Text(input.to_owned())];
    let mut engine = Engine::<PlainScopeData, _>::with_options(dict, options);
    let mut out = Vec::new();
    for token in tokens {
        out.extend(engine.push_token(token));
    }
    out.extend(engine.finish());
    out
}

/// Asserts a JSON snapshot with `sort_maps` enabled.  `insta` defaults to
/// insertion-order key emission, which would couple snapshot output to
/// `serde_json`'s active backend—when any workspace dependency enables
/// `serde_json/preserve_order`, Cargo feature unification flips ordering
/// to insertion order and silently churns every recorded `.snap`.  Sorting
/// maps centrally here pins the recorded shape regardless of the active
/// backend.
macro_rules! assert_snapshot {
    ($value:expr) => {
        insta::with_settings!({ sort_maps => true }, {
            insta::assert_json_snapshot!($value);
        });
    };
}

#[test]
fn dictionary_match_emits_single_annotation() {
    let mut dict = MapDictionary::new();
    dict.insert("學校", "학교");
    let tokens = run("學校", &dict, EngineOptions::default());
    assert_snapshot!(common::tokens_to_snapshot_value(&tokens));
}

#[test]
fn single_char_fallback_uses_unihan_reading() {
    let dict = MapDictionary::new();
    let tokens = run("學", &dict, EngineOptions::default());
    assert_snapshot!(common::tokens_to_snapshot_value(&tokens));
}

#[test]
fn fallback_applies_initial_sound_law_in_ko_kr() {
    let dict = MapDictionary::new();
    let options = EngineOptions {
        initial_sound_law: true,
        ..EngineOptions::default()
    };
    let tokens = run("來日", &dict, options);
    assert_snapshot!(common::tokens_to_snapshot_value(&tokens));
}

#[test]
fn fallback_skips_initial_sound_law_in_ko_kp() {
    let dict = MapDictionary::new();
    let options = EngineOptions {
        initial_sound_law: false,
        ..EngineOptions::default()
    };
    let tokens = run("來日", &dict, options);
    assert_snapshot!(common::tokens_to_snapshot_value(&tokens));
}

#[test]
fn mixed_script_dictionary_entry_covers_hangul_suffix() {
    let mut dict = MapDictionary::new();
    dict.insert("色깔論", "색깔론");
    let tokens = run("色깔論", &dict, EngineOptions::default());
    assert_snapshot!(common::tokens_to_snapshot_value(&tokens));
}

#[test]
fn homophone_marker_flags_shared_reading() {
    let mut dict = MapDictionary::new();
    dict.insert("天地", "천지");
    dict.insert("天池", "천지");
    let tokens = process_tokens_with_options(
        vec![InputToken::<PlainScopeData>::Text("天地".into())],
        &dict,
        EngineOptions::default(),
    );
    let marked = mark_homophones_with_detection(
        tokens,
        &dict,
        ContextWindow::PerBlock,
        HomophoneDetection::DictionaryWide,
    );
    assert_snapshot!(common::tokens_to_snapshot_value(&marked));
}

#[test]
fn trivial_dictionary_merges_with_fallback() {
    let mut dict = MapDictionary::new();
    dict.insert("洪", "홍");
    dict.insert("民", "민");
    let tokens = run("洪民憙", &dict, EngineOptions::default());
    assert_snapshot!(common::tokens_to_snapshot_value(&tokens));
}
