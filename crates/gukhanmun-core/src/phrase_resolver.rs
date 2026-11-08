// Gukhanmun: Bounded source-owned phrase constraints on fixed segmentation paths.
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

use crate::phrase_index::PhraseRuleRef;
use crate::segment::Segment;
use crate::{OwnedPhraseEvidence, SourceId};
use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

/// Maximum ranked choices inspected by additional phrase resolution per edge.
///
/// This does not truncate dictionary pools or bound their construction and
/// legacy policy selection. Oversized pools retain the legacy choice.
pub const MAX_PHRASE_CANDIDATES: usize = 256;

/// Outcomes of phrase constraints and policy selection on one fixed dictionary edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ResolutionOutcome {
    /// No phrase constraint resolved the source pool; the existing policy choice was kept.
    ///
    /// This records selection provenance without judging curated preferences.
    PolicyDefault,
    /// Remaining choices agree on form, lexical identity, alias and marks.
    Selected,
    /// Semantically distinct choices remain; legacy particle selection was retained.
    Unresolved,
    /// Individually valid constraints disagree; legacy particle selection was retained.
    Conflict,
    /// A phrase candidate or matcher budget was exceeded; no partial evidence was applied.
    Limit,
}

/// A source-owned rule participating in one local resolution decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PhraseRuleId {
    /// The registered leaf source owning the evidence.
    pub owner: SourceId,
    /// The rule's index in that immutable source's evidence.
    pub index: usize,
}

/// An inspectable decision independent of annotation rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct ResolutionEvent {
    /// UTF-8 byte offset in contiguous convertible IR text, not source markup.
    ///
    /// Offsets restart at structural/preserved boundaries and explicit flushes.
    pub byte_start: u64,
    /// Exclusive UTF-8 byte endpoint in the same flow.
    pub byte_end: u64,
    /// Rules whose restrictions were considered together.
    pub rules: Vec<PhraseRuleId>,
    /// Complete source choice retained by this resolution step, before
    /// positional realization or explicit gloss processing.
    pub chosen: crate::SourceChoice,
    /// Choices remaining after phrase and particle intersections.
    ///
    /// Empty under [`ResolutionOutcome::Limit`] means candidates were not enumerated.
    pub remaining: Vec<crate::SourceChoice>,
    /// Whether the intersection selected a choice or retained legacy behavior.
    pub outcome: ResolutionOutcome,
}

/// Optional inspection of source phrase and policy decisions accumulated by an engine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct ResolutionReport {
    /// Local decisions, in emitted edge order.
    pub events: Vec<ResolutionEvent>,
}

const MAX_MATCHER_STEPS: usize =
    crate::MAX_PHRASE_MATCHES * crate::MAX_PHRASE_CHUNKS * crate::MAX_PHRASE_ALTERNATIVES;

fn discover(handles: &mut Vec<RuleRef>, bundle: usize, found: &[PhraseRuleRef]) -> bool {
    if found.len() > MAX_MATCHER_STEPS.saturating_sub(handles.len()) {
        return false;
    }
    handles.extend(found.iter().map(|handle| RuleRef {
        bundle,
        rule: handle.rule,
        extension: handle.extension,
    }));
    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct RuleRef {
    bundle: usize,
    rule: usize,
    extension: Option<usize>,
}
#[derive(Clone)]
struct Restriction {
    segment: usize,
    allowed: Vec<bool>,
}
struct MatchedRule {
    id: PhraseRuleId,
    restrictions: Vec<Restriction>,
}

pub(crate) struct PhraseResolver<'a> {
    bundles: Vec<OwnedPhraseEvidence<'a>>,
    indices: Vec<Cow<'a, crate::PhraseIndex>>,
    horizon: usize,
    history: String,
    history_segments: Vec<Segment>,
    history_at_flow_start: bool,
    emitted_bytes: u64,
    report: ResolutionReport,
    pub(crate) reporting: bool,
}

impl<'a> PhraseResolver<'a> {
    pub(crate) fn new(bundles: Vec<OwnedPhraseEvidence<'a>>) -> Self {
        let mut retained = Vec::with_capacity(bundles.len());
        let mut indices = Vec::with_capacity(bundles.len());
        for bundle in bundles {
            let index = match bundle.index {
                Some(index) => index
                    .validate_evidence(bundle.evidence)
                    .map(|()| Cow::Borrowed(index)),
                None => crate::PhraseIndex::new(bundle.evidence).map(Cow::Owned),
            };
            match index {
                Ok(index) => {
                    retained.push(bundle);
                    indices.push(index);
                }
                Err(error) => {
                    tracing::warn!(?error, owner = ?bundle.owner, "ignoring invalid phrase evidence");
                }
            }
        }
        let horizon = indices
            .iter()
            .map(|index| index.max_span_chars())
            .max()
            .unwrap_or(0);
        Self {
            bundles: retained,
            indices,
            horizon,
            history: String::new(),
            history_segments: Vec::new(),
            history_at_flow_start: true,
            emitted_bytes: 0,
            report: ResolutionReport::default(),
            reporting: false,
        }
    }

    fn discover(&self, key: &str, handles: &mut Vec<RuleRef>) -> bool {
        self.indices.iter().enumerate().all(|(bundle, index)| {
            index
                .prefixes()
                .get(key)
                .is_none_or(|found| discover(handles, bundle, found))
        })
    }

    pub(crate) fn active(&self) -> bool {
        self.indices.iter().any(|index| !index.is_empty())
    }
    pub(crate) fn horizon(&self) -> usize {
        self.horizon
            .saturating_add(crate::particles::LOOKAHEAD)
            .saturating_add(1)
    }
    pub(crate) fn take_report(&mut self) -> ResolutionReport {
        core::mem::take(&mut self.report)
    }
    pub(crate) fn end_flow(&mut self) {
        self.history.clear();
        self.history_segments.clear();
        self.history_at_flow_start = true;
        self.emitted_bytes = 0;
    }

    pub(crate) fn resolve(
        &mut self,
        text: &str,
        segments: &mut [Segment],
        emit_end: usize,
        final_end: bool,
        hard_end: bool,
    ) {
        if !self.active() && !self.reporting {
            return;
        }
        let history_len = self.history.len();
        let mut source = self.history.clone();
        source.push_str(text);
        let mut all_segments = self.history_segments.clone();
        let history_count = all_segments.len();
        all_segments.extend(
            segments
                .iter()
                .cloned()
                .map(|segment| shift(segment, history_len)),
        );
        let mut constraints: Vec<Vec<(PhraseRuleId, Vec<bool>)>> =
            alloc::vec![Vec::new(); all_segments.len()];
        let mut limited = alloc::vec![false; all_segments.len()];
        for (start, _) in source.char_indices() {
            if !self.active() {
                break;
            }
            if start
                >= history_len
                    .saturating_add(emit_end)
                    .saturating_add(self.horizon * 4)
            {
                break;
            }
            if !left_boundary(&source, start, self.history_at_flow_start) {
                continue;
            }
            let mut handles = Vec::new();
            let mut discovery_overflow = false;
            for (end, _) in source[start..]
                .char_indices()
                .skip(1)
                .chain(core::iter::once((source.len() - start, '\0')))
                .take(self.horizon + 1)
            {
                if !self.discover(&source[start..start + end], &mut handles) {
                    discovery_overflow = true;
                    break;
                }
            }
            // A complete normalized dictionary edge may use a different glyph
            // spelling from the original input. Its retained anchor supplies
            // the exact source-owned key, without enumerating glyph products.
            if let Some((_, segment)) = segment_at_start(&all_segments, start)
                && let Some(selection) = selection(segment)
            {
                if selection.ranked.len() > MAX_PHRASE_CANDIDATES {
                    discovery_overflow = true;
                } else {
                    for choice in selection.ranked.iter() {
                        if !self.discover(&choice.anchor.key, &mut handles) {
                            discovery_overflow = true;
                            break;
                        }
                    }
                }
            }
            handles.sort();
            handles.dedup();
            let mut matches = Vec::new();
            let mut budget = if discovery_overflow {
                0
            } else {
                MAX_MATCHER_STEPS
            };
            for handle in handles {
                if budget == 0 {
                    break;
                }
                let mut restrictions = Vec::new();
                self.match_chunks(
                    &source,
                    &all_segments,
                    handle,
                    0,
                    start,
                    &mut restrictions,
                    &mut matches,
                    &mut budget,
                    final_end,
                    hard_end,
                );
                if budget == 0 {
                    break;
                }
            }
            if budget == 0 {
                let affected_end = source[start..]
                    .char_indices()
                    .nth(self.horizon + 1)
                    .map_or(source.len(), |(offset, _)| start + offset);
                for (index, segment) in all_segments.iter().enumerate() {
                    let (a, b) = bounds(segment);
                    if a < affected_end && b > start {
                        limited[index] = true;
                    }
                }
            } else {
                for matched in matches {
                    for restriction in matched.restrictions {
                        constraints[restriction.segment].push((matched.id, restriction.allowed));
                    }
                }
            }
        }
        for (local_index, segment) in segments.iter_mut().enumerate() {
            let (start, end) = bounds(segment);
            if end > emit_end {
                continue;
            }
            let index = history_count + local_index;
            let unconstrained = constraints[index].is_empty() && !limited[index];
            if unconstrained && !self.reporting {
                continue;
            }
            let Some(selection) = selection_mut(segment) else {
                continue;
            };
            if limited[index] || selection.ranked.len() > MAX_PHRASE_CANDIDATES {
                if self.reporting {
                    let mut rules = Vec::new();
                    for (id, _) in &constraints[index] {
                        if !rules.contains(id) {
                            rules.push(*id);
                        }
                    }
                    self.report.events.push(ResolutionEvent {
                        byte_start: self.emitted_bytes.saturating_add(start as u64),
                        byte_end: self.emitted_bytes.saturating_add(end as u64),
                        rules,
                        chosen: (*selection.choice).clone(),
                        remaining: Vec::new(),
                        outcome: ResolutionOutcome::Limit,
                    });
                }
                continue;
            }
            let mut allowed = alloc::vec![true; selection.ranked.len()];
            let mut rules = Vec::new();
            if !constraints[index].is_empty() {
                // Independently owned rules can attest the same complete lexical
                // choice. Partition once, then intersect semantic classes rather
                // than requiring distinct source IDs to occupy the same index.
                let mut representatives: Vec<usize> = Vec::new();
                let classes: Vec<usize> = selection
                    .ranked
                    .iter()
                    .enumerate()
                    .map(|(candidate_index, choice)| {
                        if let Some(class) = representatives.iter().position(|representative| {
                            same_semantic_choice(&selection.ranked[*representative], choice)
                        }) {
                            class
                        } else {
                            let class = representatives.len();
                            representatives.push(candidate_index);
                            class
                        }
                    })
                    .collect();
                let mut witnessed = alloc::vec![false; selection.ranked.len()];
                let mut surviving = alloc::vec![true; representatives.len()];
                let mut attested = alloc::vec![false; representatives.len()];
                for (id, restriction) in &constraints[index] {
                    if !rules.contains(id) {
                        rules.push(*id);
                    }
                    attested.fill(false);
                    for (candidate_index, direct_witness) in restriction.iter().enumerate() {
                        if *direct_witness {
                            witnessed[candidate_index] = true;
                            attested[classes[candidate_index]] = true;
                        }
                    }
                    for (keep, attested) in surviving.iter_mut().zip(&attested) {
                        *keep &= *attested;
                    }
                }
                // Equivalence is not authority: a higher source with an equal
                // form but no independently validated rule cannot borrow proof.
                for (candidate_index, keep) in allowed.iter_mut().enumerate() {
                    *keep = witnessed[candidate_index] && surviving[classes[candidate_index]];
                }
            }
            if let Some(particle_mask) = crate::particles::compatible_form_mask(
                selection.ranked.iter().map(|choice| {
                    (
                        choice.form.reading.as_str(),
                        choice.form.suffix_reading.as_deref(),
                    )
                }),
                &text[end..],
                final_end,
            ) {
                for (keep, compatible) in allowed.iter_mut().zip(particle_mask) {
                    *keep &= compatible;
                }
            }
            let remaining: Vec<usize> = allowed
                .iter()
                .enumerate()
                .filter_map(|(i, keep)| keep.then_some(i))
                .collect();
            let unique = !remaining.is_empty()
                && remaining.iter().all(|index| {
                    same_semantic_choice(&selection.ranked[remaining[0]], &selection.ranked[*index])
                });
            let outcome = if unconstrained {
                if unique {
                    continue;
                }
                ResolutionOutcome::PolicyDefault
            } else if remaining.is_empty() {
                ResolutionOutcome::Conflict
            } else if !unique {
                ResolutionOutcome::Unresolved
            } else {
                *selection.choice = selection.ranked[remaining[0]].clone();
                ResolutionOutcome::Selected
            };
            let chosen = (*selection.choice).clone();
            let remaining = remaining
                .into_iter()
                .map(|index| selection.ranked[index].clone())
                .collect();
            if outcome == ResolutionOutcome::Selected {
                synchronize(segment);
            }
            if self.reporting {
                self.report.events.push(ResolutionEvent {
                    byte_start: self.emitted_bytes.saturating_add(start as u64),
                    byte_end: self.emitted_bytes.saturating_add(end as u64),
                    rules,
                    chosen,
                    remaining,
                    outcome,
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn match_chunks(
        &self,
        source: &str,
        segments: &[Segment],
        handle: RuleRef,
        chunk_index: usize,
        start: usize,
        restrictions: &mut Vec<Restriction>,
        matches: &mut Vec<MatchedRule>,
        budget: &mut usize,
        final_end: bool,
        hard_end: bool,
    ) {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let Some(bundle) = self.bundles.get(handle.bundle) else {
            *budget = 0;
            return;
        };
        let Some(rule) = bundle.evidence.rules.get(handle.rule) else {
            *budget = 0;
            return;
        };
        let Some(chunk) = rule.chunks.get(chunk_index) else {
            *budget = 0;
            return;
        };
        let terminal = chunk_index + 1 == rule.chunks.len();
        let extension = if terminal {
            match handle.extension {
                Some(index) => {
                    let Some(extension) = rule.terminal_extensions.get(index) else {
                        *budget = 0;
                        return;
                    };
                    Some(extension)
                }
                None => None,
            }
        } else {
            None
        };
        let spellings = extension.map_or(&chunk.spellings, |extension| &extension.spellings);
        let mut literal = chunk.reading.clone();
        if let Some(extension) = extension {
            literal.push_str(&extension.literal_suffix);
        }
        let mut alternatives: Vec<(usize, Option<Restriction>, bool)> = Vec::new();
        for spelling in spellings {
            let Some((index, segment)) = segment_at_start(segments, start) else {
                continue;
            };
            let (_, end) = bounds(segment);
            let Some(selection) = selection(segment) else {
                continue;
            };
            let Some(allowed) = candidate_mask(&selection.ranked, bundle.owner, spelling) else {
                *budget = 0;
                return;
            };
            if allowed.iter().any(|keep| *keep) {
                alternatives.push((
                    end,
                    Some(Restriction {
                        segment: index,
                        allowed,
                    }),
                    false,
                ));
            }
        }
        if source[start..].starts_with(&literal) && !literal.is_empty() {
            let end = start + literal.len();
            if literal_span(segments, start, end) {
                alternatives.push((end, None, true));
            }
        }
        for (end, restriction, literal) in alternatives {
            if terminal {
                let following = &source[end..];
                if !right_boundary(following, final_end, hard_end, literal) {
                    continue;
                }
            }
            let old_len = restrictions.len();
            if let Some(restriction) = restriction {
                restrictions.push(restriction);
            }
            if terminal {
                if !restrictions.is_empty() {
                    matches.push(MatchedRule {
                        id: PhraseRuleId {
                            owner: bundle.owner,
                            index: handle.rule,
                        },
                        restrictions: restrictions.clone(),
                    });
                    if matches.len() > crate::MAX_PHRASE_MATCHES {
                        *budget = 0;
                    }
                }
            } else {
                self.match_chunks(
                    source,
                    segments,
                    handle,
                    chunk_index + 1,
                    end,
                    restrictions,
                    matches,
                    budget,
                    final_end,
                    hard_end,
                );
                if source[end..].starts_with(' ') {
                    self.match_chunks(
                        source,
                        segments,
                        handle,
                        chunk_index + 1,
                        end + 1,
                        restrictions,
                        matches,
                        budget,
                        final_end,
                        hard_end,
                    );
                }
            }
            restrictions.truncate(old_len);
        }
    }

    pub(crate) fn remember(&mut self, text: &str, segments: &[Segment], emit_end: usize) {
        if !self.active() {
            if self.reporting {
                self.emitted_bytes = self.emitted_bytes.saturating_add(emit_end as u64);
            }
            return;
        }
        let old_len = self.history.len();
        self.history.push_str(&text[..emit_end]);
        self.history_segments.extend(
            segments
                .iter()
                .filter(|segment| bounds(segment).1 <= emit_end)
                .cloned()
                .map(|segment| shift(segment, old_len)),
        );
        self.emitted_bytes = self.emitted_bytes.saturating_add(emit_end as u64);
        let keep = self.horizon.saturating_add(1);
        let cut = self
            .history
            .char_indices()
            .rev()
            .nth(keep.saturating_sub(1))
            .map_or(0, |(offset, _)| offset);
        if cut > 0 {
            self.history_at_flow_start = false;
            self.history.replace_range(..cut, "");
            self.history_segments = self
                .history_segments
                .drain(..)
                .filter_map(|segment| {
                    let (start, end) = bounds(&segment);
                    if end <= cut {
                        None
                    } else if start < cut {
                        // Plain text may be clipped without changing its role.
                        // Partial lexical edges remain opaque to phrase chunks.
                        Some(if matches!(segment, Segment::Text { .. }) {
                            Segment::Text {
                                byte_start: 0,
                                byte_end: end - cut,
                            }
                        } else {
                            Segment::Fallback {
                                byte_start: 0,
                                byte_end: end - cut,
                            }
                        })
                    } else {
                        Some(unshift(segment, cut))
                    }
                })
                .collect();
        }
    }
}

fn boundary(ch: char) -> bool {
    ch.is_whitespace() || ".,!?:;()[]{}。、！？，：；（）【】「」『』\"'".contains(ch)
}
fn left_boundary(source: &str, start: usize, flow_start: bool) -> bool {
    source[..start]
        .chars()
        .next_back()
        .map_or(flow_start, boundary)
}
fn right_boundary(following: &str, final_end: bool, hard_end: bool, literal: bool) -> bool {
    following.chars().next().map_or(hard_end, boundary)
        || (!literal && crate::particles::has_witnessed_particle(following, final_end))
}
fn literal_span(segments: &[Segment], start: usize, end: usize) -> bool {
    let mut cursor = start;
    let first = segments.partition_point(|segment| bounds(segment).1 <= start);
    for segment in &segments[first..] {
        let (a, b) = bounds(segment);
        if b <= cursor {
            continue;
        }
        if a > cursor || !matches!(segment, Segment::Text { .. }) {
            return false;
        }
        cursor = b.min(end);
        if cursor == end {
            return true;
        }
    }
    false
}
fn segment_at_start(segments: &[Segment], start: usize) -> Option<(usize, &Segment)> {
    let index = segments
        .binary_search_by_key(&start, |segment| bounds(segment).0)
        .ok()?;
    Some((index, &segments[index]))
}
fn candidate_mask(
    choices: &[crate::SourceChoice],
    owner: SourceId,
    spelling: &crate::PhraseSpelling,
) -> Option<Vec<bool>> {
    if choices.len() > MAX_PHRASE_CANDIDATES {
        return None;
    }
    Some(
        choices
            .iter()
            .map(|choice| {
                choice.anchor.source_id == owner
                    && choice.anchor.key == spelling.key
                    && spelling.candidates.iter().any(|candidate| {
                        candidate.reading == choice.anchor.reading
                    && candidate.suffix_reading == choice.anchor.suffix_reading
                    // Attestation constrains the complete output pair too.
                    && candidate.reading == choice.form.reading
                    && candidate.suffix_reading == choice.form.suffix_reading
                    })
            })
            .collect(),
    )
}

// Match SelectedReading::revalidate's equivalence contract: provenance alone
// does not create a second lexical meaning, but aliases and marks still do.
fn same_semantic_choice(left: &crate::SourceChoice, right: &crate::SourceChoice) -> bool {
    left.anchor.key == right.anchor.key
        && left.anchor.reading == right.anchor.reading
        && left.anchor.suffix_reading == right.anchor.suffix_reading
        && left.anchor.alias == right.anchor.alias
        && left.mark == right.mark
        && left.form == right.form
}

fn selection(segment: &Segment) -> Option<&crate::SelectedReading> {
    match segment {
        Segment::Dictionary { selection, .. } | Segment::TrivialDictionary { selection, .. } => {
            selection.as_ref()
        }
        _ => None,
    }
}
fn selection_mut(segment: &mut Segment) -> Option<&mut crate::SelectedReading> {
    match segment {
        Segment::Dictionary { selection, .. } | Segment::TrivialDictionary { selection, .. } => {
            selection.as_mut()
        }
        _ => None,
    }
}
fn synchronize(segment: &mut Segment) {
    // A proved phrase retains each complete lexical edge even if its legacy
    // single-character form would otherwise join an adjacent fallback run.
    if let Segment::TrivialDictionary {
        byte_start,
        byte_end,
        reading,
        suffix_reading,
        mark,
        dictionary_hanja,
        pending_selection,
        selection,
    } = segment
    {
        *segment = Segment::Dictionary {
            byte_start: *byte_start,
            byte_end: *byte_end,
            reading: core::mem::take(reading),
            suffix_reading: suffix_reading.take(),
            mark: *mark,
            dictionary_hanja: core::mem::take(dictionary_hanja),
            pending_selection: *pending_selection,
            selection: selection.take(),
        };
    }
    match segment {
        Segment::Dictionary {
            reading,
            suffix_reading,
            mark,
            dictionary_hanja,
            selection: Some(selection),
            ..
        }
        | Segment::TrivialDictionary {
            reading,
            suffix_reading,
            mark,
            dictionary_hanja,
            selection: Some(selection),
            ..
        } => {
            *reading = selection.choice.form.reading.clone();
            *suffix_reading = selection.choice.form.suffix_reading.clone();
            *mark = selection.choice.mark;
            *dictionary_hanja = selection.choice.anchor.key.clone();
        }
        _ => {}
    }
}
pub(crate) fn bounds(segment: &Segment) -> (usize, usize) {
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
        | Segment::Text {
            byte_start,
            byte_end,
        }
        | Segment::Fallback {
            byte_start,
            byte_end,
        }
        | Segment::NumeralText {
            byte_start,
            byte_end,
            ..
        } => (*byte_start, *byte_end),
    }
}
fn shift(mut segment: Segment, amount: usize) -> Segment {
    match &mut segment {
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
        | Segment::Text {
            byte_start,
            byte_end,
        }
        | Segment::Fallback {
            byte_start,
            byte_end,
        }
        | Segment::NumeralText {
            byte_start,
            byte_end,
            ..
        } => {
            *byte_start += amount;
            *byte_end += amount;
        }
    }
    segment
}
fn unshift(mut segment: Segment, amount: usize) -> Segment {
    match &mut segment {
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
        | Segment::Text {
            byte_start,
            byte_end,
        }
        | Segment::Fallback {
            byte_start,
            byte_end,
        }
        | Segment::NumeralText {
            byte_start,
            byte_end,
            ..
        } => {
            *byte_start -= amount;
            *byte_end -= amount;
        }
    }
    segment
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CandidateMatch, Engine, HanjaDictionary, InputToken, MapDictionary, Match, OutputToken,
        PhraseCandidateLink, PhraseChunk, PhraseEvidence, PhraseIndex, PhraseRule, PhraseSpelling,
        PlainScopeData, ReadingCandidate, ReadingCandidates, WordId,
    };
    use alloc::{boxed::Box, vec};

    fn custom_evidence() -> PhraseEvidence {
        PhraseEvidence::new(vec![PhraseRule::new(
            vec![WordId(1)],
            vec![
                PhraseChunk::new(
                    "무조건".into(),
                    vec![PhraseSpelling::new("無條件".into(), vec![])],
                ),
                PhraseChunk::new(
                    "항복".into(),
                    vec![PhraseSpelling::new(
                        "降服".into(),
                        vec![PhraseCandidateLink::new("항복".into(), None)],
                    )],
                ),
            ],
        )])
    }

    struct CustomDictionary<'a> {
        inner: MapDictionary,
        evidence: &'a PhraseEvidence,
        index: Option<&'a PhraseIndex>,
    }

    impl HanjaDictionary for CustomDictionary<'_> {
        fn matches_at<'a>(&'a self, text: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            self.inner.matches_at(text)
        }

        fn candidate_matches_at<'a>(
            &'a self,
            text: &'a str,
        ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
            self.inner.candidate_matches_at(text)
        }

        fn candidate_matches_at_spellings(
            &self,
            spellings: &[&str],
        ) -> Vec<(usize, CandidateMatch)> {
            self.inner.candidate_matches_at_spellings(spellings)
        }

        fn max_word_chars(&self) -> Option<usize> {
            self.inner.max_word_chars()
        }

        fn phrase_evidence(&self) -> Option<&PhraseEvidence> {
            Some(self.evidence)
        }

        fn phrase_index(&self) -> Option<&PhraseIndex> {
            self.index
        }
    }

    fn custom_dictionary<'a>(
        evidence: &'a PhraseEvidence,
        index: Option<&'a PhraseIndex>,
    ) -> CustomDictionary<'a> {
        let mut inner = MapDictionary::new();
        inner.insert_candidates(
            "降服",
            ReadingCandidates::new(vec![
                ReadingCandidate {
                    reading: "강복".into(),
                    preference: 10,
                    ..ReadingCandidate::default()
                },
                ReadingCandidate {
                    reading: "항복".into(),
                    ..ReadingCandidate::default()
                },
            ])
            .unwrap(),
        );
        CustomDictionary {
            inner,
            evidence,
            index,
        }
    }

    fn custom_output(dictionary: &CustomDictionary<'_>, input: &[&str]) -> String {
        let mut engine = Engine::<PlainScopeData, _>::new(dictionary);
        let mut output = Vec::new();
        for chunk in input {
            output.extend(engine.push_token(InputToken::Text((*chunk).into())));
        }
        output.extend(engine.finish());
        output
            .into_iter()
            .filter_map(|token| match token {
                OutputToken::Annotated(annotation) => Some(annotation.reading),
                OutputToken::Text(text) | OutputToken::Verbatim(text) => Some(text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn invalid_custom_evidence_falls_back_without_panicking() {
        let valid = custom_evidence();
        let index = PhraseIndex::new(&valid).unwrap();
        let mut no_chunks = valid.clone();
        no_chunks.rules[0].chunks.clear();
        let mut future_policy = valid.clone();
        future_policy.policy_version += 1;
        for evidence in [&no_chunks, &future_policy] {
            for index in [None, Some(&index)] {
                let dictionary = custom_dictionary(evidence, index);
                let resolver = PhraseResolver::new(dictionary.owned_phrase_evidence());
                assert!(!resolver.active());
                for input in [vec!["무조건 降服"], vec!["무", "조건 ", "降", "服"]] {
                    assert_eq!(custom_output(&dictionary, &input), "무조건 강복");
                }
            }
        }
    }

    #[test]
    fn mismatched_custom_index_is_rejected_and_valid_bundles_stay_borrowed() {
        let valid = custom_evidence();
        let mut stale = valid.clone();
        stale.rules[0].chunks[0].reading = "무조항".into();
        let stale_index = PhraseIndex::new(&stale).unwrap();
        let dictionary = custom_dictionary(&valid, Some(&stale_index));
        assert_eq!(custom_output(&dictionary, &["무조건 降服"]), "무조건 강복");

        let mut extra_rule = valid.clone();
        let mut extra = valid.rules[0].clone();
        extra.source_ids = vec![WordId(2)];
        extra_rule.rules.push(extra);
        let extra_index = PhraseIndex::new(&extra_rule).unwrap();
        let dictionary = custom_dictionary(&valid, Some(&extra_index));
        assert_eq!(custom_output(&dictionary, &["무조건 降服"]), "무조건 강복");

        let equivalent = valid.clone();
        let index = PhraseIndex::new(&equivalent).unwrap();
        let resolver = PhraseResolver::new(vec![
            OwnedPhraseEvidence::new(SourceId(0), &valid).with_index(&stale_index),
            OwnedPhraseEvidence::new(SourceId(7), &valid).with_index(&index),
        ]);
        assert_eq!(resolver.bundles.len(), 1);
        assert_eq!(resolver.bundles[0].owner, SourceId(7));
        assert!(matches!(resolver.indices[0], Cow::Borrowed(_)));
        assert!(core::ptr::eq(resolver.indices[0].as_ref(), &index));
        for index in [None, Some(&index)] {
            let dictionary = custom_dictionary(&valid, index);
            assert_eq!(custom_output(&dictionary, &["무조건 降服"]), "무조건 항복");
        }
    }

    #[test]
    fn invalid_matcher_positions_discard_partial_matches() {
        let evidence = custom_evidence();
        let resolver = PhraseResolver::new(vec![OwnedPhraseEvidence::new(SourceId(0), &evidence)]);
        for (bundle, rule, extension, chunk) in [
            (1, 0, None, 0),
            (0, 1, None, 0),
            (0, 0, None, 2),
            (0, 0, Some(0), 1),
        ] {
            let mut budget = MAX_MATCHER_STEPS;
            let mut matches = Vec::new();
            resolver.match_chunks(
                "",
                &[],
                RuleRef {
                    bundle,
                    rule,
                    extension,
                },
                chunk,
                0,
                &mut Vec::new(),
                &mut matches,
                &mut budget,
                true,
                true,
            );
            // The caller discards every accumulated match when budget is zero.
            assert_eq!(budget, 0);
            assert!(matches.is_empty());
        }
    }

    #[test]
    fn engines_borrow_the_dictionary_index_without_rebuilding_it() {
        use crate::{
            HanjaDictionary, MapDictionary, PhraseCandidateLink, PhraseChunk, PhraseEvidence,
            PhraseRule, PhraseSpelling, WordId,
        };
        let mut dictionary = MapDictionary::new();
        dictionary.insert("降服", "항복");
        dictionary
            .set_phrase_evidence(PhraseEvidence::new(alloc::vec![PhraseRule::new(
                alloc::vec![WordId(1)],
                alloc::vec![
                    PhraseChunk::new(
                        "무조건".into(),
                        alloc::vec![PhraseSpelling::new("無條件".into(), alloc::vec![])]
                    ),
                    PhraseChunk::new(
                        "항복".into(),
                        alloc::vec![PhraseSpelling::new(
                            "降服".into(),
                            alloc::vec![PhraseCandidateLink::new("항복".into(), None)]
                        )]
                    ),
                ],
            )]))
            .unwrap();
        let resolver = PhraseResolver::new(dictionary.owned_phrase_evidence());
        assert!(matches!(resolver.indices[0], Cow::Borrowed(_)));
        assert!(core::ptr::eq(
            resolver.indices[0].as_ref(),
            dictionary.phrase_index().unwrap()
        ));
    }

    #[test]
    fn semantic_equivalence_keeps_identity_alias_and_mark_differences() {
        use crate::{
            AliasKind, AliasTarget, HanjaDictionary, MapDictionary, PhraseCandidateLink,
            PhraseChunk, PhraseEvidence, PhraseRule, PhraseSpelling, WordId,
        };
        let mut dictionary = MapDictionary::new();
        dictionary.insert("降服", "항복");
        dictionary
            .set_phrase_evidence(PhraseEvidence::new(alloc::vec![PhraseRule::new(
                alloc::vec![WordId(1)],
                alloc::vec![
                    PhraseChunk::new(
                        "무조건".into(),
                        alloc::vec![PhraseSpelling::new("無條件".into(), alloc::vec![])]
                    ),
                    PhraseChunk::new(
                        "항복".into(),
                        alloc::vec![PhraseSpelling::new(
                            "降服".into(),
                            alloc::vec![PhraseCandidateLink::new("항복".into(), None)]
                        )]
                    ),
                ],
            )]))
            .unwrap();
        let matched = dictionary
            .source_matches_at("降服")
            .unwrap()
            .next()
            .unwrap();
        let original = matched.default_choice();
        let mut other = original.clone();
        other.anchor.source_id = SourceId(20);
        other.form_source = Some(SourceId(21));
        other.reason = crate::ChoiceReason::AuthorizedLoanword;
        assert!(same_semantic_choice(original, &other));
        let oversized = alloc::vec![original.clone(); MAX_PHRASE_CANDIDATES + 1];
        let spelling = PhraseSpelling::new(
            "降服".into(),
            alloc::vec![PhraseCandidateLink::new("항복".into(), None)],
        );
        assert!(candidate_mask(&oversized, SourceId(0), &spelling).is_none());
        other.anchor.key = "降伏".into();
        assert!(!same_semantic_choice(original, &other));
        other = original.clone();
        other.mark.require_hanja = true;
        assert!(!same_semantic_choice(original, &other));
        other = original.clone();
        other.anchor.alias = Some(AliasTarget {
            canonical: "降伏".into(),
            kind: AliasKind::Sibling,
        });
        assert!(!same_semantic_choice(original, &other));
    }

    #[test]
    fn oversized_prefix_bucket_is_rejected_before_materializing_handles() {
        let handle = PhraseRuleRef {
            rule: 0,
            extension: None,
        };
        let found = alloc::vec![handle; MAX_MATCHER_STEPS + 1];
        let mut handles = Vec::new();
        assert!(!discover(&mut handles, 0, &found));
        assert!(handles.is_empty());
        assert_eq!(handles.capacity(), 0);
    }
}
