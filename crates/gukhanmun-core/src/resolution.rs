// Gukhanmun: Cooperative dictionary source groups, ranked choices, and selected identity.
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

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::{
    AliasKind, AliasTarget, AuthorizationKind, CandidateEvidence, CandidateMatch, HanjaDictionary,
    Match, MatchMark, ReadingCandidate, ReadingCandidates, ReadingForm, SourceRole,
};

/// Ranking policy for explicitly registered cooperating dictionary sources.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SourcePolicy {
    /// Preserve dictionary registration order and all legacy selection behavior.
    #[default]
    Ordered,
    /// Prefer reference-authorized forms, then reference forms, then independent loans.
    ReferenceAuthorized,
    /// Prefer original reference forms without promoting loanword spellings.
    ReferenceFirst,
    /// Prefer independent loanword forms before original reference forms.
    LoanwordFirst,
}

/// A registered source's position within one composite dictionary.
///
/// An opaque legacy projection occupies the first slot of its registered
/// source range, identified by [`SelectionDiagnostic::OpaqueLegacyProjection`].
/// It does not acquire the phrase authority of a leaf in that range.
/// This is provenance only. Different source IDs alone do not imply different
/// lexical identities, homophones, or first occurrences.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceId(pub usize);

/// The source-local candidate whose identity and marks anchor a selected form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAnchor {
    /// The source supplying lexical identity and dictionary marks.
    pub source_id: SourceId,
    /// Its exact matched dictionary spelling, before rendering normalization.
    pub key: String,
    /// The anchor's original word-initial reading, independent of output form.
    pub reading: String,
    /// The anchor's original positional form, when supplied.
    pub suffix_reading: Option<String>,
    /// The selected source candidate's original alias relation.
    pub alias: Option<AliasTarget>,
    /// Lexical canonical spelling, restricted to Sibling/Lexical relations.
    pub lexical_canonical: String,
    /// Canonical spelling for homophone comparison, including Recognition aliases.
    pub homophone_canonical: String,
}

/// Why one member appears in the source-aware ranked pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChoiceReason {
    /// An authored source excludes all lower-priority sources for this span.
    Override,
    /// An original reference candidate supplies the form.
    Reference,
    /// A loanword supplies a complete form authorized by a reference anchor.
    AuthorizedLoanword,
    /// A compiled formal redirect supplies a reference-backed terminal form.
    Redirect,
    /// A loanword supplies its own independent identity, form and marks.
    IndependentLoanword,
    /// The author's gloss supplies a form without a unique dictionary identity.
    Authorial,
}

/// One complete ranked choice with explicit form, identity and mark ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceChoice {
    /// Attested initial/positional output forms, without cross-source splicing.
    pub form: ReadingForm,
    /// The original candidate supplying identity and marks.
    pub anchor: SourceAnchor,
    /// The source supplying spelling, or `None` for a formal terminal/authorial form.
    pub form_source: Option<SourceId>,
    /// Constraints owned by the anchor, never OR-ed across sources.
    pub mark: MatchMark,
    /// Why this complete choice participates in ranking.
    pub reason: ChoiceReason,
}

/// A conservative source-resolution issue retained alongside the raw groups.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SelectionDiagnostic {
    /// A parent selected from a public legacy projection without leaf authority.
    /// This can identify an anchor or a form supplier. The projection retains
    /// its old policy role and reason but never contributes phrase witnesses.
    OpaqueLegacyProjection {
        /// First reserved source slot of the projected dictionary.
        source_id: SourceId,
        /// Number of source slots reserved by that dictionary.
        source_count: usize,
    },
    /// A reference candidate has no compiled source evidence.
    UnknownEvidence {
        /// The source with unknown evidence.
        source_id: SourceId,
        /// The affected stored reading.
        reading: String,
    },
    /// Compiled formal terminal forms disagree, so the original form was retained.
    ConflictingRedirect {
        /// The source whose terminal forms conflict.
        source_id: SourceId,
        /// The affected original reading.
        reading: String,
    },
    /// A promoted form conflicts with the anchor's lexical alias target.
    ConflictingAlias {
        /// The source owning the alias.
        source_id: SourceId,
        /// The rejected promoted form.
        form: ReadingForm,
    },
}

/// One source's unchanged candidate set and optional compiled evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct SourceGroup {
    /// The registered source supplying this group.
    pub source_id: SourceId,
    /// Its explicitly registered role.
    pub role: SourceRole,
    /// Its exact matching dictionary key, including any glyph alternative.
    pub key: String,
    /// Its canonical source-local candidates, without cross-source merging.
    pub candidates: ReadingCandidates,
    /// Its original candidate evidence, with absence meaning unknown provenance.
    pub evidence: Vec<CandidateEvidence>,
    alias_targets: BTreeMap<String, ReadingCandidates>,
}

impl SourceGroup {
    pub(crate) fn from_dictionary<D: HanjaDictionary + ?Sized>(
        dictionary: &D,
        source_id: SourceId,
        role: SourceRole,
        key: String,
        candidates: ReadingCandidates,
    ) -> Self {
        let evidence = dictionary
            .source_evidence()
            .and_then(|evidence| evidence.entries.get(&key))
            .cloned()
            .unwrap_or_default();
        let mut alias_targets = BTreeMap::new();
        for candidate in candidates.as_slice() {
            if let Some(alias) = &candidate.alias
                && let Some(target) = dictionary
                    .candidate_matches_at(&alias.canonical)
                    .find(|matched| matched.byte_len == alias.canonical.len())
            {
                alias_targets.insert(alias.canonical.clone(), target.candidates);
            }
        }
        Self {
            source_id,
            role,
            key,
            candidates,
            evidence,
            alias_targets,
        }
    }

    fn choice(&self, candidate: &ReadingCandidate, reason: ChoiceReason) -> SourceChoice {
        let alias = candidate.alias.clone();
        let lexical_canonical = alias
            .as_ref()
            .filter(|alias| matches!(alias.kind, AliasKind::Sibling | AliasKind::Lexical))
            .map_or_else(|| self.key.clone(), |alias| alias.canonical.clone());
        let homophone_canonical = alias
            .as_ref()
            .map_or_else(|| self.key.clone(), |alias| alias.canonical.clone());
        SourceChoice {
            form: ReadingForm {
                reading: candidate.reading.clone(),
                suffix_reading: candidate.suffix_reading.clone(),
            },
            anchor: SourceAnchor {
                source_id: self.source_id,
                key: self.key.clone(),
                reading: candidate.reading.clone(),
                suffix_reading: candidate.suffix_reading.clone(),
                alias,
                lexical_canonical,
                homophone_canonical,
            },
            form_source: Some(self.source_id),
            mark: candidate.mark,
            reason,
        }
    }

    fn permits_alias_form(&self, candidate: &ReadingCandidate, form: &ReadingForm) -> bool {
        let Some(alias) = &candidate.alias else {
            return true;
        };
        if candidate.reading == form.reading && candidate.suffix_reading == form.suffix_reading {
            return true;
        }
        self.alias_targets
            .get(&alias.canonical)
            .is_some_and(|target| {
                target.as_slice().iter().any(|candidate| {
                    candidate.reading == form.reading
                        && candidate.suffix_reading == form.suffix_reading
                })
            })
    }
}

/// Exact legacy fallback view retained separately from leaf-owned phrase choices.
///
/// A parent ranks its children's public legacy projections without borrowing
/// descendant evidence. A cooperative child's public projection is this view's
/// first choice; an ordered or plain child's projection is the full view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacySelection {
    /// Complete fallback choices in legacy particle-selection order.
    /// Must be nonempty; opaque identities are identified in pool diagnostics.
    pub choices: Arc<[SourceChoice]>,
    /// Whether this is a plain/ordered view whose trivial characters may merge.
    pub merge_eligible: bool,
}

/// Inspectable source groups and their active ordered choices for one span.
///
/// This pool is deliberately distinct from canonical `ReadingCandidates`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMatch {
    /// Exact fallback view when phrase evidence requires a separate proof pool.
    /// `None` uses `ranked` for ordinary cooperative selection. A present view
    /// preserves legacy policy and particle decisions, including opaque parent
    /// projections that need not occur in the leaf-owned `ranked` pool.
    pub legacy_selection: Option<LegacySelection>,
    /// Queried source spelling, distinct from each source group's dictionary key.
    pub hanja: String,
    /// Consumed queried-source length in UTF-8 bytes; dictionary keys may differ.
    pub byte_len: usize,
    /// Complete raw groups, in declared source order.
    pub groups: Vec<SourceGroup>,
    /// Active choices after policy and eligibility, before particle filtering.
    /// This may be empty when only an opaque legacy fallback survives policy.
    /// Built-in consumers ignore pools only when both selection views are empty.
    pub ranked: Arc<[SourceChoice]>,
    /// Structured conservative resolution issues.
    pub diagnostics: Arc<[SelectionDiagnostic]>,
}

impl SourceMatch {
    pub(crate) fn rank(
        byte_len: usize,
        hanja: String,
        groups: Vec<SourceGroup>,
        policy: SourcePolicy,
        reference_only: bool,
    ) -> Option<Self> {
        let active: Vec<_> = groups.iter().collect();
        let mut ranked = Vec::new();
        let mut diagnostics = Vec::new();
        if let Some(group) = active
            .iter()
            .find(|group| group.role == SourceRole::Override)
        {
            ranked.extend(
                group
                    .candidates
                    .as_slice()
                    .iter()
                    .map(|candidate| group.choice(candidate, ChoiceReason::Override)),
            );
        } else {
            let loans = || {
                active
                    .iter()
                    .filter(|group| !reference_only && group.role == SourceRole::Loanword)
                    .flat_map(|group| {
                        group.candidates.as_slice().iter().map(|candidate| {
                            group.choice(candidate, ChoiceReason::IndependentLoanword)
                        })
                    })
                    .collect::<Vec<_>>()
            };
            if policy == SourcePolicy::LoanwordFirst {
                ranked.extend(loans());
            }
            for group in active
                .iter()
                .filter(|group| group.role == SourceRole::Reference)
            {
                for candidate in group.candidates.as_slice() {
                    let original = group.choice(candidate, ChoiceReason::Reference);
                    if policy != SourcePolicy::ReferenceAuthorized {
                        ranked.push(original);
                        continue;
                    }
                    let evidence = group.evidence.iter().find(|evidence| {
                        evidence.reading == candidate.reading
                            && evidence.suffix_reading == candidate.suffix_reading
                    });
                    let Some(evidence) = evidence else {
                        if active.iter().any(|other| {
                            other.source_id != group.source_id
                                && other.candidates.as_slice().iter().any(|other| {
                                    other.reading != candidate.reading
                                        || other.suffix_reading != candidate.suffix_reading
                                })
                        }) {
                            diagnostics.push(SelectionDiagnostic::UnknownEvidence {
                                source_id: group.source_id,
                                reading: candidate.reading.clone(),
                            });
                        }
                        ranked.push(original);
                        continue;
                    };
                    let mut terminals = evidence
                        .authorized_forms
                        .iter()
                        .filter(|authorized| authorized.kind == AuthorizationKind::Redirect)
                        .map(|authorized| &authorized.form)
                        .collect::<Vec<_>>();
                    terminals.sort();
                    terminals.dedup();
                    if terminals.len() > 1 {
                        diagnostics.push(SelectionDiagnostic::ConflictingRedirect {
                            source_id: group.source_id,
                            reading: candidate.reading.clone(),
                        });
                        ranked.push(original);
                        continue;
                    }
                    // The offline compiler already intersected every contributing sense.
                    // Runtime compares complete pairs; it never infers extra grants.
                    for loan in active
                        .iter()
                        .filter(|group| group.role == SourceRole::Loanword)
                    {
                        for supplier in loan.candidates.as_slice() {
                            let form = ReadingForm {
                                reading: supplier.reading.clone(),
                                suffix_reading: supplier.suffix_reading.clone(),
                            };
                            if !evidence
                                .authorized_forms
                                .iter()
                                .any(|authorized| authorized.form == form)
                            {
                                continue;
                            }
                            if !group.permits_alias_form(candidate, &form) {
                                diagnostics.push(SelectionDiagnostic::ConflictingAlias {
                                    source_id: group.source_id,
                                    form,
                                });
                                continue;
                            }
                            let mut choice = original.clone();
                            choice.form = form;
                            choice.form_source = Some(loan.source_id);
                            choice.reason = ChoiceReason::AuthorizedLoanword;
                            ranked.push(choice);
                        }
                    }
                    if let Some(form) = terminals.first() {
                        if group.permits_alias_form(candidate, form) {
                            let mut terminal = original.clone();
                            terminal.form = (*form).clone();
                            terminal.form_source = None;
                            terminal.reason = ChoiceReason::Redirect;
                            ranked.push(terminal);
                        } else {
                            diagnostics.push(SelectionDiagnostic::ConflictingAlias {
                                source_id: group.source_id,
                                form: (*form).clone(),
                            });
                        }
                    }
                    ranked.push(original);
                }
            }
            if policy != SourcePolicy::LoanwordFirst {
                ranked.extend(loans());
            }
        }
        if ranked.is_empty() {
            return None;
        }
        Some(Self {
            legacy_selection: None,
            byte_len,
            hanja,
            groups,
            ranked: ranked.into(),
            diagnostics: diagnostics.into(),
        })
    }

    /// Selects the context-free effective default without discarding raw groups.
    pub fn default_choice(&self) -> &SourceChoice {
        &self.selection_choices()[0]
    }

    pub(crate) fn selection_choices(&self) -> &[SourceChoice] {
        self.legacy_selection
            .as_ref()
            .map(|legacy| legacy.choices.as_ref())
            .filter(|choices| !choices.is_empty())
            .unwrap_or(&self.ranked)
    }

    /// Projects the effective default as a canonical singleton candidate set.
    ///
    /// This is a legacy context-free view. Inspect `groups`/`ranked` to obtain
    /// alternatives and provenance, rather than treating this as the source pool.
    /// The consumed length is measured in the queried source spelling.
    pub fn default_match(&self) -> CandidateMatch {
        CandidateMatch {
            byte_len: self.byte_len,
            candidates: ReadingCandidates::from_legacy_choice(self.default_choice()),
        }
    }

    /// Selects using witnessed Korean particles and the legacy view when present.
    /// The full eligible ranked pool remains available for phrase constraints.
    pub fn select(&self, following: &str, final_end: bool) -> (Match, SelectedReading) {
        let choices = self.selection_choices();
        let index = crate::particles::select_form_index(
            choices
                .iter()
                .map(|choice| (&*choice.form.reading, choice.form.suffix_reading.as_deref())),
            following,
            final_end,
        );
        let choice = choices[index].clone();
        let matched = Match {
            byte_len: self.byte_len,
            reading: choice.form.reading.clone(),
            suffix_reading: choice.form.suffix_reading.clone(),
            mark: choice.mark,
        };
        (
            matched,
            SelectedReading {
                legacy_merge_eligible: self
                    .legacy_selection
                    .as_ref()
                    .is_some_and(|legacy| legacy.merge_eligible),
                choice: Box::new(choice),
                ranked: self.ranked.clone(),
                diagnostics: self.diagnostics.clone(),
                directive_mark: MatchMark::default(),
            },
        )
    }
}

impl ReadingCandidates {
    fn from_legacy_choice(choice: &SourceChoice) -> Self {
        // A singleton has canonical order without merging unrelated sources.
        let candidate = ReadingCandidate {
            reading: choice.form.reading.clone(),
            suffix_reading: choice.form.suffix_reading.clone(),
            mark: choice.mark,
            preference: 0,
            alias: choice.anchor.alias.clone(),
        };
        // Legacy insertions can contain an empty form, which cannot be encoded;
        // preserve the legacy compatibility projection in that case.
        if candidate.reading.is_empty()
            || candidate
                .suffix_reading
                .as_ref()
                .is_some_and(String::is_empty)
        {
            return Self::from_legacy(candidate.reading, candidate.suffix_reading, candidate.mark);
        }
        Self::new(alloc::vec![candidate]).expect("one validated source choice is canonical")
    }
}

/// Selected source metadata retained through positional spelling and middleware.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedReading {
    /// Preserves the source pool's legacy annotation-merge eligibility.
    ///
    /// This controls rendering boundaries, not candidate identity or authority.
    /// Successful phrase selection preserves its complete attested edge instead.
    pub legacy_merge_eligible: bool,
    /// The actual selected form and its identity/mark ownership.
    /// An opaque legacy fallback may be absent from `ranked`; only a successful
    /// phrase proof replaces it with a complete leaf-owned choice.
    pub choice: Box<SourceChoice>,
    /// The complete eligible phrase pool, also used for authorial revalidation.
    /// Opaque legacy fallback projections never enter this proof pool.
    pub ranked: Arc<[SourceChoice]>,
    /// Resolution issues for inspection, independent of homophone rendering.
    pub diagnostics: Arc<[SelectionDiagnostic]>,
    /// Explicit directive constraints preserved if a later gloss changes identity.
    pub directive_mark: MatchMark,
}

impl SelectedReading {
    pub(crate) fn revalidate(&mut self, reading: &str) -> MatchMark {
        let mut matching = self.ranked.iter().filter(|choice| {
            choice.form.reading == reading || choice.form.suffix_reading.as_deref() == Some(reading)
        });
        if let Some(first) = matching.next() {
            // Source IDs do not create semantic ambiguity by themselves.
            if matching.all(|other| {
                other.anchor.key == first.anchor.key
                    && other.anchor.reading == first.anchor.reading
                    && other.anchor.suffix_reading == first.anchor.suffix_reading
                    && other.anchor.alias == first.anchor.alias
                    && other.mark == first.mark
                    && other.form == first.form
            }) {
                *self.choice = first.clone();
                return first.mark;
            }
        }
        let key = self.choice.anchor.key.clone();
        self.choice.form = ReadingForm {
            reading: reading.into(),
            suffix_reading: None,
        };
        self.choice.anchor.reading = reading.into();
        self.choice.anchor.suffix_reading = None;
        self.choice.anchor.alias = None;
        self.choice.anchor.lexical_canonical = key.clone();
        self.choice.anchor.homophone_canonical = key;
        self.choice.mark = MatchMark::default();
        self.choice.form_source = None;
        self.choice.reason = ChoiceReason::Authorial;
        MatchMark::default()
    }
}

// Engine-owned memoization keeps public dictionary Send/Sync capabilities intact.
// An Engine immutably borrows one dictionary generation and policy for its life.
#[derive(Default)]
pub(crate) struct ResolutionCache {
    entries: BTreeMap<CacheKey, Vec<(usize, SourceMatch)>>,
    bytes: usize,
    enabled: Option<bool>,
    spellings_enabled: Option<bool>,
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
enum CacheKey {
    Exact(String),
    Spellings(Vec<String>),
}

impl ResolutionCache {
    const MAX_KEYS: usize = 4096;
    const MAX_BYTES: usize = 32 * 1024 * 1024;

    pub(crate) fn exact<D: HanjaDictionary + ?Sized>(
        &mut self,
        dictionary: &D,
        key: &str,
    ) -> Option<Vec<SourceMatch>> {
        if self.enabled == Some(false) {
            return None;
        }
        // Unknown word bounds can pass an entire long suffix to lookup. Avoid
        // retaining those suffixes as cache keys; resolve the full input as
        // usual, including exact dictionary words longer than the glyph budget.
        if dictionary.max_word_chars().is_none()
            && key.len() > crate::variants::MAX_VARIANT_PREFIX_CHARS * 4
        {
            let result = dictionary.source_matches_at(key);
            self.enabled = Some(result.is_some());
            return result.map(|matches| {
                matches
                    .filter(|pool| !pool.selection_choices().is_empty())
                    .collect()
            });
        }
        let cache_key = CacheKey::Exact(key.into());
        if let Some(cached) = self.entries.get(&cache_key) {
            return Some(cached.iter().map(|(_, matched)| matched.clone()).collect());
        }
        let result = dictionary.source_matches_at(key);
        self.enabled = Some(result.is_some());
        let matches: Vec<_> = result?
            .filter(|pool| !pool.selection_choices().is_empty())
            .collect();
        self.insert(
            cache_key,
            matches
                .iter()
                .cloned()
                .map(|matched| (0, matched))
                .collect(),
        );
        Some(matches)
    }

    pub(crate) fn spellings<D: HanjaDictionary + ?Sized>(
        &mut self,
        dictionary: &D,
        spellings: &[&str],
    ) -> Option<Vec<(usize, SourceMatch)>> {
        if self.spellings_enabled == Some(false) {
            return None;
        }
        let cache_key =
            CacheKey::Spellings(spellings.iter().map(|key| String::from(*key)).collect());
        if let Some(cached) = self.entries.get(&cache_key) {
            return Some(cached.clone());
        }
        let matches = dictionary.source_matches_at_spellings(spellings);
        self.spellings_enabled = Some(matches.is_some());
        let matches: Vec<_> = matches?
            .into_iter()
            .filter(|(_, pool)| !pool.selection_choices().is_empty())
            .collect();
        self.insert(cache_key, matches.clone());
        Some(matches)
    }

    fn insert(&mut self, key: CacheKey, value: Vec<(usize, SourceMatch)>) {
        self.insert_bounded(key, value, Self::MAX_KEYS, Self::MAX_BYTES);
    }

    fn insert_bounded(
        &mut self,
        key: CacheKey,
        value: Vec<(usize, SourceMatch)>,
        max_keys: usize,
        max_bytes: usize,
    ) {
        let key_bytes = match &key {
            CacheKey::Exact(key) => key.len(),
            CacheKey::Spellings(keys) => keys
                .iter()
                .map(|key| key.len() + core::mem::size_of::<String>())
                .sum(),
        };
        // Account conservatively for owned data and tree allocations, including
        // pools that remain shared with already emitted annotations.
        let bytes = value
            .iter()
            .fold(key_bytes.saturating_add(256), |sum, (_, matched)| {
                sum.saturating_add(matched.cache_bytes())
            });
        if bytes > max_bytes || max_keys == 0 {
            return;
        }
        if self.entries.len() >= max_keys || self.bytes.saturating_add(bytes) > max_bytes {
            self.entries.clear();
            self.bytes = 0;
        }
        self.bytes = self.bytes.saturating_add(bytes);
        self.entries.insert(key, value);
    }
}

impl SourceMatch {
    fn cache_bytes(&self) -> usize {
        fn candidate_bytes(candidate: &ReadingCandidate) -> usize {
            core::mem::size_of::<ReadingCandidate>()
                + candidate.reading.capacity()
                + candidate
                    .suffix_reading
                    .as_ref()
                    .map_or(0, String::capacity)
                + candidate
                    .alias
                    .as_ref()
                    .map_or(0, |alias| alias.canonical.capacity())
        }
        fn form_bytes(form: &ReadingForm) -> usize {
            form.reading.capacity() + form.suffix_reading.as_ref().map_or(0, String::capacity)
        }
        let mut bytes = core::mem::size_of::<Self>() + self.hanja.capacity();
        for group in &self.groups {
            bytes += core::mem::size_of::<SourceGroup>() + group.key.capacity();
            bytes += group
                .candidates
                .as_slice()
                .iter()
                .map(candidate_bytes)
                .sum::<usize>();
            for (key, target) in &group.alias_targets {
                bytes += key.capacity()
                    + 256
                    + target.as_slice().iter().map(candidate_bytes).sum::<usize>();
            }
            for evidence in &group.evidence {
                bytes += core::mem::size_of::<CandidateEvidence>()
                    + evidence.reading.capacity()
                    + evidence.suffix_reading.as_ref().map_or(0, String::capacity)
                    + evidence.contributors.len()
                        * core::mem::size_of::<crate::SourceContribution>();
                for form in &evidence.authorized_forms {
                    bytes += core::mem::size_of::<crate::AuthorizedForm>()
                        + form_bytes(&form.form)
                        + form.witnesses.len() * core::mem::size_of::<crate::RelationWitness>();
                }
                for diagnostic in &evidence.diagnostics {
                    bytes += core::mem::size_of::<crate::EvidenceDiagnostic>()
                        + diagnostic.message.capacity();
                }
            }
        }
        let extra = self
            .legacy_selection
            .as_ref()
            .filter(|legacy| !Arc::ptr_eq(&legacy.choices, &self.ranked));
        for choice in self
            .ranked
            .iter()
            .chain(extra.into_iter().flat_map(|legacy| legacy.choices.iter()))
        {
            bytes += core::mem::size_of::<SourceChoice>()
                + form_bytes(&choice.form)
                + choice.anchor.key.capacity()
                + choice.anchor.reading.capacity()
                + choice
                    .anchor
                    .suffix_reading
                    .as_ref()
                    .map_or(0, String::capacity)
                + choice
                    .anchor
                    .alias
                    .as_ref()
                    .map_or(0, |alias| alias.canonical.capacity())
                + choice.anchor.lexical_canonical.capacity()
                + choice.anchor.homophone_canonical.capacity();
        }
        for diagnostic in self.diagnostics.iter() {
            bytes += core::mem::size_of::<SelectionDiagnostic>()
                + match diagnostic {
                    SelectionDiagnostic::UnknownEvidence { reading, .. }
                    | SelectionDiagnostic::ConflictingRedirect { reading, .. } => {
                        reading.capacity()
                    }
                    SelectionDiagnostic::ConflictingAlias { form, .. } => form_bytes(form),
                    SelectionDiagnostic::OpaqueLegacyProjection { .. } => 0,
                };
        }
        // Vec/BTreeMap capacities and allocator alignment can exceed payload size.
        bytes.saturating_mul(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChainDictionary, MapDictionary};
    use core::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn unknown_long_queries_are_resolved_fully_without_cache_key_retention() {
        struct Counted {
            inner: ChainDictionary<MapDictionary>,
            calls: AtomicUsize,
            known_bound: bool,
        }
        impl HanjaDictionary for Counted {
            fn matches_at<'a>(
                &'a self,
                s: &'a str,
            ) -> alloc::boxed::Box<dyn Iterator<Item = Match> + 'a> {
                self.inner.matches_at(s)
            }
            fn max_word_chars(&self) -> Option<usize> {
                self.known_bound
                    .then(|| self.inner.max_word_chars())
                    .flatten()
            }
            fn source_matches_at<'a>(
                &'a self,
                s: &'a str,
            ) -> Option<alloc::boxed::Box<dyn Iterator<Item = SourceMatch> + 'a>> {
                self.calls.fetch_add(1, Ordering::Relaxed);
                self.inner.source_matches_at(s)
            }
        }
        let key = "甲".repeat(1000);
        let mut dictionary = MapDictionary::new();
        dictionary.insert(&key, "긴말");
        let mut inner =
            ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
        inner.push_with_role(dictionary, SourceRole::Reference);
        let mut dictionary = Counted {
            inner,
            calls: AtomicUsize::new(0),
            known_bound: false,
        };
        let mut cache = ResolutionCache::default();
        let first = cache.exact(&dictionary, &key).unwrap();
        assert_eq!(first[0].hanja, key);
        assert_eq!(first[0].byte_len, key.len());
        assert_eq!(first[0].default_choice().form.reading, "긴말");
        let second = cache.exact(&dictionary, &key).unwrap();
        assert_eq!(first, second);
        assert_eq!(dictionary.calls.load(Ordering::Relaxed), 2);
        assert!(cache.entries.is_empty());
        assert_eq!(cache.bytes, 0);
        dictionary.known_bound = true;
        let mut bounded_cache = ResolutionCache::default();
        bounded_cache.exact(&dictionary, &key).unwrap();
        bounded_cache.exact(&dictionary, &key).unwrap();
        assert_eq!(dictionary.calls.load(Ordering::Relaxed), 3);
        assert_eq!(bounded_cache.entries.len(), 1);
        dictionary.known_bound = false;
        dictionary.inner.set_source_policy(SourcePolicy::Ordered);
        let mut legacy = ResolutionCache::default();
        assert!(legacy.exact(&dictionary, &key).is_none());
        assert!(legacy.exact(&dictionary, &key).is_none());
        assert_eq!(dictionary.calls.load(Ordering::Relaxed), 4);
        assert!(legacy.entries.is_empty());
        assert_eq!(legacy.enabled, Some(false));
    }

    #[test]
    fn bounded_cache_eviction_never_changes_selection_and_counts_owned_capacity() {
        let mut dictionary = MapDictionary::new();
        dictionary.insert("霧川", "무천");
        let mut chain =
            ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
        chain.push_with_role(dictionary, SourceRole::Reference);
        let matched = chain.source_matches_at("霧川").unwrap().next().unwrap();
        let mut cache = ResolutionCache::default();
        for key in ["a", "b", "c"] {
            cache.insert_bounded(
                CacheKey::Exact(key.into()),
                alloc::vec![(0, matched.clone())],
                2,
                1024 * 1024,
            );
        }
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(
            cache.entries.values().next().unwrap()[0].1.default_choice(),
            matched.default_choice()
        );
        let mut huge = matched.clone();
        let mut choices = huge.ranked.to_vec();
        choices[0].form.reading.reserve(4096);
        huge.ranked = choices.into();
        cache.insert_bounded(
            CacheKey::Exact("oversized".into()),
            alloc::vec![(0, huge)],
            2,
            4096,
        );
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.bytes <= 1024 * 1024);
    }

    #[test]
    fn engine_cache_memoizes_only_stable_source_pools_not_particle_selection() {
        struct Counted {
            inner: ChainDictionary<MapDictionary>,
            calls: AtomicUsize,
        }
        impl HanjaDictionary for Counted {
            fn matches_at<'a>(
                &'a self,
                s: &'a str,
            ) -> alloc::boxed::Box<dyn Iterator<Item = Match> + 'a> {
                self.inner.matches_at(s)
            }
            fn source_matches_at<'a>(
                &'a self,
                s: &'a str,
            ) -> Option<alloc::boxed::Box<dyn Iterator<Item = SourceMatch> + 'a>> {
                self.calls.fetch_add(1, Ordering::Relaxed);
                self.inner.source_matches_at(s)
            }
        }
        let mut reference = MapDictionary::new();
        reference.insert("霧川", "무천");
        let mut loan = MapDictionary::new();
        loan.insert("霧川", "모가");
        let mut inner =
            ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
        inner.push_with_role(reference, SourceRole::Reference);
        inner.push_with_role(loan, SourceRole::Loanword);
        let dictionary = Counted {
            inner,
            calls: AtomicUsize::new(0),
        };
        let mut cache = ResolutionCache::default();
        let first = cache.exact(&dictionary, "霧川").unwrap().remove(0);
        let second = cache.exact(&dictionary, "霧川").unwrap().remove(0);
        assert_eq!(dictionary.calls.load(Ordering::Relaxed), 1);
        assert_eq!(first.select("을", true).0.reading, "무천");
        assert_eq!(second.select("를", true).0.reading, "모가");
        let mut legacy = ResolutionCache::default();
        assert!(legacy.exact(&MapDictionary::new(), "霧川").is_none());
        assert!(legacy.entries.is_empty());
    }
}
