// Gukhanmun: Regression coverage for nested source eligibility and phrase ownership.
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

use gukhanmun_core::*;
use std::collections::BTreeMap;

type Sources = ChainDictionary<Box<dyn HanjaDictionary>>;

fn records(reading: &str) -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("國林", reading);
    dictionary
}

fn reference(authorized: bool) -> MapDictionary {
    let mut dictionary = records("국림");
    if authorized {
        dictionary
            .set_source_evidence(DictionaryEvidence {
                schema_version: 1,
                role: SourceRole::Reference,
                entries: BTreeMap::from([(
                    "國林".into(),
                    vec![CandidateEvidence {
                        reading: "국림".into(),
                        suffix_reading: None,
                        contributors: vec![SourceContribution {
                            word_id: WordId(10),
                            sense_id: Some(SenseId(11)),
                            origin: WordOrigin::Foreign,
                        }],
                        authorized_forms: vec![AuthorizedForm {
                            form: ReadingForm {
                                reading: "새이름".into(),
                                suffix_reading: None,
                            },
                            kind: AuthorizationKind::Synonym,
                            witnesses: vec![RelationWitness {
                                source: SourceReference::Sense(SenseId(11)),
                                target: SourceReference::Sense(SenseId(12)),
                            }],
                        }],
                        diagnostics: vec![],
                    }],
                )]),
            })
            .unwrap();
    }
    dictionary
        .set_phrase_evidence(PhraseEvidence::new(vec![PhraseRule::new(
            vec![WordId(20)],
            vec![
                PhraseChunk::new(
                    "무조건".into(),
                    vec![PhraseSpelling::new("無條件".into(), vec![])],
                ),
                PhraseChunk::new(
                    "국림".into(),
                    vec![PhraseSpelling::new(
                        "國林".into(),
                        vec![PhraseCandidateLink::new("국림".into(), None)],
                    )],
                ),
            ],
        )]))
        .unwrap();
    dictionary
}
fn inner(policy: SourcePolicy, reference_only: bool, authorized: bool) -> Sources {
    let mut inner = Sources::new().with_source_policy(policy);
    inner.set_reference_only(reference_only);
    inner.push_with_role(Box::new(reference(authorized)), SourceRole::Reference);
    let mut loan = MapDictionary::new();
    loan.insert_candidates(
        "國林",
        ReadingCandidates::new(vec![
            ReadingCandidate {
                reading: "새이름".into(),
                suffix_reading: None,
                mark: MatchMark::default(),
                preference: 1,
                alias: None,
            },
            ReadingCandidate {
                reading: "무관".into(),
                suffix_reading: None,
                mark: MatchMark::default(),
                preference: 0,
                alias: None,
            },
        ])
        .unwrap(),
    );
    inner.push_with_role(Box::new(loan), SourceRole::Loanword);
    inner
}
fn outer(inner: Sources, policy: SourcePolicy) -> Sources {
    let mut outer = Sources::new().with_source_policy(policy);
    outer.push_with_role(Box::new(records("외부")), SourceRole::Reference);
    outer.push_with_role(Box::new(inner), SourceRole::Reference);
    outer
}
fn matches(dictionary: &Sources) -> Vec<SourceMatch> {
    let exact = dictionary
        .source_matches_at("國林")
        .unwrap()
        .find(|m| m.byte_len == "國林".len())
        .unwrap();
    let variant = dictionary
        .source_matches_at("国林")
        .unwrap()
        .find(|m| m.byte_len == "国林".len())
        .unwrap();
    let spelling = dictionary
        .source_matches_at_spellings(&["国林", "國林"])
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .1;
    vec![exact, variant, spelling]
}
#[test]
fn nested_reference_only_does_not_resurrect_a_supplier_as_independent_loanword() {
    let inner = inner(SourcePolicy::ReferenceAuthorized, true, true);
    let before = inner.source_matches_at("國林").unwrap().next().unwrap();
    assert_eq!(before.ranked.len(), 2);
    let outer = outer(inner, SourcePolicy::ReferenceAuthorized);
    assert_eq!(outer.owned_phrase_evidence()[0].owner, SourceId(1));
    for matched in matches(&outer) {
        assert_eq!(matched.ranked.len(), 3, "{:?}", matched.ranked);
        assert!(
            !matched
                .ranked
                .iter()
                .any(|c| c.reason == ChoiceReason::IndependentLoanword)
        );
        assert!(!matched.ranked.iter().any(|c| c.form.reading == "무관"));
        let authorized = matched
            .ranked
            .iter()
            .find(|c| c.reason == ChoiceReason::AuthorizedLoanword)
            .unwrap();
        assert_eq!(authorized.anchor.source_id, SourceId(1));
        assert_eq!(authorized.form_source, Some(SourceId(2)));
        assert_eq!(authorized.form.reading, "새이름");
    }
}
#[test]
fn nested_reference_first_does_not_gain_outer_authorized_promotions() {
    let outer = outer(
        inner(SourcePolicy::ReferenceFirst, false, true),
        SourcePolicy::ReferenceAuthorized,
    );
    for matched in matches(&outer) {
        assert_eq!(matched.ranked.len(), 4, "{:?}", matched.ranked);
        assert!(
            !matched
                .ranked
                .iter()
                .any(|c| c.reason == ChoiceReason::AuthorizedLoanword)
        );
        let loans = matched
            .ranked
            .iter()
            .filter(|c| c.reason == ChoiceReason::IndependentLoanword)
            .collect::<Vec<_>>();
        assert_eq!(loans.len(), 2);
        assert!(
            loans
                .iter()
                .all(|c| c.anchor.source_id == SourceId(2) && c.form_source == Some(SourceId(2)))
        );
    }
}
#[test]
fn nested_source_diagnostics_use_the_same_leaf_ids_as_phrase_ownership() {
    let outer = outer(
        inner(SourcePolicy::ReferenceAuthorized, false, false),
        SourcePolicy::ReferenceAuthorized,
    );
    for matched in matches(&outer) {
        assert!(matched.diagnostics.iter().any(|d| matches!(d,SelectionDiagnostic::UnknownEvidence{source_id:SourceId(1),reading} if reading=="국림")));
        assert_eq!(outer.owned_phrase_evidence()[0].owner, SourceId(1));
        assert!(
            matched
                .groups
                .iter()
                .any(|g| g.source_id == SourceId(1) && g.key == "國林")
        );
    }
}

#[test]
fn nested_override_preserves_its_complete_effective_default() {
    let inner = inner(SourcePolicy::ReferenceAuthorized, true, true);
    assert_eq!(
        inner
            .source_matches_at("國林")
            .unwrap()
            .next()
            .unwrap()
            .default_choice()
            .form
            .reading,
        "새이름"
    );
    let mut outer = Sources::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    outer.push_with_role(Box::new(records("외부")), SourceRole::Reference);
    outer.push(Box::new(inner));
    for matched in matches(&outer) {
        assert_eq!(matched.default_choice().form.reading, "새이름");
        assert_eq!(matched.default_choice().anchor.source_id, SourceId(1));
        assert_eq!(matched.default_choice().form_source, Some(SourceId(1)));
        assert!(matched.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            SelectionDiagnostic::OpaqueLegacyProjection {
                source_id: SourceId(1),
                source_count: 2
            }
        )));
        assert!(
            matched
                .ranked
                .iter()
                .any(|choice| choice.anchor.source_id == SourceId(1)
                    && choice.form_source == Some(SourceId(2))
                    && choice.form.reading == "새이름"
                    && choice.reason == ChoiceReason::Override)
        );
        assert!(
            matched
                .ranked
                .iter()
                .all(|choice| choice.reason == ChoiceReason::Override)
        );
        assert!(
            !matched
                .ranked
                .iter()
                .any(|choice| choice.anchor.source_id == SourceId(0))
        );
    }
}

#[test]
fn nested_spelling_pools_keep_queried_lengths_when_anchor_utf8_width_differs() {
    let mut leaf = MapDictionary::new();
    leaf.insert("丽", "리");
    leaf.set_phrase_evidence(PhraseEvidence::new(vec![PhraseRule::new(
        vec![WordId(30)],
        vec![
            PhraseChunk::new(
                "무조건".into(),
                vec![PhraseSpelling::new("無條件".into(), vec![])],
            ),
            PhraseChunk::new(
                "리".into(),
                vec![PhraseSpelling::new(
                    "丽".into(),
                    vec![PhraseCandidateLink::new("리".into(), None)],
                )],
            ),
        ],
    )]))
    .unwrap();
    let mut inner = Sources::new().with_source_policy(SourcePolicy::ReferenceFirst);
    inner.push_with_role(Box::new(leaf), SourceRole::Reference);
    let mut outer = Sources::new().with_source_policy(SourcePolicy::ReferenceFirst);
    outer.push_with_role(Box::new(MapDictionary::new()), SourceRole::Reference);
    outer.push_with_role(Box::new(inner), SourceRole::Reference);
    let pools = outer.source_matches_at_spellings(&["丽", "丽"]).unwrap();
    assert_eq!(pools.len(), 1);
    assert_eq!(pools[0].0, 1);
    assert_eq!(pools[0].1.byte_len, "丽".len());
    assert_eq!(pools[0].1.default_choice().anchor.key, "丽");
    assert_eq!(pools[0].1.default_choice().anchor.source_id, SourceId(1));
    let projected = outer.candidate_matches_at_spellings(&["丽", "丽"]);
    assert_eq!(projected[0].1.byte_len, "丽".len());
}

#[test]
fn explicitly_registered_nested_loanword_is_excluded_by_reference_only() {
    let mut nested = Sources::new().with_source_policy(SourcePolicy::ReferenceFirst);
    nested.push_with_role(Box::new(reference(false)), SourceRole::Reference);
    let mut outer = Sources::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    outer.set_reference_only(true);
    outer.push_with_role(Box::new(nested), SourceRole::Loanword);
    assert!(outer.owned_phrase_evidence().is_empty());
    assert!(outer.source_matches_at("國林").unwrap().next().is_none());
    assert!(outer.source_matches_at("国林").unwrap().next().is_none());
    assert!(
        outer
            .source_matches_at_spellings(&["国林", "國林"])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn nested_loanword_supplies_only_its_effective_form_to_outer_reference_authorization() {
    let mut nested = Sources::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    nested.set_reference_only(true);
    nested.push_with_role(Box::new(reference(true)), SourceRole::Reference);
    nested.push_with_role(Box::new(records("새이름")), SourceRole::Loanword);
    assert_eq!(
        nested
            .source_matches_at("國林")
            .unwrap()
            .next()
            .unwrap()
            .default_choice()
            .form
            .reading,
        "새이름"
    );
    let mut outer = Sources::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    outer.set_reference_only(true);
    outer.push_with_role(Box::new(reference(true)), SourceRole::Reference);
    outer.push_with_role(Box::new(nested), SourceRole::Loanword);
    assert_eq!(outer.source_count(), 3);
    assert_eq!(outer.owned_phrase_evidence().len(), 1);
    assert_eq!(outer.owned_phrase_evidence()[0].owner, SourceId(0));
    for matched in matches(&outer) {
        let promoted = matched
            .ranked
            .iter()
            .find(|choice| {
                choice.reason == ChoiceReason::AuthorizedLoanword
                    && choice.anchor.source_id == SourceId(0)
            })
            .expect("outer reference must receive its authorized nested supplier form");
        assert_eq!(promoted.form.reading, "새이름");
        assert!(
            matched
                .ranked
                .iter()
                .all(|choice| choice.anchor.source_id == SourceId(0))
        );
        assert_eq!(
            matched.ranked.len(),
            2,
            "only authorized supplier and original outer reference remain"
        );
    }
}

#[test]
fn opaque_nested_loanword_reserves_slots_without_hiding_direct_leaf_evidence() {
    let nested = inner(SourcePolicy::ReferenceAuthorized, true, true);
    let mut outer = Sources::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    outer.push_with_role(Box::new(nested), SourceRole::Loanword);
    outer.push_with_role(Box::new(reference(false)), SourceRole::Loanword);
    let owned = outer.owned_phrase_evidence();
    assert_eq!(outer.source_count(), 3);
    assert_eq!(owned.len(), 1);
    assert_eq!(owned[0].owner, SourceId(2));
    for matched in matches(&outer) {
        assert!(
            matched
                .ranked
                .iter()
                .any(|choice| choice.reason == ChoiceReason::IndependentLoanword
                    && choice.anchor.source_id == SourceId(2))
        );
        assert!(
            matched
                .ranked
                .iter()
                .all(|choice| choice.reason == ChoiceReason::IndependentLoanword)
        );
    }
}
