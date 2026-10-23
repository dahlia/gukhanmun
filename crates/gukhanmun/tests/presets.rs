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
    assert!(Preset::KoKr.includes_bundled_loanwords());
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
    assert!(!Preset::KoKp.includes_bundled_loanwords());
    assert!(Preset::KoKp.includes_bundled_opendict_north_korean());
}

#[test]
fn preset_default_is_ko_kr() {
    assert_eq!(Preset::default(), Preset::KoKr);
}

#[cfg(all(feature = "loanwords", feature = "stdict", feature = "opendict"))]
#[test]
fn ko_kr_defaults_to_loanwords_but_user_dictionaries_win() {
    use gukhanmun::{Builder, ContextWindow, MapDictionary, Preset};
    let selected = Builder::new()
        .homophone_window(ContextWindow::Off)
        .build()
        .unwrap();
    assert_eq!(selected.convert_text_to_string("香港").unwrap(), "홍콩");
    let mut user = MapDictionary::new();
    user.insert("香港", "사용자");
    let selected = Builder::new()
        .push_dictionary(user)
        .homophone_window(ContextWindow::Off)
        .build()
        .unwrap();
    assert_eq!(selected.convert_text_to_string("香港").unwrap(), "사용자");
    let north = Builder::with_preset(Preset::KoKp)
        .homophone_window(ContextWindow::Off)
        .build()
        .unwrap();
    assert_ne!(north.convert_text_to_string("香港").unwrap(), "홍콩");
    let disabled = Builder::new()
        .no_bundled_dictionaries()
        .homophone_window(ContextWindow::Off)
        .build()
        .unwrap();
    assert_ne!(disabled.convert_text_to_string("香港").unwrap(), "홍콩");
    let disabled = Builder::new()
        .no_bundled_loanwords()
        .homophone_window(ContextWindow::Off)
        .build()
        .unwrap();
    assert_eq!(disabled.convert_text_to_string("香港").unwrap(), "샹강");

    let loanwords_only = Builder::new().no_bundled_stdict().build().unwrap();
    assert_eq!(
        loanwords_only.convert_text_to_string("香港").unwrap(),
        "홍콩"
    );

    let north_enabled = Builder::with_preset(Preset::KoKp)
        .bundled_loanwords()
        .homophone_window(ContextWindow::Off)
        .build()
        .unwrap();
    assert_eq!(
        north_enabled.convert_text_to_string("香港").unwrap(),
        "홍콩"
    );
}

#[cfg(not(feature = "loanwords"))]
#[test]
fn explicit_loanwords_without_the_feature_returns_config_error() {
    assert!(matches!(
        gukhanmun::Builder::new()
            .no_bundled_dictionaries()
            .bundled_loanwords()
            .build(),
        Err(gukhanmun::Error::Config(_))
    ));
}

#[cfg(all(not(feature = "loanwords"), feature = "stdict"))]
#[test]
fn ko_kr_without_loanwords_feature_requires_opt_out() {
    use gukhanmun::{Builder, Error};
    assert!(matches!(Builder::new().build(), Err(Error::Config(_))));
    let converter = Builder::new().no_bundled_loanwords().build().unwrap();
    assert_eq!(converter.convert_text_to_string("香港").unwrap(), "샹강");
}
