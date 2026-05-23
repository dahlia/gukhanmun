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

use gukhanmun_core::{
    Engine, EngineOptions, Error as CoreError, MapDictionary, PlainScopeData,
    RecoverableInputError, Recovery, SegmentationStrategy, process_fallible_tokens,
};

#[test]
#[tracing_test::traced_test]
fn lenient_recovery_emits_warn_event() {
    let tokens: Vec<Result<_, RecoverableInputError>> = vec![Err(RecoverableInputError::new(
        "<broken>".into(),
        CoreError::Internal("test"),
    ))];
    let _ = process_fallible_tokens::<PlainScopeData, _>(
        tokens,
        &MapDictionary::new(),
        Recovery::Lenient,
    );
    assert!(logs_contain("recovering from input reader error"));
}

#[test]
#[tracing_test::traced_test]
fn engine_creation_emits_debug_event_with_segmentation_strategy() {
    let dict = MapDictionary::new();
    let options = EngineOptions {
        segmentation: SegmentationStrategy::Lattice,
        ..EngineOptions::default()
    };
    let _engine: Engine<PlainScopeData, _> = Engine::with_options(&dict, options);
    assert!(logs_contain("engine created with segmentation strategy"));
}
