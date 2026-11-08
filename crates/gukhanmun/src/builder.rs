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

//! High-level [`Builder`] / [`Converter`] facade.

#[cfg(feature = "html")]
use gukhanmun_core::recover_input_tokens;
use gukhanmun_core::{
    Annotation, ChainDictionary, ContextWindow, DirectiveAction, HanjaDictionary, HanjaVariantSet,
    HomophoneDetection, InputToken, LexemeIndex, NumeralStrategy, PlainScopeData, Recovery,
    RenderMode, RenderOptions, RenderedToken, ScopeData, SegmentationStrategy, SourcePolicy,
    SourceRole, UserDirectives, read_plain_text,
};

use crate::configuration::{BuilderConfiguration, configuration_setters};
use crate::error::Result;
use crate::options::{ConversionOptions, Preset};
use crate::pipeline;

#[cfg(feature = "html")]
use gukhanmun_html::{
    HtmlElementInfo, HtmlReaderOptions, HtmlScopeData, try_read_html_fragment_iter_with_options,
};
#[cfg(feature = "markdown")]
use gukhanmun_markdown::{MarkdownScopeData, MarkdownVariant, read_markdown_iter};

/// Rendering configuration accepted by [`Builder::rendering`].
///
/// A bare [`RenderMode`] replaces the mode while preserving a variant set
/// selected separately through [`Builder::hanja_variant_set`]. A complete
/// [`RenderOptions`] value replaces every rendering option, including the
/// variant set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuilderRendering {
    /// Changes the rendering mode while retaining separately configured axes.
    Mode(RenderMode),

    /// Replaces the complete rendering configuration.
    Options(RenderOptions),
}

impl From<RenderMode> for BuilderRendering {
    fn from(mode: RenderMode) -> Self {
        Self::Mode(mode)
    }
}

impl From<RenderOptions> for BuilderRendering {
    fn from(options: RenderOptions) -> Self {
        Self::Options(options)
    }
}

/// Boxed dictionary used inside the umbrella chain.
type BoxedDictionary<'a> = Box<dyn HanjaDictionary + 'a>;

/// Fluent builder that assembles a [`Converter`] from a [`Preset`] plus
/// overrides.
///
/// All setters take and return `self` by value, so they can be chained
/// directly. Once configured, call [`Builder::build`] to obtain a
/// [`Converter`] that can perform conversions repeatedly without re-running
/// preset resolution.
pub struct Builder<'a> {
    config: BuilderConfiguration,
    dictionaries: Vec<(BoxedDictionary<'a>, SourceRole)>,
    directives: UserDirectives<'a>,
    #[cfg(feature = "html")]
    html_reader_options: HtmlReaderOptions<'a>,
}

impl Default for Builder<'_> {
    fn default() -> Self {
        Self::with_preset(Preset::default())
    }
}

impl<'a> Builder<'a> {
    configuration_setters!();

    /// Creates a new builder with the default preset ([`Preset::KoKr`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new builder seeded from `preset`.
    pub fn with_preset(preset: Preset) -> Self {
        Self {
            config: BuilderConfiguration::with_preset(preset),
            dictionaries: Vec::new(),
            directives: UserDirectives::new(),
            #[cfg(feature = "html")]
            html_reader_options: HtmlReaderOptions::new(),
        }
    }

    /// Appends a user-supplied dictionary to the lookup chain.
    ///
    /// Earlier `push_dictionary` calls take priority over later ones, and
    /// every user dictionary takes priority over the bundled dictionaries when
    /// both are active. Binary evidence roles do not change this authored
    /// override behavior; use [`Self::push_dictionary_with_role`] for cooperation.
    pub fn push_dictionary<D>(mut self, dictionary: D) -> Self
    where
        D: HanjaDictionary + 'a,
    {
        self.dictionaries
            .push((Box::new(dictionary), SourceRole::Override));
        self
    }

    /// Appends a boxed user-supplied dictionary.
    ///
    /// Equivalent to [`Builder::push_dictionary`] when the caller already
    /// owns a `Box<dyn HanjaDictionary>`, for example from a runtime-format
    /// detector.
    pub fn push_boxed_dictionary(mut self, dictionary: BoxedDictionary<'a>) -> Self {
        self.dictionaries.push((dictionary, SourceRole::Override));
        self
    }

    /// Registers a dictionary with an explicit cooperative or override role.
    ///
    /// The supplied role takes precedence over binary evidence metadata. Unlike
    /// this method, [`Self::push_dictionary`] always registers an authored
    /// override. Relative registration order is retained for the ordered policy.
    pub fn push_dictionary_with_role<D>(mut self, dictionary: D, role: SourceRole) -> Self
    where
        D: HanjaDictionary + 'a,
    {
        self.dictionaries.push((Box::new(dictionary), role));
        self
    }

    /// Registers an already boxed dictionary with an explicit source role.
    pub fn push_boxed_dictionary_with_role(
        mut self,
        dictionary: BoxedDictionary<'a>,
        role: SourceRole,
    ) -> Self {
        self.dictionaries.push((dictionary, role));
        self
    }

    /// Adds a literal-hanja user directive.
    pub fn directive(mut self, hanja: impl Into<String>, action: DirectiveAction) -> Self {
        self.directives.add_literal(hanja, action);
        self
    }

    /// Adds a predicate user directive.
    pub fn directive_predicate(
        mut self,
        predicate: impl Fn(&Annotation) -> bool + 'a,
        action: DirectiveAction,
    ) -> Self {
        self.directives.add_predicate(predicate, action);
        self
    }

    /// Replaces the configured user directives wholesale.
    pub fn directives(mut self, directives: UserDirectives<'a>) -> Self {
        self.directives = directives;
        self
    }

    /// Adds an HTML preserve predicate.
    ///
    /// The predicate sees an [`HtmlElementInfo`] per opened element and
    /// returns `true` to preserve the element's subtree verbatim. Replaces
    /// any previously installed predicate; combine multiple conditions inside
    /// a single closure for OR composition.
    #[cfg(feature = "html")]
    pub fn html_preserve_when<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&HtmlElementInfo<'_>) -> bool + 'a,
    {
        self.html_reader_options = HtmlReaderOptions::new().preserve_when(predicate);
        self
    }

    /// Finalizes the configuration into a [`Converter`].
    ///
    /// Returns [`crate::Error::Config`] if the configuration requests a bundled
    /// dictionary whose feature was disabled at compile time.
    pub fn build(self) -> Result<Converter<'a>> {
        let Self {
            config,
            dictionaries,
            directives,
            #[cfg(feature = "html")]
            html_reader_options,
        } = self;
        let (options, dictionary, lexemes) = config
            .assemble(dictionaries, |dictionary| -> BoxedDictionary<'a> {
                dictionary
            })?;
        #[cfg(feature = "html")]
        let html_reader_options = html_reader_options.preserve_unmatched_end_tags(
            options.punctuation != gukhanmun_core::PunctuationProfile::preserve(),
        );
        Ok(Converter {
            options,
            dictionary,
            lexemes,
            directives,
            #[cfg(feature = "html")]
            html_reader_options,
        })
    }
}

/// Immutable conversion runtime produced by [`Builder::build`].
///
/// A `Converter` borrows the resources that the builder collected (the
/// dictionary chain, directives, and HTML reader options) and exposes
/// buffered and streaming conversion methods for plain text, HTML fragments,
/// and Markdown.
pub struct Converter<'a> {
    options: ConversionOptions,
    dictionary: ChainDictionary<BoxedDictionary<'a>>,
    lexemes: LexemeIndex,
    directives: UserDirectives<'a>,
    #[cfg(feature = "html")]
    html_reader_options: HtmlReaderOptions<'a>,
}

impl<'a> Converter<'a> {
    /// Returns the active conversion options.
    pub fn options(&self) -> ConversionOptions {
        self.options
    }

    /// Returns the assembled dictionary chain.
    pub fn dictionary(&self) -> &ChainDictionary<BoxedDictionary<'a>> {
        &self.dictionary
    }

    /// Returns the configured user directives.
    pub fn directives(&self) -> &UserDirectives<'a> {
        &self.directives
    }

    /// Returns the configured HTML reader options.
    ///
    /// This lets streaming callers pair [`gukhanmun_html::HtmlFragmentReader`]
    /// with the same preserve predicates used by the converter's buffered HTML
    /// convenience methods.
    #[cfg(feature = "html")]
    pub fn html_reader_options(&self) -> &HtmlReaderOptions<'a> {
        &self.html_reader_options
    }

    /// Returns the writer options required to serialize normalized Markdown.
    #[cfg(feature = "markdown")]
    pub fn markdown_write_options(&self) -> gukhanmun_markdown::MarkdownWriteOptions {
        pipeline::markdown_write_options(self.options)
    }

    /// Converts a plain-text input and returns the result as a `String`.
    pub fn convert_text_to_string(&self, input: &str) -> Result<String> {
        pipeline::convert_text(
            &self.dictionary,
            self.options,
            &self.lexemes,
            &self.directives,
            input,
        )
    }

    /// Converts an HTML fragment input and returns the result as a `String`.
    ///
    /// Honors the converter's [`Recovery`] setting through the shared
    /// [`recover_input_tokens`] primitive: strict mode rejects the first
    /// malformed region, lenient mode preserves each malformed region verbatim
    /// and continues.
    #[cfg(feature = "html")]
    pub fn convert_html_fragment_to_string(&self, input: &str) -> Result<String> {
        pipeline::convert_html(
            &self.dictionary,
            self.options,
            &self.lexemes,
            &self.directives,
            &self.html_reader_options,
            input,
        )
    }

    /// Converts a Markdown input and returns the result as a `String`.
    ///
    /// The Markdown reader surfaces no recoverable reader errors, so this method
    /// does not consult the converter's [`Recovery`] setting; the returned
    /// `Result` carries only writer/serialization failures. See
    /// [`gukhanmun_markdown::read_markdown`] for the rationale and the future
    /// extension point.
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
            &self.directives,
            input,
            variant,
        )
    }

    /// Streams plain text through the conversion pipeline as a lazy iterator
    /// over rendered tokens.
    ///
    /// Equivalent to building input tokens with
    /// [`read_plain_text`](gukhanmun_core::read_plain_text) and calling
    /// [`Converter::convert_tokens`]. Reader errors are not represented;
    /// callers that need [`Recovery::Strict`] semantics should use
    /// [`Converter::convert_text_to_string`].
    pub fn convert_text_iter<'b>(
        &'b self,
        input: &'b str,
    ) -> impl Iterator<Item = RenderedToken<PlainScopeData>> + 'b {
        self.convert_tokens(read_plain_text(input))
    }

    /// Streams an HTML fragment through the conversion pipeline as a lazy
    /// iterator over rendered tokens.
    ///
    /// Honors the converter's [`Recovery`] setting: with
    /// [`Recovery::Strict`], malformed input is rejected up front and the
    /// caller never sees a partial token stream. With [`Recovery::Lenient`]
    /// the reader recovers in place and the returned iterator drains the
    /// recovered tokens. This one-shot iterator still receives a complete
    /// fragment string; callers with chunked input should drive
    /// [`gukhanmun_html::HtmlFragmentReader`] directly and feed recovered
    /// tokens to [`Converter::convert_tokens`].
    #[cfg(feature = "html")]
    pub fn convert_html_fragment_iter<'b>(
        &'b self,
        input: &'b str,
    ) -> Result<impl Iterator<Item = RenderedToken<HtmlScopeData>> + 'b> {
        let input_tokens = recover_input_tokens(
            try_read_html_fragment_iter_with_options(input, &self.html_reader_options),
            self.options.recovery,
        )?;
        Ok(self.convert_tokens_with_codec(
            input_tokens,
            gukhanmun_html::HtmlPunctuationTextCodec::new(),
        ))
    }

    /// Streams a Markdown input through the conversion pipeline as a lazy
    /// iterator over rendered tokens.
    ///
    /// The Markdown reader does not surface reader-level errors today, so
    /// this method does not propagate the converter's [`Recovery`] setting.
    /// The underlying `pulldown-cmark` parser scans the whole input eagerly;
    /// laziness applies from the engine stage onward.
    #[cfg(feature = "markdown")]
    pub fn convert_markdown_iter<'b>(
        &'b self,
        input: &'b str,
        variant: MarkdownVariant,
    ) -> impl Iterator<Item = RenderedToken<MarkdownScopeData>> + 'b {
        let input_tokens = read_markdown_iter(input, variant);
        self.convert_tokens(input_tokens)
    }

    /// Streams an arbitrary [`InputToken`] sequence through the conversion
    /// pipeline.
    ///
    /// This is the format-agnostic streaming entry point. Pair it with any
    /// reader that produces [`InputToken<S>`] for some [`ScopeData`] `S`—the
    /// umbrella crate ships readers for plain text, HTML fragments, and
    /// Markdown—and consume the returned iterator at the caller's pace.
    ///
    /// The implementation feeds the upstream into a streaming
    /// [`Engine`](gukhanmun_core::Engine)
    /// one token at a time, so the upstream is never drained ahead of demand
    /// (subject to the dictionary lookahead the engine inherently needs).
    /// Middlewares with non-`Off` context windows still buffer per scope or
    /// per document as documented in the design notes.
    pub fn convert_tokens<'b, S, I>(
        &'b self,
        input: I,
    ) -> impl Iterator<Item = RenderedToken<S>> + 'b
    where
        S: ScopeData + 'b,
        I: IntoIterator<Item = InputToken<S>> + 'b,
        I::IntoIter: 'b,
    {
        self.convert_tokens_with_codec(input, gukhanmun_core::LiteralTextCodec)
    }

    /// Converts tokens with a format-owned punctuation codec. HTML callers must
    /// also use safe reader options to keep unmatched tags opaque.
    pub fn convert_tokens_with_codec<'b, S, I, C>(
        &'b self,
        input: I,
        codec: C,
    ) -> impl Iterator<Item = RenderedToken<S>> + 'b
    where
        C: gukhanmun_core::PunctuationTextCodec + 'b,
        S: ScopeData + 'b,
        I: IntoIterator<Item = InputToken<S>> + 'b,
        I::IntoIter: 'b,
    {
        pipeline::convert_tokens_with_codec(
            &self.dictionary,
            self.options,
            &self.lexemes,
            &self.directives,
            input,
            codec,
        )
    }
}
