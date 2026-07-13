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
    Annotation, ChainDictionary, ContextWindow, DirectiveAction, Engine, FirstOccurrenceFilter,
    HanjaDictionary, HanjaVariantSet, HomophoneDetection, HomophoneMarker, InputToken,
    NumeralStrategy, OutputToken, PlainScopeData, Recovery, RedundantParenCollapser, RenderMode,
    RenderOptions, RenderedToken, ScopeData, SegmentationStrategy, UserDirectives,
    apply_user_directives, apply_user_directives_iter, collapse_redundant_parens,
    filter_first_occurrences, mark_homophones_with_detection, process_tokens_iter_with_options,
    read_plain_text, render_tokens_iter, write_plain_text,
};

#[cfg(any(not(feature = "opendict"), not(feature = "stdict")))]
use crate::error::Error;
use crate::error::Result;
use crate::options::{ConversionOptions, Preset};

#[cfg(feature = "html")]
use gukhanmun_html::{
    HtmlElementInfo, HtmlReaderOptions, HtmlScopeData, try_read_html_fragment_iter_with_options,
    write_html_fragment,
};
#[cfg(feature = "markdown")]
use gukhanmun_markdown::{MarkdownScopeData, MarkdownVariant, read_markdown_iter, write_markdown};

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

/// Adapter iterator that wraps a streaming [`Engine`] as
/// `Iterator<Item = OutputToken<S>>` over an arbitrary input-token source.
///
/// Calls [`Engine::push_token`] one input token at a time, buffers whatever
/// the engine reports as ready to emit, and finally drains [`Engine::finish`]
/// when the upstream is exhausted. The wrapper is what lets
/// [`Converter::convert_tokens`] propagate output without first collecting the
/// entire upstream into a `Vec`.
struct EngineIter<'a, S, D, I>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized + 'a,
    I: Iterator<Item = InputToken<S>>,
{
    upstream: I,
    engine: Option<Engine<'a, S, D>>,
    buffer: std::vec::IntoIter<OutputToken<S>>,
}

impl<'a, S, D, I> Iterator for EngineIter<'a, S, D, I>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized + 'a,
    I: Iterator<Item = InputToken<S>>,
{
    type Item = OutputToken<S>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(token) = self.buffer.next() {
                return Some(token);
            }
            let engine = self.engine.as_mut()?;
            if let Some(input) = self.upstream.next() {
                let produced = engine.push_token(input);
                self.buffer = produced.into_iter();
                continue;
            }
            let engine = self.engine.take().expect("engine present");
            self.buffer = engine.finish().into_iter();
        }
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
    options: ConversionOptions,
    hanja_variant_set_override: Option<HanjaVariantSet>,
    bundled_stdict: bool,
    bundled_opendict_north_korean: bool,
    dictionaries: Vec<BoxedDictionary<'a>>,
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
    /// Creates a new builder with the default preset ([`Preset::KoKr`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new builder seeded from `preset`.
    pub fn with_preset(preset: Preset) -> Self {
        Self {
            options: preset.options(),
            hanja_variant_set_override: None,
            bundled_stdict: preset.includes_bundled_stdict(),
            bundled_opendict_north_korean: preset.includes_bundled_opendict_north_korean(),
            dictionaries: Vec::new(),
            directives: UserDirectives::new(),
            #[cfg(feature = "html")]
            html_reader_options: HtmlReaderOptions::new(),
        }
    }

    /// Overrides the [`RenderOptions`] used by the converter.
    ///
    /// Accepts either a bare [`RenderMode`](gukhanmun_core::RenderMode) or a
    /// fully populated [`RenderOptions`] value.
    pub fn rendering(mut self, rendering: impl Into<BuilderRendering>) -> Self {
        match rendering.into() {
            BuilderRendering::Mode(mode) => {
                self.options.rendering = mode.into();
                if let Some(variant_set) = self.hanja_variant_set_override {
                    self.options.rendering.hanja_variant_set = variant_set;
                }
            }
            BuilderRendering::Options(options) => {
                self.options.rendering = options;
                self.hanja_variant_set_override = None;
            }
        }
        self
    }

    /// Selects the hanja variant set used by every rendering mode.
    pub fn hanja_variant_set(mut self, variant_set: HanjaVariantSet) -> Self {
        self.hanja_variant_set_override = Some(variant_set);
        self.options.rendering.hanja_variant_set = variant_set;
        self
    }

    /// Overrides the engine's lattice / eager segmentation strategy.
    pub fn segmentation(mut self, strategy: SegmentationStrategy) -> Self {
        self.options.engine.segmentation = strategy;
        self
    }

    /// Overrides the engine's hanja-numeral strategy.
    pub fn numerals(mut self, strategy: NumeralStrategy) -> Self {
        self.options.engine.numeral_strategy = strategy;
        self
    }

    /// Enables or disables the South Korean initial sound law for fallback
    /// readings.
    pub fn initial_sound_law(mut self, enabled: bool) -> Self {
        self.options.engine.initial_sound_law = enabled;
        self
    }

    /// Sets the homophone disambiguation context window.
    pub fn homophone_window(mut self, window: ContextWindow) -> Self {
        self.options.homophone_window = window;
        self
    }

    /// Sets the homophone detection strategy.
    ///
    /// Defaults to [`HomophoneDetection::ContextLocal`]; pass
    /// [`HomophoneDetection::DictionaryWide`] to gloss readings shared by other
    /// dictionary entries even when no homophone appears in the text.
    pub fn homophone_detection(mut self, detection: HomophoneDetection) -> Self {
        self.options.homophone_detection = detection;
        self
    }

    /// Sets the first-occurrence reset context window.
    pub fn first_occurrence_window(mut self, window: ContextWindow) -> Self {
        self.options.first_occurrence_window = window;
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
        self.options.collapse_redundant_parens = enabled;
        self
    }

    /// Sets the reader-error recovery policy.
    pub fn recovery(mut self, recovery: Recovery) -> Self {
        self.options.recovery = recovery;
        self
    }

    /// Disables inclusion of all bundled dictionaries.
    ///
    /// Useful when constructing a converter that should consult only
    /// user-supplied dictionaries.
    pub fn no_bundled_dictionaries(mut self) -> Self {
        self.bundled_stdict = false;
        self.bundled_opendict_north_korean = false;
        self
    }

    /// Disables inclusion of the bundled *Standard Korean Language Dictionary*.
    ///
    /// This leaves other bundled dictionaries selected by the preset enabled.
    /// Use [`Builder::no_bundled_dictionaries`] to disable every bundled
    /// dictionary.
    pub fn no_bundled_stdict(mut self) -> Self {
        self.bundled_stdict = false;
        self
    }

    /// Disables inclusion of bundled Open Korean Dictionary data.
    ///
    /// This leaves other bundled dictionaries selected by the preset enabled.
    /// Use [`Builder::no_bundled_dictionaries`] to disable every bundled
    /// dictionary.
    pub fn no_bundled_opendict(mut self) -> Self {
        self.bundled_opendict_north_korean = false;
        self
    }

    /// Forces inclusion of the bundled *Standard Korean Language Dictionary*.
    ///
    /// Requires the `stdict` feature. Returns the builder unchanged; the
    /// missing feature is reported from [`Builder::build`] as
    /// [`crate::Error::Config`].
    pub fn bundled_stdict(mut self) -> Self {
        self.bundled_stdict = true;
        self
    }

    /// Appends a user-supplied dictionary to the lookup chain.
    ///
    /// Earlier `push_dictionary` calls take priority over later ones, and
    /// every user dictionary takes priority over the bundled dictionaries when
    /// both are active.
    pub fn push_dictionary<D>(mut self, dictionary: D) -> Self
    where
        D: HanjaDictionary + 'a,
    {
        self.dictionaries.push(Box::new(dictionary));
        self
    }

    /// Appends a boxed user-supplied dictionary.
    ///
    /// Equivalent to [`Builder::push_dictionary`] when the caller already
    /// owns a `Box<dyn HanjaDictionary>`, for example from a runtime-format
    /// detector.
    pub fn push_boxed_dictionary(mut self, dictionary: BoxedDictionary<'a>) -> Self {
        self.dictionaries.push(dictionary);
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
            options,
            hanja_variant_set_override: _,
            bundled_stdict,
            bundled_opendict_north_korean,
            dictionaries,
            directives,
            #[cfg(feature = "html")]
            html_reader_options,
        } = self;

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

        #[cfg(any(feature = "opendict", feature = "stdict"))]
        let dictionaries = {
            let mut dictionaries = dictionaries;
            #[cfg(feature = "opendict")]
            if bundled_opendict_north_korean {
                dictionaries.push(Box::new(gukhanmun_opendict::north_korean()));
            }
            #[cfg(feature = "stdict")]
            if bundled_stdict {
                dictionaries.push(Box::new(gukhanmun_stdict::ko_kr()));
            }
            dictionaries
        };
        #[cfg(not(any(feature = "opendict", feature = "stdict")))]
        let dictionaries = dictionaries;

        let chain = ChainDictionary::from_iter(dictionaries);
        Ok(Converter {
            options,
            dictionary: chain,
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

    /// Converts a plain-text input and returns the result as a `String`.
    pub fn convert_text_to_string(&self, input: &str) -> Result<String> {
        let input_tokens = read_plain_text(input);
        let rendered = self.run_buffered(input_tokens);
        Ok(write_plain_text(rendered))
    }

    /// Converts an HTML fragment input and returns the result as a `String`.
    ///
    /// Honors the converter's [`Recovery`] setting through the shared
    /// [`recover_input_tokens`] primitive: strict mode rejects the first
    /// malformed region, lenient mode preserves each malformed region verbatim
    /// and continues.
    #[cfg(feature = "html")]
    pub fn convert_html_fragment_to_string(&self, input: &str) -> Result<String> {
        let input_tokens = recover_input_tokens(
            try_read_html_fragment_iter_with_options(input, &self.html_reader_options),
            self.options.recovery,
        )?;
        let rendered = self.run_buffered(input_tokens);
        Ok(write_html_fragment(rendered))
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
        let input_tokens = gukhanmun_markdown::read_markdown(input, variant);
        let rendered = self.run_buffered(input_tokens);
        Ok(write_markdown(rendered)?)
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
        Ok(self.convert_tokens(input_tokens))
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
    /// The implementation feeds the upstream into a streaming [`Engine`]
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
        let engine_iter = EngineIter {
            upstream: input.into_iter(),
            engine: Some(Engine::<S, _>::with_options(
                &self.dictionary,
                self.options.engine,
            )),
            buffer: Vec::new().into_iter(),
        };
        // Runs first, immediately after the engine, so later stages observe the
        // corrected reading and presentation flags.
        let collapse_iter = MiddlewareIter::new(
            engine_iter,
            RedundantParenCollapser::new(self.options.collapse_redundant_parens),
            RedundantParenCollapser::push_token,
            RedundantParenCollapser::finish,
        );
        let homophone_iter = MiddlewareIter::new(
            collapse_iter,
            HomophoneMarker::with_detection(
                &self.dictionary,
                self.options.homophone_window,
                self.options.homophone_detection,
            ),
            HomophoneMarker::push_token,
            HomophoneMarker::finish,
        );
        let first_occurrence_iter = MiddlewareIter::new(
            homophone_iter,
            FirstOccurrenceFilter::new(self.options.first_occurrence_window),
            FirstOccurrenceFilter::push_token,
            FirstOccurrenceFilter::finish,
        );
        let directives_iter = apply_user_directives_iter(first_occurrence_iter, &self.directives);
        render_tokens_iter(directives_iter, self.options.rendering)
    }

    fn run_buffered<S>(
        &self,
        input_tokens: impl IntoIterator<Item = InputToken<S>>,
    ) -> Vec<RenderedToken<S>>
    where
        S: ScopeData,
    {
        let output_tokens =
            process_tokens_iter_with_options(input_tokens, &self.dictionary, self.options.engine);
        let output_tokens =
            collapse_redundant_parens(output_tokens, self.options.collapse_redundant_parens);
        let output_tokens = mark_homophones_with_detection(
            output_tokens,
            &self.dictionary,
            self.options.homophone_window,
            self.options.homophone_detection,
        );
        let output_tokens =
            filter_first_occurrences(output_tokens, self.options.first_occurrence_window);
        let output_tokens = apply_user_directives(output_tokens, &self.directives);
        render_tokens_iter(output_tokens, self.options.rendering).collect()
    }
}

/// Adapter iterator that turns a `push_token` / `finish` middleware pair into
/// a lazy `Iterator<Item = OutputToken<S>>`.
struct MiddlewareIter<I, M, S, P, F>
where
    I: Iterator<Item = OutputToken<S>>,
    P: FnMut(&mut M, OutputToken<S>) -> Vec<OutputToken<S>>,
    F: FnOnce(M) -> Vec<OutputToken<S>>,
    S: ScopeData,
{
    upstream: I,
    middleware: Option<M>,
    push: P,
    finish: Option<F>,
    buffer: std::vec::IntoIter<OutputToken<S>>,
}

impl<I, M, S, P, F> MiddlewareIter<I, M, S, P, F>
where
    I: Iterator<Item = OutputToken<S>>,
    P: FnMut(&mut M, OutputToken<S>) -> Vec<OutputToken<S>>,
    F: FnOnce(M) -> Vec<OutputToken<S>>,
    S: ScopeData,
{
    fn new(upstream: I, middleware: M, push: P, finish: F) -> Self {
        Self {
            upstream,
            middleware: Some(middleware),
            push,
            finish: Some(finish),
            buffer: Vec::new().into_iter(),
        }
    }
}

impl<I, M, S, P, F> Iterator for MiddlewareIter<I, M, S, P, F>
where
    I: Iterator<Item = OutputToken<S>>,
    P: FnMut(&mut M, OutputToken<S>) -> Vec<OutputToken<S>>,
    F: FnOnce(M) -> Vec<OutputToken<S>>,
    S: ScopeData,
{
    type Item = OutputToken<S>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(token) = self.buffer.next() {
                return Some(token);
            }
            let middleware = self.middleware.as_mut()?;
            if let Some(input) = self.upstream.next() {
                let produced = (self.push)(middleware, input);
                self.buffer = produced.into_iter();
                continue;
            }
            let middleware = self.middleware.take().expect("middleware present");
            let finish = self.finish.take().expect("finish callback present");
            self.buffer = finish(middleware).into_iter();
        }
    }
}
