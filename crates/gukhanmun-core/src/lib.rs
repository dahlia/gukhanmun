//! Core types and algorithms for gukhanmun.
//!
//! This crate is the home for the format-neutral intermediate representation,
//! conversion engine, dictionary traits, lattice segmentation, and fallback
//! hanja reading logic. Format adapters, command-line I/O, and language
//! bindings live in separate crates.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

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

    /// Whether another known hanja form shares this reading.
    pub homophone: bool,

    /// Whether the original hanja should be visible in rendered output.
    pub require_hanja: bool,

    /// Whether a hangul gloss should be visible when hanja remains primary.
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
/// supplied string. This shape supports the later lattice segmenter; the MVP
/// engine currently selects the longest returned match.
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct DictionaryEntry {
    reading: String,
    mark: MatchMark,
}

/// A small in-memory dictionary backed by an ordered map.
///
/// This implementation is intended for tests, user-supplied custom entries,
/// and early pipeline validation. It returns all prefix matches at a cursor so
/// the engine can later swap greedy selection for lattice segmentation without
/// changing the dictionary contract.
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

/// Processes input tokens with the MVP hanja conversion engine.
///
/// The engine preserves structural and verbatim tokens, skips text under any
/// preserving scope, and annotates the longest dictionary match found inside a
/// contiguous hanja run. Unknown hanja text is preserved unchanged.
pub fn process_tokens<S, D>(
    tokens: impl IntoIterator<Item = InputToken<S>>,
    dictionary: &D,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
    D: HanjaDictionary + ?Sized,
{
    let mut output = Vec::new();
    let mut scopes = Vec::new();

    for token in tokens {
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
                if scopes.iter().any(|scope| scope.data().is_preserve()) {
                    output.push(OutputToken::Text(text));
                } else {
                    process_text(&text, dictionary, &mut output);
                }
            }
            InputToken::Verbatim(text) => output.push(OutputToken::Verbatim(text)),
        }
    }

    output
}

fn process_text<S, D>(text: &str, dictionary: &D, output: &mut Vec<OutputToken<S>>)
where
    D: HanjaDictionary + ?Sized,
{
    let mut cursor = 0;

    while cursor < text.len() {
        let rest = &text[cursor..];
        let Some(first) = rest.chars().next() else {
            break;
        };

        if !is_hanja(first) {
            let next = cursor + first.len_utf8();
            push_text(output, &text[cursor..next]);
            cursor = next;
            continue;
        }

        if let Some(best_match) = longest_match(dictionary.matches_at(rest)) {
            let hanja = &rest[..best_match.byte_len];
            output.push(OutputToken::Annotated(Annotation {
                hanja: hanja.to_string(),
                reading: best_match.reading.clone(),
                homophone: dictionary.has_homophone(hanja, &best_match.reading),
                require_hanja: best_match.mark.require_hanja,
                require_hangul: best_match.mark.require_hangul,
                first_in_context: true,
                from_dictionary: true,
            }));
            cursor += best_match.byte_len;
        } else {
            let next = cursor + first.len_utf8();
            push_text(output, &text[cursor..next]);
            cursor = next;
        }
    }
}

fn longest_match(matches: impl IntoIterator<Item = Match>) -> Option<Match> {
    matches.into_iter().max_by_key(|matched| matched.byte_len)
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

/// Returns whether `ch` is in the MVP hanja character range.
///
/// This covers the basic CJK Unified Ideographs block used by the current core
/// tests. Wider Unicode coverage belongs to the fallback phoneticizer phase.
pub fn is_hanja(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
}

/// The concrete rendering mode for annotated hanja words.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderMode {
    /// Emits only hangul unless annotation flags require hanja disambiguation.
    HangulOnly,

    /// Always emits hangul followed by the original hanja in parentheses.
    HangulHanjaParens,
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
    let input_tokens = read_plain_text(input);
    let output_tokens = process_tokens(input_tokens, dictionary);
    let rendered_tokens = render_tokens(output_tokens, mode);
    write_plain_text(rendered_tokens)
}
