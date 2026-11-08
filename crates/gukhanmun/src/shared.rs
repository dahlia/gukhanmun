// Gukhanmun: Owned thread-safe conversion facade for concurrent callers.
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

//! Owned configuration and a compiler-verified thread-safe conversion runtime.

use gukhanmun_core::{
    ChainDictionary, ContextWindow, DirectiveAction, HanjaDictionary, HanjaVariantSet,
    HomophoneDetection, LexemeIndex, NumeralStrategy, Recovery, SegmentationStrategy, SourcePolicy,
    SourceRole, UserDirectives,
};
#[cfg(feature = "html")]
use gukhanmun_html::{HtmlElementInfo, HtmlReaderOptions};
#[cfg(feature = "markdown")]
use gukhanmun_markdown::MarkdownVariant;

use crate::configuration::{BuilderConfiguration, SharedBoxedDictionary, configuration_setters};
use crate::{BuilderRendering, ConversionOptions, Preset, Result, pipeline};

#[cfg(feature = "html")]
type SharedHtmlPredicate = dyn Fn(&HtmlElementInfo<'_>) -> bool + Send + Sync + 'static;

/// Builds an owned [`SharedConverter`] for concurrent conversion calls.
///
/// Presets, configuration precedence and bundled dictionaries match
/// [`Builder`](crate::Builder). Dictionaries and the optional HTML predicate
/// must own their captured data and implement `Send + Sync`. Literal directives
/// are retained as data; unrestricted directive callbacks remain available
/// through the local builder.
///
/// ```
/// use gukhanmun::{MapDictionary, SharedBuilder};
/// use std::sync::Arc;
///
/// let mut dictionary = MapDictionary::new();
/// dictionary.insert("學校", "학교");
/// let converter = Arc::new(SharedBuilder::new()
///     .no_bundled_dictionaries()
///     .push_dictionary(Arc::new(dictionary))
///     .build()?);
/// let shared = Arc::clone(&converter);
/// let result = std::thread::spawn(move || shared.convert_text_to_string("學校"))
///     .join().expect("conversion thread")?;
/// assert_eq!(result, "학교");
/// # Ok::<(), gukhanmun::Error>(())
/// ```
pub struct SharedBuilder {
    config: BuilderConfiguration,
    dictionaries: Vec<(SharedBoxedDictionary, SourceRole)>,
    literals: Vec<(String, DirectiveAction)>,
    #[cfg(feature = "html")]
    html_preserve: Option<Box<SharedHtmlPredicate>>,
}

impl Default for SharedBuilder {
    fn default() -> Self {
        Self::with_preset(Preset::default())
    }
}

impl SharedBuilder {
    configuration_setters!();

    /// Creates a builder using [`Preset::KoKr`].
    ///
    /// The preset requests bundled dictionaries just as the local builder does.
    /// Disable them with [`Self::no_bundled_dictionaries`] when loading explicit
    /// dictionaries or building without the corresponding features.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an owned builder with the same defaults as the local preset.
    pub fn with_preset(preset: Preset) -> Self {
        Self {
            config: BuilderConfiguration::with_preset(preset),
            dictionaries: Vec::new(),
            literals: Vec::new(),
            #[cfg(feature = "html")]
            html_preserve: None,
        }
    }

    /// Appends an owned, thread-safe authored dictionary.
    ///
    /// Earlier dictionaries have priority over later ones and bundled sources.
    /// This registers an override, regardless of embedded evidence roles; use
    /// [`Self::push_dictionary_with_role`] to register a cooperating source.
    /// The dictionary must outlive every conversion without borrowing local data.
    ///
    /// A valid dictionary backed by `Rc` still cannot be shared between threads:
    ///
    /// ```compile_fail
    /// use gukhanmun::{HanjaDictionary, MapDictionary, SharedBuilder};
    /// use gukhanmun_core::Match;
    /// use std::rc::Rc;
    /// struct LocalDictionary(Rc<MapDictionary>);
    /// impl HanjaDictionary for LocalDictionary {
    ///     fn matches_at<'a>(&'a self, input: &'a str)
    ///         -> Box<dyn Iterator<Item = Match> + 'a>
    ///     {
    ///         self.0.matches_at(input)
    ///     }
    /// }
    /// let dictionary = LocalDictionary(Rc::new(MapDictionary::new()));
    /// SharedBuilder::new().push_dictionary(dictionary);
    /// ```
    pub fn push_dictionary<D>(self, dictionary: D) -> Self
    where
        D: HanjaDictionary + Send + Sync + 'static,
    {
        self.push_dictionary_with_role(dictionary, SourceRole::Override)
    }

    /// Appends an already boxed, owned, thread-safe override dictionary.
    ///
    /// This accepts runtime-selected backends without erasing their `Send + Sync`
    /// bounds. Priority is the same as [`Self::push_dictionary`].
    pub fn push_boxed_dictionary(self, dictionary: SharedBoxedDictionary) -> Self {
        self.push_boxed_dictionary_with_role(dictionary, SourceRole::Override)
    }

    /// Appends an owned, thread-safe dictionary with an explicit source role.
    ///
    /// The role overrides embedded metadata. Registration order, nested source
    /// numbering, source policies and leaf-owned phrase evidence are preserved.
    pub fn push_dictionary_with_role<D>(self, dictionary: D, role: SourceRole) -> Self
    where
        D: HanjaDictionary + Send + Sync + 'static,
    {
        self.push_boxed_dictionary_with_role(Box::new(dictionary), role)
    }

    /// Appends a boxed, owned, thread-safe dictionary with an explicit role.
    ///
    /// Equivalent to [`Self::push_dictionary_with_role`] for an erased backend.
    pub fn push_boxed_dictionary_with_role(
        mut self,
        dictionary: SharedBoxedDictionary,
        role: SourceRole,
    ) -> Self {
        self.dictionaries.push((dictionary, role));
        self
    }

    /// Adds an owned literal-hanja directive in registration order.
    ///
    /// Literal rules retain the local facade's lexical identity matching and
    /// run after first-occurrence filtering. Their strings are cloned into a
    /// temporary directive adapter for each conversion. For arbitrary local
    /// callbacks, use [`Builder::directive_predicate`](crate::Builder::directive_predicate).
    pub fn directive(mut self, hanja: impl Into<String>, action: DirectiveAction) -> Self {
        self.literals.push((hanja.into(), action));
        self
    }

    /// Replaces the owned predicate used to preserve HTML element subtrees.
    ///
    /// The closure receives the same element view as the local builder. It may
    /// execute concurrently for different documents and must own its captures.
    /// Combine conditions inside one closure for OR composition. This hook
    /// applies to HTML fragment conversion, matching the local facade.
    ///
    /// ```
    /// # #[cfg(feature = "html")] {
    /// use gukhanmun::SharedBuilder;
    /// let class = String::from("class=\"keep\"");
    /// let converter = SharedBuilder::new()
    ///     .no_bundled_dictionaries()
    ///     .html_preserve_when(move |info| info.raw_attributes.contains(&class))
    ///     .build()?;
    /// assert_eq!(converter.convert_html_fragment_to_string(
    ///     "<p class=\"keep\">學校</p>")?, "<p class=\"keep\">學校</p>");
    /// # } Ok::<(), gukhanmun::Error>(())
    /// ```
    ///
    /// Capturing `Rc` is rejected by the compiler:
    ///
    /// ```compile_fail
    /// use gukhanmun::SharedBuilder;
    /// use std::rc::Rc;
    /// let class = Rc::new(String::from("class=\"keep\""));
    /// SharedBuilder::new().html_preserve_when(move |info| {
    ///     info.raw_attributes.contains(class.as_str())
    /// });
    /// ```
    #[cfg(feature = "html")]
    pub fn html_preserve_when<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&HtmlElementInfo<'_>) -> bool + Send + Sync + 'static,
    {
        self.html_preserve = Some(Box::new(predicate));
        self
    }

    /// Resolves configuration and constructs the immutable shared runtime.
    ///
    /// Bundled loading and lexical-index construction happen here once.
    /// Invalid punctuation and unavailable bundled features return
    /// [`Error::Config`](crate::Error::Config), in the same order as the local
    /// builder. No conversion lock or background worker is introduced.
    pub fn build(self) -> Result<SharedConverter> {
        let (options, dictionary, lexemes) = self
            .config
            .assemble(self.dictionaries, |dictionary| dictionary)?;
        Ok(SharedConverter {
            options,
            dictionary,
            lexemes,
            literals: self.literals,
            #[cfg(feature = "html")]
            html_preserve: self.html_preserve,
        })
    }
}

/// Owned, immutable configuration for concurrent one-shot conversions.
///
/// This runtime is `Send + Sync + 'static` through its field types. Share it
/// through [`std::sync::Arc`] to convert independent documents concurrently.
/// Every call creates its own parser, engine and middleware state, borrowing
/// the same dictionaries and construction-time lexical index. The optional
/// HTML predicate may be invoked concurrently.
///
/// Unlike [`Converter`](crate::Converter), it owns its dictionaries and accepts
/// only literal directives and an owned thread-safe HTML predicate. The local
/// facade remains available for borrowed dictionaries, local callbacks and
/// iterator conversion. Both use the same execution pipeline.
pub struct SharedConverter {
    options: ConversionOptions,
    dictionary: ChainDictionary<SharedBoxedDictionary>,
    lexemes: LexemeIndex,
    literals: Vec<(String, DirectiveAction)>,
    #[cfg(feature = "html")]
    html_preserve: Option<Box<SharedHtmlPredicate>>,
}

impl SharedConverter {
    /// Returns the resolved conversion options, copied from the immutable runtime.
    pub fn options(&self) -> ConversionOptions {
        self.options
    }

    fn directives(&self) -> UserDirectives<'_> {
        let mut directives = UserDirectives::new();
        for (hanja, action) in &self.literals {
            directives.add_literal(hanja.clone(), *action);
        }
        directives
    }

    /// Converts a complete plain-text document with fresh document-local state.
    ///
    /// Returns the same text and errors as the local facade. The result is
    /// buffered; dictionary data and resolved options are reused across calls.
    pub fn convert_text_to_string(&self, input: &str) -> Result<String> {
        pipeline::convert_text(
            &self.dictionary,
            self.options,
            &self.lexemes,
            &self.directives(),
            input,
        )
    }

    /// Converts a complete HTML fragment with fresh document-local state.
    ///
    /// Strict recovery rejects malformed regions; lenient recovery preserves
    /// them verbatim and continues. The reader borrows the configured shared
    /// preserve predicate only for this call.
    #[cfg(feature = "html")]
    pub fn convert_html_fragment_to_string(&self, input: &str) -> Result<String> {
        let mut reader_options = HtmlReaderOptions::new().preserve_unmatched_end_tags(
            self.options.punctuation != gukhanmun_core::PunctuationProfile::preserve(),
        );
        if let Some(predicate) = &self.html_preserve {
            reader_options = reader_options.preserve_when(move |info| predicate(info));
        }
        pipeline::convert_html(
            &self.dictionary,
            self.options,
            &self.lexemes,
            &self.directives(),
            &reader_options,
            input,
        )
    }

    /// Converts a complete Markdown document using the selected parser variant.
    ///
    /// Like the local facade, the current Markdown reader has no recoverable
    /// reader errors; only writer/serialization errors are returned. Each call
    /// creates fresh conversion state and reuses the owned dictionaries.
    #[cfg(feature = "markdown")]
    pub fn convert_markdown_to_string(
        &self,
        input: &str,
        variant: MarkdownVariant,
    ) -> Result<String> {
        pipeline::convert_markdown(
            &self.dictionary,
            self.options,
            &self.lexemes,
            &self.directives(),
            input,
            variant,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Builder, MapDictionary, MatchMark, PhraseCandidateLink, PhraseChunk, PhraseEvidence,
        PhraseRule, PhraseSpelling, ReadingCandidate, ReadingCandidates, SourceId, WordId,
    };

    fn phrase_dictionary() -> MapDictionary {
        let mut dictionary = MapDictionary::new();
        dictionary.insert("無條件", "무조건");
        dictionary.insert_candidates(
            "降服",
            ReadingCandidates::new(vec![
                ReadingCandidate {
                    reading: "강복".into(),
                    suffix_reading: None,
                    mark: MatchMark::default(),
                    preference: 10,
                    alias: None,
                },
                ReadingCandidate {
                    reading: "항복".into(),
                    suffix_reading: None,
                    mark: MatchMark::default(),
                    preference: 0,
                    alias: None,
                },
            ])
            .unwrap(),
        );
        let chunks = [("無條件", "무조건"), ("降服", "항복")]
            .into_iter()
            .map(|(hanja, reading)| {
                PhraseChunk::new(
                    reading.into(),
                    vec![PhraseSpelling::new(
                        hanja.into(),
                        vec![PhraseCandidateLink::new(reading.into(), None)],
                    )],
                )
            })
            .collect();
        dictionary
            .set_phrase_evidence(PhraseEvidence::new(vec![PhraseRule::new(
                vec![WordId(1)],
                chunks,
            )]))
            .unwrap();
        dictionary
    }

    #[test]
    fn assembled_nested_sources_keep_exact_roles_slots_and_phrase_owners() {
        for policy in [
            SourcePolicy::Ordered,
            SourcePolicy::ReferenceAuthorized,
            SourcePolicy::ReferenceFirst,
            SourcePolicy::LoanwordFirst,
        ] {
            for role in [
                SourceRole::Reference,
                SourceRole::Loanword,
                SourceRole::Override,
            ] {
                for reference_only in [false, true] {
                    let mut nested =
                        ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
                    nested.set_reference_only(true);
                    nested.push_with_role(MapDictionary::new(), SourceRole::Override);
                    nested.push_with_role(phrase_dictionary(), SourceRole::Reference);
                    let local = Builder::new()
                        .no_bundled_dictionaries()
                        .source_policy(policy)
                        .reference_only(reference_only)
                        .push_dictionary_with_role(nested.clone(), role)
                        .push_dictionary_with_role(phrase_dictionary(), SourceRole::Reference)
                        .build()
                        .unwrap();
                    let shared = SharedBuilder::new()
                        .no_bundled_dictionaries()
                        .source_policy(policy)
                        .reference_only(reference_only)
                        .push_dictionary_with_role(nested, role)
                        .push_dictionary_with_role(phrase_dictionary(), SourceRole::Reference)
                        .build()
                        .unwrap();
                    let a = local.dictionary();
                    let b = &shared.dictionary;
                    assert_eq!(a.len(), 2);
                    assert_eq!(b.len(), 2);
                    assert_eq!(a.source_count(), 3);
                    assert_eq!(b.source_count(), 3);
                    assert_eq!(a.source_roles(), b.source_roles());
                    assert_eq!(b.source_roles(), [role, SourceRole::Reference]);
                    assert_eq!(a.source_policy(), b.source_policy());
                    assert_eq!(a.reference_only(), b.reference_only());
                    let a_owners = a
                        .owned_phrase_evidence()
                        .into_iter()
                        .map(|entry| entry.owner)
                        .collect::<Vec<_>>();
                    let b_owners = b
                        .owned_phrase_evidence()
                        .into_iter()
                        .map(|entry| entry.owner)
                        .collect::<Vec<_>>();
                    assert_eq!(a_owners, b_owners);
                    assert_eq!(
                        b_owners,
                        if role == SourceRole::Loanword {
                            vec![SourceId(2)]
                        } else {
                            vec![SourceId(1), SourceId(2)]
                        }
                    );
                    for input in ["降服", "無條件", "降服"] {
                        let a_matches = a
                            .source_matches_at(input)
                            .map(|matches| matches.collect::<Vec<_>>());
                        let b_matches = b
                            .source_matches_at(input)
                            .map(|matches| matches.collect::<Vec<_>>());
                        // Complete equality checks anchors, candidate forms, marks,
                        // policy exclusions and opaque projection diagnostics.
                        assert_eq!(a_matches, b_matches);
                    }
                    for input in ["무조건降服", "무조건  降服", "降服"] {
                        assert_eq!(
                            local.convert_text_to_string(input).unwrap(),
                            shared.convert_text_to_string(input).unwrap()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn later_phrase_owner_and_choice_anchor_keep_reserved_opaque_slots() {
        let mut nested = ChainDictionary::new();
        nested.push(MapDictionary::new());
        nested.push(MapDictionary::new());
        let shared = SharedBuilder::new()
            .no_bundled_dictionaries()
            .source_policy(SourcePolicy::ReferenceFirst)
            .push_dictionary_with_role(nested, SourceRole::Loanword)
            .push_dictionary_with_role(phrase_dictionary(), SourceRole::Reference)
            .build()
            .unwrap();
        assert_eq!(shared.dictionary.source_count(), 3);
        assert_eq!(
            shared.dictionary.owned_phrase_evidence()[0].owner,
            SourceId(2)
        );
        let matched = shared
            .dictionary
            .source_matches_at("降服")
            .unwrap()
            .next()
            .unwrap();
        assert!(
            matched
                .ranked
                .iter()
                .all(|choice| choice.anchor.source_id == SourceId(2))
        );
        assert_eq!(
            shared.convert_text_to_string("무조건降服").unwrap(),
            "무조건항복"
        );
    }
}
