// Gukhanmun: End-to-end regressions for cooperative source selection and identity.
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

type Sources = ChainDictionary<MapDictionary>;
fn records(items: &[(&str, &str)]) -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    for (key, reading) in items {
        dictionary.insert(*key, *reading);
    }
    dictionary
}
fn chain(
    reference: MapDictionary,
    loan: MapDictionary,
    policy: SourcePolicy,
    reverse: bool,
) -> Sources {
    let mut chain = Sources::new().with_source_policy(policy);
    if reverse {
        chain.push_with_role(loan, SourceRole::Loanword);
        chain.push_with_role(reference, SourceRole::Reference);
    } else {
        chain.push_with_role(reference, SourceRole::Reference);
        chain.push_with_role(loan, SourceRole::Loanword);
    }
    chain
}
fn grant(
    dictionary: &mut MapDictionary,
    key: &str,
    kind: AuthorizationKind,
    forms: &[ReadingForm],
    origin: WordOrigin,
) {
    let matched = dictionary
        .candidate_matches_at(key)
        .find(|m| m.byte_len == key.len())
        .unwrap();
    let entries = matched
        .candidates
        .as_slice()
        .iter()
        .map(|candidate| CandidateEvidence {
            reading: candidate.reading.clone(),
            suffix_reading: candidate.suffix_reading.clone(),
            contributors: vec![
                SourceContribution {
                    word_id: WordId(101),
                    sense_id: Some(SenseId(101)),
                    origin,
                },
                SourceContribution {
                    word_id: WordId(102),
                    sense_id: Some(SenseId(102)),
                    origin,
                },
            ],
            authorized_forms: forms
                .iter()
                .map(|form| AuthorizedForm {
                    form: form.clone(),
                    kind,
                    witnesses: vec![
                        RelationWitness {
                            source: SourceReference::Sense(SenseId(101)),
                            target: SourceReference::Sense(SenseId(201)),
                        },
                        RelationWitness {
                            source: SourceReference::Sense(SenseId(102)),
                            target: SourceReference::Word(WordId(201)),
                        },
                    ],
                })
                .collect(),
            diagnostics: vec![],
        })
        .collect();
    dictionary
        .set_source_evidence(DictionaryEvidence {
            schema_version: 1,
            role: SourceRole::Reference,
            entries: BTreeMap::from([(key.into(), entries)]),
        })
        .unwrap();
}
fn form(reading: &str) -> ReadingForm {
    ReadingForm {
        reading: reading.into(),
        suffix_reading: None,
    }
}
fn annotations(
    dictionary: &impl HanjaDictionary,
    input: &str,
    strategy: SegmentationStrategy,
) -> Vec<Annotation> {
    let mut engine = Engine::<PlainScopeData, _>::with_options(
        dictionary,
        EngineOptions {
            segmentation: strategy,
            ..Default::default()
        },
    );
    let mut output = engine.push_token(InputToken::Text(input.into()));
    output.extend(engine.finish());
    output
        .into_iter()
        .filter_map(|token| {
            if let OutputToken::Annotated(annotation) = token {
                Some(annotation)
            } else {
                None
            }
        })
        .collect()
}
fn text(dictionary: &impl HanjaDictionary, input: &str) -> String {
    convert_plain_text(input, dictionary, RenderMode::HangulOnly)
}
fn pair(policy: SourcePolicy) -> Sources {
    chain(
        records(&[("霧川", "무천")]),
        records(&[("霧川", "모가")]),
        policy,
        false,
    )
}

#[test]
fn policies_and_registration_order_are_explicit() {
    for reverse in [false, true] {
        let make = |policy| {
            chain(
                records(&[("霧川", "무천")]),
                records(&[("霧川", "모가")]),
                policy,
                reverse,
            )
        };
        assert_eq!(
            text(&make(SourcePolicy::ReferenceAuthorized), "霧川"),
            "무천"
        );
        assert_eq!(text(&make(SourcePolicy::ReferenceFirst), "霧川"), "무천");
        assert_eq!(text(&make(SourcePolicy::LoanwordFirst), "霧川"), "모가");
        assert_eq!(
            text(&make(SourcePolicy::Ordered), "霧川"),
            if reverse { "모가" } else { "무천" }
        );
    }
}

#[test]
fn default_chain_is_ordered_and_ordinary_push_is_an_override() {
    let mut sources = Sources::new();
    assert_eq!(sources.source_policy(), SourcePolicy::Ordered);
    let mut authored = records(&[("霧川", "우두")]);
    grant(
        &mut authored,
        "霧川",
        AuthorizationKind::Redirect,
        &[form("모가")],
        WordOrigin::Foreign,
    );
    sources.push(authored);
    sources.push_with_role(records(&[("霧川", "무천")]), SourceRole::Reference);
    sources.set_source_policy(SourcePolicy::ReferenceAuthorized);
    assert_eq!(
        sources.source_roles(),
        [SourceRole::Override, SourceRole::Reference]
    );
    assert_eq!(text(&sources, "霧川을"), "우두을");
}

#[test]
fn authorized_supplier_preserves_reference_identity_and_marks() {
    let mut reference = MapDictionary::new();
    reference.insert_marked(
        "霧川",
        "무천",
        MatchMark {
            require_hanja: true,
            require_hangul: false,
        },
    );
    grant(
        &mut reference,
        "霧川",
        AuthorizationKind::Synonym,
        &[form("모가")],
        WordOrigin::Foreign,
    );
    let mut loan = MapDictionary::new();
    loan.insert_marked(
        "霧川",
        "모가",
        MatchMark {
            require_hanja: false,
            require_hangul: true,
        },
    );
    let sources = chain(reference, loan, SourcePolicy::ReferenceAuthorized, false);
    let annotation = &annotations(&sources, "霧川", SegmentationStrategy::Lattice)[0];
    let selected = annotation.selection.as_ref().unwrap();
    assert_eq!(annotation.reading, "모가");
    assert_eq!(selected.choice.anchor.reading, "무천");
    assert_eq!(selected.choice.reason, ChoiceReason::AuthorizedLoanword);
    assert_eq!(selected.choice.anchor.source_id, SourceId(0));
    assert_eq!(selected.choice.form_source, Some(SourceId(1)));
    assert!(annotation.require_hanja);
    assert!(!annotation.require_hangul);
    let raw = sources.source_matches_at("霧川").unwrap().next().unwrap();
    assert_eq!(raw.groups.len(), 2);
    assert_eq!(raw.groups[0].evidence[0].contributors.len(), 2);
    assert_eq!(raw.groups[0].candidates.as_slice()[0].reading, "무천");
}

#[test]
fn synonym_needs_an_attested_supplier_but_redirect_does_not() {
    for kind in [AuthorizationKind::Synonym, AuthorizationKind::Redirect] {
        let mut reference = records(&[("霧川", "무천")]);
        grant(
            &mut reference,
            "霧川",
            kind,
            &[form("누벨 리브")],
            WordOrigin::Foreign,
        );
        let sources = chain(
            reference,
            MapDictionary::new(),
            SourcePolicy::ReferenceAuthorized,
            false,
        );
        assert_eq!(
            text(&sources, "霧川"),
            if kind == AuthorizationKind::Redirect {
                "누벨 리브"
            } else {
                "무천"
            }
        );
        let raw = sources.source_matches_at("霧川").unwrap().next().unwrap();
        assert!(
            raw.ranked
                .iter()
                .any(|choice| choice.form.reading == "무천")
        );
    }
}

#[test]
fn unknown_native_and_mixed_evidence_cannot_invent_a_grant() {
    for origin in [WordOrigin::Native, WordOrigin::Mixed, WordOrigin::Unknown] {
        let mut reference = records(&[("霧川", "무천")]);
        grant(
            &mut reference,
            "霧川",
            AuthorizationKind::Synonym,
            &[],
            origin,
        );
        let sources = chain(
            reference,
            records(&[("霧川", "모가")]),
            SourcePolicy::ReferenceAuthorized,
            false,
        );
        assert_eq!(text(&sources, "霧川"), "무천");
    }
    let raw = pair(SourcePolicy::ReferenceAuthorized)
        .source_matches_at("霧川")
        .unwrap()
        .next()
        .unwrap();
    assert!(matches!(
        raw.diagnostics[0],
        SelectionDiagnostic::UnknownEvidence { .. }
    ));
}

#[test]
fn particle_selection_uses_the_whole_open_pool_once() {
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        let sources = pair(SourcePolicy::ReferenceAuthorized);
        for (input, expected, reason) in [
            ("霧川을", "무천", ChoiceReason::Reference),
            ("霧川를", "모가", ChoiceReason::IndependentLoanword),
            ("霧川を", "무천", ChoiceReason::Reference),
            ("霧川를가", "무천", ChoiceReason::Reference),
        ] {
            let anns = annotations(&sources, input, strategy);
            assert_eq!(anns[0].reading, expected, "{input}");
            assert_eq!(anns[0].selection.as_ref().unwrap().choice.reason, reason);
        }
        let mut closed = pair(SourcePolicy::ReferenceAuthorized);
        closed.set_reference_only(true);
        assert!(closed.reference_only());
        assert_eq!(annotations(&closed, "霧川를", strategy)[0].reading, "무천");
    }
}

#[test]
fn reference_only_keeps_authorized_realizations_but_removes_independent_tail() {
    let mut reference = records(&[("霧川", "무천")]);
    grant(
        &mut reference,
        "霧川",
        AuthorizationKind::Synonym,
        &[form("모가")],
        WordOrigin::Foreign,
    );
    let mut sources = chain(
        reference,
        records(&[("霧川", "모가")]),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    sources.set_reference_only(true);
    let pool = sources.source_matches_at("霧川").unwrap().next().unwrap();
    assert_eq!(pool.default_choice().form.reading, "모가");
    assert!(
        pool.ranked
            .iter()
            .all(|choice| choice.reason != ChoiceReason::IndependentLoanword)
    );
}

#[test]
fn every_character_split_preserves_open_pool_particle_choices() {
    for policy in [
        SourcePolicy::ReferenceAuthorized,
        SourcePolicy::ReferenceFirst,
        SourcePolicy::LoanwordFirst,
        SourcePolicy::Ordered,
    ] {
        let sources = pair(policy);
        for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
            for input in ["霧川를 霧川을.", "霧川으로는 霧川로만.", "霧川を 霧川를가."]
            {
                let mut baseline = Engine::<PlainScopeData, _>::with_options(
                    &sources,
                    EngineOptions {
                        segmentation: strategy,
                        ..Default::default()
                    },
                );
                let mut expected = baseline.push_token(InputToken::Text(input.into()));
                expected.extend(baseline.finish());
                for split in input.char_indices().map(|(i, _)| i).chain([input.len()]) {
                    let mut engine = Engine::<PlainScopeData, _>::with_options(
                        &sources,
                        EngineOptions {
                            segmentation: strategy,
                            ..Default::default()
                        },
                    );
                    let mut actual = engine.push_token(InputToken::Text(input[..split].into()));
                    actual.extend(engine.push_token(InputToken::Text(input[split..].into())));
                    actual.extend(engine.finish());
                    assert_eq!(actual, expected, "{policy:?}/{strategy:?}/{input}/{split}");
                }
            }
        }
    }
}

#[test]
fn short_override_does_not_hide_a_longer_dictionary_edge() {
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        let mut sources = Sources::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
        sources.push(records(&[("霧川", "우두")]));
        sources.push_with_role(
            records(&[("霧川", "무천"), ("霧川郡", "무천군")]),
            SourceRole::Reference,
        );
        assert_eq!(
            annotations(&sources, "霧川郡", strategy)[0].reading,
            "무천군"
        );
    }
}

#[test]
fn override_glyph_alternative_precedes_later_reference_exact() {
    let mut sources = Sources::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    sources.push(records(&[("國", "우두")]));
    sources.push_with_role(records(&[("国", "국")]), SourceRole::Reference);
    let anns = annotations(&sources, "国", SegmentationStrategy::Lattice);
    assert_eq!(anns[0].reading, "우두");
    assert_eq!(anns[0].dictionary_hanja.as_deref(), Some("國"));
    assert_eq!(
        anns[0].selection.as_ref().unwrap().choice.reason,
        ChoiceReason::Override
    );
}

#[test]
fn supplier_positional_pair_is_atomic_and_never_spliced() {
    let mut reference = MapDictionary::new();
    reference.insert_with_suffix("霧川", "무천", "무쳔");
    grant(
        &mut reference,
        "霧川",
        AuthorizationKind::Synonym,
        &[form("모가")],
        WordOrigin::Foreign,
    );
    let mut loan = MapDictionary::new();
    loan.insert_with_suffix("霧川", "모가", "마가");
    let sources = chain(reference, loan, SourcePolicy::ReferenceAuthorized, false);
    let pool = sources.source_matches_at("霧川").unwrap().next().unwrap();
    assert_eq!(pool.default_choice().form.reading, "무천");
    assert_eq!(
        pool.ranked.last().unwrap().form,
        ReadingForm {
            reading: "모가".into(),
            suffix_reading: Some("마가".into())
        }
    );
}

#[test]
fn effective_default_enumeration_matches_exact_lookup() {
    let sources = pair(SourcePolicy::ReferenceAuthorized);
    let entry = sources.candidate_entries().unwrap().next().unwrap();
    let matched = sources.candidate_matches_at(&entry.hanja).next().unwrap();
    assert_eq!(entry.candidates, matched.candidates);
    assert_eq!(entry.candidates.as_slice().len(), 1);
    let shared = std::sync::Arc::new(sources);
    let reference = &shared;
    assert!(reference.source_matches_at("霧川").is_some());
    assert!(reference.source_entries().is_some());
}

#[test]
fn first_occurrence_distinguishes_anchor_reading_but_not_source_id() {
    let mut reference = MapDictionary::new();
    reference.insert_candidates(
        "霧川",
        ReadingCandidates::new(vec![
            ReadingCandidate {
                reading: "무천".into(),
                preference: 1,
                ..Default::default()
            },
            ReadingCandidate {
                reading: "모가".into(),
                ..Default::default()
            },
        ])
        .unwrap(),
    );
    let sources = chain(
        reference,
        MapDictionary::new(),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    let anns = annotations(
        &sources,
        "霧川을 霧川를 霧川을",
        SegmentationStrategy::Lattice,
    );
    let output = filter_first_occurrences(
        anns.into_iter()
            .map(OutputToken::<PlainScopeData>::Annotated),
        ContextWindow::PerDocument,
    );
    let flags: Vec<_> = output
        .into_iter()
        .filter_map(|token| {
            if let OutputToken::Annotated(a) = token {
                Some(a.first_in_context)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(flags, [true, true, false]);
    let mut anns = annotations(&sources, "霧川 霧川", SegmentationStrategy::Lattice);
    anns[1].selection.as_mut().unwrap().choice.anchor.source_id = SourceId(1000);
    let output = filter_first_occurrences(
        anns.into_iter()
            .map(OutputToken::<PlainScopeData>::Annotated),
        ContextWindow::PerDocument,
    );
    assert!(matches!(&output[1],OutputToken::Annotated(a) if !a.first_in_context));
}

#[test]
fn actual_nondefault_loan_identity_drives_both_homophone_modes() {
    let sources = chain(
        records(&[("霧川", "무천"), ("晴城", "모가")]),
        records(&[("霧川", "모가")]),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    let anns = annotations(&sources, "霧川를 晴城", SegmentationStrategy::Lattice);
    for detection in [
        HomophoneDetection::ContextLocal,
        HomophoneDetection::DictionaryWide,
    ] {
        let output = mark_homophones_with_detection(
            anns.clone()
                .into_iter()
                .map(OutputToken::<PlainScopeData>::Annotated),
            &sources,
            ContextWindow::PerDocument,
            detection,
        );
        assert!(
            output
                .iter()
                .all(|token| matches!(token,OutputToken::Annotated(a) if a.homophone))
        );
    }
}

#[test]
fn source_aware_trivial_edges_keep_identity_and_preserve_hangul_bytes() {
    let reference = records(&[("年", "연"), ("金", "금")]);
    let legacy = chain(
        reference.clone(),
        MapDictionary::new(),
        SourcePolicy::Ordered,
        false,
    );
    let sources = chain(
        reference,
        MapDictionary::new(),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    for input in ["年金", "三年金", "甲年金", "年金 年金"] {
        assert_eq!(text(&sources, input), text(&legacy, input), "{input}");
        assert!(
            annotations(&sources, input, SegmentationStrategy::Lattice)
                .iter()
                .filter(|ann| ann.from_dictionary)
                .all(|ann| ann.selection.is_some())
        );
    }
    let anns = annotations(&sources, "年金", SegmentationStrategy::Lattice);
    assert_eq!(anns.len(), 2);
}

fn reading_alias_sources() -> Sources {
    let mut reference = MapDictionary::new();
    reference.insert("降伏", "항복");
    reference.insert_candidates(
        "降服",
        ReadingCandidates::new(vec![
            ReadingCandidate {
                reading: "항복".into(),
                preference: 1,
                alias: Some(AliasTarget {
                    canonical: "降伏".into(),
                    kind: AliasKind::Sibling,
                }),
                ..Default::default()
            },
            ReadingCandidate {
                reading: "강복".into(),
                ..Default::default()
            },
        ])
        .unwrap(),
    );
    chain(
        reference,
        MapDictionary::new(),
        SourcePolicy::ReferenceAuthorized,
        false,
    )
}

#[test]
fn parentheses_revalidate_both_orders_and_remove_stale_alias_identity() {
    let sources = reading_alias_sources();
    for input in ["降服(강복)", "강복(降服)"] {
        let mut engine = Engine::<PlainScopeData, _>::new(&sources);
        let mut output = engine.push_token(InputToken::Text(input.into()));
        output.extend(engine.finish());
        let output = collapse_redundant_parens(output, true);
        let annotation = output
            .iter()
            .find_map(|token| {
                if let OutputToken::Annotated(a) = token {
                    Some(a)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(annotation.reading, "강복");
        assert!(
            annotation
                .selection
                .as_ref()
                .unwrap()
                .choice
                .anchor
                .alias
                .is_none()
        );
        assert_eq!(
            annotation
                .selection
                .as_ref()
                .unwrap()
                .choice
                .anchor
                .lexical_canonical,
            "降服"
        );
    }
}

#[test]
fn authorial_unattested_reading_clears_alias_but_retains_directive_constraints() {
    let mut reference = MapDictionary::new();
    reference.insert("降伏", "항복");
    reference
        .insert_alias("降服", "항복", "降伏", AliasKind::Sibling)
        .unwrap();
    let sources = chain(
        reference,
        MapDictionary::new(),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    let annotation = annotations(&sources, "降服", SegmentationStrategy::Lattice).remove(0);
    let mut directives = UserDirectives::new();
    directives.add_literal("降伏", DirectiveAction::RequireHanja);
    directives.add_literal("降服", DirectiveAction::SkipAnnotation);
    let annotation = directives.apply(OutputToken::<PlainScopeData>::Annotated(annotation));
    let output = collapse_redundant_parens([annotation, OutputToken::Text("(강복)".into())], true);
    let annotation = output
        .iter()
        .find_map(|token| {
            if let OutputToken::Annotated(a) = token {
                Some(a)
            } else {
                None
            }
        })
        .unwrap();
    let selected = annotation.selection.as_ref().unwrap();
    assert_eq!(selected.choice.reason, ChoiceReason::Authorial);
    assert!(selected.choice.anchor.alias.is_none());
    assert_eq!(selected.choice.anchor.reading, "강복");
    assert!(selected.directive_mark.require_hanja);
    assert!(annotation.skip_annotation);
}

#[test]
fn synonym_and_redirect_promotions_cannot_reuse_an_incompatible_alias() {
    let mut reference = MapDictionary::new();
    reference.insert("降伏", "항복");
    reference
        .insert_alias("降服", "항복", "降伏", AliasKind::Sibling)
        .unwrap();
    grant(
        &mut reference,
        "降服",
        AuthorizationKind::Synonym,
        &[form("강복")],
        WordOrigin::Foreign,
    );
    let sources = chain(
        reference,
        records(&[("降服", "강복")]),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    let pool = sources.source_matches_at("降服").unwrap().next().unwrap();
    assert_eq!(pool.default_choice().form.reading, "항복");
    assert!(
        pool.diagnostics
            .iter()
            .any(|d| matches!(d, SelectionDiagnostic::ConflictingAlias { .. }))
    );
}

#[test]
fn replication_and_preference_scale_never_create_cross_source_votes() {
    for offset in [0, 1000, 1_000_000] {
        let mut reference = MapDictionary::new();
        reference.insert_candidates(
            "霧川",
            ReadingCandidates::new(vec![
                ReadingCandidate {
                    reading: "무천".into(),
                    preference: offset + 10,
                    ..Default::default()
                },
                ReadingCandidate {
                    reading: "무진".into(),
                    preference: offset,
                    ..Default::default()
                },
            ])
            .unwrap(),
        );
        let mut sources = chain(
            reference,
            records(&[("霧川", "모가")]),
            SourcePolicy::ReferenceAuthorized,
            true,
        );
        for _ in 0..5 {
            sources.push_with_role(records(&[("霧川", "모가")]), SourceRole::Loanword);
        }
        assert_eq!(text(&sources, "霧川"), "무천");
        assert_eq!(
            annotations(&sources, "霧川를", SegmentationStrategy::Lattice)[0].reading,
            "모가"
        );
    }
}

#[test]
fn no_particle_compatible_choice_retains_the_original_default() {
    let sources = chain(
        records(&[("霧川", "무천")]),
        records(&[("霧川", "모강")]),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    assert_eq!(
        annotations(&sources, "霧川를", SegmentationStrategy::Lattice)[0].reading,
        "무천"
    );
}

#[test]
fn dangling_alias_is_owned_by_the_selected_source_not_lower_identity() {
    let mut reference = MapDictionary::new();
    reference
        .insert_alias("夜浦", "야포", "暗浦", AliasKind::Sibling)
        .unwrap();
    let mut loan = MapDictionary::new();
    loan.insert_alias("夜浦", "야포", "晨浦", AliasKind::Sibling)
        .unwrap();
    let sources = chain(reference, loan, SourcePolicy::ReferenceAuthorized, false);
    let annotation = annotations(&sources, "夜浦", SegmentationStrategy::Lattice).remove(0);
    assert_eq!(
        annotation
            .selection
            .as_ref()
            .unwrap()
            .choice
            .anchor
            .lexical_canonical,
        "暗浦"
    );
    let mut directives = UserDirectives::new();
    directives.add_literal("晨浦", DirectiveAction::SkipAnnotation);
    let output = directives.apply(OutputToken::<PlainScopeData>::Annotated(annotation));
    assert!(matches!(output,OutputToken::Annotated(a) if !a.skip_annotation));
}

#[test]
fn conflicting_redirects_withhold_all_promoted_forms() {
    let mut reference = records(&[("霧川", "무천")]);
    grant(
        &mut reference,
        "霧川",
        AuthorizationKind::Redirect,
        &[form("모가"), form("라리")],
        WordOrigin::Foreign,
    );
    let sources = chain(
        reference,
        records(&[("霧川", "모가")]),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    let raw = sources.source_matches_at("霧川").unwrap().next().unwrap();
    assert_eq!(raw.default_choice().form.reading, "무천");
    assert!(
        raw.ranked
            .iter()
            .all(|choice| choice.reason != ChoiceReason::AuthorizedLoanword
                && choice.reason != ChoiceReason::Redirect)
    );
    assert!(
        raw.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            SelectionDiagnostic::ConflictingRedirect { .. }
        ))
    );
}

#[test]
fn lookup_only_homophone_fallback_uses_selected_canonical_without_recursion() {
    struct LookupOnly(MapDictionary);
    impl HanjaDictionary for LookupOnly {
        fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            self.0.matches_at(s)
        }
        fn has_homophone_canonical(&self, hanja: &str, reading: &str) -> bool {
            self.0.has_homophone_canonical(hanja, reading)
        }
    }
    let mut sources = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    sources.push_with_role(
        LookupOnly(records(&[("霧川", "무천"), ("晴城", "무천")])),
        SourceRole::Reference,
    );
    let annotations = annotations(&sources, "霧川", SegmentationStrategy::Lattice);
    let output = mark_homophones_with_detection(
        annotations
            .into_iter()
            .map(OutputToken::<PlainScopeData>::Annotated),
        &sources,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );
    assert!(matches!(&output[0],OutputToken::Annotated(a) if a.homophone));
}

#[test]
fn phrase_forms_collapse_in_both_parenthetical_orders_and_at_every_split() {
    let mut reference = records(&[("霧川", "무천")]);
    grant(
        &mut reference,
        "霧川",
        AuthorizationKind::Redirect,
        &[form("누벨 리브")],
        WordOrigin::Native,
    );
    let sources = chain(
        reference,
        MapDictionary::new(),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    for input in ["霧川(누벨 리브)", "누벨 리브(霧川)"] {
        let mut engine = Engine::<PlainScopeData, _>::new(&sources);
        let mut tokens = engine.push_token(InputToken::Text(input.into()));
        tokens.extend(engine.finish());
        let expected = collapse_redundant_parens(tokens, true);
        assert!(expected.iter().any(|token|matches!(token,OutputToken::Annotated(a) if a.from_source_gloss && a.reading=="누벨 리브")));
        for split in input.char_indices().map(|(i, _)| i).chain([input.len()]) {
            let mut engine = Engine::<PlainScopeData, _>::new(&sources);
            let mut tokens = engine.push_token(InputToken::Text(input[..split].into()));
            tokens.extend(engine.push_token(InputToken::Text(input[split..].into())));
            tokens.extend(engine.finish());
            let mut collapser = RedundantParenCollapser::new(true);
            let mut actual = Vec::new();
            for token in tokens {
                actual.extend(collapser.push_token(token));
            }
            actual.extend(collapser.finish());
            assert_eq!(actual, expected, "{input}/{split}");
        }
    }
}

#[test]
fn complete_spelling_projection_reports_the_selected_anchor_spelling() {
    let sources = chain(
        records(&[("国", "국")]),
        records(&[("國", "구니")]),
        SourcePolicy::ReferenceAuthorized,
        true,
    );
    let projected = sources.candidate_matches_at_spellings(&["国", "國"]);
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].0, 0);
    assert_eq!(projected[0].1.select_default().reading, "국");
    let raw = sources.source_matches_at_spellings(&["国", "國"]).unwrap();
    assert_eq!(raw[0].1.default_choice().anchor.key, "国");
    assert_eq!(raw[0].1.groups[0].key, "國");
}

#[test]
fn glyph_lookup_enumeration_and_default_homophone_index_share_effective_choices() {
    let sources = chain(
        records(&[("国", "국"), ("晴城", "구니")]),
        records(&[("國", "구니")]),
        SourcePolicy::ReferenceAuthorized,
        true,
    );
    let entries: Vec<_> = sources.candidate_entries().unwrap().collect();
    for entry in &entries {
        let lookup = sources
            .candidate_matches_at(&entry.hanja)
            .find(|matched| matched.byte_len == entry.hanja.len())
            .unwrap();
        assert_eq!(entry.candidates, lookup.candidates, "{}", entry.hanja);
        assert_eq!(
            annotations(&sources, &entry.hanja, SegmentationStrategy::Lattice)[0].reading,
            entry.select_default().reading
        );
    }
    let anns = annotations(&sources, "晴城", SegmentationStrategy::Lattice);
    let marked = mark_homophones_with_detection(
        anns.into_iter()
            .map(OutputToken::<PlainScopeData>::Annotated),
        &sources,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );
    assert!(matches!(&marked[0],OutputToken::Annotated(a) if !a.homophone));
}

#[test]
fn legacy_homophone_query_projects_only_the_cooperative_default_identity() {
    let mut reference = MapDictionary::new();
    reference.insert("連絡", "연락");
    reference
        .insert_alias("聯絡", "연락", "連絡", AliasKind::Sibling)
        .unwrap();
    let sources = chain(
        reference,
        MapDictionary::new(),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    assert!(!sources.has_homophone("聯絡", "연락"));
    assert!(!sources.has_homophone("連絡", "연락"));
    assert!(!sources.has_homophone_canonical("連絡", "연락"));
}

#[test]
fn source_entries_resolve_pools_lazily_without_losing_projection() {
    use std::cell::Cell;
    struct Counted {
        dictionary: MapDictionary,
        lookups: Cell<usize>,
    }
    impl HanjaDictionary for Counted {
        fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            self.lookups.set(self.lookups.get() + 1);
            self.dictionary.matches_at(s)
        }
        fn candidate_matches_at<'a>(
            &'a self,
            s: &'a str,
        ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
            self.lookups.set(self.lookups.get() + 1);
            self.dictionary.candidate_matches_at(s)
        }
        fn candidate_entries<'a>(
            &'a self,
        ) -> Option<Box<dyn Iterator<Item = CandidateRecord> + 'a>> {
            self.dictionary.candidate_entries()
        }
    }
    let mut sources = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    sources.push_with_role(
        Counted {
            dictionary: records(&[("霧川", "무천"), ("晴城", "청성"), ("夜浦", "야포")]),
            lookups: Cell::new(0),
        },
        SourceRole::Reference,
    );
    let mut entries = sources.source_entries().unwrap();
    assert_eq!(
        sources.dictionaries()[0].lookups.get(),
        0,
        "enumeration must not build source pools eagerly"
    );
    let first = entries.next().unwrap();
    assert!(sources.dictionaries()[0].lookups.get() > 0);
    assert_eq!(
        first.default_choice().form.reading,
        text(&sources, &first.hanja)
    );
    assert_eq!(entries.count(), 2);
}

#[test]
fn cooperative_pipeline_uses_selected_identity_with_empty_or_full_legacy_index() {
    let mut reference = MapDictionary::new();
    reference.insert("假國", "가국");
    reference
        .insert_alias("僞國", "위국", "假國", AliasKind::Sibling)
        .unwrap();
    let mut loan = MapDictionary::new();
    loan.insert("偽国", "위구");
    loan.insert("霧川", "위국");
    let sources = chain(reference, loan, SourcePolicy::ReferenceAuthorized, true);
    let full = sources.lexemes().unwrap();
    // A stored plain loan glyph key can project an alias from a reference variant.
    assert_eq!(full.canonical("偽国", "위국"), "假國");
    let mut engine = Engine::<PlainScopeData, _>::new(&sources);
    let mut tokens = engine.push_token(InputToken::Text("偽国 僞國 霧川".into()));
    tokens.extend(engine.finish());
    let mut directives = UserDirectives::new();
    directives.add_literal("假國", DirectiveAction::RequireHanja);
    let pipeline = |lexemes: LexemeIndex| {
        let mut marker = HomophoneMarker::with_detection_and_lexemes(
            &sources,
            ContextWindow::PerDocument,
            HomophoneDetection::ContextLocal,
            &lexemes,
        );
        let mut marked = Vec::new();
        for token in tokens.clone() {
            marked.extend(marker.push_token(token));
        }
        marked.extend(marker.finish());
        let filtered = filter_first_occurrences_with_lexemes(
            marked,
            ContextWindow::PerDocument,
            lexemes.clone(),
        );
        apply_user_directives_with_lexemes(filtered, &directives, lexemes)
    };
    assert_eq!(pipeline(LexemeIndex::default()), pipeline(full));
}

#[test]
fn lattice_streaming_commits_only_shared_frontier_paths_at_every_split() {
    for length in [2usize, 64] {
        for policy in [SourcePolicy::Ordered, SourcePolicy::ReferenceAuthorized] {
            let key = "甲".repeat(length);
            let input = "甲".repeat(if length == 2 { 8 } else { 128 });
            let sources = chain(
                records(&[("甲", "갑"), (&key, "장문")]),
                MapDictionary::new(),
                policy,
                false,
            );
            for strategy in [SegmentationStrategy::Lattice, SegmentationStrategy::Eager] {
                let options = EngineOptions {
                    segmentation: strategy,
                    ..Default::default()
                };
                let expected = process_tokens_with_options(
                    [InputToken::<PlainScopeData>::Text(input.clone())],
                    &sources,
                    options,
                );
                assert_eq!(
                    write_plain_text(render_tokens(expected.clone(), RenderMode::HangulOnly)),
                    "장문".repeat(input.chars().count() / length)
                );
                for split in input
                    .char_indices()
                    .map(|(index, _)| index)
                    .chain([input.len()])
                {
                    let mut engine = Engine::<PlainScopeData, _>::with_options(&sources, options);
                    let mut actual = engine.push_token(InputToken::Text(input[..split].into()));
                    actual.extend(engine.push_token(InputToken::Text(input[split..].into())));
                    actual.extend(engine.finish());
                    assert_eq!(
                        actual, expected,
                        "length{length}/{policy:?}/{strategy:?}/split{split}"
                    );
                }
                let mut engine = Engine::<PlainScopeData, _>::with_options(&sources, options);
                let mut actual = Vec::new();
                for ch in input.chars() {
                    actual.extend(engine.push_token(InputToken::Text(ch.to_string())));
                }
                actual.extend(engine.finish());
                assert_eq!(
                    actual, expected,
                    "single-character chunks/{length}/{policy:?}/{strategy:?}"
                );
            }
        }
    }
}

#[test]
fn settled_lattice_prefixes_still_emit_before_eof() {
    for policy in [SourcePolicy::Ordered, SourcePolicy::ReferenceAuthorized] {
        let sources = chain(
            records(&[("甲乙", "가을")]),
            MapDictionary::new(),
            policy,
            false,
        );
        let input = "甲乙".repeat(40);
        let expected = process_tokens_with_options(
            [InputToken::<PlainScopeData>::Text(input.clone())],
            &sources,
            EngineOptions::default(),
        );
        let mut engine = Engine::<PlainScopeData, _>::new(&sources);
        let mut actual = engine.push_token(InputToken::Text(input));
        assert!(
            !actual.is_empty(),
            "settled frontier must make progress before EOF"
        );
        assert!(engine.buffered_chars() <= sources.max_word_chars().unwrap() + 4);
        actual.extend(engine.finish());
        assert_eq!(actual, expected);
    }
}

#[test]
fn open_numerals_wait_while_settled_lattice_prefixes_emit() {
    for strategy in [
        NumeralStrategy::PositionalArabic,
        NumeralStrategy::AdditiveArabic,
        NumeralStrategy::Smart,
    ] {
        for policy in [SourcePolicy::Ordered, SourcePolicy::ReferenceAuthorized] {
            let sources = chain(
                records(&[("甲", "가가")]),
                MapDictionary::new(),
                policy,
                false,
            );
            for input in [
                "甲一二三四五六七八九",
                "甲一千一百二十",
                "甲十一二三四五六七八九",
                "甲千一百",
            ] {
                let options = EngineOptions {
                    numeral_strategy: strategy,
                    ..Default::default()
                };
                let expected = process_tokens_with_options(
                    [InputToken::<PlainScopeData>::Text(input.into())],
                    &sources,
                    options,
                );
                for split in input
                    .char_indices()
                    .map(|(index, _)| index)
                    .chain([input.len()])
                {
                    let mut engine = Engine::<PlainScopeData, _>::with_options(&sources, options);
                    let mut actual = engine.push_token(InputToken::Text(input[..split].into()));
                    actual.extend(engine.push_token(InputToken::Text(input[split..].into())));
                    actual.extend(engine.finish());
                    assert_eq!(
                        actual, expected,
                        "{strategy:?}/{policy:?}/{input}/split{split}"
                    );
                }
                let mut engine = Engine::<PlainScopeData, _>::with_options(&sources, options);
                if input.starts_with("甲一") {
                    assert!(!engine.push_token(InputToken::Text(input.into())).is_empty());
                }
            }
        }
    }
}

#[test]
fn review_query_lengths_count_source_bytes_while_indexed_spellings_count_selected_bytes() {
    let sources = chain(
        records(&[("丽", "테스트")]),
        MapDictionary::new(),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    let query = "丽다";
    let raw = sources.source_matches_at(query).unwrap().next().unwrap();
    assert_eq!(raw.byte_len, "丽".len());
    let candidate = sources.candidate_matches_at(query).next().unwrap();
    assert_eq!(candidate.byte_len, "丽".len());
    assert!(query.is_char_boundary(candidate.byte_len));
    assert_eq!(&query[..candidate.byte_len], "丽");
    let matched = sources.matches_at(query).next().unwrap();
    assert_eq!(matched.byte_len, "丽".len());
    assert_eq!(raw.default_match().byte_len, "丽".len());
    assert_eq!(raw.select("다", true).0.byte_len, "丽".len());
    let spellings = ["丽", "丽"];
    let indexed = sources.candidate_matches_at_spellings(&spellings);
    assert_eq!(indexed.len(), 1);
    assert_eq!(indexed[0].0, 1);
    assert_eq!(indexed[0].1.byte_len, "丽".len());
    let legacy = sources.matches_at_spellings(&spellings);
    assert_eq!(legacy[0].0, 1);
    assert_eq!(legacy[0].1.byte_len, "丽".len());
}

#[test]
fn review_parenthetical_reselection_updates_matched_spelling_in_both_orders() {
    let sources = chain(
        records(&[("国", "국")]),
        records(&[("國", "구니")]),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    for input in ["国(구니)", "구니(国)"] {
        let tokens = process_tokens_with_options(
            [InputToken::<PlainScopeData>::Text(input.into())],
            &sources,
            EngineOptions::default(),
        );
        let collapsed = collapse_redundant_parens(tokens, true);
        let annotation = collapsed
            .iter()
            .find_map(|token| {
                if let OutputToken::Annotated(annotation) = token {
                    Some(annotation)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(annotation.hanja, "国");
        assert_eq!(annotation.reading, "구니");
        assert_eq!(
            annotation.selection.as_ref().unwrap().choice.anchor.key,
            "國"
        );
        assert_eq!(annotation.dictionary_hanja.as_deref(), Some("國"));
        assert_eq!(annotation.canonical_hanja(), "國");
        let options = RenderOptions {
            mode: RenderMode::HangulHanjaParens,
            hanja_variant_set: HanjaVariantSet::AsDictionary,
            ..Default::default()
        };
        assert_eq!(
            write_plain_text(render_tokens(collapsed.clone(), options)),
            "구니(國)"
        );
        let mut directives = UserDirectives::new();
        directives.add_predicate(
            |annotation| annotation.dictionary_hanja.as_deref() == Some("國"),
            DirectiveAction::SkipAnnotation,
        );
        let directed = apply_user_directives(collapsed, &directives);
        assert!(directed.iter().any(
            |token| matches!(token,OutputToken::Annotated(annotation) if annotation.skip_annotation)
        ));
    }
}

struct CustomSourcePools {
    legacy: MapDictionary,
    valid: SourceMatch,
    homophone: SourceMatch,
    empty: SourceMatch,
    exact_source_capability: bool,
}
impl CustomSourcePools {
    fn new(exact_source_capability: bool) -> Self {
        let legacy = records(&[("國", "국"), ("局", "국")]);
        let sources = chain(
            legacy.clone(),
            MapDictionary::new(),
            SourcePolicy::ReferenceAuthorized,
            false,
        );
        let valid = sources
            .source_matches_at("國")
            .unwrap()
            .find(|pool| pool.byte_len == "國".len())
            .unwrap();
        let homophone = sources
            .source_matches_at("局")
            .unwrap()
            .find(|pool| pool.byte_len == "局".len())
            .unwrap();
        let mut empty = valid.clone();
        empty.ranked = Vec::new().into();
        Self {
            // The extra homophone is supplied only by source_entries, so the
            // index test also verifies that valid custom pools are retained.
            legacy: records(&[("國", "국")]),
            valid,
            homophone,
            empty,
            exact_source_capability,
        }
    }
}
impl HanjaDictionary for CustomSourcePools {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        self.legacy.matches_at(s)
    }
    fn candidate_entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = CandidateRecord> + 'a>> {
        self.legacy.candidate_entries()
    }
    fn lexemes(&self) -> Option<LexemeIndex> {
        self.legacy.lexemes()
    }
    fn source_matches_at<'a>(
        &'a self,
        s: &'a str,
    ) -> Option<Box<dyn Iterator<Item = SourceMatch> + 'a>> {
        if !self.exact_source_capability {
            return None;
        }
        let pools = if s.starts_with("國") {
            vec![self.empty.clone(), self.valid.clone()]
        } else {
            vec![]
        };
        Some(Box::new(pools.into_iter()))
    }
    fn source_matches_at_spellings(&self, spellings: &[&str]) -> Option<Vec<(usize, SourceMatch)>> {
        Some(
            spellings
                .iter()
                .position(|spelling| *spelling == "國")
                .map_or_else(Vec::new, |index| {
                    vec![(index, self.empty.clone()), (index, self.valid.clone())]
                }),
        )
    }
    fn source_entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = SourceMatch> + 'a>> {
        Some(Box::new(
            vec![
                self.empty.clone(),
                self.valid.clone(),
                self.homophone.clone(),
            ]
            .into_iter(),
        ))
    }
}

#[test]
fn custom_empty_exact_pools_are_ignored_without_losing_valid_choices() {
    let sources = CustomSourcePools::new(true);
    // The long query also exercises the unknown-word-bound cache bypass.
    for repetitions in [1, 50] {
        let input = "國".repeat(repetitions);
        let output = process_tokens_with_options(
            [InputToken::<PlainScopeData>::Text(input)],
            &sources,
            EngineOptions::default(),
        );
        let selected = output
            .iter()
            .filter(|token| {
                matches!(token,
            OutputToken::Annotated(annotation) if annotation.selection.is_some())
            })
            .count();
        assert_eq!(selected, repetitions);
        assert_eq!(
            write_plain_text(render_tokens(output, RenderMode::HangulOnly)),
            "국".repeat(repetitions)
        );
    }
}

#[test]
fn custom_empty_spelling_pools_are_ignored_without_losing_variant_identity() {
    let sources = CustomSourcePools::new(false);
    let annotations = annotations(&sources, "国", SegmentationStrategy::Lattice);
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].reading, "국");
    assert_eq!(annotations[0].dictionary_hanja.as_deref(), Some("國"));
    assert_eq!(
        annotations[0].selection.as_ref().unwrap().choice.anchor.key,
        "國"
    );
}

#[test]
fn custom_empty_enumerated_pools_do_not_hide_real_homophones() {
    let sources = CustomSourcePools::new(true);
    let valid = chain(
        records(&[("國", "국")]),
        MapDictionary::new(),
        SourcePolicy::ReferenceAuthorized,
        false,
    );
    let annotations = annotations(&valid, "國", SegmentationStrategy::Lattice);
    let output = mark_homophones_with_detection(
        annotations
            .into_iter()
            .map(OutputToken::<PlainScopeData>::Annotated),
        &sources,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );
    assert!(matches!(&output[0],OutputToken::Annotated(annotation) if annotation.homophone));
}
