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

//! Core types and algorithms for Gukhanmun.
//!
//! This crate is the home for the format-neutral intermediate representation,
//! conversion engine, dictionary traits, lattice segmentation, and fallback
//! hanja reading logic. Format adapters, command-line I/O, and language
//! bindings live in separate crates.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

extern crate alloc;

mod fallback;
mod generated;
mod segment;

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use fallback::{
    FallbackPart, FallbackState, fallback_reading_for_run, phoneticize_fallback_run_with_state,
};
use generated::unihan_readings::KHANGUL_READINGS;
use segment::{Segment, segment_text};

/// Error returned by fallible core pipeline entry points.
///
/// The core engine is mostly infallible today because dictionary lookup is a
/// synchronous trait contract. This type is still the common structured error
/// surface for reader/engine/writer boundaries and for future engine
/// invariants that callers may need to inspect.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Loading or preparing a dictionary failed before conversion could run.
    #[error("dictionary load failed: {0}")]
    DictionaryLoad(String),

    /// Lattice segmentation failed for a specific source string.
    #[error("segmentation failed for {hanja:?}: {reason}")]
    Segmentation {
        /// The hanja source span that could not be segmented.
        hanja: String,

        /// Human-readable reason for the segmentation failure.
        reason: String,
    },

    /// A dictionary or fallback path produced a reading that is not accepted.
    #[error("invalid hangul reading {reading:?} for hanja {hanja:?}")]
    InvalidReading {
        /// The hanja source string associated with the reading.
        hanja: String,

        /// The rejected hangul reading.
        reading: String,
    },

    /// An internal invariant was violated.
    #[error("internal invariant violated: {0}")]
    Internal(&'static str),

    /// A boxed error from an extension point that has no more specific core
    /// variant yet.
    #[error(transparent)]
    Other(#[from] Box<dyn core::error::Error + Send + Sync + 'static>),
}

/// Stream-level error recovery policy.
///
/// `Strict` is the default and returns the first recoverable reader error.
/// `Lenient` logs the error and emits the original unrecognized region as a
/// verbatim token so downstream tokens can continue flowing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Recovery {
    /// Return the first reader, engine, or writer error and stop processing.
    #[default]
    Strict,

    /// Preserve recoverable bad input regions and continue processing.
    Lenient,
}

/// A recoverable reader error plus the original source region.
///
/// Readers use this value when they can identify a malformed region and know
/// how to preserve its source bytes or text in lenient mode. Strict mode
/// returns the stored error directly.
#[derive(Debug)]
pub struct RecoverableInputError {
    original: String,
    error: Error,
}

impl RecoverableInputError {
    /// Creates a recoverable input error from original source and cause.
    pub fn new(original: String, error: Error) -> Self {
        Self { original, error }
    }

    /// Returns the original source region that can be preserved in lenient
    /// mode.
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Returns the structured error describing why the region was rejected.
    pub fn error(&self) -> &Error {
        &self.error
    }

    /// Consumes the error and returns the original source plus cause.
    pub fn into_parts(self) -> (String, Error) {
        (self.original, self.error)
    }
}

/// Adapter-owned data attached to an intermediate-representation scope.
///
/// The engine treats this trait as an opaque policy boundary. Format adapters
/// can encode HTML elements, Markdown events, or plain-text scopes in the
/// concrete type, while the engine only asks whether text should be preserved
/// and whether later stages may insert inline markup.
pub trait ScopeData: Clone + 'static {
    /// Returns whether text inside this scope must pass through untouched.
    fn is_preserve(&self) -> bool;

    /// Returns whether inline markup may be inserted inside this scope.
    fn allows_inline_markup(&self) -> bool {
        true
    }

    /// Returns whether this scope resets block-oriented stateful stages.
    fn is_block_boundary(&self) -> bool {
        false
    }
}

/// A structural scope in the format-neutral token stream.
///
/// `Scope` carries only adapter-owned data. The engine may clone and stack
/// scopes, but it does not inspect the concrete data beyond the `ScopeData`
/// methods.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scope<S> {
    data: S,
}

impl<S> Scope<S> {
    /// Creates a scope from adapter-specific data.
    pub fn new(data: S) -> Self {
        Self { data }
    }

    /// Returns a shared reference to the adapter-specific scope data.
    pub fn data(&self) -> &S {
        &self.data
    }

    /// Consumes the scope and returns its adapter-specific data.
    pub fn into_data(self) -> S {
        self.data
    }
}

/// A token emitted by a reader before hanja conversion has run.
///
/// This type intentionally has no annotation variant: annotations are produced
/// by the engine and consumed by renderers, so input adapters cannot inject
/// already-converted positions into the stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputToken<S> {
    /// Enters a structural scope.
    Open(Scope<S>),

    /// Leaves the most recent structural scope.
    Close,

    /// Text that the engine may convert unless a preserving scope is active.
    Text(String),

    /// Text that must pass through untouched.
    Verbatim(String),
}

/// A token emitted by the engine after hanja conversion.
///
/// Most tokens pass through from `InputToken`, but converted dictionary matches
/// become `Annotated` so middlewares and renderers can choose their final
/// surface form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputToken<S> {
    /// Enters a structural scope.
    Open(Scope<S>),

    /// Leaves the most recent structural scope.
    Close,

    /// Text that needs no annotation-aware rendering.
    Text(String),

    /// Text that must pass through untouched.
    Verbatim(String),

    /// A converted hanja word plus metadata for later stages.
    Annotated(Annotation),
}

/// A token emitted by a renderer after all annotations have been expanded.
///
/// Writers consume this stream because it cannot contain unrendered
/// annotations. That makes the renderer-to-writer boundary explicit in the type
/// system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenderedToken<S> {
    /// Enters a structural scope.
    Open(Scope<S>),

    /// Leaves the most recent structural scope.
    Close,

    /// Text ready for serialization.
    Text(String),

    /// Verbatim text ready for serialization.
    Verbatim(String),
}

/// Metadata for a dictionary-backed hanja conversion.
///
/// The engine fills this value when it turns source hanja into a hangul
/// reading. The flags describe known constraints; middlewares may adjust them
/// before a renderer chooses the concrete output form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Annotation {
    /// The original hanja text from the input.
    pub hanja: String,

    /// The hangul reading selected for the hanja text.
    pub reading: String,

    /// Whether another hanja form in the active context shares this reading.
    pub homophone: bool,

    /// Whether rendered output must keep the original hanja visible.
    pub require_hanja: bool,

    /// Whether rendered output must include a hangul gloss when hanja remains
    /// primary.
    pub require_hangul: bool,

    /// Whether this is the first occurrence in the active context window.
    pub first_in_context: bool,

    /// Whether this annotation came from a dictionary match.
    pub from_dictionary: bool,
}

/// Dictionary-provided rendering constraints for a match.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MatchMark {
    /// Whether this dictionary entry should always show its hanja form.
    pub require_hanja: bool,

    /// Whether this dictionary entry should always show its hangul reading.
    pub require_hangul: bool,
}

/// A dictionary match that starts at the queried cursor position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Match {
    /// The matched prefix length in UTF-8 bytes.
    pub byte_len: usize,

    /// The hangul reading for the matched hanja prefix.
    pub reading: String,

    /// Dictionary-provided rendering constraints for this match.
    pub mark: MatchMark,
}

/// A hanja dictionary queried by the conversion engine.
///
/// The key operation returns every entry that starts at the beginning of the
/// supplied string. This shape supports lattice segmentation because the
/// engine must consider every candidate path through a hanja run.
pub trait HanjaDictionary {
    /// Yields every dictionary match that starts at the beginning of `s`.
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a>;

    /// Returns the greatest dictionary entry length in Unicode scalar values.
    fn max_word_chars(&self) -> Option<usize> {
        None
    }

    /// Returns whether another hanja spelling has the same hangul reading.
    fn has_homophone(&self, _hanja: &str, _reading: &str) -> bool {
        false
    }
}

impl<D> HanjaDictionary for &D
where
    D: HanjaDictionary + ?Sized,
{
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        (**self).matches_at(s)
    }

    fn max_word_chars(&self) -> Option<usize> {
        (**self).max_word_chars()
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        (**self).has_homophone(hanja, reading)
    }
}

impl<D> HanjaDictionary for Box<D>
where
    D: HanjaDictionary + ?Sized,
{
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        (**self).matches_at(s)
    }

    fn max_word_chars(&self) -> Option<usize> {
        (**self).max_word_chars()
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        (**self).has_homophone(hanja, reading)
    }
}

/// Per-character Unihan fallback readings exposed as a dictionary.
///
/// This type reads the same generated `kHangul` table used by the engine's
/// fallback phoneticizer, but it deliberately returns canonical pre-initial
/// sound law readings. Stateful orthographic rules such as the initial sound
/// law, `列`/`律`, and numeral grouping remain engine fallback behavior rather
/// than dictionary behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UnihanCharDict;

impl HanjaDictionary for UnihanCharDict {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        let matched = s.chars().next().and_then(|ch| {
            khangul_reading(ch).map(|reading| Match {
                byte_len: ch.len_utf8(),
                reading: reading.to_string(),
                mark: MatchMark::default(),
            })
        });
        Box::new(matched.into_iter())
    }

    fn max_word_chars(&self) -> Option<usize> {
        Some(1)
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        let mut chars = hanja.chars();
        let Some(hanja) = chars.next() else {
            return false;
        };
        if chars.next().is_some() {
            return false;
        }
        KHANGUL_READINGS
            .iter()
            .any(|&(other_hanja, other_reading)| other_hanja != hanja && other_reading == reading)
    }
}

/// A dictionary composition that preserves caller-supplied priority order.
///
/// Dictionaries are stored from highest to lowest priority. During lookup,
/// matches of different byte lengths are all returned so the lattice segmenter
/// can still compare shorter high-priority entries with longer low-priority
/// entries. When two dictionaries produce a match with the same byte length,
/// only the first one is kept.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChainDictionary<D> {
    dictionaries: Vec<D>,
}

impl<D> ChainDictionary<D> {
    /// Creates an empty chain.
    pub fn new() -> Self {
        Self {
            dictionaries: Vec::new(),
        }
    }

    /// Appends a dictionary with lower priority than the existing entries.
    pub fn push(&mut self, dictionary: D) {
        self.dictionaries.push(dictionary);
    }

    /// Returns the number of dictionaries in the chain.
    pub fn len(&self) -> usize {
        self.dictionaries.len()
    }

    /// Returns whether the chain contains no dictionaries.
    pub fn is_empty(&self) -> bool {
        self.dictionaries.is_empty()
    }

    /// Returns the chained dictionaries in priority order.
    pub fn dictionaries(&self) -> &[D] {
        &self.dictionaries
    }

    /// Consumes the chain and returns its dictionaries in priority order.
    pub fn into_dictionaries(self) -> Vec<D> {
        self.dictionaries
    }
}

impl<D> FromIterator<D> for ChainDictionary<D> {
    fn from_iter<T: IntoIterator<Item = D>>(iter: T) -> Self {
        Self {
            dictionaries: Vec::from_iter(iter),
        }
    }
}

impl<D> HanjaDictionary for ChainDictionary<D>
where
    D: HanjaDictionary,
{
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        let mut seen_lengths = BTreeSet::new();
        let mut matches = Vec::new();

        for dictionary in &self.dictionaries {
            for matched in dictionary.matches_at(s) {
                if seen_lengths.insert(matched.byte_len) {
                    matches.push(matched);
                }
            }
        }

        matches.sort_by_key(|matched| matched.byte_len);
        Box::new(matches.into_iter())
    }

    fn max_word_chars(&self) -> Option<usize> {
        let mut max = None;
        for dictionary in &self.dictionaries {
            let word_chars = dictionary.max_word_chars()?;
            max = Some(max.map_or(word_chars, |current: usize| current.max(word_chars)));
        }
        max
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        self.dictionaries
            .iter()
            .any(|dictionary| dictionary.has_homophone(hanja, reading))
    }
}

fn khangul_reading(ch: char) -> Option<&'static str> {
    KHANGUL_READINGS
        .binary_search_by_key(&ch, |(hanja, _)| *hanja)
        .ok()
        .map(|index| KHANGUL_READINGS[index].1)
}

/// Engine-level options that affect hanja conversion before rendering.
///
/// These options apply to fallback text that is not covered by the supplied
/// dictionary. Dictionary matches are assumed to already contain the desired
/// reading and are not rewritten by fallback orthography rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EngineOptions {
    /// Whether fallback readings should apply South Korean initial sound law.
    pub initial_sound_law: bool,

    /// How fallback hanja numerals are rendered.
    pub numeral_strategy: NumeralStrategy,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            initial_sound_law: true,
            numeral_strategy: NumeralStrategy::HangulPhonetic,
        }
    }
}

/// Strategy for rendering hanja numerals encountered in fallback text.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumeralStrategy {
    /// Render hanja numerals as their hangul phonetic readings.
    HangulPhonetic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DictionaryEntry {
    reading: String,
    mark: MatchMark,
}

/// A small in-memory dictionary backed by an ordered map.
///
/// This implementation is intended for tests, user-supplied custom entries,
/// and early pipeline validation. It returns all prefix matches at a cursor so
/// the engine can score every candidate path through a hanja run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MapDictionary {
    entries: BTreeMap<String, DictionaryEntry>,
    max_word_chars: Option<usize>,
}

impl MapDictionary {
    /// Creates an empty map dictionary.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts an entry with no special rendering constraints.
    pub fn insert(&mut self, hanja: impl Into<String>, reading: impl Into<String>) {
        self.insert_marked(hanja, reading, MatchMark::default());
    }

    /// Inserts an entry with dictionary-provided rendering constraints.
    pub fn insert_marked(
        &mut self,
        hanja: impl Into<String>,
        reading: impl Into<String>,
        mark: MatchMark,
    ) {
        let hanja = hanja.into();
        let word_chars = hanja.chars().count();
        self.max_word_chars = Some(self.max_word_chars.map_or(word_chars, |max| {
            if word_chars > max { word_chars } else { max }
        }));
        self.entries.insert(
            hanja,
            DictionaryEntry {
                reading: reading.into(),
                mark,
            },
        );
    }

    /// Returns whether the dictionary has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of dictionary entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl HanjaDictionary for MapDictionary {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        Box::new(
            self.entries
                .iter()
                .filter(move |(hanja, _)| s.starts_with(hanja.as_str()))
                .map(|(hanja, entry)| Match {
                    byte_len: hanja.len(),
                    reading: entry.reading.clone(),
                    mark: entry.mark,
                }),
        )
    }

    fn max_word_chars(&self) -> Option<usize> {
        self.max_word_chars
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        self.entries
            .iter()
            .any(|(other_hanja, entry)| other_hanja != hanja && entry.reading == reading)
    }
}

/// Scope data used by the plain-text adapter.
///
/// Plain text has no preserved regions, markup restrictions, or block
/// boundaries in this MVP adapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlainScopeData;

impl ScopeData for PlainScopeData {
    fn is_preserve(&self) -> bool {
        false
    }
}

/// Reads a plain-text string into the core input-token stream.
///
/// The adapter wraps the input in a plain scope and emits the entire input as a
/// single `Text` token.
pub fn read_plain_text(input: &str) -> Vec<InputToken<PlainScopeData>> {
    Vec::from([
        InputToken::Open(Scope::new(PlainScopeData)),
        InputToken::Text(input.to_string()),
        InputToken::Close,
    ])
}

/// Writes rendered plain-text tokens back to a string.
///
/// Structural tokens are ignored because plain text has no serialized scope
/// markers. `Text` and `Verbatim` tokens are concatenated in stream order.
pub fn write_plain_text<S>(tokens: impl IntoIterator<Item = RenderedToken<S>>) -> String {
    let mut output = String::new();
    for token in tokens {
        match token {
            RenderedToken::Open(_) | RenderedToken::Close => {}
            RenderedToken::Text(text) | RenderedToken::Verbatim(text) => output.push_str(&text),
        }
    }
    output
}

/// Processes input tokens with the default hanja conversion engine options.
///
/// The engine preserves structural and verbatim tokens, skips text when the
/// current scope is preserving, and uses lattice segmentation to annotate
/// dictionary and fallback matches inside text tokens.
pub fn process_tokens<S, D>(
    tokens: impl IntoIterator<Item = InputToken<S>>,
    dictionary: &D,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    process_tokens_with_options(tokens, dictionary, EngineOptions::default())
}

/// Processes input tokens with explicit hanja conversion engine options.
///
/// This is the lower-level entry point for callers that need to disable
/// fallback initial sound law or choose a non-default numeral strategy.
pub fn process_tokens_with_options<S, D>(
    tokens: impl IntoIterator<Item = InputToken<S>>,
    dictionary: &D,
    options: EngineOptions,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    let mut output = Vec::new();
    let mut scopes = Vec::new();

    for token in tokens {
        process_input_token(token, dictionary, options, &mut scopes, &mut output);
    }

    output
}

/// Processes fallible input tokens with default engine options.
///
/// Reader errors are handled according to `recovery`. In strict mode the first
/// error is returned. In lenient mode each recoverable region is logged and
/// emitted as `OutputToken::Verbatim`, after which later tokens continue
/// through the normal engine path.
pub fn process_fallible_tokens<S, D>(
    tokens: impl IntoIterator<Item = Result<InputToken<S>, RecoverableInputError>>,
    dictionary: &D,
    recovery: Recovery,
) -> Result<Vec<OutputToken<S>>, Error>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    process_fallible_tokens_with_options(tokens, dictionary, EngineOptions::default(), recovery)
}

/// Processes fallible input tokens with explicit engine options.
///
/// This is the recovery-aware counterpart to
/// [`process_tokens_with_options`]. It does not make the dictionary trait
/// fallible; it only handles reader errors that carry enough original source
/// text for lenient preservation.
pub fn process_fallible_tokens_with_options<S, D>(
    tokens: impl IntoIterator<Item = Result<InputToken<S>, RecoverableInputError>>,
    dictionary: &D,
    options: EngineOptions,
    recovery: Recovery,
) -> Result<Vec<OutputToken<S>>, Error>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    let mut output = Vec::new();
    let mut scopes = Vec::new();

    for token in tokens {
        match token {
            Ok(token) => process_input_token(token, dictionary, options, &mut scopes, &mut output),
            Err(error) => match recovery {
                Recovery::Strict => return Err(error.into_parts().1),
                Recovery::Lenient => {
                    let (original, error) = error.into_parts();
                    tracing::warn!(error = %error, "recovering from input reader error");
                    output.push(OutputToken::Verbatim(original));
                }
            },
        }
    }

    Ok(output)
}

fn process_input_token<S, D>(
    token: InputToken<S>,
    dictionary: &D,
    options: EngineOptions,
    scopes: &mut Vec<Scope<S>>,
    output: &mut Vec<OutputToken<S>>,
) where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    match token {
        InputToken::Open(scope) => {
            scopes.push(scope.clone());
            output.push(OutputToken::Open(scope));
        }
        InputToken::Close => {
            scopes.pop();
            output.push(OutputToken::Close);
        }
        InputToken::Text(text) => {
            if scopes
                .last()
                .is_some_and(|scope| scope.data().is_preserve())
            {
                output.push(OutputToken::Text(text));
            } else {
                process_text(&text, dictionary, options, output);
            }
        }
        InputToken::Verbatim(text) => output.push(OutputToken::Verbatim(text)),
    }
}

fn process_text<S, D>(
    text: &str,
    dictionary: &D,
    options: EngineOptions,
    output: &mut Vec<OutputToken<S>>,
) where
    D: HanjaDictionary + ?Sized,
{
    let segments = segment_text(text, dictionary);
    let mut index = 0;
    let mut fallback_state = FallbackState::default();

    while index < segments.len() {
        match &segments[index] {
            Segment::Dictionary {
                byte_start,
                byte_end,
                reading,
                mark,
            } => {
                let source = &text[*byte_start..*byte_end];
                output.push(OutputToken::Annotated(Annotation {
                    hanja: source.to_string(),
                    homophone: false,
                    reading: reading.clone(),
                    require_hanja: mark.require_hanja,
                    require_hangul: mark.require_hangul,
                    first_in_context: true,
                    from_dictionary: true,
                }));
                if should_preserve_dictionary_context(source, reading, options) {
                    update_fallback_state_for_reading(reading, &mut fallback_state);
                } else {
                    fallback_state = FallbackState::default();
                }
                index += 1;
            }
            Segment::Fallback {
                byte_start,
                byte_end,
            } => {
                let mut fallback_end = *byte_end;
                while let Some(Segment::Fallback { byte_end, .. }) = segments.get(index + 1) {
                    fallback_end = *byte_end;
                    index += 1;
                }
                process_fallback_text(
                    &text[*byte_start..fallback_end],
                    options,
                    &mut fallback_state,
                    output,
                );
                index += 1;
            }
            Segment::Text {
                byte_start,
                byte_end,
            } => {
                let text_segment = &text[*byte_start..*byte_end];
                push_text(output, text_segment);
                update_fallback_state_for_text(text_segment, &mut fallback_state);
                index += 1;
            }
        }
    }
}

fn process_fallback_text<S>(
    text: &str,
    options: EngineOptions,
    state: &mut FallbackState,
    output: &mut Vec<OutputToken<S>>,
) {
    for part in phoneticize_fallback_run_with_state(text, options, state) {
        match part {
            FallbackPart::Annotation { hanja, reading } => {
                output.push(OutputToken::Annotated(Annotation {
                    hanja,
                    reading,
                    homophone: false,
                    require_hanja: false,
                    require_hangul: false,
                    first_in_context: true,
                    from_dictionary: false,
                }));
            }
            FallbackPart::Text(text) => push_text(output, &text),
        }
    }
}

fn update_fallback_state_for_text(text: &str, state: &mut FallbackState) {
    if text.is_empty() {
        return;
    }

    if text.chars().all(char::is_whitespace) {
        *state = FallbackState::default();
        return;
    }

    let Some(last) = text.chars().rev().find(|ch| !ch.is_whitespace()) else {
        return;
    };

    if last.is_alphanumeric() {
        state.starts_word = false;
        state.previous_reading = Some(last);
    } else {
        *state = FallbackState::default();
    }
}

fn should_preserve_dictionary_context(source: &str, reading: &str, options: EngineOptions) -> bool {
    if reading.chars().all(char::is_whitespace) {
        return false;
    }

    if source.chars().all(is_hanja) {
        match fallback_reading_for_run(source, options) {
            Some(fallback_reading) => {
                fallback_reading == reading || has_one_hangul_syllable_per_hanja(source, reading)
            }
            None => has_one_hangul_syllable_per_hanja(source, reading),
        }
    } else {
        true
    }
}

fn has_one_hangul_syllable_per_hanja(source: &str, reading: &str) -> bool {
    let source_len = source.chars().count();
    let mut reading_len = 0;

    for ch in reading.chars() {
        if !is_hangul_syllable(ch) {
            return false;
        }
        reading_len += 1;
    }

    reading_len == source_len
}

fn is_hangul_syllable(ch: char) -> bool {
    ('\u{ac00}'..='\u{d7a3}').contains(&ch)
}

fn update_fallback_state_for_reading(reading: &str, state: &mut FallbackState) {
    let Some(last) = reading.chars().rev().find(|ch| !ch.is_whitespace()) else {
        *state = FallbackState::default();
        return;
    };

    if last.is_alphanumeric() {
        state.starts_word = false;
        state.previous_reading = Some(last);
    } else {
        *state = FallbackState::default();
    }
}

fn push_text<S>(output: &mut Vec<OutputToken<S>>, text: &str) {
    if text.is_empty() {
        return;
    }

    match output.last_mut() {
        Some(OutputToken::Text(existing)) => existing.push_str(text),
        _ => output.push(OutputToken::Text(text.to_string())),
    }
}

/// Returns whether `ch` is in a known CJK ideograph range.
pub fn is_hanja(ch: char) -> bool {
    matches!(
        ch,
        '\u{2F00}'..='\u{2FFF}'
            | '\u{3007}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
            | '\u{2CEB0}'..='\u{2EBEF}'
            | '\u{2EBF0}'..='\u{2EE5F}'
            | '\u{2F800}'..='\u{2FA1F}'
            | '\u{30000}'..='\u{3134F}'
            | '\u{31350}'..='\u{323AF}'
            | '\u{323B0}'..='\u{3347F}'
    )
}

/// The concrete rendering mode for annotated hanja words.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderMode {
    /// Emits only hangul unless annotation flags require hanja disambiguation.
    HangulOnly,

    /// Always emits hangul followed by the original hanja in parentheses.
    HangulHanjaParens,

    /// Always emits original hanja followed by the hangul reading in
    /// parentheses.
    HanjaHangulParens,

    /// Emits original hanja, adding a hangul gloss only when requested.
    Original,
}

/// The context boundary used by stateful annotation middlewares.
///
/// `PerBlock` resets when a scope reports [`ScopeData::is_block_boundary`].
/// Plain-text streams have no block scopes, so they behave like one document
/// context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextWindow {
    /// Disable the middleware and leave tokens unchanged.
    Off,

    /// Reset state at format-adapter block boundaries.
    PerBlock,

    /// Use the entire token stream as one context.
    PerDocument,
}

/// Literal user rules that force annotation presentation flags.
///
/// This early directive type intentionally supports only literal hanja sets.
/// It adjusts policy flags and leaves rendering form decisions to
/// [`render_tokens`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UserDirectives {
    require_hanja: BTreeSet<String>,
    require_hangul: BTreeSet<String>,
}

impl UserDirectives {
    /// Creates an empty directive set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks a literal hanja form as requiring visible hanja in output.
    pub fn require_hanja(&mut self, hanja: impl Into<String>) {
        self.require_hanja.insert(hanja.into());
    }

    /// Marks a literal hanja form as requiring a visible hangul gloss.
    pub fn require_hangul(&mut self, hanja: impl Into<String>) {
        self.require_hangul.insert(hanja.into());
    }

    /// Returns whether no literal directive rules are configured.
    pub fn is_empty(&self) -> bool {
        self.require_hanja.is_empty() && self.require_hangul.is_empty()
    }
}

/// Sets `homophone` on dictionary annotations sharing a reading in context.
///
/// The marker looks only at dictionary-backed annotations that actually appear
/// in the stream. It does not ask the dictionary whether other unseen words
/// share the same reading, and it ignores fallback annotations because those
/// are phonetic fragments rather than known lexical homophones.
pub fn mark_homophones<S>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    window: ContextWindow,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
{
    apply_contexts(tokens, window, mark_homophones_in_context)
}

/// Clears repeat gloss requirements after the first occurrence of each hanja.
///
/// The first occurrence key is the original hanja form. Later annotations for
/// the same form have `first_in_context` set to false and no longer require
/// either side to be shown.
pub fn filter_first_occurrences<S>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    window: ContextWindow,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
{
    apply_contexts(tokens, window, filter_first_occurrences_in_context)
}

/// Applies literal user directives to annotation policy flags.
///
/// Rules only set flags; they do not render, remove, or reorder tokens.
pub fn apply_user_directives<S>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    directives: &UserDirectives,
) -> Vec<OutputToken<S>> {
    tokens
        .into_iter()
        .map(|token| match token {
            OutputToken::Annotated(mut annotation) => {
                if directives.require_hanja.contains(&annotation.hanja) {
                    annotation.require_hanja = true;
                }
                if directives.require_hangul.contains(&annotation.hanja) {
                    annotation.require_hangul = true;
                }
                OutputToken::Annotated(annotation)
            }
            token => token,
        })
        .collect()
}

fn apply_contexts<S>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    window: ContextWindow,
    mut apply: impl FnMut(&mut [OutputToken<S>]),
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
{
    match window {
        ContextWindow::Off => tokens.into_iter().collect(),
        ContextWindow::PerDocument => {
            let mut tokens = tokens.into_iter().collect::<Vec<_>>();
            apply(&mut tokens);
            tokens
        }
        ContextWindow::PerBlock => {
            let mut output = Vec::new();
            let mut context = Vec::new();
            let mut scope_boundaries = Vec::new();

            for token in tokens {
                match &token {
                    OutputToken::Open(scope) => {
                        let is_boundary = scope.data().is_block_boundary();
                        if is_boundary {
                            flush_context(&mut context, &mut output, &mut apply);
                        }
                        scope_boundaries.push(is_boundary);
                        context.push(token);
                    }
                    OutputToken::Close => {
                        let closes_boundary = scope_boundaries.pop().unwrap_or(false);
                        context.push(token);
                        if closes_boundary {
                            flush_context(&mut context, &mut output, &mut apply);
                        }
                    }
                    _ => context.push(token),
                }
            }

            flush_context(&mut context, &mut output, &mut apply);
            output
        }
    }
}

fn flush_context<S>(
    context: &mut Vec<OutputToken<S>>,
    output: &mut Vec<OutputToken<S>>,
    apply: &mut impl FnMut(&mut [OutputToken<S>]),
) {
    if context.is_empty() {
        return;
    }

    apply(context);
    output.append(context);
}

fn mark_homophones_in_context<S>(tokens: &mut [OutputToken<S>]) {
    let mut forms_by_reading = BTreeMap::<String, BTreeSet<String>>::new();

    for token in tokens.iter() {
        if let OutputToken::Annotated(annotation) = token
            && annotation.from_dictionary
        {
            forms_by_reading
                .entry(annotation.reading.clone())
                .or_default()
                .insert(annotation.hanja.clone());
        }
    }

    for token in tokens.iter_mut() {
        if let OutputToken::Annotated(annotation) = token {
            annotation.homophone = annotation.from_dictionary
                && forms_by_reading
                    .get(&annotation.reading)
                    .is_some_and(|forms| forms.len() > 1);
        }
    }
}

fn filter_first_occurrences_in_context<S>(tokens: &mut [OutputToken<S>]) {
    let mut seen = BTreeSet::new();

    for token in tokens.iter_mut() {
        if let OutputToken::Annotated(annotation) = token {
            if seen.insert(annotation.hanja.clone()) {
                annotation.first_in_context = true;
            } else {
                annotation.first_in_context = false;
                annotation.require_hanja = false;
                annotation.require_hangul = false;
            }
        }
    }
}

/// Renders engine output tokens into annotation-free tokens.
///
/// Structural and text tokens pass through. Each annotation is expanded into a
/// concrete text form according to `mode` and its flags.
pub fn render_tokens<S>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    mode: RenderMode,
) -> Vec<RenderedToken<S>> {
    tokens
        .into_iter()
        .map(|token| match token {
            OutputToken::Open(scope) => RenderedToken::Open(scope),
            OutputToken::Close => RenderedToken::Close,
            OutputToken::Text(text) => RenderedToken::Text(text),
            OutputToken::Verbatim(text) => RenderedToken::Verbatim(text),
            OutputToken::Annotated(annotation) => {
                RenderedToken::Text(render_annotation(&annotation, mode))
            }
        })
        .collect()
}

fn render_annotation(annotation: &Annotation, mode: RenderMode) -> String {
    match mode {
        RenderMode::HangulOnly if annotation.require_hanja || annotation.homophone => {
            parens(&annotation.reading, &annotation.hanja)
        }
        RenderMode::HangulOnly => annotation.reading.clone(),
        RenderMode::HangulHanjaParens => parens(&annotation.reading, &annotation.hanja),
        RenderMode::HanjaHangulParens => parens(&annotation.hanja, &annotation.reading),
        RenderMode::Original if annotation.require_hangul => {
            parens(&annotation.hanja, &annotation.reading)
        }
        RenderMode::Original => annotation.hanja.clone(),
    }
}

fn parens(reading: &str, hanja: &str) -> String {
    let mut output = String::new();
    output.push_str(reading);
    output.push('(');
    output.push_str(hanja);
    output.push(')');
    output
}

/// Converts plain text through reader, engine, renderer, and writer stages.
///
/// This is a convenience for the plain-text MVP path. More capable format
/// adapters should call the individual stages so they can preserve their own
/// structural tokens.
pub fn convert_plain_text<D>(input: &str, dictionary: &D, mode: RenderMode) -> String
where
    D: HanjaDictionary + ?Sized,
{
    convert_plain_text_with_options(input, dictionary, mode, EngineOptions::default())
}

/// Converts plain text with explicit hanja conversion engine options.
///
/// This is the option-aware variant of [`convert_plain_text`].
pub fn convert_plain_text_with_options<D>(
    input: &str,
    dictionary: &D,
    mode: RenderMode,
    options: EngineOptions,
) -> String
where
    D: HanjaDictionary + ?Sized,
{
    let input_tokens = read_plain_text(input);
    let output_tokens = process_tokens_with_options(input_tokens, dictionary, options);
    let output_tokens = mark_homophones(output_tokens, ContextWindow::PerBlock);
    let rendered_tokens = render_tokens(output_tokens, mode);
    write_plain_text(rendered_tokens)
}
