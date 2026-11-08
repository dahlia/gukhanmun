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

//! Shared private preset resolution and dictionary assembly.

use crate::{ConversionOptions, Error, Preset, Result};
use gukhanmun_core::{ChainDictionary, HanjaDictionary, LexemeIndex, SourcePolicy, SourceRole};

pub(super) type SharedBoxedDictionary = Box<dyn HanjaDictionary + Send + Sync + 'static>;

pub(super) struct BuilderConfiguration {
    pub(super) options: ConversionOptions,
    pub(super) hanja_variant_set_override: Option<gukhanmun_core::HanjaVariantSet>,
    pub(super) bundled_stdict: bool,
    pub(super) bundled_loanwords: bool,
    pub(super) bundled_opendict_north_korean: bool,
}

impl BuilderConfiguration {
    pub(super) fn with_preset(preset: Preset) -> Self {
        Self {
            options: preset.options(),
            hanja_variant_set_override: None,
            bundled_stdict: preset.includes_bundled_stdict(),
            bundled_loanwords: preset.includes_bundled_loanwords(),
            bundled_opendict_north_korean: preset.includes_bundled_opendict_north_korean(),
        }
    }

    pub(super) fn assemble<D: HanjaDictionary>(
        self,
        mut dictionaries: Vec<(D, SourceRole)>,
        mut into_storage: impl FnMut(SharedBoxedDictionary) -> D,
    ) -> Result<(ConversionOptions, ChainDictionary<D>, LexemeIndex)> {
        self.options
            .punctuation
            .validate()
            .map_err(|e| Error::Config(e.to_string()))?;
        let Self {
            options,
            bundled_stdict,
            bundled_loanwords,
            bundled_opendict_north_korean,
            hanja_variant_set_override: _,
        } = self;
        #[cfg(not(feature = "loanwords"))]
        if bundled_loanwords {
            return Err(Error::Config(
                "bundled loanwords requested but `loanwords` feature is disabled".into(),
            ));
        }
        #[cfg(not(feature = "opendict"))]
        {
            if bundled_opendict_north_korean {
                return Err(Error::Config(
                    "bundled Open Korean Dictionary North Korean vocabulary requested but the \
                     `opendict` feature is disabled"
                        .into(),
                ));
            }
        }
        #[cfg(not(feature = "stdict"))]
        {
            if bundled_stdict {
                return Err(Error::Config(
                    "bundled Standard Korean Language Dictionary requested but the `stdict` \
                     feature is disabled"
                        .into(),
                ));
            }
        }

        #[cfg(any(feature = "opendict", feature = "stdict", feature = "loanwords"))]
        let bundled = {
            let mut dictionaries: Vec<(SharedBoxedDictionary, SourceRole)> = Vec::new();
            #[cfg(feature = "loanwords")]
            if bundled_loanwords {
                dictionaries.push((
                    Box::new(gukhanmun_loanwords::dictionary()),
                    SourceRole::Loanword,
                ));
            }
            #[cfg(feature = "opendict")]
            if bundled_opendict_north_korean {
                dictionaries.push((
                    Box::new(gukhanmun_opendict::north_korean()),
                    SourceRole::Reference,
                ));
            }
            #[cfg(feature = "stdict")]
            if bundled_stdict {
                dictionaries.push((Box::new(gukhanmun_stdict::ko_kr()), SourceRole::Reference));
            }
            dictionaries
        };
        #[cfg(not(any(feature = "opendict", feature = "stdict", feature = "loanwords")))]
        let bundled: Vec<(SharedBoxedDictionary, SourceRole)> = Vec::new();

        dictionaries.extend(
            bundled
                .into_iter()
                .map(|(dictionary, role)| (into_storage(dictionary), role)),
        );
        let mut chain = ChainDictionary::new().with_source_policy(options.source_policy);
        chain.set_reference_only(options.reference_only);
        for (dictionary, role) in dictionaries {
            chain.push_with_role(dictionary, role);
        }
        // Cooperative selections carry their source-local lexical identity.
        // Only ordered resolution needs a merged index for pipeline middleware.
        let lexemes = if options.source_policy == SourcePolicy::Ordered {
            chain
                .lexemes()
                .or_else(|| LexemeIndex::from_dictionary(&chain))
                .unwrap_or_default()
        } else {
            LexemeIndex::default()
        };
        Ok((options, chain, lexemes))
    }
}

// The same setters preserve precedence for both public facades.
macro_rules! configuration_setters {
    () => {
        /// Overrides the [`RenderOptions`] used by the converter.
        ///
        /// Accepts either a bare [`RenderMode`](gukhanmun_core::RenderMode) or a
        /// fully populated [`RenderOptions`] value.
        pub fn rendering(mut self, rendering: impl Into<BuilderRendering>) -> Self {
            match rendering.into() {
                BuilderRendering::Mode(mode) => {
                    self.config.options.rendering = mode.into();
                    if let Some(variant_set) = self.config.hanja_variant_set_override {
                        self.config.options.rendering.hanja_variant_set = variant_set;
                    }
                }
                BuilderRendering::Options(options) => {
                    self.config.options.rendering = options;
                    self.config.hanja_variant_set_override = None;
                }
            }
            self
        }

        /// Selects the hanja variant set used by every rendering mode.
        pub fn hanja_variant_set(mut self, variant_set: HanjaVariantSet) -> Self {
            self.config.hanja_variant_set_override = Some(variant_set);
            self.config.options.rendering.hanja_variant_set = variant_set;
            self
        }

        /// Selects cooperation between reference and loanword dictionaries.
        ///
        /// Defaults to [`SourcePolicy::ReferenceAuthorized`]. Select
        /// [`SourcePolicy::Ordered`] to restore the original dictionary-order policy.
        pub fn source_policy(mut self, policy: SourcePolicy) -> Self {
            self.config.options.source_policy = policy;
            self
        }

        /// Excludes independent loanword choices, retaining reference-authorized
        /// realizations. The legacy ordered policy ignores this eligibility filter.
        pub fn reference_only(mut self, enabled: bool) -> Self {
            self.config.options.reference_only = enabled;
            self
        }

        /// Overrides the engine's lattice / eager segmentation strategy.
        pub fn segmentation(mut self, strategy: SegmentationStrategy) -> Self {
            self.config.options.engine.segmentation = strategy;
            self
        }

        /// Overrides the engine's hanja-numeral strategy.
        pub fn numerals(mut self, strategy: NumeralStrategy) -> Self {
            self.config.options.engine.numeral_strategy = strategy;
            self
        }

        /// Enables or disables the South Korean initial sound law for fallback
        /// readings.
        pub fn initial_sound_law(mut self, enabled: bool) -> Self {
            self.config.options.engine.initial_sound_law = enabled;
            self
        }

        /// Sets the homophone disambiguation context window.
        pub fn homophone_window(mut self, window: ContextWindow) -> Self {
            self.config.options.homophone_window = window;
            self
        }

        /// Sets the homophone detection strategy.
        ///
        /// Defaults to [`HomophoneDetection::ContextLocal`]; pass
        /// [`HomophoneDetection::DictionaryWide`] to gloss readings shared by other
        /// dictionary entries even when no homophone appears in the text.
        pub fn homophone_detection(mut self, detection: HomophoneDetection) -> Self {
            self.config.options.homophone_detection = detection;
            self
        }

        /// Sets the first-occurrence reset context window.
        pub fn first_occurrence_window(mut self, window: ContextWindow) -> Self {
            self.config.options.first_occurrence_window = window;
            self
        }

        /// Enables or disables collapsing of redundant parenthetical reading
        /// annotations.
        ///
        /// Enabled by default.  When enabled, an explicit gloss such as `庫間(곳간)`
        /// or `곳간(庫間)` is recognised, the redundant parenthetical is removed, and
        /// the annotation is shown in both scripts in every render mode; a
        /// parenthetical that pins an alternative reading (for example `數字(수자)`)
        /// overrides the dictionary reading for that occurrence.
        pub fn collapse_redundant_parens(mut self, enabled: bool) -> Self {
            self.config.options.collapse_redundant_parens = enabled;
            self
        }

        /// Sets the reader-error recovery policy.
        pub fn recovery(mut self, recovery: Recovery) -> Self {
            self.config.options.recovery = recovery;
            self
        }

        /// Disables inclusion of all bundled dictionaries.
        ///
        /// Useful when constructing a converter that should consult only
        /// user-supplied dictionaries.
        pub fn no_bundled_dictionaries(mut self) -> Self {
            self.config.bundled_stdict = false;
            self.config.bundled_loanwords = false;
            self.config.bundled_opendict_north_korean = false;
            self
        }

        /// Disables inclusion of the bundled *Standard Korean Language Dictionary*.
        ///
        /// This leaves other bundled dictionaries selected by the preset enabled.
        /// Use [`Self::no_bundled_dictionaries`] to disable every bundled
        /// dictionary.
        pub fn no_bundled_stdict(mut self) -> Self {
            self.config.bundled_stdict = false;
            self
        }

        /// Disables inclusion of bundled Open Korean Dictionary data.
        ///
        /// This leaves other bundled dictionaries selected by the preset enabled.
        /// Use [`Self::no_bundled_dictionaries`] to disable every bundled
        /// dictionary.
        pub fn no_bundled_opendict(mut self) -> Self {
            self.config.bundled_opendict_north_korean = false;
            self
        }

        /// Forces inclusion of the bundled *Standard Korean Language Dictionary*.
        ///
        /// Requires the `stdict` feature. Returns the builder unchanged; the
        /// missing feature is reported from [`Self::build`] as
        /// [`crate::Error::Config`].
        pub fn bundled_stdict(mut self) -> Self {
            self.config.bundled_stdict = true;
            self
        }

        /// Enables bundled loanwords ahead of preset dictionaries, after user dictionaries.
        /// Enabled by default for [`Preset::KoKr`], disabled for [`Preset::KoKp`].
        /// Requires the `loanwords` feature; otherwise [`Self::build`] returns
        /// [`crate::Error::Config`].
        pub fn bundled_loanwords(mut self) -> Self {
            self.config.bundled_loanwords = true;
            self
        }

        /// Disables only bundled loanwords while retaining the preset dictionaries.
        pub fn no_bundled_loanwords(mut self) -> Self {
            self.config.bundled_loanwords = false;
            self
        }

        /// Selects an output punctuation profile independently of the language preset.
        /// Invalid custom marks are rejected by [`Self::build`].
        pub fn punctuation(mut self, profile: gukhanmun_core::PunctuationProfile) -> Self {
            self.config.options.punctuation = profile;
            self
        }
    };
}
pub(super) use configuration_setters;
