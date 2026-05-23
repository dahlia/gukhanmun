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

//! Conversion presets and the consolidated option bag.

use gukhanmun_core::{
    ContextWindow, EngineOptions, NumeralStrategy, Recovery, RenderOptions, SegmentationStrategy,
};

/// Conversion preset that selects the orthographic conventions of a Korean
/// variety.
///
/// The variant determines which downstream defaults apply when neither the
/// caller nor a Builder method overrides them: bundled dictionary inclusion,
/// initial sound law, and the default homophone disambiguation window.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Preset {
    /// South Korean convention (한국어 / 韓國語).
    ///
    /// Defaults: bundled *Standard Korean Language Dictionary* enabled, initial
    /// sound law on, per-block homophone disambiguation.
    #[default]
    KoKr,

    /// North Korean convention (조선말 / 朝鮮말).
    ///
    /// Defaults: no bundled dictionary, initial sound law off, no homophone
    /// disambiguation (the orthographic convention writes Sino-Korean words
    /// in hangul without parenthesized hanja).
    KoKp,
}

impl Preset {
    /// Returns the [`ConversionOptions`] this preset chooses.
    pub fn options(self) -> ConversionOptions {
        match self {
            Preset::KoKr => ConversionOptions {
                rendering: RenderOptions::default(),
                engine: EngineOptions {
                    segmentation: SegmentationStrategy::default(),
                    initial_sound_law: true,
                    numeral_strategy: NumeralStrategy::HangulPhonetic,
                },
                homophone_window: ContextWindow::PerBlock,
                first_occurrence_window: ContextWindow::Off,
                recovery: Recovery::Strict,
            },
            Preset::KoKp => ConversionOptions {
                rendering: RenderOptions::default(),
                engine: EngineOptions {
                    segmentation: SegmentationStrategy::default(),
                    initial_sound_law: false,
                    numeral_strategy: NumeralStrategy::HangulPhonetic,
                },
                homophone_window: ContextWindow::Off,
                first_occurrence_window: ContextWindow::Off,
                recovery: Recovery::Strict,
            },
        }
    }

    /// Returns whether the bundled *Standard Korean Language Dictionary*
    /// should be included by default for this preset.
    pub fn includes_bundled_stdict(self) -> bool {
        matches!(self, Preset::KoKr)
    }
}

/// Consolidated option bag carried through the umbrella facade.
///
/// `ConversionOptions` is `#[non_exhaustive]` so additional knobs can be added
/// without a breaking change. Construct it through [`Preset::options`] and
/// adjust individual fields, or use the [`Builder`](crate::Builder) helpers
/// that mutate it for you.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct ConversionOptions {
    /// How annotations are turned into concrete text or markup.
    pub rendering: RenderOptions,

    /// Options forwarded to the conversion engine (segmentation, initial
    /// sound law, numeral strategy).
    pub engine: EngineOptions,

    /// Context window over which homophone disambiguation is computed.
    pub homophone_window: ContextWindow,

    /// Context window for clearing repeated `require_hanja` /
    /// `require_hangul` flags after the first occurrence of each hanja form.
    pub first_occurrence_window: ContextWindow,

    /// How the engine handles reader-level errors.
    pub recovery: Recovery,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Preset::default().options()
    }
}
