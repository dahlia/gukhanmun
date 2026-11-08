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

//! Borrowing execution helpers shared by local and owning facades.

use crate::{ConversionOptions, Result};
use gukhanmun_core::{
    Engine, FirstOccurrenceFilter, HanjaDictionary, HomophoneMarker, InputToken, LexemeIndex,
    OutputToken, RedundantParenCollapser, RenderedToken, ScopeData, UserDirectives,
    apply_user_directives_iter_with_lexemes, read_plain_text, render_tokens_iter, write_plain_text,
};
#[cfg(feature = "html")]
use gukhanmun_html::HtmlReaderOptions;
#[cfg(feature = "markdown")]
use gukhanmun_markdown::MarkdownVariant;

/// Adapter iterator that wraps a streaming [`Engine`] as
/// `Iterator<Item = OutputToken<S>>` over an arbitrary input-token source.
///
/// Calls [`Engine::push_token`] one input token at a time, buffers whatever
/// the engine reports as ready to emit, and finally drains [`Engine::finish`]
/// when the upstream is exhausted. The wrapper is what lets
/// [`crate::Converter::convert_tokens`] propagate output without first collecting the
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

pub(super) fn convert_tokens_with_codec<'a, S, I, C, D>(
    dictionary: &'a D,
    options: ConversionOptions,
    lexemes: &'a LexemeIndex,
    directives: &'a UserDirectives<'a>,
    input: I,
    codec: C,
) -> impl Iterator<Item = RenderedToken<S>> + 'a
where
    D: HanjaDictionary + ?Sized + 'a,
    C: gukhanmun_core::PunctuationTextCodec + 'a,
    S: ScopeData + 'a,
    I: IntoIterator<Item = InputToken<S>> + 'a,
    I::IntoIter: 'a,
{
    let engine_iter = EngineIter {
        upstream: input.into_iter(),
        engine: Some(Engine::<S, _>::with_options(dictionary, options.engine)),
        buffer: Vec::new().into_iter(),
    };
    // Runs first, immediately after the engine, so later stages observe the
    // corrected reading and presentation flags.
    let collapse_iter = MiddlewareIter::new(
        engine_iter,
        RedundantParenCollapser::new(options.collapse_redundant_parens),
        RedundantParenCollapser::push_token,
        RedundantParenCollapser::finish,
    );
    let homophone_iter = MiddlewareIter::new(
        collapse_iter,
        HomophoneMarker::with_detection_and_lexemes(
            dictionary,
            options.homophone_window,
            options.homophone_detection,
            lexemes,
        ),
        HomophoneMarker::push_token,
        HomophoneMarker::finish,
    );
    let first_occurrence_iter = MiddlewareIter::new(
        homophone_iter,
        FirstOccurrenceFilter::with_lexemes(options.first_occurrence_window, lexemes.clone()),
        FirstOccurrenceFilter::push_token,
        FirstOccurrenceFilter::finish,
    );
    let directives_iter =
        apply_user_directives_iter_with_lexemes(first_occurrence_iter, directives, lexemes.clone());
    let punctuation_iter = MiddlewareIter::new(
        directives_iter,
        gukhanmun_core::PunctuationNormalizer::with_codec(options.punctuation, codec),
        gukhanmun_core::PunctuationNormalizer::push_token,
        gukhanmun_core::PunctuationNormalizer::finish,
    );
    render_tokens_iter(punctuation_iter, options.rendering)
}

pub(super) fn convert_text<D: HanjaDictionary + ?Sized>(
    dictionary: &D,
    options: ConversionOptions,
    lexemes: &LexemeIndex,
    directives: &UserDirectives<'_>,
    input: &str,
) -> Result<String> {
    let rendered = convert_tokens_with_codec(
        dictionary,
        options,
        lexemes,
        directives,
        read_plain_text(input),
        gukhanmun_core::LiteralTextCodec,
    )
    .collect::<Vec<_>>();
    Ok(write_plain_text(rendered))
}

#[cfg(feature = "html")]
pub(super) fn convert_html<D: HanjaDictionary + ?Sized>(
    dictionary: &D,
    options: ConversionOptions,
    lexemes: &LexemeIndex,
    directives: &UserDirectives<'_>,
    reader_options: &HtmlReaderOptions<'_>,
    input: &str,
) -> Result<String> {
    let input_tokens = gukhanmun_core::recover_input_tokens(
        gukhanmun_html::try_read_html_fragment_iter_with_options(input, reader_options),
        options.recovery,
    )?;
    let rendered = convert_tokens_with_codec(
        dictionary,
        options,
        lexemes,
        directives,
        input_tokens,
        gukhanmun_html::HtmlPunctuationTextCodec::new(),
    );
    Ok(gukhanmun_html::write_html_fragment(rendered))
}

#[cfg(feature = "markdown")]
pub(super) fn markdown_write_options(
    options: ConversionOptions,
) -> gukhanmun_markdown::MarkdownWriteOptions {
    gukhanmun_markdown::MarkdownWriteOptions {
        protect_text_syntax: options.punctuation != gukhanmun_core::PunctuationProfile::preserve(),
    }
}

#[cfg(feature = "markdown")]
pub(super) fn convert_markdown<D: HanjaDictionary + ?Sized>(
    dictionary: &D,
    options: ConversionOptions,
    lexemes: &LexemeIndex,
    directives: &UserDirectives<'_>,
    input: &str,
    variant: MarkdownVariant,
) -> Result<String> {
    let rendered = convert_tokens_with_codec(
        dictionary,
        options,
        lexemes,
        directives,
        gukhanmun_markdown::read_markdown(input, variant),
        gukhanmun_core::LiteralTextCodec,
    )
    .collect::<Vec<_>>();
    Ok(gukhanmun_markdown::write_markdown_with_options(
        rendered,
        markdown_write_options(options),
    )?)
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
