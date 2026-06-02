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

//! Preset contract tests: the `Preset::options()` snapshot must match the
//! values the CLI's `resolve_options` historically produced.

use gukhanmun::{ContextWindow, NumeralStrategy, Preset, Recovery, SegmentationStrategy};

#[test]
fn ko_kr_preset_defaults_match_design_table() {
    let options = Preset::KoKr.options();
    assert_eq!(options.engine.segmentation, SegmentationStrategy::Lattice);
    assert!(options.engine.initial_sound_law);
    assert_eq!(
        options.engine.numeral_strategy,
        NumeralStrategy::HangulPhonetic
    );
    assert_eq!(options.homophone_window, ContextWindow::PerBlock);
    assert_eq!(options.first_occurrence_window, ContextWindow::Off);
    assert_eq!(options.recovery, Recovery::Strict);
    assert!(Preset::KoKr.includes_bundled_stdict());
    assert!(!Preset::KoKr.includes_bundled_opendict_north_korean());
}

#[test]
fn ko_kp_preset_disables_initial_sound_law_and_includes_north_korean_opendict() {
    let options = Preset::KoKp.options();
    assert_eq!(options.engine.segmentation, SegmentationStrategy::Lattice);
    assert!(!options.engine.initial_sound_law);
    assert_eq!(
        options.engine.numeral_strategy,
        NumeralStrategy::HangulPhonetic
    );
    assert_eq!(options.homophone_window, ContextWindow::Off);
    assert_eq!(options.first_occurrence_window, ContextWindow::Off);
    assert_eq!(options.recovery, Recovery::Strict);
    assert!(!Preset::KoKp.includes_bundled_stdict());
    assert!(Preset::KoKp.includes_bundled_opendict_north_korean());
}

#[test]
fn preset_default_is_ko_kr() {
    assert_eq!(Preset::default(), Preset::KoKr);
}
