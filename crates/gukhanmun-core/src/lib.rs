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
use core::marker::PhantomData;

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
    ///
    /// This flag is about *structural* permission for markup at the current
    /// position, not about whether the engine actually converts text here.
    /// A scope may legitimately set [`Self::is_preserve`] to `true` (so no
    /// annotation is produced) while still reporting `true` for this method,
    /// because preserve does not by itself restrict what a deeper non-preserved
    /// child may emit. Adapters should return `false` only when an HTML5
    /// text-only content model (such as `<title>` or `<option>`) or an
    /// analogous host rule actually forbids markup at this position.
    ///
    /// Scope-aware renderers treat inline markup as allowed only when *every*
    /// open ancestor reports `true`; a nested allow-markup scope cannot
    /// re-enable markup that an ancestor has forbidden.
    fn allows_inline_markup(&self) -> bool {
        true
    }

    /// Returns whether this scope resets block-oriented stateful stages.
    fn is_block_boundary(&self) -> bool {
        false
    }

    /// Returns whether this scope resets section-oriented stateful stages.
    fn is_section_boundary(&self) -> bool {
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

    /// A structural ruby annotation pairing a base text with an `rt` gloss.
    ///
    /// Writers serialize this in a format-appropriate way: HTML emits a
    /// `<ruby>` element, Markdown emits inline HTML, and plain text falls back
    /// to parenthesized text. Because the variant carries the base and gloss
    /// as separate strings rather than pre-built markup, each writer is
    /// responsible for escaping the contents according to its own rules — the
    /// renderer never injects raw HTML produced by string concatenation.
    ///
    /// Renderers only emit this variant when the active scope reports
    /// [`ScopeData::allows_inline_markup`] as `true`; scopes that disallow
    /// inline markup receive a plain `Text` fallback instead.
    Ruby {
        /// Base text shown as the primary side of the ruby annotation.
        base: String,

        /// Gloss text shown in the `rt` position.
        rt: String,
    },
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

    /// Whether renderers should collapse this annotation to its primary plain
    /// text form instead of adding annotation markup or parentheses.
    pub skip_annotation: bool,

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

/// A complete dictionary entry exposed for batch policy analysis.
///
/// Conversion only needs prefix lookup through [`HanjaDictionary::matches_at`],
/// but middlewares such as homophone marking need to reason about the effective
/// entry set without repeatedly probing the dictionary. Backends that can
/// enumerate entries should return these records from
/// [`HanjaDictionary::entries`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DictionaryRecord {
    /// The hanja spelling stored as a dictionary key.
    pub hanja: String,

    /// The hangul reading selected for this hanja spelling.
    pub reading: String,

    /// Dictionary-provided rendering constraints for this entry.
    pub mark: MatchMark,
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

    /// Enumerates complete dictionary entries when the backend supports it.
    ///
    /// The default returns `None`, which keeps custom lookup-only dictionaries
    /// valid. Homophone-aware middlewares use this as an optional batch path so
    /// built-in backends can avoid per-token full-dictionary scans.
    fn entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = DictionaryRecord> + 'a>> {
        None
    }

    /// Returns whether another hanja spelling has the same hangul reading.
    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        self.entries().is_some_and(|mut entries| {
            entries.any(|record| record.hanja != hanja && record.reading == reading)
        })
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

    fn entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = DictionaryRecord> + 'a>> {
        (**self).entries()
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

    fn entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = DictionaryRecord> + 'a>> {
        (**self).entries()
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

    fn entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = DictionaryRecord> + 'a>> {
        Some(Box::new(KHANGUL_READINGS.iter().map(|(hanja, reading)| {
            DictionaryRecord {
                hanja: hanja.to_string(),
                reading: reading.to_string(),
                mark: MatchMark::default(),
            }
        })))
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

    fn entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = DictionaryRecord> + 'a>> {
        let mut records = BTreeMap::<String, DictionaryRecord>::new();

        for dictionary in &self.dictionaries {
            for record in dictionary.entries()? {
                records.entry(record.hanja.clone()).or_insert(record);
            }
        }

        Some(Box::new(records.into_values()))
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        if let Some(mut records) = self.entries() {
            return records.any(|record| record.hanja != hanja && record.reading == reading);
        }

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
    /// How hanja-containing spans are split into dictionary and fallback
    /// segments.
    pub segmentation: SegmentationStrategy,

    /// Whether fallback readings should apply South Korean initial sound law.
    pub initial_sound_law: bool,

    /// How fallback hanja numerals are rendered.
    pub numeral_strategy: NumeralStrategy,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            segmentation: SegmentationStrategy::Lattice,
            initial_sound_law: true,
            numeral_strategy: NumeralStrategy::HangulPhonetic,
        }
    }
}

/// Strategy used to segment hanja-containing spans.
///
/// `Lattice` considers every dictionary path and chooses the best coverage,
/// while `Eager` greedily takes the longest match at each cursor.  The eager
/// strategy can reduce work for callers that prefer speed over segmentation
/// accuracy.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SegmentationStrategy {
    /// Use dynamic programming to maximize dictionary coverage.
    #[default]
    Lattice,

    /// Use left-to-right eager longest-match segmentation.
    Eager,
}

/// Strategy for rendering hanja numerals encountered in fallback text.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumeralStrategy {
    /// Render hanja numerals as their hangul phonetic readings.
    ///
    /// This strategy emits fallback annotations so renderers can still expose
    /// the original hanja in annotation-oriented render modes.
    HangulPhonetic,

    /// Normalize positional digit-only hanja numerals to Arabic digits.
    ///
    /// Arabic normalization emits plain text rather than annotations. Renderers
    /// and user directives therefore cannot later recover the original numeral
    /// hanja for the normalized span.
    PositionalArabic,

    /// Normalize additive hanja numerals with place markers to Arabic digits.
    ///
    /// This parser handles small units such as `十`, `百`, and `千` and large
    /// units through `澗`. Malformed or overflowing numerals fall back to
    /// [`NumeralStrategy::HangulPhonetic`] for that run.
    AdditiveArabic,

    /// Choose Arabic normalization for common numeric contexts and otherwise
    /// keep hangul phonetic fallback behavior.
    ///
    /// Well-formed additive numerals are normalized to Arabic. Pure positional
    /// digit runs are normalized only when they contain at least four digits,
    /// matching common year notation. Other numerals remain hangul annotations.
    Smart,
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

    fn entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = DictionaryRecord> + 'a>> {
        Some(Box::new(self.entries.iter().map(|(hanja, entry)| {
            DictionaryRecord {
                hanja: hanja.clone(),
                reading: entry.reading.clone(),
                mark: entry.mark,
            }
        })))
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        self.entries
            .iter()
            .any(|(other_hanja, entry)| other_hanja != hanja && entry.reading == reading)
    }
}

/// Scope data used by the plain-text adapter.
///
/// Plain text has no preserved regions or block boundaries, and inline markup
/// such as `<ruby>` is not meaningful in a plain-text stream. Reporting
/// [`ScopeData::allows_inline_markup`] as `false` lets scope-aware renderers
/// fall back to parenthesized text before any [`RenderedToken::Ruby`] reaches
/// the plain-text writer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlainScopeData;

impl ScopeData for PlainScopeData {
    fn is_preserve(&self) -> bool {
        false
    }

    fn allows_inline_markup(&self) -> bool {
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
/// `Ruby` tokens are not expected because [`PlainScopeData`] disallows inline
/// markup, but they are defensively serialized as `base(rt)` rather than
/// dropped silently if one ever reaches the writer.
pub fn write_plain_text<S>(tokens: impl IntoIterator<Item = RenderedToken<S>>) -> String {
    let mut output = String::new();
    for token in tokens {
        match token {
            RenderedToken::Open(_) | RenderedToken::Close => {}
            RenderedToken::Text(text) | RenderedToken::Verbatim(text) => output.push_str(&text),
            RenderedToken::Ruby { base, rt } => {
                output.push_str(&parens(&base, &rt));
            }
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
    process_tokens_iter(tokens, dictionary).collect()
}

/// Processes input tokens through the default engine options and returns an
/// iterator over the collected output.
///
/// This is an iterator-shaped compatibility adapter, not the low-level
/// streaming surface: it consumes the supplied input before returning. For
/// true incremental processing, use [`Engine`] directly and call
/// [`Engine::push_token`] as chunks arrive.
pub fn process_tokens_iter<S, D>(
    tokens: impl IntoIterator<Item = InputToken<S>>,
    dictionary: &D,
) -> alloc::vec::IntoIter<OutputToken<S>>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    process_tokens_with_options(tokens, dictionary, EngineOptions::default()).into_iter()
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
    let mut engine = Engine::collecting(dictionary, options);
    let mut output = Vec::new();

    for token in tokens {
        output.extend(engine.push_token(token));
    }

    output.extend(engine.finish());
    output
}

/// Processes input tokens through explicit engine options and returns an
/// iterator over the collected output.
///
/// This convenience adapter preserves the existing collect-into-`Vec` behavior
/// while exposing an iterator-shaped API for callers that compose pipeline
/// stages. Use [`Engine`] for chunk-by-chunk output.
pub fn process_tokens_iter_with_options<S, D>(
    tokens: impl IntoIterator<Item = InputToken<S>>,
    dictionary: &D,
    options: EngineOptions,
) -> alloc::vec::IntoIter<OutputToken<S>>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    process_tokens_with_options(tokens, dictionary, options).into_iter()
}

/// Resolves a fallible reader token stream into recovered input tokens.
///
/// This is the single place where the stream-level [`Recovery`] policy is
/// applied to a reader's output. Format adapters (such as the HTML scanner)
/// emit `Ok(InputToken)` for well-formed regions and
/// `Err(RecoverableInputError)` for malformed regions they can describe and
/// preserve; this function turns that stream into the plain
/// [`InputToken`] sequence the rest of the pipeline consumes:
///
///  -  In [`Recovery::Strict`] mode the first error stops processing and its
///     cause is returned, so the caller never sees a partial token stream.
///  -  In [`Recovery::Lenient`] mode each error is logged at `warn` level once
///     and replaced by an [`InputToken::Verbatim`] holding the original source
///     region, so the malformed bytes pass through untouched while surrounding
///     tokens continue to flow.
///
/// It sits one stage before the [`Engine`]: feed its output into
/// [`process_tokens_with_options`] or a streaming [`Engine`]. The recovery-aware
/// engine entry points ([`process_fallible_tokens`] and
/// [`process_fallible_tokens_with_options`]) are thin wrappers that call this
/// and then run the engine.
pub fn recover_input_tokens<S>(
    tokens: impl IntoIterator<Item = Result<InputToken<S>, RecoverableInputError>>,
    recovery: Recovery,
) -> Result<Vec<InputToken<S>>, Error>
where
    S: ScopeData,
{
    let mut recovered = Vec::new();
    for token in tokens {
        match token {
            Ok(token) => recovered.push(token),
            Err(error) => match recovery {
                Recovery::Strict => return Err(error.into_parts().1),
                Recovery::Lenient => {
                    let (original, error) = error.into_parts();
                    tracing::warn!(error = %error, "recovering from input reader error");
                    recovered.push(InputToken::Verbatim(original));
                }
            },
        }
    }
    Ok(recovered)
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
    let recovered = recover_input_tokens(tokens, recovery)?;
    Ok(process_tokens_with_options(recovered, dictionary, options))
}

/// Stateful hanja conversion engine for chunked token streams.
///
/// `Engine` is the low-level streaming surface. Call [`Engine::push_token`] for
/// each incoming token and then [`Engine::finish`] once the upstream reader is
/// exhausted. When the dictionary reports a maximum word length, text chunks are
/// buffered only at the tail so dictionary matches can cross chunk boundaries
/// without requiring the whole document in memory. A trailing fallback hanja run
/// is also kept buffered until a non-convertible boundary or EOF so render modes
/// that expose annotation spans match one-shot conversion. Dictionaries with an
/// unknown maximum keep hanja-containing text until a non-convertible boundary
/// or EOF so long custom entries remain observable.
pub struct Engine<'a, S, D>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    dictionary: &'a D,
    options: EngineOptions,
    scopes: Vec<Scope<S>>,
    pending_text: String,
    pending_unflushable_fallback_run_bytes: Option<usize>,
    fallback_state: FallbackState,
    incremental_flush: bool,
}

impl<'a, S, D> Engine<'a, S, D>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    /// Creates a streaming engine with default options.
    pub fn new(dictionary: &'a D) -> Self {
        Self::with_options(dictionary, EngineOptions::default())
    }

    /// Creates a streaming engine with explicit conversion options.
    pub fn with_options(dictionary: &'a D, options: EngineOptions) -> Self {
        Self::with_incremental_flush(dictionary, options, true)
    }

    fn collecting(dictionary: &'a D, options: EngineOptions) -> Self {
        Self::with_incremental_flush(dictionary, options, false)
    }

    fn with_incremental_flush(
        dictionary: &'a D,
        options: EngineOptions,
        incremental_flush: bool,
    ) -> Self {
        tracing::debug!(
            strategy = ?options.segmentation,
            "engine created with segmentation strategy"
        );
        Self {
            dictionary,
            options,
            scopes: Vec::new(),
            pending_text: String::new(),
            pending_unflushable_fallback_run_bytes: None,
            fallback_state: FallbackState::default(),
            incremental_flush,
        }
    }

    /// Pushes one input token and returns output tokens that are now safe to
    /// emit.
    pub fn push_token(&mut self, token: InputToken<S>) -> Vec<OutputToken<S>> {
        let mut output = Vec::new();
        match token {
            InputToken::Open(scope) => {
                self.flush_into(&mut output);
                if scope.data().is_block_boundary() {
                    self.reset_fallback_context();
                }
                self.scopes.push(scope.clone());
                output.push(OutputToken::Open(scope));
            }
            InputToken::Close => {
                self.flush_into(&mut output);
                let closes_block_boundary = self
                    .scopes
                    .pop()
                    .is_some_and(|scope| scope.data().is_block_boundary());
                output.push(OutputToken::Close);
                if closes_block_boundary {
                    self.reset_fallback_context();
                }
            }
            InputToken::Text(text) => {
                if self
                    .scopes
                    .last()
                    .is_some_and(|scope| scope.data().is_preserve())
                {
                    self.flush_into(&mut output);
                    self.reset_fallback_context();
                    output.push(OutputToken::Text(text));
                } else {
                    let previous_pending_bytes = self.pending_text.len();
                    self.pending_text.push_str(&text);
                    if self
                        .pending_unflushable_fallback_run_bytes
                        .is_some_and(|bytes| bytes == previous_pending_bytes)
                    {
                        self.pending_unflushable_fallback_run_bytes = Some(previous_pending_bytes);
                    } else {
                        self.pending_unflushable_fallback_run_bytes = None;
                    }
                    if self.incremental_flush {
                        self.flush_safe_into(&mut output);
                    }
                }
            }
            InputToken::Verbatim(text) => {
                self.flush_into(&mut output);
                self.reset_fallback_context();
                output.push(OutputToken::Verbatim(text));
            }
        }
        output
    }

    /// Flushes all pending text without ending the engine.
    pub fn flush(&mut self) -> Vec<OutputToken<S>> {
        let mut output = Vec::new();
        self.flush_into(&mut output);
        output
    }

    /// Finishes the stream and returns every remaining output token.
    pub fn finish(mut self) -> Vec<OutputToken<S>> {
        self.flush()
    }

    /// Returns the number of Unicode scalar values currently buffered.
    pub fn buffered_chars(&self) -> usize {
        self.pending_text.chars().count()
    }

    fn tail_bound(&self) -> Option<usize> {
        self.dictionary.max_word_chars().filter(|bound| *bound > 0)
    }

    fn flush_safe_into(&mut self, output: &mut Vec<OutputToken<S>>) {
        if self.pending_text.is_empty() {
            return;
        }
        if !self.pending_text.chars().any(is_hanja) {
            self.flush_non_hanja_safe_into(output);
            return;
        }

        let Some(bound) = self.tail_bound() else {
            let Some(flush_end) = safe_unknown_bound_flush_end(&self.pending_text) else {
                return;
            };
            self.flush_prefix_into(flush_end, output);
            if !self.pending_text.chars().any(is_hanja) {
                self.flush_non_hanja_safe_into(output);
            }
            return;
        };
        if let Some(flush_end) = safe_unknown_bound_flush_end(&self.pending_text) {
            self.flush_prefix_into(flush_end, output);
            if !self.pending_text.chars().any(is_hanja) {
                self.flush_non_hanja_safe_into(output);
            }
            return;
        }
        let buffered_chars = self.buffered_chars();
        if buffered_chars > bound.saturating_mul(10) {
            tracing::debug!(
                buffered_chars,
                dict_max_word_chars = bound,
                "streaming tail buffer is unusually large"
            );
        }
        if buffered_chars <= bound {
            return;
        }

        if self.extends_unflushable_fallback_run(bound) {
            self.pending_unflushable_fallback_run_bytes = Some(self.pending_text.len());
            return;
        }

        let safe_chars = buffered_chars.saturating_sub(bound).saturating_add(1);
        let segments = segment_text(
            &self.pending_text,
            self.dictionary,
            self.options.segmentation,
        );
        let mut flush_end = 0;
        let mut flush_segments = Vec::new();
        for segment in &segments {
            let (byte_start, byte_end) = segment_bounds(segment);
            let start_chars = self.pending_text[..byte_start].chars().count();
            let end_chars = self.pending_text[..byte_end].chars().count();
            if byte_start > flush_end || (start_chars > safe_chars && flush_end > 0) {
                break;
            }
            if end_chars > safe_chars {
                break;
            }
            flush_end = byte_end;
            flush_segments.push(segment.clone());
        }

        // Fallback runs render as one annotation in non-default render modes.
        // Keep a trailing fallback run buffered because the next chunk may
        // extend it, even when the dictionary lookahead bound is only one char.
        if let Some(fallback_start) = trailing_fallback_run_start(&segments, flush_end) {
            flush_end = fallback_start;
            while flush_segments
                .last()
                .is_some_and(|segment| segment_bounds(segment).1 > flush_end)
            {
                flush_segments.pop();
            }
        }

        if flush_end > 0 {
            self.pending_unflushable_fallback_run_bytes = None;
            self.flush_segments_prefix_into(flush_end, &flush_segments, output);
            if !self.pending_text.chars().any(is_hanja) {
                self.flush_non_hanja_safe_into(output);
            }
        } else if trailing_fallback_run_start(&segments, self.pending_text.len()) == Some(0) {
            self.pending_unflushable_fallback_run_bytes = Some(self.pending_text.len());
        }
    }

    fn extends_unflushable_fallback_run(&self, bound: usize) -> bool {
        let Some(previous_bytes) = self.pending_unflushable_fallback_run_bytes else {
            return false;
        };
        if previous_bytes == 0
            || previous_bytes > self.pending_text.len()
            || !self.pending_text.is_char_boundary(previous_bytes)
        {
            return false;
        }

        let appended = &self.pending_text[previous_bytes..];
        if appended.is_empty() {
            return true;
        }
        if appended.chars().any(|ch| !is_hanja(ch)) {
            return false;
        }

        // The existing prefix was already segmented as one fallback run.  Only
        // the old suffix that can participate in a cross-chunk dictionary match
        // and the newly appended text need to be inspected here.
        let probe_start = suffix_start_for_char_count(
            &self.pending_text[..previous_bytes],
            bound.saturating_sub(1),
        );
        let probe = &self.pending_text[probe_start..];
        segment_text(probe, self.dictionary, self.options.segmentation)
            .iter()
            .all(|segment| matches!(segment, Segment::Fallback { .. }))
    }

    fn flush_non_hanja_safe_into(&mut self, output: &mut Vec<OutputToken<S>>) {
        let flush_end = match self.tail_bound() {
            Some(bound) => safe_non_hanja_flush_end(&self.pending_text, bound),
            None => safe_unknown_bound_flush_end(&self.pending_text),
        };
        if let Some(flush_end) = flush_end {
            self.flush_prefix_into(flush_end, output);
        }
    }

    fn flush_prefix_into(&mut self, flush_end: usize, output: &mut Vec<OutputToken<S>>) {
        if flush_end == self.pending_text.len() {
            self.flush_into(output);
            return;
        }
        self.pending_unflushable_fallback_run_bytes = None;
        let prefix = self.pending_text[..flush_end].to_string();
        let segments = segment_text(&prefix, self.dictionary, self.options.segmentation);
        self.flush_segments_prefix_into(flush_end, &segments, output);
    }

    fn flush_segments_prefix_into(
        &mut self,
        flush_end: usize,
        segments: &[Segment],
        output: &mut Vec<OutputToken<S>>,
    ) {
        let prefix = self.pending_text[..flush_end].to_string();
        process_segments_with_state(
            &prefix,
            segments,
            self.dictionary,
            self.options,
            &mut self.fallback_state,
            output,
        );
        self.pending_text.replace_range(..flush_end, "");
    }

    fn flush_into(&mut self, output: &mut Vec<OutputToken<S>>) {
        if self.pending_text.is_empty() {
            return;
        }
        self.pending_unflushable_fallback_run_bytes = None;
        let text = core::mem::take(&mut self.pending_text);
        process_text_with_state(
            &text,
            self.dictionary,
            self.options,
            &mut self.fallback_state,
            output,
        );
    }

    fn reset_fallback_context(&mut self) {
        self.fallback_state = FallbackState::default();
    }
}

fn safe_non_hanja_flush_end(text: &str, bound: usize) -> Option<usize> {
    if text.is_empty() {
        return None;
    }

    let keep_chars = bound.saturating_sub(1);
    let span_start = text
        .char_indices()
        .rfind(|(_, ch)| ch.is_whitespace())
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let suffix = &text[span_start..];
    let suffix_chars = suffix.chars().count();
    if suffix_chars <= keep_chars {
        return (span_start > 0).then_some(span_start);
    }

    let flush_suffix_chars = suffix_chars - keep_chars;
    let flush_end = suffix
        .char_indices()
        .nth(flush_suffix_chars)
        .map_or(text.len(), |(index, _)| span_start + index);
    (flush_end > 0).then_some(flush_end)
}

fn safe_unknown_bound_flush_end(text: &str) -> Option<usize> {
    text.char_indices()
        .rfind(|(_, ch)| ch.is_whitespace())
        .map(|(index, ch)| index + ch.len_utf8())
}

fn suffix_start_for_char_count(text: &str, count: usize) -> usize {
    if count == 0 {
        return text.len();
    }

    text.char_indices()
        .rev()
        .nth(count.saturating_sub(1))
        .map_or(0, |(index, _)| index)
}

fn trailing_fallback_run_start(segments: &[Segment], split_byte: usize) -> Option<usize> {
    if split_byte == 0 {
        return None;
    }

    for (index, segment) in segments.iter().enumerate() {
        let (byte_start, byte_end) = segment_bounds(segment);
        if byte_end != split_byte {
            continue;
        }
        if !matches!(segment, Segment::Fallback { .. }) {
            return None;
        }
        if let Some(next) = segments.get(index + 1)
            && !matches!(next, Segment::Fallback { .. })
        {
            return None;
        }

        let mut run_start = byte_start;
        for previous in segments[..index].iter().rev() {
            let (previous_start, previous_end) = segment_bounds(previous);
            if previous_end != run_start || !matches!(previous, Segment::Fallback { .. }) {
                break;
            }
            run_start = previous_start;
        }
        return (run_start < split_byte).then_some(run_start);
    }

    None
}

fn process_text_with_state<S, D>(
    text: &str,
    dictionary: &D,
    options: EngineOptions,
    fallback_state: &mut FallbackState,
    output: &mut Vec<OutputToken<S>>,
) where
    D: HanjaDictionary + ?Sized,
{
    let segments = segment_text(text, dictionary, options.segmentation);
    process_segments_with_state(text, &segments, dictionary, options, fallback_state, output);
}

fn process_segments_with_state<S, D>(
    text: &str,
    segments: &[Segment],
    _dictionary: &D,
    options: EngineOptions,
    fallback_state: &mut FallbackState,
    output: &mut Vec<OutputToken<S>>,
) where
    D: HanjaDictionary + ?Sized,
{
    let mut index = 0;

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
                    skip_annotation: false,
                    from_dictionary: true,
                }));
                if should_preserve_dictionary_context(source, reading, options) {
                    update_fallback_state_for_reading(reading, fallback_state);
                } else {
                    *fallback_state = FallbackState::default();
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
                    fallback_state,
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
                update_fallback_state_for_text(text_segment, fallback_state);
                index += 1;
            }
        }
    }
}

fn segment_bounds(segment: &Segment) -> (usize, usize) {
    match segment {
        Segment::Dictionary {
            byte_start,
            byte_end,
            ..
        }
        | Segment::Fallback {
            byte_start,
            byte_end,
        }
        | Segment::Text {
            byte_start,
            byte_end,
        } => (*byte_start, *byte_end),
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
                    skip_annotation: false,
                    from_dictionary: false,
                }));
            }
            FallbackPart::ReadingText(text) => push_text(output, &text),
            FallbackPart::Text(text) => push_text(output, &text),
        }
    }
}

fn update_fallback_state_for_text(text: &str, state: &mut FallbackState) {
    if text.is_empty() {
        return;
    }

    if text
        .chars()
        .last()
        .is_some_and(|character| character.is_whitespace())
    {
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

    /// Emits a `<ruby>` element pairing hangul reading and source hanja.
    ///
    /// The [`RubyBase`] sub-mode chooses which side becomes the base text.
    /// When the active scope reports
    /// [`ScopeData::allows_inline_markup`] as `false`, the renderer falls back
    /// to parenthesized text so that adapters which cannot embed markup still
    /// receive a sensible surface form.
    Ruby(RubyBase),

    /// Emits original hanja, adding a hangul gloss only when requested.
    Original,
}

/// Selects which side of a `<ruby>` element is the base text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RubyBase {
    /// `<ruby>hangul<rt>hanja</rt></ruby>`; hangul is the base, hanja is the gloss.
    OnHangul,

    /// `<ruby>hanja<rt>hangul</rt></ruby>`; hanja is the base, hangul is the gloss.
    OnHanja,
}

/// Form for the gloss attached to annotations in [`RenderMode::Original`].
///
/// `Original` keeps the source hanja as primary text and only attaches a
/// hangul gloss when the annotation flags or a user directive demand one.
/// This option controls how that gloss appears. Because `Original` always
/// treats hanja as primary, the ruby form uses hanja as the base and hangul
/// as the `rt` gloss; there is no sub-mode to flip the sides.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OriginalGloss {
    /// `hanja(hangul)`; matches the legacy behavior.
    #[default]
    Parens,

    /// A `<ruby>` element with hanja as the base and hangul as the `rt`
    /// gloss, falling back to parens when the active scope disallows inline
    /// markup.
    Ruby,
}

/// Rendering options that combine a [`RenderMode`] with per-mode sub-options.
///
/// Most pipelines configure rendering by mode alone, so `RenderOptions`
/// implements `From<RenderMode>` and `Default` to keep existing call sites
/// terse. Pipelines that need finer control (such as a ruby gloss in
/// [`RenderMode::Original`]) construct a `RenderOptions` value directly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderOptions {
    /// Top-level rendering mode applied to every annotation.
    pub mode: RenderMode,

    /// Gloss form used by [`RenderMode::Original`]. Ignored by other modes.
    pub original_gloss: OriginalGloss,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            mode: RenderMode::HangulOnly,
            original_gloss: OriginalGloss::Parens,
        }
    }
}

impl From<RenderMode> for RenderOptions {
    fn from(mode: RenderMode) -> Self {
        Self {
            mode,
            original_gloss: OriginalGloss::default(),
        }
    }
}

/// The context boundary used by stateful annotation middlewares.
///
/// `PerBlock` resets when a scope reports [`ScopeData::is_block_boundary`].
/// `PerSection` resets when a later scope reports
/// [`ScopeData::is_section_boundary`].  Plain-text streams have no block or
/// section scopes, so those windows behave like one document context.  This is
/// required for exact homophone rendering because a later plain-text line can
/// make an earlier annotation ambiguous after it would otherwise have been
/// written.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextWindow {
    /// Disable the middleware and leave tokens unchanged.
    Off,

    /// Reset state at format-adapter block boundaries.
    PerBlock,

    /// Reset state at format-adapter section boundaries.
    PerSection,

    /// Use the entire token stream as one context.
    PerDocument,
}

/// Action applied when a user directive predicate matches an annotation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectiveAction {
    /// Require rendered output to keep the original hanja visible.
    RequireHanja,

    /// Require rendered output to include a hangul gloss.
    RequireHangul,

    /// Collapse the annotation to plain primary text for the active renderer.
    SkipAnnotation,
}

/// User rules that adjust annotation presentation policy.
///
/// Literal helpers cover common hanja-form rules.  Callers that need richer
/// matching can add closure predicates over the whole [`Annotation`], which
/// keeps the core API independent of CLI-only pattern syntaxes.
#[derive(Default)]
pub struct UserDirectives<'a> {
    rules: Vec<UserDirectiveRule<'a>>,
}

impl<'a> UserDirectives<'a> {
    /// Creates an empty directive set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks a literal hanja form as requiring visible hanja in output.
    pub fn require_hanja(&mut self, hanja: impl Into<String>) {
        self.add_literal(hanja, DirectiveAction::RequireHanja);
    }

    /// Marks a literal hanja form as requiring a visible hangul gloss.
    pub fn require_hangul(&mut self, hanja: impl Into<String>) {
        self.add_literal(hanja, DirectiveAction::RequireHangul);
    }

    /// Marks a literal hanja form as not receiving annotation rendering.
    pub fn skip_annotation(&mut self, hanja: impl Into<String>) {
        self.add_literal(hanja, DirectiveAction::SkipAnnotation);
    }

    /// Adds a literal hanja-form directive.
    pub fn add_literal(&mut self, hanja: impl Into<String>, action: DirectiveAction) {
        self.rules.push(UserDirectiveRule {
            predicate: UserDirectivePredicate::Literal(hanja.into()),
            action,
        });
    }

    /// Adds a predicate directive over the complete annotation metadata.
    pub fn add_predicate(
        &mut self,
        predicate: impl Fn(&Annotation) -> bool + 'a,
        action: DirectiveAction,
    ) {
        self.rules.push(UserDirectiveRule {
            predicate: UserDirectivePredicate::Predicate(Box::new(predicate)),
            action,
        });
    }

    /// Returns whether no directive rules are configured.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Applies every configured directive to a single output token.
    ///
    /// Non-[`OutputToken::Annotated`] tokens pass through unchanged. For an
    /// annotation, each matching rule sets the corresponding flag in priority
    /// of declaration order. This method is the per-token primitive used by
    /// streaming pipelines that want to apply directives without buffering.
    pub fn apply<S>(&self, token: OutputToken<S>) -> OutputToken<S> {
        match token {
            OutputToken::Annotated(mut annotation) => {
                for rule in &self.rules {
                    if !rule.predicate.matches(&annotation) {
                        continue;
                    }
                    match rule.action {
                        DirectiveAction::RequireHanja => annotation.require_hanja = true,
                        DirectiveAction::RequireHangul => annotation.require_hangul = true,
                        DirectiveAction::SkipAnnotation => annotation.skip_annotation = true,
                    }
                }
                OutputToken::Annotated(annotation)
            }
            token => token,
        }
    }
}

struct UserDirectiveRule<'a> {
    predicate: UserDirectivePredicate<'a>,
    action: DirectiveAction,
}

enum UserDirectivePredicate<'a> {
    Literal(String),
    Predicate(Box<dyn Fn(&Annotation) -> bool + 'a>),
}

impl UserDirectivePredicate<'_> {
    fn matches(&self, annotation: &Annotation) -> bool {
        match self {
            Self::Literal(hanja) => annotation.hanja == *hanja,
            Self::Predicate(predicate) => predicate(annotation),
        }
    }
}

/// Sets `homophone` on dictionary annotations sharing a reading.
///
/// The marker builds one optional homophone index from the supplied dictionary
/// and falls back to [`HanjaDictionary::has_homophone`] for lookup-only
/// dictionaries. It also preserves the context-local heuristic. Fallback
/// annotations are ignored because they are phonetic fragments rather than
/// known lexical homophones.
pub fn mark_homophones<S, D>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    dictionary: &D,
    window: ContextWindow,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    if window == ContextWindow::Off {
        return tokens.into_iter().collect();
    }

    let index = HomophoneIndex::from_dictionary(dictionary);
    let lookup_fallback = index.is_none().then_some(dictionary);
    ContextMiddleware::new(window, |tokens| {
        mark_homophones_in_context(tokens, index.as_ref(), lookup_fallback);
    })
    .process(tokens)
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
    ContextMiddleware::new(window, filter_first_occurrences_in_context).process(tokens)
}

type ContextApply<S> = fn(&mut [OutputToken<S>]);
type HomophoneApply<'a, S> = Box<dyn FnMut(&mut [OutputToken<S>]) + 'a>;

/// Streaming homophone marker middleware.
///
/// Context windows that require lookahead buffer only until their configured
/// boundary. `PerDocument`, and scoped windows on streams that never emit the
/// corresponding boundary, buffer until [`HomophoneMarker::finish`].  For
/// example, exact plain-text homophone marking with `PerBlock` is document-wide
/// because plain text has no block scopes.
pub struct HomophoneMarker<'a, S>
where
    S: ScopeData,
{
    inner: ContextMiddleware<S, HomophoneApply<'a, S>>,
}

impl<'a, S> HomophoneMarker<'a, S>
where
    S: ScopeData,
{
    /// Creates a homophone marker for the selected context window.
    pub fn new<D>(dictionary: &'a D, window: ContextWindow) -> Self
    where
        D: HanjaDictionary + ?Sized,
    {
        let index = if window == ContextWindow::Off {
            None
        } else {
            HomophoneIndex::from_dictionary(dictionary)
        };
        let lookup_fallback = index.is_none().then_some(dictionary);
        Self {
            inner: ContextMiddleware::new(
                window,
                Box::new(move |tokens| {
                    mark_homophones_in_context(tokens, index.as_ref(), lookup_fallback);
                }),
            ),
        }
    }

    /// Pushes one output token and returns tokens ready for downstream stages.
    pub fn push_token(&mut self, token: OutputToken<S>) -> Vec<OutputToken<S>> {
        self.inner.push_token(token)
    }

    /// Finishes the middleware and returns buffered tokens.
    pub fn finish(self) -> Vec<OutputToken<S>> {
        self.inner.finish()
    }
}

/// Streaming first-occurrence middleware.
///
/// Repeated annotations inside a context have `first_in_context` cleared and
/// presentation requirements removed once the context is flushed.
pub struct FirstOccurrenceFilter<S>
where
    S: ScopeData,
{
    inner: ContextMiddleware<S, ContextApply<S>>,
}

impl<S> FirstOccurrenceFilter<S>
where
    S: ScopeData,
{
    /// Creates a first-occurrence filter for the selected context window.
    pub fn new(window: ContextWindow) -> Self {
        Self {
            inner: ContextMiddleware::new(window, filter_first_occurrences_in_context::<S>),
        }
    }

    /// Pushes one output token and returns tokens ready for downstream stages.
    pub fn push_token(&mut self, token: OutputToken<S>) -> Vec<OutputToken<S>> {
        self.inner.push_token(token)
    }

    /// Finishes the middleware and returns buffered tokens.
    pub fn finish(self) -> Vec<OutputToken<S>> {
        self.inner.finish()
    }
}

/// Applies literal user directives to annotation policy flags.
///
/// Rules only set flags; they do not render, remove, or reorder tokens.
pub fn apply_user_directives<S>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    directives: &UserDirectives<'_>,
) -> Vec<OutputToken<S>> {
    apply_user_directives_iter(tokens, directives).collect()
}

/// Lazily applies literal user directives to an output token stream.
///
/// Returns an iterator that walks the input tokens without intermediate
/// buffering. Use this variant in streaming pipelines that need to chain
/// directive application with other lazy stages such as [`render_tokens_iter`].
pub fn apply_user_directives_iter<'a, S>(
    tokens: impl IntoIterator<Item = OutputToken<S>> + 'a,
    directives: &'a UserDirectives<'_>,
) -> impl Iterator<Item = OutputToken<S>> + 'a {
    tokens.into_iter().map(|token| directives.apply(token))
}

struct ContextMiddleware<S, F>
where
    S: ScopeData,
    F: FnMut(&mut [OutputToken<S>]),
{
    window: ContextWindow,
    apply: F,
    context: Vec<OutputToken<S>>,
    scope_boundaries: Vec<bool>,
}

impl<S, F> ContextMiddleware<S, F>
where
    S: ScopeData,
    F: FnMut(&mut [OutputToken<S>]),
{
    fn new(window: ContextWindow, apply: F) -> Self {
        Self {
            window,
            apply,
            context: Vec::new(),
            scope_boundaries: Vec::new(),
        }
    }

    fn process(mut self, tokens: impl IntoIterator<Item = OutputToken<S>>) -> Vec<OutputToken<S>> {
        let mut output = Vec::new();
        for token in tokens {
            output.extend(self.push_token(token));
        }
        output.extend(self.finish());
        output
    }

    fn push_token(&mut self, token: OutputToken<S>) -> Vec<OutputToken<S>> {
        let mut output = Vec::new();
        match self.window {
            ContextWindow::Off => output.push(token),
            ContextWindow::PerDocument => self.context.push(token),
            ContextWindow::PerBlock | ContextWindow::PerSection => match &token {
                OutputToken::Open(scope) => {
                    let is_boundary = match self.window {
                        ContextWindow::PerBlock => scope.data().is_block_boundary(),
                        ContextWindow::PerSection => scope.data().is_section_boundary(),
                        ContextWindow::Off | ContextWindow::PerDocument => false,
                    };
                    if is_boundary {
                        self.flush_context(&mut output);
                    }
                    self.scope_boundaries.push(is_boundary);
                    self.context.push(token);
                }
                OutputToken::Close => {
                    let closes_boundary = self.scope_boundaries.pop().unwrap_or(false);
                    self.context.push(token);
                    if closes_boundary && self.window == ContextWindow::PerBlock {
                        self.flush_context(&mut output);
                    }
                }
                _ => self.context.push(token),
            },
        }
        output
    }

    fn finish(mut self) -> Vec<OutputToken<S>> {
        let mut output = Vec::new();
        self.flush_context(&mut output);
        output
    }

    fn flush_context(&mut self, output: &mut Vec<OutputToken<S>>) {
        if self.context.is_empty() {
            return;
        }

        (self.apply)(&mut self.context);
        output.append(&mut self.context);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct HomophoneIndex {
    forms_by_reading: BTreeMap<String, BTreeSet<String>>,
}

impl HomophoneIndex {
    fn from_dictionary<D>(dictionary: &D) -> Option<Self>
    where
        D: HanjaDictionary + ?Sized,
    {
        let mut forms_by_reading = BTreeMap::<String, BTreeSet<String>>::new();
        for record in dictionary.entries()? {
            forms_by_reading
                .entry(record.reading)
                .or_default()
                .insert(record.hanja);
        }
        Some(Self { forms_by_reading })
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        self.forms_by_reading
            .get(reading)
            .is_some_and(|forms| forms.iter().any(|form| form != hanja))
    }
}

fn mark_homophones_in_context<S, D>(
    tokens: &mut [OutputToken<S>],
    index: Option<&HomophoneIndex>,
    lookup_fallback: Option<&D>,
) where
    D: HanjaDictionary + ?Sized,
{
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
                && (index.is_some_and(|index| {
                    index.has_homophone(&annotation.hanja, &annotation.reading)
                }) || lookup_fallback.is_some_and(|dictionary| {
                    dictionary.has_homophone(&annotation.hanja, &annotation.reading)
                }) || forms_by_reading
                    .get(&annotation.reading)
                    .is_some_and(|forms| forms.len() > 1));
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
/// concrete rendered token according to the supplied options, the current
/// scope, and the annotation's flags. `options` accepts either a bare
/// [`RenderMode`] (via the `From<RenderMode>` impl on [`RenderOptions`]) or a
/// full [`RenderOptions`] value.
pub fn render_tokens<S, O>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    options: O,
) -> Vec<RenderedToken<S>>
where
    S: ScopeData,
    O: Into<RenderOptions>,
{
    render_tokens_iter(tokens, options).collect()
}

/// Renders engine output tokens into annotation-free tokens as an iterator.
///
/// The renderer maintains a small scope stack so that annotation expansion can
/// consult the active scope's [`ScopeData::allows_inline_markup`] when
/// choosing between an inline-markup form and a parenthesized fallback. Every
/// other token maps one-to-one to its rendered counterpart.
pub fn render_tokens_iter<S, O>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    options: O,
) -> impl Iterator<Item = RenderedToken<S>>
where
    S: ScopeData,
    O: Into<RenderOptions>,
{
    TokenRenderer {
        upstream: tokens.into_iter(),
        options: options.into(),
        markup_stack: Vec::new(),
        disallowing_ancestors: 0,
        _scope: PhantomData,
    }
}

struct TokenRenderer<I, S> {
    upstream: I,
    options: RenderOptions,
    /// Cached `allows_inline_markup` value for each open scope. Storing the
    /// boolean instead of the whole scope keeps the renderer free of an extra
    /// `S: Clone` bound at this layer (it already requires it via `ScopeData`)
    /// and avoids the cost of cloning adapter-owned data.
    markup_stack: Vec<bool>,
    /// Number of currently open scopes whose `allows_inline_markup` is
    /// `false`. Inline markup is safe at the current cursor only when this
    /// counter is zero; otherwise some ancestor forbids markup and a nested
    /// allow-markup scope cannot override that restriction.
    disallowing_ancestors: usize,
    _scope: PhantomData<fn(S)>,
}

impl<I, S> Iterator for TokenRenderer<I, S>
where
    I: Iterator<Item = OutputToken<S>>,
    S: ScopeData,
{
    type Item = RenderedToken<S>;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.upstream.next()?;
        Some(match token {
            OutputToken::Open(scope) => {
                let allows = scope.data().allows_inline_markup();
                if !allows {
                    self.disallowing_ancestors += 1;
                }
                self.markup_stack.push(allows);
                RenderedToken::Open(scope)
            }
            OutputToken::Close => {
                if let Some(false) = self.markup_stack.pop() {
                    // Saturating guard for malformed streams that emit more
                    // Close than Open tokens; the renderer should never
                    // panic on broken input.
                    self.disallowing_ancestors = self.disallowing_ancestors.saturating_sub(1);
                }
                RenderedToken::Close
            }
            OutputToken::Text(text) => RenderedToken::Text(text),
            OutputToken::Verbatim(text) => RenderedToken::Verbatim(text),
            OutputToken::Annotated(annotation) => {
                // Inline markup is allowed only when no open ancestor scope
                // forbids it. The plain-text reader wraps its input in a
                // scope whose `allows_inline_markup` is false, so plain text
                // still falls back to parens; HTML and Markdown root
                // contexts emit no enclosing scope and therefore start with
                // an empty stack, leaving annotations free to use markup.
                let allows_inline_markup = self.disallowing_ancestors == 0;
                render_annotation(&annotation, &self.options, allows_inline_markup)
            }
        })
    }
}

fn render_annotation<S>(
    annotation: &Annotation,
    options: &RenderOptions,
    allows_inline_markup: bool,
) -> RenderedToken<S> {
    if annotation.skip_annotation {
        let primary = match options.mode {
            RenderMode::HangulOnly | RenderMode::HangulHanjaParens => annotation.reading.clone(),
            RenderMode::HanjaHangulParens | RenderMode::Original => annotation.hanja.clone(),
            RenderMode::Ruby(RubyBase::OnHangul) => annotation.reading.clone(),
            RenderMode::Ruby(RubyBase::OnHanja) => annotation.hanja.clone(),
        };
        return RenderedToken::Text(primary);
    }

    match options.mode {
        RenderMode::HangulOnly if annotation.require_hanja || annotation.homophone => {
            RenderedToken::Text(parens(&annotation.reading, &annotation.hanja))
        }
        RenderMode::HangulOnly => RenderedToken::Text(annotation.reading.clone()),
        RenderMode::HangulHanjaParens => {
            RenderedToken::Text(parens(&annotation.reading, &annotation.hanja))
        }
        RenderMode::HanjaHangulParens => {
            RenderedToken::Text(parens(&annotation.hanja, &annotation.reading))
        }
        RenderMode::Ruby(base) => render_ruby(annotation, base, allows_inline_markup),
        RenderMode::Original if annotation.require_hangul => match options.original_gloss {
            OriginalGloss::Parens => {
                RenderedToken::Text(parens(&annotation.hanja, &annotation.reading))
            }
            // `Original` keeps hanja as the primary text, so its ruby form
            // always uses hanja as the base regardless of any other setting.
            OriginalGloss::Ruby => render_ruby(annotation, RubyBase::OnHanja, allows_inline_markup),
        },
        RenderMode::Original => RenderedToken::Text(annotation.hanja.clone()),
    }
}

fn render_ruby<S>(
    annotation: &Annotation,
    base: RubyBase,
    allows_inline_markup: bool,
) -> RenderedToken<S> {
    let (base_text, rt_text) = match base {
        RubyBase::OnHangul => (&annotation.reading, &annotation.hanja),
        RubyBase::OnHanja => (&annotation.hanja, &annotation.reading),
    };
    if !allows_inline_markup {
        return RenderedToken::Text(parens(base_text, rt_text));
    }
    RenderedToken::Ruby {
        base: base_text.clone(),
        rt: rt_text.clone(),
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
/// structural tokens. The `render` argument accepts either a [`RenderMode`]
/// (converted via `From<RenderMode>` for [`RenderOptions`]) or a full
/// [`RenderOptions`] value.
pub fn convert_plain_text<D, R>(input: &str, dictionary: &D, render: R) -> String
where
    D: HanjaDictionary + ?Sized,
    R: Into<RenderOptions>,
{
    convert_plain_text_with_options(input, dictionary, render, EngineOptions::default())
}

/// Converts plain text with explicit hanja conversion engine options.
///
/// This is the option-aware variant of [`convert_plain_text`].
pub fn convert_plain_text_with_options<D, R>(
    input: &str,
    dictionary: &D,
    render: R,
    options: EngineOptions,
) -> String
where
    D: HanjaDictionary + ?Sized,
    R: Into<RenderOptions>,
{
    let input_tokens = read_plain_text(input);
    let output_tokens = process_tokens_with_options(input_tokens, dictionary, options);
    let output_tokens = mark_homophones(output_tokens, dictionary, ContextWindow::PerBlock);
    let rendered_tokens = render_tokens(output_tokens, render);
    write_plain_text(rendered_tokens)
}
