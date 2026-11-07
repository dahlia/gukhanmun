// Gukhanmun: Offline typed source evidence compilation.
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

//! Compiles source relations before dictionary admission discards source records.
//!
//! Word and sense identifiers occupy separate namespaces. Only explicit outgoing
//! synonym edges and formal redirects authorize forms; headword text is never
//! used to guess an edge. A candidate receives the intersection of every
//! contributing sense's authorization, retaining its original form on failure.

use std::collections::{BTreeMap, BTreeSet};

use gukhanmun_core::{
    AuthorizationKind, AuthorizedForm, CandidateEvidence, EvidenceDiagnostic,
    EvidenceDiagnosticKind, ReadingForm, RelationWitness, SenseId, SourceContribution,
    SourceReference, WordId, WordOrigin,
};

/// A machine-readable redirect, distinct from an incidental definition link.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SourceRedirect {
    /// The sense has a substantive definition rather than a redirect.
    #[default]
    None,
    /// The source explicitly names a typed target.
    Target(SourceReference),
    /// A redirect was present but its machine target was not unambiguous.
    Invalid,
}

/// A sense retained even when its word is excluded from lookup admission.
#[derive(Clone, Debug)]
pub struct SourceSense {
    /// Identifier in the source's sense namespace, when provided.
    pub id: Option<SenseId>,
    /// Whether the source explicitly classifies this sense as a foreign name.
    pub foreign_name: bool,
    /// Formal redirect parsed from machine-readable definition markup.
    pub redirect: SourceRedirect,
    /// Direct outgoing synonym targets; `None` records an untyped/malformed edge.
    pub synonyms: Vec<Option<SourceReference>>,
}

/// A raw word retained independently of its hanja lookup-key eligibility.
#[derive(Clone, Debug)]
pub struct SourceWord {
    /// Identifier in the source's word namespace.
    pub id: WordId,
    /// Output spelling with editorial markers removed and phrase spaces retained.
    pub reading: String,
    /// Origin from explicit source metadata, never inferred from lookup overlap.
    pub origin: WordOrigin,
    /// Every sense of this record, including senses with no usable relations.
    pub senses: Vec<SourceSense>,
    /// Word-level direct outgoing synonyms, applied only to foreign-name senses.
    pub synonyms: Vec<Option<SourceReference>>,
}

/// Explicit work limits for offline graph compilation.
#[derive(Clone, Copy, Debug)]
pub struct EvidenceBudget {
    /// Maximum number of redirect edges along a path (default 32).
    pub redirect_hops: usize,
    /// Maximum distinct additional forms per candidate (default 256).
    pub authorized_forms: usize,
    /// Maximum word/sense inspections per target expansion (default 65,536).
    pub target_visits: usize,
}

impl Default for EvidenceBudget {
    fn default() -> Self {
        Self {
            redirect_hops: 32,
            authorized_forms: 256,
            target_visits: 65_536,
        }
    }
}

/// Complete typed source indexes used only during dictionary generation.
///
/// Construct this after ingesting every shard, before compiling any candidate.
/// Duplicate identifiers remain visible and reject promotions instead of being
/// silently overwritten according to shard order.
pub struct SourceGraph {
    words: Vec<SourceWord>,
    word_index: BTreeMap<WordId, Vec<usize>>,
    sense_index: BTreeMap<SenseId, Vec<(usize, usize)>>,
    unresolved_senses: BTreeSet<SenseId>,
}

impl SourceGraph {
    /// Indexes identified raw words and senses without lookup admission rules.
    ///
    /// Also call [`Self::with_unresolved_senses`] for sense IDs on raw records
    /// whose owning words have no usable identifier.
    pub fn new(words: Vec<SourceWord>) -> Self {
        let mut word_index = BTreeMap::<WordId, Vec<usize>>::new();
        let mut sense_index = BTreeMap::<SenseId, Vec<(usize, usize)>>::new();
        for (wi, word) in words.iter().enumerate() {
            word_index.entry(word.id).or_default().push(wi);
            for (si, sense) in word.senses.iter().enumerate() {
                if let Some(id) = sense.id {
                    sense_index.entry(id).or_default().push((wi, si));
                }
            }
        }
        Self {
            words,
            word_index,
            sense_index,
            unresolved_senses: BTreeSet::new(),
        }
    }

    /// Retains sense IDs whose owning raw records lack a word identifier.
    ///
    /// Such senses cannot supply a terminal form. If an identified record shares
    /// the same sense ID, both its target and contributor uses are ambiguous.
    /// This preserves incomplete source evidence without inventing word IDs.
    pub fn with_unresolved_senses(mut self, ids: impl IntoIterator<Item = SenseId>) -> Self {
        self.unresolved_senses.extend(ids);
        self
    }

    fn ambiguous_sense(&self, id: SenseId) -> bool {
        self.unresolved_senses.contains(&id)
            || self
                .sense_index
                .get(&id)
                .is_some_and(|indices| indices.len() != 1)
    }

    /// Compiles permissions for a preserved dictionary candidate and all its
    /// contributing word IDs, including contributors to its suffix form.
    ///
    /// Each source sense must independently allow a replacement. Any missing,
    /// ambiguous, cyclic, malformed, or over-budget eligible relation invalidates
    /// the candidate's entire promotion, while diagnostics and contributors remain.
    /// The original candidate form is implicit and is never emitted as a grant.
    pub fn compile_candidate(
        &self,
        form: ReadingForm,
        word_ids: impl IntoIterator<Item = WordId>,
        budget: EvidenceBudget,
    ) -> CandidateEvidence {
        let mut evidence = CandidateEvidence {
            reading: form.reading.clone(),
            suffix_reading: form.suffix_reading.clone(),
            contributors: Vec::new(),
            authorized_forms: Vec::new(),
            diagnostics: Vec::new(),
        };
        let mut common: Option<BTreeMap<ReadingForm, AuthorizedForm>> = None;
        let mut failed = false;
        for id in word_ids.into_iter().collect::<BTreeSet<_>>() {
            let source = SourceReference::Word(id);
            let Some(indices) = self.word_index.get(&id) else {
                evidence.contributors.push(SourceContribution {
                    word_id: id,
                    sense_id: None,
                    origin: WordOrigin::Unknown,
                });
                evidence.diagnostics.push(diagnostic(
                    EvidenceDiagnosticKind::MissingTarget,
                    source,
                    "contributing word is absent from the complete source index",
                ));
                failed = true;
                continue;
            };
            if indices.len() != 1 {
                evidence.diagnostics.push(diagnostic(
                    EvidenceDiagnosticKind::AmbiguousTarget,
                    source,
                    "contributing word identifier is not unique",
                ));
                failed = true;
            }
            for &wi in indices {
                let word = &self.words[wi];
                if word.senses.is_empty() {
                    evidence.contributors.push(SourceContribution {
                        word_id: id,
                        sense_id: None,
                        origin: word.origin,
                    });
                    intersect(&mut common, BTreeMap::new());
                }
                for sense in &word.senses {
                    evidence.contributors.push(SourceContribution {
                        word_id: id,
                        sense_id: sense.id,
                        origin: word.origin,
                    });
                    let source = sense.id.map(SourceReference::Sense).unwrap_or(source);
                    if sense.id.is_some_and(|id| self.ambiguous_sense(id)) {
                        evidence.diagnostics.push(diagnostic(
                            EvidenceDiagnosticKind::AmbiguousTarget,
                            source,
                            "contributing sense identifier is not unique",
                        ));
                        failed = true;
                    }
                    if word.origin == WordOrigin::Unknown {
                        evidence.diagnostics.push(diagnostic(
                            EvidenceDiagnosticKind::UnknownOrigin,
                            source,
                            "source origin is unknown; synonyms cannot authorize a form",
                        ));
                    }
                    match self.allowed_forms(word, sense, source, &form, budget) {
                        Ok(forms) => intersect(&mut common, forms),
                        Err(error) => {
                            evidence.diagnostics.push(error);
                            failed = true;
                        }
                    }
                }
            }
        }
        let mut contributors = BTreeMap::new();
        for contribution in evidence.contributors {
            contributors
                .entry((contribution.word_id, contribution.sense_id))
                .and_modify(|origin| {
                    if *origin != contribution.origin {
                        *origin = WordOrigin::Unknown;
                    }
                })
                .or_insert(contribution.origin);
        }
        evidence.contributors = contributors
            .into_iter()
            .map(|((word_id, sense_id), origin)| SourceContribution {
                word_id,
                sense_id,
                origin,
            })
            .collect();
        evidence.diagnostics.sort();
        evidence.diagnostics.dedup();
        if !failed {
            evidence.authorized_forms = common.unwrap_or_default().into_values().collect();
        }
        evidence
    }

    fn allowed_forms(
        &self,
        word: &SourceWord,
        sense: &SourceSense,
        source: SourceReference,
        original_form: &ReadingForm,
        budget: EvidenceBudget,
    ) -> Result<BTreeMap<ReadingForm, AuthorizedForm>, EvidenceDiagnostic> {
        let mut targets = Vec::new();
        match sense.redirect {
            SourceRedirect::None => {}
            SourceRedirect::Target(target) => {
                targets.push((source, target, AuthorizationKind::Redirect))
            }
            SourceRedirect::Invalid => {
                return Err(diagnostic(
                    EvidenceDiagnosticKind::UnsupportedRelation,
                    source,
                    "formal redirect has no unique typed machine target",
                ));
            }
        }
        if word.origin == WordOrigin::Foreign && sense.foreign_name {
            for (edge_source, target) in word
                .synonyms
                .iter()
                .map(|target| (SourceReference::Word(word.id), target))
                .chain(sense.synonyms.iter().map(|target| (source, target)))
            {
                let Some(target) = target else {
                    return Err(diagnostic(
                        EvidenceDiagnosticKind::UnsupportedRelation,
                        edge_source,
                        "outgoing synonym has no supported typed target",
                    ));
                };
                targets.push((edge_source, *target, AuthorizationKind::Synonym));
            }
        }
        let mut forms = BTreeMap::<ReadingForm, AuthorizedForm>::new();
        for (edge_source, target, kind) in targets {
            let initial_hops = usize::from(kind == AuthorizationKind::Redirect);
            let (form, mut witnesses) =
                self.resolve_target(edge_source, target, initial_hops, budget)?;
            if &form == original_form {
                continue;
            }
            witnesses.sort();
            witnesses.dedup();
            if let Some(existing) = forms.get_mut(&form) {
                // A formal redirect is sufficient even when a synonym agrees.
                if kind == AuthorizationKind::Redirect {
                    existing.kind = kind;
                }
                existing.witnesses.extend(witnesses);
                existing.witnesses.sort();
                existing.witnesses.dedup();
            } else {
                forms.insert(
                    form.clone(),
                    AuthorizedForm {
                        form,
                        kind,
                        witnesses,
                    },
                );
            }
            if forms.len() > budget.authorized_forms {
                return Err(diagnostic(
                    EvidenceDiagnosticKind::BudgetExceeded,
                    source,
                    "candidate authorized-form budget exceeded",
                ));
            }
        }
        Ok(forms)
    }

    fn resolve_target(
        &self,
        source: SourceReference,
        target: SourceReference,
        initial_hops: usize,
        budget: EvidenceBudget,
    ) -> Result<(ReadingForm, Vec<RelationWitness>), EvidenceDiagnostic> {
        if initial_hops > budget.redirect_hops {
            return Err(diagnostic(
                EvidenceDiagnosticKind::BudgetExceeded,
                source,
                "redirect hop budget exceeded",
            ));
        }
        // A synonym's source has not been traversed as a redirect node. A
        // substantive self-synonym is harmless and must not suppress other grants.
        let path = if initial_hops == 0 {
            Vec::new()
        } else {
            vec![source]
        };
        let mut pending = vec![(target, path, initial_hops)];
        let mut visits = 0usize;
        let mut outcomes = BTreeSet::new();
        let mut witnesses = vec![RelationWitness { source, target }];
        while let Some((node, mut path, depth)) = pending.pop() {
            if visits >= budget.target_visits {
                return Err(diagnostic(
                    EvidenceDiagnosticKind::BudgetExceeded,
                    node,
                    "target traversal budget exceeded",
                ));
            }
            visits += 1;
            if path.contains(&node) {
                return Err(diagnostic(
                    EvidenceDiagnosticKind::Cycle,
                    node,
                    "formal redirect cycle",
                ));
            }
            path.push(node);
            let (wi, senses): (usize, Vec<usize>) = match node {
                SourceReference::Word(id) => {
                    let indices = self.word_index.get(&id).ok_or_else(|| {
                        diagnostic(
                            EvidenceDiagnosticKind::MissingTarget,
                            node,
                            "word target is missing",
                        )
                    })?;
                    if indices.len() != 1 {
                        return Err(diagnostic(
                            EvidenceDiagnosticKind::AmbiguousTarget,
                            node,
                            "word target identifier is not unique",
                        ));
                    }
                    let wi = indices[0];
                    (wi, (0..self.words[wi].senses.len()).collect())
                }
                SourceReference::Sense(id) => {
                    let indices = self.sense_index.get(&id).ok_or_else(|| {
                        diagnostic(
                            EvidenceDiagnosticKind::MissingTarget,
                            node,
                            if self.unresolved_senses.contains(&id) {
                                "sense target has an unidentified owning word"
                            } else {
                                "sense target is missing"
                            },
                        )
                    })?;
                    if self.ambiguous_sense(id) {
                        return Err(diagnostic(
                            EvidenceDiagnosticKind::AmbiguousTarget,
                            node,
                            "sense target identifier is not unique",
                        ));
                    }
                    let (wi, si) = indices[0];
                    (wi, vec![si])
                }
            };
            let word = &self.words[wi];
            if word.reading.is_empty() || senses.is_empty() {
                return Err(diagnostic(
                    EvidenceDiagnosticKind::MissingTarget,
                    node,
                    "target has no usable spelling or senses",
                ));
            }
            for si in senses {
                if visits >= budget.target_visits {
                    return Err(diagnostic(
                        EvidenceDiagnosticKind::BudgetExceeded,
                        node,
                        "target traversal budget exceeded",
                    ));
                }
                visits += 1;
                let sense = &self.words[wi].senses[si];
                let redirect_source = sense.id.map(SourceReference::Sense).unwrap_or(node);
                if sense.id.is_some_and(|id| self.ambiguous_sense(id)) {
                    return Err(diagnostic(
                        EvidenceDiagnosticKind::AmbiguousTarget,
                        redirect_source,
                        "target member sense identifier is not unique",
                    ));
                }
                match sense.redirect {
                    SourceRedirect::None => {
                        outcomes.insert(word.reading.clone());
                    }
                    SourceRedirect::Invalid => {
                        return Err(diagnostic(
                            EvidenceDiagnosticKind::UnsupportedRelation,
                            node,
                            "target redirect has no unique typed machine target",
                        ));
                    }
                    SourceRedirect::Target(next) => {
                        if depth >= budget.redirect_hops {
                            return Err(diagnostic(
                                EvidenceDiagnosticKind::BudgetExceeded,
                                node,
                                "redirect hop budget exceeded",
                            ));
                        }
                        witnesses.push(RelationWitness {
                            source: redirect_source,
                            target: next,
                        });
                        pending.push((next, path.clone(), depth + 1));
                    }
                }
            }
        }
        if outcomes.len() != 1 {
            return Err(diagnostic(
                EvidenceDiagnosticKind::AmbiguousTarget,
                target,
                "target senses disagree on the terminal output spelling",
            ));
        }
        Ok((
            ReadingForm {
                reading: outcomes.into_iter().next().expect("one outcome"),
                suffix_reading: None,
            },
            witnesses,
        ))
    }
}

fn intersect(
    common: &mut Option<BTreeMap<ReadingForm, AuthorizedForm>>,
    allowed: BTreeMap<ReadingForm, AuthorizedForm>,
) {
    let Some(common) = common else {
        *common = Some(allowed);
        return;
    };
    common.retain(|form, grant| {
        let Some(other) = allowed.get(form) else {
            return false;
        };
        if other.kind != AuthorizationKind::Redirect {
            grant.kind = AuthorizationKind::Synonym;
        }
        grant.witnesses.extend(other.witnesses.clone());
        grant.witnesses.sort();
        grant.witnesses.dedup();
        true
    });
}

fn diagnostic(
    kind: EvidenceDiagnosticKind,
    source: SourceReference,
    message: &str,
) -> EvidenceDiagnostic {
    EvidenceDiagnostic {
        kind,
        source: Some(source),
        message: message.to_owned(),
    }
}

/// Parses a sense's formal Standard Korean Dictionary redirect target.
///
/// Only arrow definitions with exactly one well-formed `<sense_no>` or
/// `<word_no>` reference are accepted. These tags select separate identifier
/// namespaces; unsupported markup (including `<sp_no>`) is not guessed.
/// Incidental links in substantive definitions are ignored. Visible target
/// labels are never used to resolve identities or output forms.
pub fn parse_sense_redirect(definition_original: Option<&str>) -> SourceRedirect {
    let Some(definition) = definition_original.map(str::trim) else {
        return SourceRedirect::None;
    };
    let Some(rest) = definition.strip_prefix('→') else {
        return SourceRedirect::None;
    };
    let rest = rest.trim_start();
    let (rest, closing, sense_target) = if let Some(rest) = rest.strip_prefix("<sense_no>") {
        (rest, "</sense_no>", true)
    } else if let Some(rest) = rest.strip_prefix("<word_no>") {
        (rest, "</word_no>", false)
    } else {
        return SourceRedirect::Invalid;
    };
    let Some((number, tail)) = rest.split_once(closing) else {
        return SourceRedirect::Invalid;
    };
    if tail.contains(['<', '>'])
        || number.is_empty()
        || !number.bytes().all(|byte| byte.is_ascii_digit())
    {
        return SourceRedirect::Invalid;
    }
    let Ok(id) = number.parse::<u64>() else {
        return SourceRedirect::Invalid;
    };
    SourceRedirect::Target(if sense_target {
        SourceReference::Sense(SenseId(id))
    } else {
        SourceReference::Word(WordId(id))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(reading: &str) -> ReadingForm {
        ReadingForm {
            reading: reading.into(),
            suffix_reading: None,
        }
    }

    fn sense(id: u64) -> SourceSense {
        SourceSense {
            id: Some(SenseId(id)),
            foreign_name: true,
            redirect: SourceRedirect::None,
            synonyms: Vec::new(),
        }
    }

    fn word(id: u64, reading: &str, sense_id: u64) -> SourceWord {
        SourceWord {
            id: WordId(id),
            reading: reading.into(),
            origin: WordOrigin::Foreign,
            senses: vec![sense(sense_id)],
            synonyms: Vec::new(),
        }
    }

    fn compile(words: Vec<SourceWord>) -> CandidateEvidence {
        SourceGraph::new(words).compile_candidate(
            form("가온"),
            [WordId(1)],
            EvidenceBudget::default(),
        )
    }

    #[test]
    fn sense_targets_never_resolve_as_same_numeric_word_id() {
        let mut origin = word(1, "가온", 10);
        origin.senses[0]
            .synonyms
            .push(Some(SourceReference::Sense(SenseId(2))));
        let evidence = compile(vec![origin, word(2, "무관", 20), word(3, "누리", 2)]);
        assert_eq!(evidence.authorized_forms[0].form.reading, "누리");
        assert_eq!(
            evidence.authorized_forms[0].witnesses[0].target,
            SourceReference::Sense(SenseId(2))
        );
    }

    #[test]
    fn all_contributing_senses_and_words_must_authorize() {
        let mut origin = word(1, "가온", 10);
        origin.senses[0]
            .synonyms
            .push(Some(SourceReference::Sense(SenseId(20))));
        origin.senses.push(sense(11));
        assert!(
            compile(vec![origin.clone(), word(2, "누리", 20)])
                .authorized_forms
                .is_empty()
        );
        origin.senses.pop();
        let graph = SourceGraph::new(vec![origin, word(2, "누리", 20), word(3, "가온", 30)]);
        let evidence = graph.compile_candidate(
            form("가온"),
            [WordId(1), WordId(3)],
            EvidenceBudget::default(),
        );
        assert_eq!(evidence.contributors.len(), 2);
        assert!(evidence.authorized_forms.is_empty());
    }

    #[test]
    fn native_mixed_unknown_and_non_name_synonyms_do_not_promote() {
        for origin in [WordOrigin::Native, WordOrigin::Mixed, WordOrigin::Unknown] {
            let mut source = word(1, "가온", 10);
            source.origin = origin;
            source.senses[0]
                .synonyms
                .push(Some(SourceReference::Sense(SenseId(20))));
            assert!(
                compile(vec![source, word(2, "누리", 20)])
                    .authorized_forms
                    .is_empty()
            );
        }
        let mut source = word(1, "가온", 10);
        source.senses[0].foreign_name = false;
        source.senses[0]
            .synonyms
            .push(Some(SourceReference::Sense(SenseId(20))));
        assert!(
            compile(vec![source, word(2, "누리", 20)])
                .authorized_forms
                .is_empty()
        );
    }

    #[test]
    fn formal_redirect_preserves_excluded_phrase_and_needs_no_loanword() {
        let mut origin = word(1, "가온", 10);
        origin.origin = WordOrigin::Native;
        origin.senses[0].redirect = SourceRedirect::Target(SourceReference::Sense(SenseId(20)));
        let evidence = compile(vec![origin, word(2, "새 이름", 20)]);
        assert_eq!(evidence.authorized_forms[0].form.reading, "새 이름");
        assert_eq!(
            evidence.authorized_forms[0].kind,
            AuthorizationKind::Redirect
        );
    }

    #[test]
    fn synonyms_are_direct_outgoing_and_not_transitive_or_reversed() {
        let mut origin = word(1, "가온", 10);
        origin.senses[0]
            .synonyms
            .push(Some(SourceReference::Sense(SenseId(20))));
        let mut target = word(2, "누리", 20);
        target.senses[0].synonyms.extend([
            Some(SourceReference::Sense(SenseId(30))),
            Some(SourceReference::Sense(SenseId(10))),
        ]);
        let evidence = compile(vec![origin, target, word(3, "다솜", 30)]);
        assert_eq!(evidence.authorized_forms.len(), 1);
        assert_eq!(evidence.authorized_forms[0].form.reading, "누리");
        assert!(evidence.diagnostics.is_empty());
        let mut reverse = word(2, "누리", 20);
        reverse.senses[0]
            .synonyms
            .push(Some(SourceReference::Sense(SenseId(10))));
        assert!(
            compile(vec![word(1, "가온", 10), reverse])
                .authorized_forms
                .is_empty()
        );
    }

    #[test]
    fn missing_and_cyclic_edges_invalidate_all_other_valid_grants() {
        let mut origin = word(1, "가온", 10);
        origin.senses[0].synonyms.extend([
            Some(SourceReference::Sense(SenseId(20))),
            Some(SourceReference::Sense(SenseId(30))),
        ]);
        let missing = compile(vec![origin.clone(), word(2, "누리", 20)]);
        assert!(missing.authorized_forms.is_empty());
        assert_eq!(
            missing.diagnostics[0].kind,
            EvidenceDiagnosticKind::MissingTarget
        );
        let mut cycle = word(3, "다솜", 30);
        cycle.senses[0].redirect = SourceRedirect::Target(SourceReference::Sense(SenseId(30)));
        let evidence = compile(vec![origin, word(2, "누리", 20), cycle]);
        assert!(evidence.authorized_forms.is_empty());
        assert_eq!(evidence.diagnostics[0].kind, EvidenceDiagnosticKind::Cycle);
    }

    #[test]
    fn word_targets_require_every_terminal_outcome_to_agree() {
        let mut origin = word(1, "가온", 10);
        origin.senses[0].redirect = SourceRedirect::Target(SourceReference::Word(WordId(2)));
        let mut target = word(2, "누리", 20);
        let mut other = sense(21);
        other.redirect = SourceRedirect::Target(SourceReference::Sense(SenseId(30)));
        target.senses.push(other);
        let evidence = compile(vec![origin, target, word(3, "다솜", 30)]);
        assert!(evidence.authorized_forms.is_empty());
        assert_eq!(
            evidence.diagnostics[0].kind,
            EvidenceDiagnosticKind::AmbiguousTarget
        );
    }

    #[test]
    fn budgets_reject_whole_promotion_and_duplicate_ids_are_ambiguous() {
        let mut origin = word(1, "가온", 10);
        origin.senses[0].synonyms.extend([
            Some(SourceReference::Sense(SenseId(20))),
            Some(SourceReference::Sense(SenseId(30))),
        ]);
        let graph = SourceGraph::new(vec![
            origin.clone(),
            word(2, "누리", 20),
            word(3, "다솜", 30),
        ]);
        let evidence = graph.compile_candidate(
            form("가온"),
            [WordId(1)],
            EvidenceBudget {
                authorized_forms: 1,
                ..EvidenceBudget::default()
            },
        );
        assert!(evidence.authorized_forms.is_empty());
        assert_eq!(
            evidence.diagnostics[0].kind,
            EvidenceDiagnosticKind::BudgetExceeded
        );
        origin.senses[0].synonyms.pop();
        let duplicate = compile(vec![origin, word(2, "누리", 20), word(3, "다솜", 20)]);
        assert_eq!(
            duplicate.diagnostics[0].kind,
            EvidenceDiagnosticKind::AmbiguousTarget
        );
    }

    #[test]
    fn redirect_hop_budget_includes_the_initial_redirect() {
        let mut origin = word(1, "가온", 10);
        origin.senses[0].redirect = SourceRedirect::Target(SourceReference::Sense(SenseId(20)));
        let graph = SourceGraph::new(vec![origin, word(2, "누리", 20)]);
        let evidence = graph.compile_candidate(
            form("가온"),
            [WordId(1)],
            EvidenceBudget {
                redirect_hops: 0,
                ..EvidenceBudget::default()
            },
        );
        assert!(evidence.authorized_forms.is_empty());
        assert_eq!(
            evidence.diagnostics[0].kind,
            EvidenceDiagnosticKind::BudgetExceeded
        );
        let evidence = graph.compile_candidate(
            form("가온"),
            [WordId(1)],
            EvidenceBudget {
                redirect_hops: 1,
                ..EvidenceBudget::default()
            },
        );
        assert_eq!(evidence.authorized_forms[0].form.reading, "누리");
    }

    #[test]
    fn shard_order_does_not_change_compilation() {
        let mut origin = word(1, "가온", 10);
        origin.senses[0].synonyms.extend([
            Some(SourceReference::Sense(SenseId(30))),
            Some(SourceReference::Sense(SenseId(20))),
        ]);
        let mut words = vec![origin, word(2, "누리", 20), word(3, "다솜", 30)];
        let expected = compile(words.clone());
        words.reverse();
        assert_eq!(compile(words), expected);
    }

    #[test]
    fn missing_contributing_word_retains_valid_unknown_provenance() {
        let evidence = compile(Vec::new());
        assert_eq!(
            evidence.contributors,
            [SourceContribution {
                word_id: WordId(1),
                sense_id: None,
                origin: WordOrigin::Unknown,
            }]
        );
        assert!(evidence.authorized_forms.is_empty());
        assert_eq!(
            evidence.diagnostics[0].kind,
            EvidenceDiagnosticKind::MissingTarget
        );
        gukhanmun_core::DictionaryEvidence {
            schema_version: 1,
            role: gukhanmun_core::SourceRole::Reference,
            entries: BTreeMap::from([("宙名".into(), vec![evidence])]),
        }
        .validate()
        .unwrap();
    }

    #[test]
    fn self_synonym_is_not_a_redirect_cycle_or_a_reason_to_drop_other_grants() {
        let mut origin = word(1, "가온", 10);
        origin.senses[0].synonyms.extend([
            Some(SourceReference::Sense(SenseId(10))),
            Some(SourceReference::Sense(SenseId(20))),
        ]);
        let evidence = SourceGraph::new(vec![origin, word(2, "누리", 20)]).compile_candidate(
            form("가온"),
            [WordId(1)],
            EvidenceBudget {
                authorized_forms: 1,
                ..EvidenceBudget::default()
            },
        );
        assert!(evidence.diagnostics.is_empty());
        assert_eq!(evidence.authorized_forms.len(), 1);
        assert_eq!(evidence.authorized_forms[0].form.reading, "누리");
    }

    #[test]
    fn word_target_redirect_witness_names_the_actual_member_sense() {
        let mut origin = word(1, "가온", 10);
        origin.senses[0].redirect = SourceRedirect::Target(SourceReference::Word(WordId(2)));
        let mut target = word(2, "누리", 20);
        target.senses[0].redirect = SourceRedirect::Target(SourceReference::Sense(SenseId(30)));
        let evidence = compile(vec![origin.clone(), target.clone(), word(3, "다솜", 30)]);
        assert_eq!(evidence.authorized_forms[0].form.reading, "다솜");
        assert_eq!(
            evidence.authorized_forms[0].witnesses,
            [
                RelationWitness {
                    source: SourceReference::Sense(SenseId(10)),
                    target: SourceReference::Word(WordId(2))
                },
                RelationWitness {
                    source: SourceReference::Sense(SenseId(20)),
                    target: SourceReference::Sense(SenseId(30))
                },
            ]
        );
        let evidence = compile(vec![
            origin,
            target,
            word(3, "다솜", 30),
            word(4, "무관", 20),
        ]);
        assert!(evidence.authorized_forms.is_empty());
        assert_eq!(
            evidence.diagnostics[0].kind,
            EvidenceDiagnosticKind::AmbiguousTarget
        );
    }

    #[test]
    fn word_level_synonym_preserves_the_source_word_namespace() {
        let mut origin = word(1, "가온", 10);
        origin
            .synonyms
            .push(Some(SourceReference::Sense(SenseId(20))));
        let evidence = compile(vec![origin, word(2, "누리", 20)]);
        assert_eq!(
            evidence.authorized_forms[0].witnesses,
            [RelationWitness {
                source: SourceReference::Word(WordId(1)),
                target: SourceReference::Sense(SenseId(20)),
            }]
        );
    }

    #[test]
    fn only_unique_machine_redirect_targets_are_parsed() {
        assert_eq!(
            parse_sense_redirect(Some("→ <sense_no>42</sense_no>표제어.")),
            SourceRedirect::Target(SourceReference::Sense(SenseId(42)))
        );
        assert_eq!(
            parse_sense_redirect(Some("뜻풀이 <sense_no>42</sense_no>표제어")),
            SourceRedirect::None
        );
        assert_eq!(
            parse_sense_redirect(Some("→ <word_no>42</word_no>표제어.")),
            SourceRedirect::Target(SourceReference::Word(WordId(42)))
        );
        for value in [
            "→ 표제어",
            "→ <sp_no>42</sp_no>표제어",
            "→ <word_no>42</word_no>가 <sense_no>43</sense_no>나",
            "→ <sense_no>42</sense_no>가 <word_no>43</word_no>나",
            "→ <sense_no>42</sense_no>가 <sp_no>43</sp_no>나",
            "→ <word_no>42</sense_no>표제어",
            "→ <sense_no>word42</sense_no>표제어",
            "→ <sense_no>42</sense_no>가 <sense_no>43</sense_no>나",
            "→ <sense_no>18446744073709551616</sense_no>표제어",
        ] {
            assert_eq!(parse_sense_redirect(Some(value)), SourceRedirect::Invalid);
        }
    }
}
