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
mod variants;

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::marker::PhantomData;

use fallback::{
    FallbackPart, FallbackState, apply_initial_sound_law_to_first_syllable,
    fallback_reading_for_run, is_hanja_numeral, khangul_all_readings,
    phoneticize_fallback_run_with_state, phoneticize_hanja_char,
    reading_matches_with_initial_sound_law, should_apply_yeol_yul,
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
    /// responsible for escaping the contents according to its own rules—the
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
///
/// This struct is `#[non_exhaustive]`, so additional flags can be added without
/// a breaking change. Construct it from [`Annotation::default`] and set the
/// fields you need; the public fields stay readable and writable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct Annotation {
    /// The original hanja text from the input.
    pub hanja: String,

    /// The spelling that produced the dictionary match, when the annotation
    /// came from a dictionary. This may differ from [`hanja`](Self::hanja)
    /// after variant normalization and provides the stable lexical identity
    /// used by policy middlewares and variant-set rendering.
    pub dictionary_hanja: Option<String>,

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

    /// Whether the presentation requirements
    /// ([`require_hanja`](Self::require_hanja) /
    /// [`require_hangul`](Self::require_hangul)) were requested by an explicit
    /// parenthetical gloss in the source, rather than by the dictionary.
    ///
    /// [`RedundantParenCollapser`] sets this when it collapses an author-written
    /// gloss.  [`FirstOccurrenceFilter`] preserves the requirements on such
    /// annotations instead of clearing them on repeats, so a word the author
    /// glossed every time stays fully annotated every time.
    pub from_source_gloss: bool,
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
    ///
    /// This is the word-initial reading, which already reflects South Korean
    /// initial sound law where it applies (for example `年` reads `연`).
    pub reading: String,

    /// The reading to use when this match is *not* word-initial, when it
    /// differs from [`Match::reading`] by initial sound law.
    ///
    /// Dictionaries set this for multi-syllable entries whose leading morpheme
    /// keeps its original sound outside word-initial position, as the Standard
    /// Korean Language Dictionary records through its suffix and bound-noun
    /// head words (for example `年代` reads `연대` word-initially but `년대`
    /// after a number). Single-hanja initial sound law is handled by the engine
    /// from the bundled unihan readings and does not need this field. `None`
    /// means the reading is position independent.
    pub suffix_reading: Option<String>,

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
                suffix_reading: None,
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

/// Strategy for rendering hanja numerals.
///
/// The hangul phonetic strategy is fallback-only, so dictionary matches keep
/// their lexicalized readings. Arabic strategies also participate in
/// segmentation as plain-text numeral edges, allowing numeric normalization to
/// take precedence over dictionary calendar entries such as `六月`.
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
    /// Additive numerals are normalized to Arabic when they begin with a digit.
    /// Runs that begin with a small place marker such as `十`, `百`, or `千`
    /// are normalized only when the next character is not an ambiguous
    /// non-unit hanja word character. Pure positional digit runs are normalized
    /// when they contain at least four digits (matching common year notation)
    /// or when a unit hanja (`年月日時分秒號世紀` and so on) immediately follows.
    /// Standalone large place markers such as `萬` or `京`, and other
    /// ambiguous numerals, remain hangul annotations.
    Smart,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DictionaryEntry {
    reading: String,
    suffix_reading: Option<String>,
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
        self.insert_entry(hanja, reading, None, mark);
    }

    /// Inserts an entry that carries a distinct non-word-initial reading.
    ///
    /// `suffix` is the reading used when the match is not word-initial (see
    /// [`Match::suffix_reading`]); `reading` is the word-initial reading.
    pub fn insert_with_suffix(
        &mut self,
        hanja: impl Into<String>,
        reading: impl Into<String>,
        suffix: impl Into<String>,
    ) {
        self.insert_entry(hanja, reading, Some(suffix.into()), MatchMark::default());
    }

    fn insert_entry(
        &mut self,
        hanja: impl Into<String>,
        reading: impl Into<String>,
        suffix_reading: Option<String>,
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
                suffix_reading,
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
                    suffix_reading: entry.suffix_reading.clone(),
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
        recovered.push(recover_input_token(token, recovery)?);
    }
    Ok(recovered)
}

/// Resolves one fallible reader item according to a [`Recovery`] policy.
///
/// This is the per-token form of [`recover_input_tokens`] for streaming
/// pipelines. In strict mode an error is returned immediately. In lenient mode
/// the error is logged once and replaced with an [`InputToken::Verbatim`]
/// carrying the original malformed region.
pub fn recover_input_token<S>(
    token: Result<InputToken<S>, RecoverableInputError>,
    recovery: Recovery,
) -> Result<InputToken<S>, Error>
where
    S: ScopeData,
{
    match token {
        Ok(token) => Ok(token),
        Err(error) => match recovery {
            Recovery::Strict => Err(error.into_parts().1),
            Recovery::Lenient => {
                let (original, error) = error.into_parts();
                tracing::warn!(error = %error, "recovering from input reader error");
                Ok(InputToken::Verbatim(original))
            }
        },
    }
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
        let segments = segment_text(&self.pending_text, self.dictionary, self.options);
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
        segment_text(probe, self.dictionary, self.options)
            .iter()
            .all(|segment| {
                matches!(
                    segment,
                    Segment::Fallback { .. } | Segment::TrivialDictionary { .. }
                )
            })
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
        let segments = segment_text(&prefix, self.dictionary, self.options);
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
        if !matches!(
            segment,
            Segment::Fallback { .. } | Segment::TrivialDictionary { .. }
        ) {
            return None;
        }
        if let Some(next) = segments.get(index + 1)
            && !matches!(
                next,
                Segment::Fallback { .. } | Segment::TrivialDictionary { .. }
            )
        {
            return None;
        }

        let mut run_start = byte_start;
        for previous in segments[..index].iter().rev() {
            let (previous_start, previous_end) = segment_bounds(previous);
            if previous_end != run_start
                || !matches!(
                    previous,
                    Segment::Fallback { .. } | Segment::TrivialDictionary { .. }
                )
            {
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
    let segments = segment_text(text, dictionary, options);
    process_segments_with_state(text, &segments, dictionary, options, fallback_state, output);
}

fn process_trivial_fallback_run<S>(
    run_segments: &[Segment],
    text: &str,
    options: EngineOptions,
    state: &mut FallbackState,
    output: &mut Vec<OutputToken<S>>,
) {
    let run_start = segment_bounds(&run_segments[0]).0;
    let run_end = segment_bounds(&run_segments[run_segments.len() - 1]).1;
    let capacity = run_end.saturating_sub(run_start);
    let mut hanja = String::with_capacity(capacity);
    let mut dictionary_hanja_accumulator = String::with_capacity(capacity);
    let mut reading = String::with_capacity(capacity);
    let mut has_dictionary = false;
    let mut last_trivial_source: Option<char> = None;
    let mut last_trivial_reading: Option<String> = None;

    let mut seg_index = 0;
    while seg_index < run_segments.len() {
        match &run_segments[seg_index] {
            Segment::TrivialDictionary {
                byte_start,
                byte_end,
                reading: dict_reading,
                suffix_reading,
                dictionary_hanja,
                ..
            } => {
                let source = &text[*byte_start..*byte_end];
                let effective = dictionary_effective_reading(
                    source,
                    dict_reading,
                    suffix_reading.as_deref(),
                    options,
                    state.starts_word,
                    state.previous_reading,
                );
                if !hanja.is_empty()
                    && last_trivial_reading.as_deref() == Some(&effective)
                    && last_trivial_source != source.chars().next()
                {
                    output.push(OutputToken::Annotated(Annotation {
                        hanja: core::mem::take(&mut hanja),
                        dictionary_hanja: take_dictionary_hanja(
                            &mut dictionary_hanja_accumulator,
                            has_dictionary,
                        ),
                        reading: core::mem::take(&mut reading),
                        homophone: false,
                        require_hanja: false,
                        require_hangul: false,
                        first_in_context: true,
                        skip_annotation: false,
                        from_dictionary: has_dictionary,
                        from_source_gloss: false,
                    }));
                }
                hanja.push_str(source);
                dictionary_hanja_accumulator.push_str(dictionary_hanja);
                reading.push_str(&effective);
                update_fallback_state_for_reading(&effective, state);
                has_dictionary = true;
                last_trivial_source = source.chars().next();
                last_trivial_reading = Some(effective);
                seg_index += 1;
            }
            Segment::Fallback { byte_start: _, .. } => {
                last_trivial_source = None;
                last_trivial_reading = None;
                let fb_start = seg_index;
                while seg_index < run_segments.len()
                    && matches!(&run_segments[seg_index], Segment::Fallback { .. })
                {
                    seg_index += 1;
                }
                let fb_text = &text[segment_bounds(&run_segments[fb_start]).0
                    ..segment_bounds(&run_segments[seg_index - 1]).1];
                for part in phoneticize_fallback_run_with_state(fb_text, options, state) {
                    match part {
                        FallbackPart::Annotation {
                            hanja: part_hanja,
                            reading: part_reading,
                        } => {
                            if part_hanja.chars().any(is_hanja_numeral) {
                                if !hanja.is_empty() {
                                    output.push(OutputToken::Annotated(Annotation {
                                        hanja: core::mem::take(&mut hanja),
                                        dictionary_hanja: take_dictionary_hanja(
                                            &mut dictionary_hanja_accumulator,
                                            has_dictionary,
                                        ),
                                        reading: core::mem::take(&mut reading),
                                        homophone: false,
                                        require_hanja: false,
                                        require_hangul: false,
                                        first_in_context: true,
                                        skip_annotation: false,
                                        from_dictionary: has_dictionary,
                                        from_source_gloss: false,
                                    }));
                                    has_dictionary = false;
                                }
                                output.push(OutputToken::Annotated(Annotation {
                                    hanja: part_hanja,
                                    dictionary_hanja: None,
                                    reading: part_reading,
                                    homophone: false,
                                    require_hanja: false,
                                    require_hangul: false,
                                    first_in_context: true,
                                    skip_annotation: false,
                                    from_dictionary: false,
                                    from_source_gloss: false,
                                }));
                            } else {
                                hanja.push_str(&part_hanja);
                                dictionary_hanja_accumulator.push_str(&part_hanja);
                                reading.push_str(&part_reading);
                            }
                        }
                        FallbackPart::ReadingText(t) | FallbackPart::Text(t) => {
                            if !hanja.is_empty() {
                                output.push(OutputToken::Annotated(Annotation {
                                    hanja: core::mem::take(&mut hanja),
                                    dictionary_hanja: take_dictionary_hanja(
                                        &mut dictionary_hanja_accumulator,
                                        has_dictionary,
                                    ),
                                    reading: core::mem::take(&mut reading),
                                    homophone: false,
                                    require_hanja: false,
                                    require_hangul: false,
                                    first_in_context: true,
                                    skip_annotation: false,
                                    from_dictionary: has_dictionary,
                                    from_source_gloss: false,
                                }));
                                has_dictionary = false;
                            }
                            push_text(output, &t);
                        }
                    }
                }
            }
            _ => unreachable!("run must contain only TrivialDictionary | Fallback"),
        }
    }

    if !hanja.is_empty() {
        output.push(OutputToken::Annotated(Annotation {
            hanja,
            dictionary_hanja: has_dictionary.then_some(dictionary_hanja_accumulator),
            reading,
            homophone: false,
            require_hanja: false,
            require_hangul: false,
            first_in_context: true,
            skip_annotation: false,
            from_dictionary: has_dictionary,
            from_source_gloss: false,
        }));
    }
}

fn take_dictionary_hanja(buffer: &mut String, from_dictionary: bool) -> Option<String> {
    let value = core::mem::take(buffer);
    from_dictionary.then_some(value)
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
                suffix_reading,
                mark,
                dictionary_hanja,
            } => {
                let source = &text[*byte_start..*byte_end];
                let effective = dictionary_effective_reading(
                    source,
                    reading,
                    suffix_reading.as_deref(),
                    options,
                    fallback_state.starts_word,
                    fallback_state.previous_reading,
                );
                output.push(OutputToken::Annotated(Annotation {
                    hanja: source.to_string(),
                    dictionary_hanja: Some(dictionary_hanja.clone()),
                    homophone: false,
                    reading: effective.clone(),
                    require_hanja: mark.require_hanja,
                    require_hangul: mark.require_hangul,
                    first_in_context: true,
                    skip_annotation: false,
                    from_dictionary: true,
                    from_source_gloss: false,
                }));
                if should_preserve_dictionary_context(source, &effective, options) {
                    update_fallback_state_for_reading(&effective, fallback_state);
                } else {
                    *fallback_state = FallbackState::default();
                }
                index += 1;
            }
            Segment::TrivialDictionary {
                byte_start,
                byte_end,
                ..
            }
            | Segment::Fallback {
                byte_start,
                byte_end,
            } => {
                let run_start = index;
                let mut merged_end = *byte_end;
                while let Some(
                    Segment::TrivialDictionary {
                        byte_end: next_end, ..
                    }
                    | Segment::Fallback {
                        byte_end: next_end, ..
                    },
                ) = segments.get(index + 1)
                {
                    merged_end = *next_end;
                    index += 1;
                }
                let has_dictionary = segments[run_start..=index]
                    .iter()
                    .any(|s| matches!(s, Segment::TrivialDictionary { .. }));
                if has_dictionary {
                    process_trivial_fallback_run(
                        &segments[run_start..=index],
                        text,
                        options,
                        fallback_state,
                        output,
                    );
                } else {
                    process_fallback_text(
                        &text[*byte_start..merged_end],
                        options,
                        fallback_state,
                        output,
                    );
                }
                index += 1;
            }
            Segment::NumeralText { text, .. } => {
                push_text(output, text);
                update_fallback_state_for_text(text, fallback_state);
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
        | Segment::TrivialDictionary {
            byte_start,
            byte_end,
            ..
        }
        | Segment::Fallback {
            byte_start,
            byte_end,
        }
        | Segment::NumeralText {
            byte_start,
            byte_end,
            ..
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
                    dictionary_hanja: None,
                    reading,
                    homophone: false,
                    require_hanja: false,
                    require_hangul: false,
                    first_in_context: true,
                    skip_annotation: false,
                    from_dictionary: false,
                    from_source_gloss: false,
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

/// Chooses the reading a dictionary match should emit at its position.
///
/// South Korean initial sound law (頭音法則) makes some morphemes read
/// differently word-initially than elsewhere. The bundled dictionary stores the
/// word-initial form, so a bare match would render `1998年` as `1998연` instead
/// of `1998년`. This applies the position-correct reading:
///
///  -  When the match carries an explicit [`Match::suffix_reading`] (a
///     multi-syllable entry the Standard Korean Language Dictionary records with
///     a distinct suffix or bound-noun form, such as `年代`), that suffix
///     reading is used outside word-initial position.
///  -  Otherwise, for a single hanja whose bundled unihan reading undergoes
///     initial sound law, the original (non-word-initial) reading is recovered
///     from the unihan table. This covers every such hanja without per-entry
///     data. The match's reading must already be one of the two law variants so
///     unrelated readings (and non-law hanja) are left untouched. The
///     `렬`/`률` → `열`/`율` rule after a vowel or `ㄴ` coda is honored through
///     [`should_apply_yeol_yul`], matching fallback behavior.
///
/// With initial sound law disabled (for example the North Korean preset) the
/// original reading is used everywhere.
fn dictionary_effective_reading(
    source: &str,
    reading: &str,
    suffix_reading: Option<&str>,
    options: EngineOptions,
    starts_word: bool,
    previous_reading: Option<char>,
) -> String {
    if let Some(suffix) = suffix_reading {
        return if starts_word && options.initial_sound_law {
            reading.to_string()
        } else {
            suffix.to_string()
        };
    }

    let mut chars = source.chars();
    if let (Some(ch), None) = (chars.next(), chars.next())
        && let Some(base) = phoneticize_hanja_char(ch)
    {
        let initial = apply_initial_sound_law_to_first_syllable(base);
        if initial != base && (reading == base || reading == initial) {
            let apply_law = options.initial_sound_law
                && (starts_word || should_apply_yeol_yul(previous_reading, base));
            return if apply_law { initial } else { base.to_string() };
        }
    }

    reading.to_string()
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

/// Selects the hanja variant set used wherever a renderer emits hanja.
///
/// Recognition is independent of this setting. The engine always recognizes
/// supported variants, then this profile transforms the dictionary spelling
/// at the final rendering stage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HanjaVariantSet {
    /// Keeps the spelling selected by the dictionary.
    #[default]
    AsDictionary,
    /// Uses the Japanese Joyo kanji new forms.
    Shinjitai,
    /// Prefers traditional and compatibility-folded forms.
    Kanxi,
    /// Uses the first Unicode `kSimplifiedVariant` mapping when available.
    Simplified,
    /// Uses shinjitai plus Gukhanmun's verified Asahi character subset.
    Asahimoji,
}

/// Selects which side of a `<ruby>` element is the base text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RubyBase {
    /// `<ruby>hangul<rp>(</rp><rt>hanja</rt><rp>)</rp></ruby>`; hangul is the
    /// base, hanja is the gloss. The `<rp>` elements provide parenthesized
    /// fallback text for browsers without `<ruby>` support.
    OnHangul,

    /// `<ruby>hanja<rp>(</rp><rt>hangul</rt><rp>)</rp></ruby>`; hanja is the
    /// base, hangul is the gloss. The `<rp>` elements provide parenthesized
    /// fallback text for browsers without `<ruby>` support.
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

    /// Variant set applied after dictionary recognition.
    pub hanja_variant_set: HanjaVariantSet,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            mode: RenderMode::HangulOnly,
            original_gloss: OriginalGloss::Parens,
            hanja_variant_set: HanjaVariantSet::AsDictionary,
        }
    }
}

impl From<RenderMode> for RenderOptions {
    fn from(mode: RenderMode) -> Self {
        Self {
            mode,
            original_gloss: OriginalGloss::default(),
            hanja_variant_set: HanjaVariantSet::default(),
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

/// How homophone disambiguation decides that an annotation needs its hanja
/// shown in [`RenderMode::HangulOnly`].
///
/// The two strategies differ in what counts as a homophone collision:
///
/// `ContextLocal` (the default) marks an annotation only when another reading
/// with a *different* hanja form actually appears within the same context
/// window.  This keeps hangul-only output clean: a Sino-Korean word is glossed
/// only when the surrounding text genuinely makes it ambiguous.
///
/// `DictionaryWide` additionally marks an annotation whenever its reading is
/// shared by any other hanja form anywhere in the dictionary, regardless of
/// whether those alternatives occur in the text.  With a large reference
/// dictionary such as the Standard Korean Dictionary almost every common
/// reading has some homophone, so this strategy glosses most Sino-Korean
/// words.  It is preserved as an opt-in for callers that want maximal
/// disambiguation; words that should always be glossed regardless of context
/// are better expressed through [`MatchMark::require_hanja`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HomophoneDetection {
    /// Mark only readings that collide within the active context window.
    #[default]
    ContextLocal,

    /// Also mark readings shared by other hanja forms anywhere in the
    /// dictionary.
    DictionaryWide,
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
            Self::Literal(hanja) => {
                annotation.hanja == *hanja || annotation.dictionary_hanja.as_ref() == Some(hanja)
            }
            Self::Predicate(predicate) => predicate(annotation),
        }
    }
}

/// Sets `homophone` on dictionary annotations sharing a reading.
///
/// Uses [`HomophoneDetection::ContextLocal`], marking only readings that
/// collide within the active context window.  Use
/// [`mark_homophones_with_detection`] to opt into dictionary-wide marking.
pub fn mark_homophones<S, D>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    dictionary: &D,
    window: ContextWindow,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    mark_homophones_with_detection(tokens, dictionary, window, HomophoneDetection::ContextLocal)
}

/// Sets `homophone` on dictionary annotations sharing a reading, choosing the
/// detection strategy explicitly.
///
/// With [`HomophoneDetection::ContextLocal`] an annotation is marked only when
/// another hanja form with the same reading occurs within the context window,
/// so no dictionary index is built.  With
/// [`HomophoneDetection::DictionaryWide`] the marker also builds one homophone
/// index from the supplied dictionary and falls back to
/// [`HanjaDictionary::has_homophone`] for lookup-only dictionaries.  Fallback
/// (non-dictionary) annotations are ignored either way because they are
/// phonetic fragments rather than known lexical homophones.
pub fn mark_homophones_with_detection<S, D>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    dictionary: &D,
    window: ContextWindow,
    detection: HomophoneDetection,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    if window == ContextWindow::Off {
        return tokens.into_iter().collect();
    }

    let index = match detection {
        HomophoneDetection::ContextLocal => None,
        HomophoneDetection::DictionaryWide => HomophoneIndex::from_dictionary(dictionary),
    };
    let lookup_fallback = match detection {
        HomophoneDetection::ContextLocal => None,
        HomophoneDetection::DictionaryWide => index.is_none().then_some(dictionary),
    };
    ContextMiddleware::new(window, |tokens| {
        mark_homophones_in_context(tokens, index.as_ref(), lookup_fallback);
    })
    .process(tokens)
}

/// Clears repeat gloss requirements after the first occurrence of each hanja.
///
/// The first occurrence key is the canonical dictionary form when available.
/// Later annotations for the same lexical form have `first_in_context` set to
/// false and no longer require either side to be shown.
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
    /// Creates a homophone marker for the selected context window using
    /// [`HomophoneDetection::ContextLocal`].
    ///
    /// Use [`HomophoneMarker::with_detection`] to opt into dictionary-wide
    /// marking.
    pub fn new<D>(dictionary: &'a D, window: ContextWindow) -> Self
    where
        D: HanjaDictionary + ?Sized,
    {
        Self::with_detection(dictionary, window, HomophoneDetection::ContextLocal)
    }

    /// Creates a homophone marker for the selected context window and detection
    /// strategy.
    ///
    /// With [`HomophoneDetection::ContextLocal`] no dictionary index is built;
    /// only readings that collide within the context window are marked.  With
    /// [`HomophoneDetection::DictionaryWide`] a homophone index (or
    /// [`HanjaDictionary::has_homophone`] fallback) is consulted as well.
    pub fn with_detection<D>(
        dictionary: &'a D,
        window: ContextWindow,
        detection: HomophoneDetection,
    ) -> Self
    where
        D: HanjaDictionary + ?Sized,
    {
        let index = match detection {
            _ if window == ContextWindow::Off => None,
            HomophoneDetection::ContextLocal => None,
            HomophoneDetection::DictionaryWide => HomophoneIndex::from_dictionary(dictionary),
        };
        let lookup_fallback = match detection {
            HomophoneDetection::ContextLocal => None,
            HomophoneDetection::DictionaryWide => index.is_none().then_some(dictionary),
        };
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

/// Streaming middleware that collapses an explicit parenthetical reading
/// annotation into the converted hanja word it duplicates.
///
/// Mixed-script input sometimes spells a word together with a parenthetical
/// gloss, either hanja-first (`庫間(곳간)`) or hangul-first (`곳간(庫間)`).  Left
/// alone, the converter would render the hanja *and* keep the parenthetical,
/// producing a redundant `곳간(곳간)`.  An author who wrote such a gloss meant
/// "annotate this word fully", so this middleware detects the two patterns,
/// removes the now-redundant parenthetical text, and sets both
/// [`Annotation::require_hanja`] and [`Annotation::require_hangul`] on the
/// surviving annotation.  Setting both flags reproduces the author's intent in
/// every render mode: [`RenderMode::HangulOnly`] honours `require_hanja`
/// (`곳간(庫間)`) while [`RenderMode::Original`] honours `require_hangul`
/// (`庫間(곳간)`).
///
/// A parenthetical may also *pin an alternative reading*.  `數字` is normally
/// read `숫자`, but in the sense "a few characters" it reads `수자`; writing
/// `數字(수자)` fixes the reading for that occurrence.  Such a reading
/// annotation is told apart from a definition gloss like
/// `庫間(물건을 간직하여 두는 곳)` with a two-tier test against the candidate
/// hangul `R`:
///
/// 1. **Exact match** — `R` equals the annotation's reading.  Collapse and keep
///    the reading.
/// 2. **Valid alternative reading** — `R` has exactly one hangul syllable per
///    hanja character and every syllable is a recorded Unihan reading of its
///    character (or the initial-sound-law variant of one).  Collapse and
///    override the reading with `R`.
///
/// Anything else (definition glosses, foreign transliterations such as
/// `蔣介石(장제스)`, or a syllable-count mismatch) is left untouched.
///
/// The middleware runs immediately after the engine, before
/// [`HomophoneMarker`] and [`FirstOccurrenceFilter`], so later stages observe
/// the corrected reading and flags.  It coalesces adjacent
/// [`OutputToken::Text`] tokens (the streaming engine flushes non-hanja text at
/// safe points, so `(곳간)` can arrive split as `(곳간` then `)`) and buffers
/// only a bounded amount: a held annotation, the trailing matchable suffix of
/// the preceding text, and the following parenthetical until it can be
/// classified.  This keeps the streaming result identical to a one-shot
/// conversion while staying responsive on long hanja-free runs.
/// [`OutputToken::Open`], [`OutputToken::Close`], and [`OutputToken::Verbatim`]
/// flush the buffer and pass through, so a match never crosses a scope
/// boundary.  When `enabled` is `false` the middleware is an exact
/// pass-through.
///
/// # Limitation
///
/// The collapser runs after the engine and never re-derives readings, so a
/// hanja-first gloss immediately followed (with no space) by an initial-sound-law
/// (頭音法則) character keeps the reading the engine chose with the parenthetical
/// acting as a word boundary.  For example `學(학)率` collapses to `학(學)율`
/// rather than `학률`: the engine read `率` as word-initial `율` because `)`
/// separated it from `學`, and removing the gloss cannot recover the
/// non-word-initial `률`.  This is narrow in practice; an intended compound is
/// normally written `學率(학률)`.  Insert a space (`學(학) 率`) or gloss the whole
/// compound to control the reading.
pub struct RedundantParenCollapser<S>
where
    S: ScopeData,
{
    enabled: bool,
    /// Coalesced trailing text held while no annotation is pending: a bounded
    /// suffix (`[hangul]*` optionally ending in `(`) that could still become a
    /// hangul-first match's preceding text once the next annotation arrives.
    /// Everything before that suffix is emitted eagerly so streaming stays
    /// responsive even for long hanja-free runs.
    held_tail: String,
    /// A held annotation whose following text is still being accumulated.
    pending_annotation: Option<Annotation>,
    /// The text immediately preceding [`Self::pending_annotation`].
    preceding: String,
    /// Coalesced text following [`Self::pending_annotation`], accumulated until
    /// the parenthetical can be classified.
    following: String,
    _scope: PhantomData<fn(S)>,
}

impl<S> RedundantParenCollapser<S>
where
    S: ScopeData,
{
    /// Creates a collapser.  When `enabled` is `false` every token passes
    /// through unchanged.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            held_tail: String::new(),
            pending_annotation: None,
            preceding: String::new(),
            following: String::new(),
            _scope: PhantomData,
        }
    }

    /// Pushes one output token and returns tokens ready for downstream stages.
    pub fn push_token(&mut self, token: OutputToken<S>) -> Vec<OutputToken<S>> {
        if !self.enabled {
            return Vec::from([token]);
        }
        let mut output = Vec::new();
        match token {
            OutputToken::Annotated(annotation) => {
                // End any in-progress following text run by forcing a decision,
                // then the held tail becomes this annotation's preceding text.
                self.finalize_pending(&mut output);
                self.preceding = core::mem::take(&mut self.held_tail);
                self.pending_annotation = Some(annotation);
            }
            OutputToken::Text(text) => {
                if self.pending_annotation.is_some() {
                    self.following.push_str(&text);
                    self.resolve_following(&mut output);
                } else {
                    self.held_tail.push_str(&text);
                    self.emit_held_prefix(&mut output);
                }
            }
            boundary => {
                // Open / Close / Verbatim: a match may not cross this boundary,
                // so finalize everything before passing the boundary through.
                self.finalize_pending(&mut output);
                if !self.held_tail.is_empty() {
                    output.push(OutputToken::Text(core::mem::take(&mut self.held_tail)));
                }
                output.push(boundary);
            }
        }
        output
    }

    /// Flushes buffered tokens and returns them.
    pub fn finish(mut self) -> Vec<OutputToken<S>> {
        if !self.enabled {
            return Vec::new();
        }
        let mut output = Vec::new();
        self.finalize_pending(&mut output);
        if !self.held_tail.is_empty() {
            output.push(OutputToken::Text(core::mem::take(&mut self.held_tail)));
        }
        output
    }

    /// Emits the part of [`Self::held_tail`] that can no longer participate in a
    /// hangul-first match, keeping the bounded matchable suffix.
    fn emit_held_prefix(&mut self, output: &mut Vec<OutputToken<S>>) {
        let split = hangul_first_tail_start(&self.held_tail);
        if split > 0 {
            // Keep the (possibly long) prefix in the existing buffer and split
            // off only the bounded suffix, avoiding a large copy and shift.
            let suffix = self.held_tail.split_off(split);
            let prefix = core::mem::replace(&mut self.held_tail, suffix);
            output.push(OutputToken::Text(prefix));
        }
    }

    /// Forces a pending annotation to resolve as if no further following text
    /// will arrive (called at a boundary, a new annotation, or EOF).
    fn finalize_pending(&mut self, output: &mut Vec<OutputToken<S>>) {
        if self.pending_annotation.is_some() {
            self.decide_following(true, output);
        }
    }

    /// Resolves a pending annotation against the accumulated following text,
    /// buffering more text when the parenthetical is still incomplete.
    fn resolve_following(&mut self, output: &mut Vec<OutputToken<S>>) {
        self.decide_following(false, output);
    }

    /// Classifies the pending annotation against `preceding` / `following`.
    ///
    /// With `flush` set, an otherwise-undecidable case is treated as a
    /// non-match instead of requesting more text.
    fn decide_following(&mut self, flush: bool, output: &mut Vec<OutputToken<S>>) {
        let annotation = self
            .pending_annotation
            .as_ref()
            .expect("decide_following called with a pending annotation");
        match classify_following(&self.preceding, annotation, &self.following, flush) {
            FollowingMatch::NeedMore => return,
            FollowingMatch::NoMatch => {
                if !self.preceding.is_empty() {
                    output.push(OutputToken::Text(core::mem::take(&mut self.preceding)));
                }
                output.push(OutputToken::Annotated(
                    self.pending_annotation.take().expect("pending annotation"),
                ));
                // The following text run continues as ordinary trailing text.
                self.held_tail = core::mem::take(&mut self.following);
            }
            FollowingMatch::HanjaFirst {
                collapsed,
                leftover,
            } => {
                // The preceding text is unrelated; emit it verbatim.
                if !self.preceding.is_empty() {
                    output.push(OutputToken::Text(core::mem::take(&mut self.preceding)));
                }
                output.push(OutputToken::Annotated(collapsed));
                self.pending_annotation = None;
                self.held_tail = leftover;
                self.following.clear();
            }
            FollowingMatch::HangulFirst {
                remaining_preceding,
                collapsed,
                leftover,
            } => {
                if !remaining_preceding.is_empty() {
                    output.push(OutputToken::Text(remaining_preceding));
                }
                output.push(OutputToken::Annotated(collapsed));
                self.pending_annotation = None;
                self.preceding.clear();
                self.held_tail = leftover;
                self.following.clear();
            }
        }
        self.emit_held_prefix(output);
    }
}

/// Upper bound on how many trailing hangul syllables are held as a hangul-first
/// reading candidate.  A Sino-Korean reading written before `(` is at most a
/// handful of syllables; this generous cap keeps `held_tail` bounded even for a
/// pathological space-free hangul run (the only cost of exceeding it is that an
/// implausibly long reading is not collapsed).
const MAX_PRECEDING_READING_CHARS: usize = 64;

/// Byte index where the matchable suffix of a held text run begins: up to
/// [`MAX_PRECEDING_READING_CHARS`] trailing hangul syllables plus an optional
/// final `(`.  Everything before this index can be emitted because it can no
/// longer be the preceding text of a hangul-first match.
fn hangul_first_tail_start(text: &str) -> usize {
    let mut start = text.len();
    let mut chars = text.char_indices().rev().peekable();
    if let Some(&(index, '(')) = chars.peek() {
        start = index;
        chars.next();
    }
    let mut held = 0;
    while held < MAX_PRECEDING_READING_CHARS {
        match chars.peek() {
            Some(&(index, ch)) if is_hangul_syllable(ch) => {
                start = index;
                held += 1;
                chars.next();
            }
            _ => break,
        }
    }
    start
}

/// Buffered counterpart to [`RedundantParenCollapser`] for non-streaming
/// callers, mirroring [`mark_homophones_with_detection`] and
/// [`filter_first_occurrences`].
pub fn collapse_redundant_parens<S>(
    tokens: impl IntoIterator<Item = OutputToken<S>>,
    enabled: bool,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
{
    if !enabled {
        return tokens.into_iter().collect();
    }
    let mut collapser = RedundantParenCollapser::new(true);
    let mut output = Vec::new();
    for token in tokens {
        output.extend(collapser.push_token(token));
    }
    output.extend(collapser.finish());
    output
}

/// Classification of a parenthetical hangul string against an annotation.
enum ReadingMatch {
    /// The parenthetical equals the annotation's reading; keep the reading.
    Keep,
    /// The parenthetical is a valid alternative reading; override with it.
    Override(String),
}

/// Classifies the parenthetical hangul `candidate` against an annotation's
/// `hanja`/`reading`, returning `None` when it is neither the reading nor a
/// valid alternative reading (so the tokens are left untouched).
fn classify_reading(hanja: &str, reading: &str, candidate: &str) -> Option<ReadingMatch> {
    if candidate == reading {
        Some(ReadingMatch::Keep)
    } else if is_valid_alternative_reading(hanja, candidate) {
        Some(ReadingMatch::Override(candidate.to_string()))
    } else {
        None
    }
}

/// Returns whether `candidate` is a valid Sino-Korean reading of `hanja`: one
/// hangul syllable per hanja character, each a recorded Unihan reading of its
/// character or the initial-sound-law variant of one.
fn is_valid_alternative_reading(hanja: &str, candidate: &str) -> bool {
    let mut hanja_chars = hanja.chars();
    let mut candidate_chars = candidate.chars();
    let mut matched_any = false;
    loop {
        match (hanja_chars.next(), candidate_chars.next()) {
            (Some(hanja_char), Some(syllable)) => {
                if !is_valid_char_reading(hanja_char, syllable) {
                    return false;
                }
                matched_any = true;
            }
            (None, None) => return matched_any,
            // Differing lengths: not a one-syllable-per-character reading.
            _ => return false,
        }
    }
}

/// Returns whether `syllable` is a valid reading of the source character
/// `source`: a recorded Unihan reading (or its initial-sound-law 頭音法則
/// variant) when `source` is a hanja character, or the same syllable verbatim
/// when `source` is itself hangul (as in a mixed-script entry such as `色깔論`).
fn is_valid_char_reading(source: char, syllable: char) -> bool {
    if !is_hangul_syllable(syllable) {
        return false;
    }
    let readings = khangul_all_readings(source);
    if readings.is_empty() {
        // No recorded Sino-Korean reading: the source is the hangul portion of
        // a mixed-script entry (or otherwise non-hanja), so it must appear
        // verbatim in the candidate reading.
        return source == syllable;
    }
    readings.iter().any(|reading| {
        reading_is_syllable(reading, syllable)
            || reading_matches_with_initial_sound_law(reading, syllable)
    })
}

/// Returns whether the single-syllable `reading` is exactly `syllable`.
fn reading_is_syllable(reading: &str, syllable: char) -> bool {
    let mut chars = reading.chars();
    chars.next() == Some(syllable) && chars.next().is_none()
}

/// Builds the collapsed annotation: both presentation flags set, with the
/// reading overridden when the parenthetical pinned an alternative one.
fn collapse_annotation(mut annotation: Annotation, reading_match: ReadingMatch) -> Annotation {
    if let ReadingMatch::Override(reading) = reading_match {
        annotation.reading = reading;
    }
    annotation.require_hanja = true;
    annotation.require_hangul = true;
    annotation.from_source_gloss = true;
    annotation
}

/// Outcome of classifying a pending annotation against the text that follows
/// it (and, for the hangul-first pattern, the text that precedes it).
enum FollowingMatch {
    /// The following text is an incomplete parenthetical; buffer more.
    NeedMore,
    /// Neither pattern applies; emit the tokens unchanged.
    NoMatch,
    /// Hanja-first `Annotated` + `(R)`: collapse, keeping the text after `)`.
    HanjaFirst {
        collapsed: Annotation,
        leftover: String,
    },
    /// Hangul-first `R(` + `Annotated` + `)`: collapse, keeping the preceding
    /// text before `R(` and the following text after `)`.
    HangulFirst {
        remaining_preceding: String,
        collapsed: Annotation,
        leftover: String,
    },
}

/// Classifies a pending annotation against the accumulated `preceding` and
/// `following` text.
///
/// `following` is coalesced across adjacent text tokens; the hanja-first arm
/// buffers (returns [`FollowingMatch::NeedMore`]) until it sees the closing `)`
/// or can rule a match out, which keeps the buffer bounded by the longest
/// possible reading.  With `flush` set (a boundary or EOF ended the run) an
/// otherwise-undecidable parenthetical is treated as a non-match.
fn classify_following(
    preceding: &str,
    annotation: &Annotation,
    following: &str,
    flush: bool,
) -> FollowingMatch {
    let Some(first) = following.chars().next() else {
        return if flush {
            FollowingMatch::NoMatch
        } else {
            FollowingMatch::NeedMore
        };
    };
    match first {
        ')' => match match_hangul_first(preceding, annotation, following) {
            Some((remaining_preceding, collapsed)) => FollowingMatch::HangulFirst {
                remaining_preceding,
                collapsed,
                leftover: following[')'.len_utf8()..].to_string(),
            },
            None => FollowingMatch::NoMatch,
        },
        '(' => {
            let content = &following['('.len_utf8()..];
            match content.find(')') {
                Some(close) => {
                    let candidate = &content[..close];
                    match classify_reading(&annotation.hanja, &annotation.reading, candidate) {
                        Some(reading_match) => FollowingMatch::HanjaFirst {
                            collapsed: collapse_annotation(annotation.clone(), reading_match),
                            leftover: content[close + ')'.len_utf8()..].to_string(),
                        },
                        None => FollowingMatch::NoMatch,
                    }
                }
                None => {
                    // A reading is at most max(reading, hanja) syllables long, so
                    // once the unclosed content exceeds that it cannot match.
                    let max_reading = annotation
                        .reading
                        .chars()
                        .count()
                        .max(annotation.hanja.chars().count());
                    if flush || content.chars().count() > max_reading {
                        FollowingMatch::NoMatch
                    } else {
                        FollowingMatch::NeedMore
                    }
                }
            }
        }
        _ => FollowingMatch::NoMatch,
    }
}

/// Matches the hangul-first pattern preceding `Text("…R(")` + `Annotated` +
/// following `Text(")…")`.  On success returns the preceding text remaining
/// after stripping `R(` and the collapsed annotation.
fn match_hangul_first(
    preceding: &str,
    annotation: &Annotation,
    following: &str,
) -> Option<(String, Annotation)> {
    if !following.starts_with(')') {
        return None;
    }
    let before = preceding.strip_suffix('(')?;

    // Tier 1: the text just before `(` ends with the annotation's reading.
    if !annotation.reading.is_empty()
        && let Some(remaining) = before.strip_suffix(&annotation.reading)
    {
        let collapsed = collapse_annotation(annotation.clone(), ReadingMatch::Keep);
        return Some((remaining.to_string(), collapsed));
    }

    // Tier 2: the trailing hanja-character count of hangul syllables form a
    // valid alternative reading.  Slice `before` directly at the byte boundary
    // of those trailing syllables rather than collecting it into a `Vec<char>`.
    let syllable_count = annotation.hanja.chars().count();
    if syllable_count == 0 {
        return None;
    }
    let (split, _) = before.char_indices().rev().nth(syllable_count - 1)?;
    let candidate = &before[split..];
    let reading_match = classify_reading(&annotation.hanja, &annotation.reading, candidate)?;
    Some((
        before[..split].to_string(),
        collapse_annotation(annotation.clone(), reading_match),
    ))
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
                .insert(
                    annotation
                        .dictionary_hanja
                        .as_ref()
                        .unwrap_or(&annotation.hanja)
                        .clone(),
                );
        }
    }

    for token in tokens.iter_mut() {
        if let OutputToken::Annotated(annotation) = token {
            annotation.homophone = annotation.from_dictionary
                && (index.is_some_and(|index| {
                    index.has_homophone(
                        annotation
                            .dictionary_hanja
                            .as_deref()
                            .unwrap_or(&annotation.hanja),
                        &annotation.reading,
                    )
                }) || lookup_fallback.is_some_and(|dictionary| {
                    dictionary.has_homophone(
                        annotation
                            .dictionary_hanja
                            .as_deref()
                            .unwrap_or(&annotation.hanja),
                        &annotation.reading,
                    )
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
            if seen.insert(
                annotation
                    .dictionary_hanja
                    .as_ref()
                    .unwrap_or(&annotation.hanja)
                    .clone(),
            ) {
                annotation.first_in_context = true;
            } else {
                annotation.first_in_context = false;
                // An explicit parenthetical gloss is the author asking for the
                // annotation at every occurrence, so its requirements survive
                // first-occurrence clearing; dictionary requirements do not.
                if !annotation.from_source_gloss {
                    annotation.require_hanja = false;
                    annotation.require_hangul = false;
                }
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
    RendererIter {
        upstream: tokens.into_iter(),
        renderer: Renderer::new(options),
    }
}

/// Stateful renderer for chunked [`OutputToken`] streams.
///
/// `Renderer` is the push-based counterpart to [`render_tokens_iter`]. It
/// preserves the active scope stack across calls so format writers can consume
/// rendered tokens as soon as upstream engine and middleware stages release
/// them, without losing inline-markup restrictions from earlier chunks.
pub struct Renderer<S>
where
    S: ScopeData,
{
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

impl<S> Renderer<S>
where
    S: ScopeData,
{
    /// Creates a renderer with the supplied rendering options.
    pub fn new<O>(options: O) -> Self
    where
        O: Into<RenderOptions>,
    {
        Self {
            options: options.into(),
            markup_stack: Vec::new(),
            disallowing_ancestors: 0,
            _scope: PhantomData,
        }
    }

    /// Pushes one output token and returns its rendered counterpart.
    pub fn push_token(&mut self, token: OutputToken<S>) -> RenderedToken<S> {
        match token {
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
        }
    }
}

struct RendererIter<I, S>
where
    S: ScopeData,
{
    upstream: I,
    renderer: Renderer<S>,
}

impl<I, S> Iterator for RendererIter<I, S>
where
    I: Iterator<Item = OutputToken<S>>,
    S: ScopeData,
{
    type Item = RenderedToken<S>;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.upstream.next()?;
        Some(self.renderer.push_token(token))
    }
}

fn render_annotation<S>(
    annotation: &Annotation,
    options: &RenderOptions,
    allows_inline_markup: bool,
) -> RenderedToken<S> {
    if annotation.skip_annotation
        && matches!(
            options.mode,
            RenderMode::HangulOnly
                | RenderMode::HangulHanjaParens
                | RenderMode::Ruby(RubyBase::OnHangul)
        )
    {
        return RenderedToken::Text(annotation.reading.clone());
    }
    if !annotation.skip_annotation
        && options.mode == RenderMode::HangulOnly
        && !annotation.require_hanja
        && !annotation.homophone
    {
        return RenderedToken::Text(annotation.reading.clone());
    }
    let dictionary_hanja = annotation
        .dictionary_hanja
        .as_deref()
        .unwrap_or(&annotation.hanja);
    let rendered_hanja = variants::render_hanja(dictionary_hanja, options.hanja_variant_set);
    if annotation.skip_annotation {
        return RenderedToken::Text(rendered_hanja.into_owned());
    }

    match options.mode {
        RenderMode::HangulOnly => {
            RenderedToken::Text(parens(&annotation.reading, rendered_hanja.as_ref()))
        }
        RenderMode::HangulHanjaParens => {
            RenderedToken::Text(parens(&annotation.reading, rendered_hanja.as_ref()))
        }
        RenderMode::HanjaHangulParens => {
            RenderedToken::Text(parens(rendered_hanja.as_ref(), &annotation.reading))
        }
        RenderMode::Ruby(base) => render_ruby(
            annotation,
            rendered_hanja.as_ref(),
            base,
            allows_inline_markup,
        ),
        RenderMode::Original if annotation.require_hangul => match options.original_gloss {
            OriginalGloss::Parens => {
                RenderedToken::Text(parens(rendered_hanja.as_ref(), &annotation.reading))
            }
            // `Original` keeps hanja as the primary text, so its ruby form
            // always uses hanja as the base regardless of any other setting.
            OriginalGloss::Ruby => render_ruby(
                annotation,
                rendered_hanja.as_ref(),
                RubyBase::OnHanja,
                allows_inline_markup,
            ),
        },
        RenderMode::Original => RenderedToken::Text(rendered_hanja.into_owned()),
    }
}

fn render_ruby<S>(
    annotation: &Annotation,
    hanja: &str,
    base: RubyBase,
    allows_inline_markup: bool,
) -> RenderedToken<S> {
    let (base_text, rt_text) = match base {
        RubyBase::OnHangul => (annotation.reading.as_str(), hanja),
        RubyBase::OnHanja => (hanja, annotation.reading.as_str()),
    };
    if !allows_inline_markup {
        return RenderedToken::Text(parens(base_text, rt_text));
    }
    RenderedToken::Ruby {
        base: String::from(base_text),
        rt: String::from(rt_text),
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
///
/// Like the high-level umbrella default, this collapses redundant parenthetical
/// reading annotations ([`RedundantParenCollapser`]); callers that need finer
/// control (including disabling that step) should drive the individual stages
/// instead.
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
    let output_tokens = collapse_redundant_parens(output_tokens, true);
    let output_tokens = mark_homophones(output_tokens, dictionary, ContextWindow::PerBlock);
    let rendered_tokens = render_tokens(output_tokens, render);
    write_plain_text(rendered_tokens)
}
