// Gukhanmun: Output punctuation profiles and streaming normalization.
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

//! Output punctuation profiles and format-neutral normalization.

use crate::{OutputToken, ScopeData};
use alloc::{
    collections::VecDeque,
    string::{String, ToString},
    vec::Vec,
};

/// Space policy following an eligible comma or period. Only U+0020 is changed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "kebab-case")
)]
pub enum SpaceAfter {
    /// Keep the original run and position of ordinary spaces.
    #[default]
    Preserve,
    /// Remove ordinary spaces, including before a boundary or EOF.
    Remove,
    /// Use one space before ordinary content; trim before a closing boundary.
    Single,
}
/// Output mark and trailing-space policy for one stop role.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct StopStyle {
    /// Exactly one Unicode scalar used as the output mark.
    pub mark: char,
    /// Policy for following U+0020 characters.
    pub after: SpaceAfter,
}
impl StopStyle {
    /// Creates a style; profile validation checks the mark's category and role.
    pub const fn new(mark: char, after: SpaceAfter) -> Self {
        Self { mark, after }
    }
}
/// Output opening and closing marks for one delimiter role.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct DelimiterStyle {
    /// Opening punctuation (Unicode Ps or Pi).
    pub open: char,
    /// Closing punctuation (Unicode Pe or Pf).
    pub close: char,
}
impl DelimiterStyle {
    /// Creates a delimiter style; profile validation checks both directions.
    pub const fn new(open: char, close: char) -> Self {
        Self { open, close }
    }
}
/// Independently selectable output styles. `None` preserves that input role.
///
/// Input recognition is fixed and independent of these output marks. Custom
/// marks outside the input alphabet are not inferred on subsequent conversions.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct PunctuationProfile {
    /// Comma normalization, or preservation.
    pub comma: Option<StopStyle>,
    /// Period normalization, or preservation.
    pub period: Option<StopStyle>,
    /// Single delimiter normalization, or preservation.
    pub single: Option<DelimiterStyle>,
    /// Double delimiter normalization, or preservation.
    pub double: Option<DelimiterStyle>,
}
/// Invalid output profile, separate from engine conversion errors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid punctuation profile: {0}")]
pub struct PunctuationConfigError(pub &'static str);
impl PunctuationProfile {
    /// Preserves all punctuation and bypasses normalization.
    pub const fn preserve() -> Self {
        Self {
            comma: None,
            period: None,
            single: None,
            double: None,
        }
    }
    /// Modern horizontal Korean punctuation with contextual ordinary spaces.
    pub const fn modern() -> Self {
        Self::styles(',', '.', '〈', '〉', '《', '》', SpaceAfter::Single)
    }
    /// Traditional horizontal mixed-script Korean punctuation.
    pub const fn traditional() -> Self {
        Self::styles('、', '。', '「', '」', '『', '』', SpaceAfter::Remove)
    }
    /// Explicit vertical presentation forms; does not configure CSS layout.
    pub const fn traditional_vertical() -> Self {
        Self::styles('︑', '︒', '﹁', '﹂', '﹃', '﹄', SpaceAfter::Remove)
    }
    const fn styles(
        a: char,
        b: char,
        c: char,
        d: char,
        e: char,
        f: char,
        after: SpaceAfter,
    ) -> Self {
        Self {
            comma: Some(StopStyle::new(a, after)),
            period: Some(StopStyle::new(b, after)),
            single: Some(DelimiterStyle::new(c, d)),
            double: Some(DelimiterStyle::new(e, f)),
        }
    }
    /// Resolves a built-in name. Unknown names are rejected.
    pub fn preset(name: &str) -> Result<Self, PunctuationConfigError> {
        match name {
            "preserve" => Ok(Self::preserve()),
            "modern" => Ok(Self::modern()),
            "traditional" => Ok(Self::traditional()),
            "traditional-vertical" => Ok(Self::traditional_vertical()),
            _ => Err(PunctuationConfigError("unknown preset")),
        }
    }
    /// Validates Unicode categories, distinct roles, and fixed-input collisions.
    pub fn validate(self) -> Result<Self, PunctuationConfigError> {
        let marks = [
            self.comma.map(|x| x.mark),
            self.period.map(|x| x.mark),
            self.single.map(|x| x.open),
            self.single.map(|x| x.close),
            self.double.map(|x| x.open),
            self.double.map(|x| x.close),
        ];
        for (i, mark) in marks.iter().enumerate() {
            let Some(c) = *mark else { continue };
            let cat = category(c);
            let valid = match i {
                2 | 4 => matches!(cat, "Ps" | "Pi"),
                3 | 5 => matches!(cat, "Pe" | "Pf"),
                _ => {
                    (cat.starts_with('P') || cat.starts_with('S'))
                        && !matches!(cat, "Ps" | "Pi" | "Pe" | "Pf")
                }
            };
            if !valid || matches!(c, '<' | '>' | '&' | '\u{fffd}') {
                return Err(PunctuationConfigError(
                    "mark has an invalid category or direction",
                ));
            }
            if input_role(c).is_some_and(|role| role != i) {
                return Err(PunctuationConfigError(
                    "mark collides with another input role",
                ));
            }
            if marks[..i].contains(&Some(c)) {
                return Err(PunctuationConfigError("output roles must be distinct"));
            }
        }
        Ok(self)
    }
    fn replacement(self, role: usize) -> Option<(char, SpaceAfter)> {
        match role {
            0 => self.comma.map(|s| (s.mark, s.after)),
            1 => self.period.map(|s| (s.mark, s.after)),
            2 => self.single.map(|s| (s.open, SpaceAfter::Preserve)),
            3 => self.single.map(|s| (s.close, SpaceAfter::Preserve)),
            4 => self.double.map(|s| (s.open, SpaceAfter::Preserve)),
            5 => self.double.map(|s| (s.close, SpaceAfter::Preserve)),
            _ => None,
        }
    }
}
/// Boundary policy exposed by format adapters for punctuation lookahead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PunctuationBoundary {
    /// Space lookahead may cross this scope; technical guards reset here.
    Transparent,
    /// Trim contextual trailing spaces at a block boundary.
    Block,
    /// Trim contextual trailing spaces at an explicit line break.
    LineBreak,
    /// Keep existing spaces without inserting new spaces across this boundary.
    Opaque,
    /// Skip punctuation in this subtree, independently of hanja conversion.
    Preserve,
}
/// A raw source span and its once-decoded text value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PunctuationTextUnit {
    /// Original spelling, retained when this unit is not normalized.
    pub raw: String,
    /// One or more decoded scalars used exclusively for punctuation context.
    pub decoded: String,
}
/// Format-owned incremental text codec, keeping markup out of the core.
///
/// `finish` ends a contiguous text region, not merely an I/O chunk. A codec may
/// retain an incomplete character reference between calls. Returned raw spans
/// must partition the supplied input exactly once.
pub trait PunctuationTextCodec {
    /// Decodes the next text chunk, or finalizes it at a structural boundary.
    fn decode(&mut self, text: &str, finish: bool) -> Vec<PunctuationTextUnit>;
    /// Serializes a newly generated mark safely for the target text format.
    fn encode(&self, text: &str) -> String {
        text.to_string()
    }
}
/// Literal text codec for plain text and already-decoded Markdown text.
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteralTextCodec;
impl PunctuationTextCodec for LiteralTextCodec {
    fn decode(&mut self, text: &str, _: bool) -> Vec<PunctuationTextUnit> {
        text.chars()
            .map(|c| {
                let s = c.to_string();
                PunctuationTextUnit {
                    raw: s.clone(),
                    decoded: s,
                }
            })
            .collect()
    }
}

pub(crate) fn category(c: char) -> &'static str {
    let table = super::punctuation_categories::CATEGORIES;
    let i = table.partition_point(|&(start, _, _)| start <= c as u32);
    if i > 0 && c as u32 <= table[i - 1].1 {
        table[i - 1].2
    } else {
        "Cn"
    }
}
fn input_role(c: char) -> Option<usize> {
    match c {
        ',' | '，' | '、' | '︐' | '︑' => Some(0),
        '.' | '．' | '。' | '︒' => Some(1),
        '〈' | '〈' | '「' | '︿' | '﹁' => Some(2),
        '〉' | '〉' | '」' | '﹀' | '﹂' => Some(3),
        '《' | '『' | '︽' | '﹃' => Some(4),
        '》' | '』' | '︾' | '﹄' => Some(5),
        _ => None,
    }
}
fn alnum(c: char) -> bool {
    c.is_ascii_alphanumeric() || ('０'..='９').contains(&c) || latin(c)
}
fn latin(c: char) -> bool {
    c.is_ascii_alphabetic() || ('Ａ'..='Ｚ').contains(&c) || ('ａ'..='ｚ').contains(&c)
}
fn digit(c: char) -> bool {
    c.is_ascii_digit() || ('０'..='９').contains(&c)
}
fn spacing(c: char) -> Option<bool> {
    if c.is_whitespace()
        || matches!(input_role(c), Some(0 | 1 | 3 | 5))
        || matches!(c, '!' | '?' | ';' | ':' | '！' | '？' | '；' | '：')
    {
        return Some(false);
    }
    if c == '’' {
        return None;
    }
    match category(c) {
        "Ps" | "Pi" => Some(true),
        "Pe" | "Pf" => Some(false),
        cat if cat.starts_with('P') || cat.starts_with('S') => None,
        _ => Some(true),
    }
}

struct QueuedUnit {
    unit: PunctuationTextUnit,
    repeats: usize,
}
impl core::ops::Deref for QueuedUnit {
    type Target = PunctuationTextUnit;
    fn deref(&self) -> &Self::Target {
        &self.unit
    }
}
enum Deferred<S> {
    Token(OutputToken<S>),
    Spaces { raw: String, repeats: usize },
}
struct PunctuationFrame {
    boundary: PunctuationBoundary,
    preserve: bool,
    force_subtree: bool,
}
struct PendingSpace<S> {
    after: SpaceAfter,
    count: usize,
    events: Vec<Deferred<S>>,
    min_depth: usize,
    insertion: usize,
}
/// Stateful post-annotation punctuation middleware.
///
/// Call `finish` only after upstream middleware has finished. Ordinary text
/// chunks share lexical context; all structural events reset technical guards.
/// Each scope's [`ScopeData::is_preserve`] is an already-resolved effective
/// policy; only [`PunctuationBoundary::Preserve`] forces preservation through
/// descendants. Space lookahead crosses at most 256 transparent structural events. A single
/// unfinished reference or varying encoded space spellings can require unbounded
/// storage. Repeated identical space spellings are stored as run lengths.
pub struct PunctuationNormalizer<S, C = LiteralTextCodec> {
    profile: PunctuationProfile,
    codec: C,
    units: VecDeque<QueuedUnit>,
    previous: Option<char>,
    url: bool,
    period_run: bool,
    stack: Vec<PunctuationFrame>,
    pending: Option<PendingSpace<S>>,
    comma_scan: usize,
}
impl<S: ScopeData> PunctuationNormalizer<S> {
    /// Creates a middleware using literal text. Validate custom profiles first.
    pub fn new(profile: PunctuationProfile) -> Self {
        Self::with_codec(profile, LiteralTextCodec)
    }
}
impl<S: ScopeData, C: PunctuationTextCodec> PunctuationNormalizer<S, C> {
    /// Creates middleware with a format-specific text codec.
    pub fn with_codec(profile: PunctuationProfile, codec: C) -> Self {
        Self {
            profile,
            codec,
            units: VecDeque::new(),
            previous: None,
            url: false,
            period_run: false,
            stack: Vec::new(),
            pending: None,
            comma_scan: 1,
        }
    }
    fn append_units(&mut self, units: Vec<PunctuationTextUnit>) {
        for unit in units {
            if unit.decoded == " "
                && let Some(last) = self.units.back_mut()
                && last.decoded == " "
                && last.raw == unit.raw
            {
                last.repeats += 1;
            } else {
                self.units.push_back(QueuedUnit { unit, repeats: 1 });
            }
        }
    }
    fn preserved(&self) -> bool {
        self.stack.last().is_some_and(|x| x.preserve)
    }
    /// Pushes one output token and returns the available normalized tokens.
    pub fn push_token(&mut self, token: OutputToken<S>) -> Vec<OutputToken<S>> {
        if self.profile == PunctuationProfile::preserve() {
            return alloc::vec![token];
        }
        let mut out = Vec::new();
        if let OutputToken::Text(text) = token {
            if text.is_empty() {
                return out;
            }
            if self.preserved() {
                self.resolve_space(None, &mut out);
                out.push(OutputToken::Text(text));
            } else {
                let units = self.codec.decode(&text, false);
                self.append_units(units);
                self.process(false, &mut out);
            }
            return out;
        }
        let units = self.codec.decode("", true);
        self.append_units(units);
        self.process(true, &mut out);
        if let OutputToken::Annotated(annotation) = &token {
            self.resolve_space(
                annotation.reading.chars().next().and_then(spacing),
                &mut out,
            );
        }
        self.previous = None;
        self.url = false;
        self.period_run = false;
        let boundary = match &token {
            OutputToken::Open(s) => s.data().punctuation_boundary(),
            OutputToken::Close => self
                .stack
                .last()
                .map_or(PunctuationBoundary::Opaque, |x| x.boundary),
            _ => PunctuationBoundary::Opaque,
        };
        if boundary != PunctuationBoundary::Transparent {
            self.resolve_space(
                match boundary {
                    PunctuationBoundary::Block | PunctuationBoundary::LineBreak => Some(false),
                    _ => None,
                },
                &mut out,
            );
        }
        match &token {
            OutputToken::Open(s) => {
                let force_subtree = self.stack.last().is_some_and(|f| f.force_subtree)
                    || boundary == PunctuationBoundary::Preserve;
                // Reader flags already resolve language overrides. Only a
                // punctuation-specific subtree rule (e.g. image alt) is inherited.
                let preserve = s.data().is_preserve() || force_subtree;
                self.stack.push(PunctuationFrame {
                    boundary,
                    preserve,
                    force_subtree,
                });
            }
            OutputToken::Close => {
                self.stack.pop();
            }
            _ => {}
        }
        if self.pending.as_ref().is_some_and(|p| p.count == 256) {
            self.resolve_space(None, &mut out);
        }
        if let Some(p) = &mut self.pending {
            p.events.push(Deferred::Token(token));
            p.count += 1;
            let depth = self.stack.len();
            if depth < p.min_depth {
                p.min_depth = depth;
                p.insertion = p.events.len();
            }
        } else {
            out.push(token);
        }
        out
    }
    fn emit_text(out: &mut Vec<OutputToken<S>>, text: String) {
        if text.is_empty() {
            return;
        }
        if let Some(OutputToken::Text(last)) = out.last_mut() {
            last.push_str(&text)
        } else {
            out.push(OutputToken::Text(text))
        }
    }
    fn resolve_space(&mut self, next: Option<bool>, out: &mut Vec<OutputToken<S>>) {
        let Some(p) = self.pending.take() else { return };
        let insert = p.after == SpaceAfter::Single && next == Some(true);
        let keep =
            p.after == SpaceAfter::Preserve || (p.after == SpaceAfter::Single && next.is_none());
        if keep {
            for event in p.events {
                match event {
                    Deferred::Token(t) => out.push(t),
                    Deferred::Spaces { raw, repeats } => Self::emit_text(out, raw.repeat(repeats)),
                }
            }
        } else {
            let len = p.events.len();
            for (i, event) in p.events.into_iter().enumerate() {
                if insert && i == p.insertion {
                    Self::emit_text(out, self.codec.encode(" "));
                }
                if let Deferred::Token(token) = event {
                    out.push(token);
                }
            }
            if insert && p.insertion == len {
                Self::emit_text(out, self.codec.encode(" "));
            }
        }
    }
    fn process(&mut self, finish: bool, out: &mut Vec<OutputToken<S>>) {
        while let Some(unit) = self.units.front() {
            let c = unit.decoded.chars().next().unwrap_or('\0');
            // Prefix detection uses bounded lookahead and never decodes recursively.
            let prefix: String = self
                .units
                .iter()
                .take(8)
                .flat_map(|u| u.decoded.chars())
                .take(8)
                .collect();
            let lower = prefix.to_ascii_lowercase();
            let prefixes = ["http://", "https://", "mailto:", "www."];
            if !finish && prefixes.iter().any(|p| p.starts_with(&lower)) && lower.len() < 8 {
                break;
            }
            if prefixes.iter().any(|p| lower.starts_with(p)) {
                self.url = true;
            }
            if c.is_whitespace() {
                self.url = false;
            }
            let next = self.units.get(1).and_then(|u| u.decoded.chars().next());
            let role = if unit.decoded.chars().count() == 1 {
                input_role(c)
            } else {
                None
            };
            if !finish && role.is_some() && next.is_none() {
                break;
            }
            let mut protected = self.url;
            if let Some(r @ (0 | 1)) = role {
                if matches!(c, ',' | '，' | '.' | '．')
                    && self.previous.is_some_and(digit)
                    && next.is_some_and(digit)
                {
                    protected = true;
                }
                if r == 1 {
                    protected |= self.period_run || next.is_some_and(|c| input_role(c) == Some(1));
                    if matches!(c, '.' | '．') {
                        protected |= self.previous.is_some_and(alnum) || next.is_some_and(alnum);
                    }
                } else if matches!(c, ',' | '，') && self.previous.is_some_and(alnum) {
                    protected |= next.is_some_and(alnum);
                    if next == Some(' ') {
                        let following = self
                            .units
                            .iter()
                            .skip(self.comma_scan)
                            .find_map(|u| u.decoded.chars().find(|&c| c != ' '));
                        if following.is_none() {
                            self.comma_scan = self.units.len();
                        }
                        if following.is_none() && !finish {
                            break;
                        }
                        protected |= following.is_some_and(latin);
                    }
                }
            }
            let queued = self.units.pop_front().unwrap();
            let repeats = queued.repeats;
            let unit = queued.unit;
            self.comma_scan = 1;
            if let Some(pending) = self.pending.as_mut()
                && unit.decoded == " "
            {
                if let Some(Deferred::Spaces {
                    raw,
                    repeats: count,
                }) = pending.events.last_mut()
                    && *raw == unit.raw
                {
                    *count += repeats;
                } else {
                    pending.events.push(Deferred::Spaces {
                        raw: unit.raw,
                        repeats,
                    });
                }
            } else {
                self.resolve_space(spacing(c), out);
                if let Some((mark, after)) = role
                    .filter(|_| !protected)
                    .and_then(|r| self.profile.replacement(r))
                {
                    Self::emit_text(out, self.codec.encode(&mark.to_string()));
                    if matches!(role, Some(0 | 1)) && after != SpaceAfter::Preserve {
                        let depth = self.stack.len();
                        self.pending = Some(PendingSpace {
                            after,
                            count: 0,
                            events: Vec::new(),
                            min_depth: depth,
                            insertion: 0,
                        });
                    }
                } else {
                    Self::emit_text(out, unit.raw.repeat(repeats));
                }
            }
            self.previous = unit.decoded.chars().last();
            self.period_run = role == Some(1);
        }
    }
    /// Finalizes text and discards contextual trailing spaces at document EOF.
    pub fn finish(mut self) -> Vec<OutputToken<S>> {
        let mut out = Vec::new();
        let units = self.codec.decode("", true);
        self.append_units(units);
        self.process(true, &mut out);
        self.resolve_space(Some(false), &mut out);
        out
    }
}
/// Serializable punctuation configuration shared by CLI and JavaScript bindings.
///
/// A string selects a preset. An object inherits `base` (default `preserve`);
/// omitted roles inherit while explicit `null` preserves that role.
#[cfg(feature = "serde")]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum PunctuationConfig {
    /// Built-in preset name.
    Preset(String),
    /// Partial custom profile.
    Custom(CustomPunctuationProfile),
}
/// Custom profile overrides with explicit null versus omission semantics.
#[cfg(feature = "serde")]
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomPunctuationProfile {
    /// Built-in profile inherited by omitted fields.
    pub base: Option<String>,
    /// Comma override. Explicit null preserves commas.
    #[serde(default, deserialize_with = "override_value")]
    pub comma: Option<Option<StopStyle>>,
    /// Period override. Explicit null preserves periods.
    #[serde(default, deserialize_with = "override_value")]
    pub period: Option<Option<StopStyle>>,
    /// Single delimiter override. Explicit null preserves single delimiters.
    #[serde(default, deserialize_with = "override_value")]
    pub single: Option<Option<DelimiterStyle>>,
    /// Double delimiter override. Explicit null preserves double delimiters.
    #[serde(default, deserialize_with = "override_value")]
    pub double: Option<Option<DelimiterStyle>>,
}
#[cfg(feature = "serde")]
fn override_value<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    <Option<T> as serde::Deserialize>::deserialize(deserializer).map(Some)
}
#[cfg(feature = "serde")]
impl PunctuationConfig {
    /// Resolves inheritance and validates the complete resulting profile.
    pub fn resolve(&self) -> Result<PunctuationProfile, PunctuationConfigError> {
        match self {
            Self::Preset(name) => PunctuationProfile::preset(name),
            Self::Custom(c) => {
                let mut p = PunctuationProfile::preset(c.base.as_deref().unwrap_or("preserve"))?;
                if let Some(x) = c.comma {
                    p.comma = x;
                }
                if let Some(x) = c.period {
                    p.period = x;
                }
                if let Some(x) = c.single {
                    p.single = x;
                }
                if let Some(x) = c.double {
                    p.double = x;
                }
                p.validate()
            }
        }
    }
}

/// Lazy iterator driving a punctuation middleware through upstream EOF.
pub struct PunctuationIter<I, S, C = LiteralTextCodec> {
    input: I,
    normalizer: Option<PunctuationNormalizer<S, C>>,
    ready: VecDeque<OutputToken<S>>,
}
impl<I, S, C> Iterator for PunctuationIter<I, S, C>
where
    I: Iterator<Item = OutputToken<S>>,
    S: ScopeData,
    C: PunctuationTextCodec,
{
    type Item = OutputToken<S>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(token) = self.ready.pop_front() {
                return Some(token);
            }
            let normalizer = self.normalizer.as_mut()?;
            if let Some(token) = self.input.next() {
                self.ready.extend(normalizer.push_token(token));
            } else {
                self.ready.extend(self.normalizer.take().unwrap().finish());
            }
        }
    }
}
/// Normalizes literal text lazily after annotations and before rendering.
pub fn normalize_punctuation_iter<S, I>(
    tokens: I,
    profile: PunctuationProfile,
) -> PunctuationIter<I::IntoIter, S>
where
    S: ScopeData,
    I: IntoIterator<Item = OutputToken<S>>,
{
    normalize_punctuation_iter_with_codec(tokens, profile, LiteralTextCodec)
}
/// Normalizes text lazily with the format's source-spelling codec.
pub fn normalize_punctuation_iter_with_codec<S, I, C>(
    tokens: I,
    profile: PunctuationProfile,
    codec: C,
) -> PunctuationIter<I::IntoIter, S, C>
where
    S: ScopeData,
    I: IntoIterator<Item = OutputToken<S>>,
    C: PunctuationTextCodec,
{
    PunctuationIter {
        input: tokens.into_iter(),
        normalizer: Some(PunctuationNormalizer::with_codec(profile, codec)),
        ready: VecDeque::new(),
    }
}
/// Collects normalized literal text tokens. No annotation payloads are changed.
pub fn normalize_punctuation<S, I>(tokens: I, profile: PunctuationProfile) -> Vec<OutputToken<S>>
where
    S: ScopeData,
    I: IntoIterator<Item = OutputToken<S>>,
{
    normalize_punctuation_iter(tokens, profile).collect()
}
