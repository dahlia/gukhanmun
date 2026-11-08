// Gukhanmun: Legacy annotation boundaries with unrelated phrase evidence.
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

fn dictionary(evidence: bool) -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    for (key, reading) in [
        ("洪", "홍"),
        ("民", "민"),
        ("部長", "부장"),
        ("降服", "항복"),
    ] {
        dictionary.insert(key, reading);
    }
    if evidence {
        dictionary
            .set_phrase_evidence(PhraseEvidence::new(vec![PhraseRule::new(
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
            )]))
            .unwrap();
    }
    dictionary
}

fn convert(
    dictionary: &impl HanjaDictionary,
    chunks: &[&str],
    strategy: SegmentationStrategy,
    reporting: bool,
) -> String {
    let mut engine = Engine::<PlainScopeData, _>::with_options(
        dictionary,
        EngineOptions {
            segmentation: strategy,
            ..Default::default()
        },
    );
    if reporting {
        engine = engine.with_resolution_reporting();
    }
    let mut output = vec![];
    for chunk in chunks {
        output.extend(engine.push_token(InputToken::Text((*chunk).into())));
    }
    output.extend(engine.finish());
    output
        .into_iter()
        .filter_map(|token| match token {
            OutputToken::Annotated(annotation) => {
                Some(format!("{}({})", annotation.reading, annotation.hanja))
            }
            OutputToken::Text(text) | OutputToken::Verbatim(text) => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn unrelated_phrase_evidence_preserves_ordered_proper_name_annotations() {
    let input = "洪民憙 部長";
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        for reporting in [false, true] {
            for evidence in [false, true] {
                let mut chain = ChainDictionary::new().with_source_policy(SourcePolicy::Ordered);
                chain.push(dictionary(evidence));
                let expected = "홍민희(洪民憙) 부장(部長)";
                assert_eq!(convert(&chain, &[input], strategy, reporting), expected);
                for (split, _) in input.char_indices() {
                    assert_eq!(
                        convert(
                            &chain,
                            &[&input[..split], &input[split..]],
                            strategy,
                            reporting
                        ),
                        expected
                    );
                }
                let scalars: Vec<_> = input
                    .char_indices()
                    .map(|(start, ch)| &input[start..start + ch.len_utf8()])
                    .collect();
                assert_eq!(convert(&chain, &scalars, strategy, reporting), expected);
            }
        }
    }
}

#[test]
fn cooperative_policy_keeps_source_aware_annotation_boundaries() {
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        let mut chain = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
        chain.push_with_role(dictionary(true), SourceRole::Reference);
        assert_eq!(
            convert(&chain, &["洪民憙 部長"], strategy, false),
            "홍(洪)민(民)희(憙) 부장(部長)"
        );
        for evidence in [false, true] {
            let mut inner = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
            inner.push_with_role(dictionary(evidence), SourceRole::Reference);
            let mut ordered = ChainDictionary::new().with_source_policy(SourcePolicy::Ordered);
            ordered.push(inner);
            assert_eq!(
                convert(&ordered, &["洪民憙 部長"], strategy, false),
                "홍민희(洪民憙) 부장(部長)"
            );
        }
    }
}

#[test]
fn unrelated_phrase_evidence_preserves_long_streamed_names() {
    let input = format!("{} 部長", "洪民憙".repeat(40));
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        let mut baseline = ChainDictionary::new().with_source_policy(SourcePolicy::Ordered);
        baseline.push(dictionary(false));
        let mut with_evidence = ChainDictionary::new().with_source_policy(SourcePolicy::Ordered);
        with_evidence.push(dictionary(true));
        let expected = convert(&baseline, &[&input], strategy, false);
        let scalars: Vec<_> = input
            .char_indices()
            .map(|(start, ch)| &input[start..start + ch.len_utf8()])
            .collect();
        assert_eq!(convert(&with_evidence, &scalars, strategy, false), expected);
    }
}

#[test]
fn applicable_phrase_proof_retains_each_attested_edge() {
    let mut dictionary = dictionary(false);
    dictionary
        .set_phrase_evidence(PhraseEvidence::new(vec![PhraseRule::new(
            vec![WordId(2)],
            [("洪", "홍"), ("民", "민")]
                .into_iter()
                .map(|(key, reading)| {
                    PhraseChunk::new(
                        reading.into(),
                        vec![PhraseSpelling::new(
                            key.into(),
                            vec![PhraseCandidateLink::new(reading.into(), None)],
                        )],
                    )
                })
                .collect(),
        )]))
        .unwrap();
    let mut chain = ChainDictionary::new().with_source_policy(SourcePolicy::Ordered);
    chain.push(dictionary);
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        for chunks in [vec!["洪民"], vec!["洪", "民"]] {
            assert_eq!(convert(&chain, &chunks, strategy, true), "홍(洪)민(民)");
        }
    }
}

fn particle_dictionary(evidence: bool, attested: bool) -> MapDictionary {
    particle_dictionary_at("字", evidence, attested)
}

fn particle_dictionary_at(key: &str, evidence: bool, attested: bool) -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    dictionary.insert_candidates(
        key,
        ReadingCandidates::new(
            [("글", 10), ("사", 0)]
                .into_iter()
                .map(|(reading, preference)| ReadingCandidate {
                    reading: reading.into(),
                    suffix_reading: None,
                    mark: MatchMark::default(),
                    preference,
                    alias: None,
                })
                .collect(),
        )
        .unwrap(),
    );
    dictionary.insert("降服", "항복");
    if evidence {
        let (key, reading) = if attested {
            (key, "사")
        } else {
            ("降服", "항복")
        };
        dictionary
            .set_phrase_evidence(PhraseEvidence::new(vec![PhraseRule::new(
                vec![WordId(3)],
                vec![
                    PhraseChunk::new(
                        "무조건".into(),
                        vec![PhraseSpelling::new("無條件".into(), vec![])],
                    ),
                    PhraseChunk::new(
                        reading.into(),
                        vec![PhraseSpelling::new(
                            key.into(),
                            vec![PhraseCandidateLink::new(reading.into(), None)],
                        )],
                    ),
                ],
            )]))
            .unwrap();
    }
    dictionary
}

fn assert_streamed(dictionary: &impl HanjaDictionary, input: &str, expected: &str) {
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        for reporting in [false, true] {
            assert_eq!(convert(dictionary, &[input], strategy, reporting), expected);
            for (split, _) in input.char_indices() {
                assert_eq!(
                    convert(
                        dictionary,
                        &[&input[..split], &input[split..]],
                        strategy,
                        reporting
                    ),
                    expected
                );
            }
            let scalars: Vec<_> = input
                .char_indices()
                .map(|(start, ch)| &input[start..start + ch.len_utf8()])
                .collect();
            assert_eq!(convert(dictionary, &scalars, strategy, reporting), expected);
        }
    }
}

#[test]
fn unrelated_evidence_preserves_nested_ordered_particle_fallback() {
    for evidence in [false, true] {
        let mut inner = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
        inner.push(particle_dictionary(evidence, false));
        let mut outer = ChainDictionary::new();
        outer.push(inner);
        assert_streamed(&outer, "字를", "글(字)를");
        let mut another = ChainDictionary::new();
        another.push(outer);
        assert_streamed(&another, "字를", "글(字)를");
    }
}

#[test]
fn plain_and_cooperative_particle_selection_keep_their_full_pools() {
    for evidence in [false, true] {
        let leaf = particle_dictionary(evidence, false);
        assert_streamed(&leaf, "字를", "사(字)를");
        let mut ordered = ChainDictionary::new();
        ordered.push(leaf.clone());
        assert_streamed(&ordered, "字를", "사(字)를");
        let mut cooperative =
            ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
        cooperative.push(leaf);
        assert_streamed(&cooperative, "字를", "사(字)를");
    }
}

#[test]
fn nested_ordered_phrase_proof_can_choose_outside_legacy_projection() {
    let mut inner = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
    inner.push(MapDictionary::new());
    inner.push(particle_dictionary(true, true));
    let mut outer = ChainDictionary::new();
    outer.push(inner);
    assert_streamed(&outer, "字를", "글(字)를");
    let pool = outer.source_matches_at("字를").unwrap().next().unwrap();
    let (_, selected) = pool.select("를", true);
    assert_eq!(selected.ranked.len(), 2);
    assert_eq!(selected.choice.form.reading, "글");
    assert_eq!(selected.choice.anchor.reading, "글");
    assert_eq!(selected.choice.anchor.source_id, SourceId(0));
    assert!(selected.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        SelectionDiagnostic::OpaqueLegacyProjection {
            source_id: SourceId(0),
            source_count: 2
        }
    )));
    assert_streamed(&outer, "무조건字를", "무조건사(字)를");
    let mut engine = Engine::<PlainScopeData, _>::new(&outer).with_resolution_reporting();
    engine.push_token(InputToken::Text("무조건字를".into()));
    let (_, report) = engine.finish_with_report();
    let event = report
        .events
        .iter()
        .find(|event| event.chosen.anchor.key == "字")
        .unwrap();
    assert_eq!(event.outcome, ResolutionOutcome::Selected);
    assert_eq!(event.chosen.form.reading, "사");
    assert_eq!(event.chosen.anchor.reading, "사");
    assert_eq!(event.chosen.anchor.source_id, SourceId(1));
    assert_eq!(event.rules[0].owner, SourceId(1));
}

#[test]
fn nested_ordered_variant_particle_fallback_preserves_anchor_and_proof_pool() {
    for evidence in [false, true] {
        let mut inner = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
        inner.push(particle_dictionary_at("降", evidence, true));
        let mut outer = ChainDictionary::new();
        outer.push(inner);
        assert_streamed(&outer, "降를", "글(降)를");
        if evidence {
            assert_streamed(&outer, "무조건降를", "무조건사(降)를");
            let matches = outer.source_matches_at_spellings(&["降", "降"]).unwrap();
            let (_, selected) = matches[0].1.select("를", true);
            assert_eq!(selected.choice.anchor.key, "降");
            assert_eq!(selected.choice.form.reading, "글");
            assert_eq!(selected.ranked.len(), 2);
        }
    }
}

#[test]
fn nested_policy_role_matrix_preserves_unconstrained_particle_fallback() {
    let policies = [
        SourcePolicy::Ordered,
        SourcePolicy::ReferenceFirst,
        SourcePolicy::ReferenceAuthorized,
        SourcePolicy::LoanwordFirst,
    ];
    for inner_role in [SourceRole::Reference, SourceRole::Loanword] {
        for inner_policy in policies {
            for outer_policy in policies {
                for role in [
                    SourceRole::Reference,
                    SourceRole::Override,
                    SourceRole::Loanword,
                ] {
                    for reference_only in [false, true] {
                        let build = |evidence| {
                            let mut inner = ChainDictionary::new().with_source_policy(inner_policy);
                            inner.push_with_role(particle_dictionary(evidence, false), inner_role);
                            let mut outer = ChainDictionary::new().with_source_policy(outer_policy);
                            outer.set_reference_only(reference_only);
                            outer.push_with_role(inner, role);
                            outer
                        };
                        let baseline = build(false);
                        let expected =
                            convert(&baseline, &["字를"], SegmentationStrategy::Eager, false);
                        assert_streamed(&build(true), "字를", &expected);
                    }
                }
            }
        }
    }
}

fn promoted_alias_dictionary(phrase: bool) -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    let alias = AliasTarget {
        canonical: "國林".into(),
        kind: AliasKind::Sibling,
    };
    let candidate = |reading: &str, alias| ReadingCandidate {
        reading: reading.into(),
        suffix_reading: Some(format!("{reading}끝")),
        mark: MatchMark {
            require_hanja: true,
            require_hangul: false,
        },
        preference: 0,
        alias,
    };
    dictionary.insert_candidates(
        "國琳",
        ReadingCandidates::new(vec![candidate("국림", Some(alias))]).unwrap(),
    );
    dictionary.insert_candidates(
        "國林",
        ReadingCandidates::new(vec![candidate("국림", None), candidate("새이름", None)]).unwrap(),
    );
    dictionary.insert("降服", "항복");
    dictionary
        .set_source_evidence(DictionaryEvidence {
            schema_version: 1,
            role: SourceRole::Reference,
            entries: std::collections::BTreeMap::from([(
                "國琳".into(),
                vec![CandidateEvidence {
                    reading: "국림".into(),
                    suffix_reading: Some("국림끝".into()),
                    contributors: vec![SourceContribution {
                        word_id: WordId(1),
                        sense_id: Some(SenseId(2)),
                        origin: WordOrigin::Foreign,
                    }],
                    authorized_forms: vec![AuthorizedForm {
                        form: ReadingForm {
                            reading: "새이름".into(),
                            suffix_reading: Some("새이름끝".into()),
                        },
                        kind: AuthorizationKind::Redirect,
                        witnesses: vec![RelationWitness {
                            source: SourceReference::Sense(SenseId(2)),
                            target: SourceReference::Sense(SenseId(3)),
                        }],
                    }],
                    diagnostics: vec![],
                }],
            )]),
        })
        .unwrap();
    if phrase {
        let evidence = particle_dictionary(true, false)
            .phrase_evidence()
            .unwrap()
            .clone();
        dictionary.set_phrase_evidence(evidence).unwrap();
    }
    dictionary
}

#[test]
fn nested_promoted_alias_fallback_preserves_complete_form_and_marks() {
    for policy in [
        SourcePolicy::Ordered,
        SourcePolicy::ReferenceFirst,
        SourcePolicy::ReferenceAuthorized,
        SourcePolicy::LoanwordFirst,
    ] {
        for role in [
            SourceRole::Reference,
            SourceRole::Override,
            SourceRole::Loanword,
        ] {
            let build = |evidence| {
                let mut inner =
                    ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
                inner.push_with_role(promoted_alias_dictionary(evidence), SourceRole::Reference);
                let mut outer = ChainDictionary::new().with_source_policy(policy);
                outer.push_with_role(inner, role);
                outer
            };
            let baseline = build(false);
            let expected = convert(&baseline, &["國琳"], SegmentationStrategy::Eager, false);
            assert_eq!(expected, "새이름(國琳)");
            let with_evidence = build(true);
            assert_streamed(&with_evidence, "國琳", &expected);
            if with_evidence.owned_phrase_evidence().is_empty() {
                continue;
            }
            let pool = with_evidence
                .source_matches_at("國琳")
                .unwrap()
                .next()
                .unwrap();
            let chosen = pool.default_choice();
            assert_eq!(chosen.form.reading, "새이름");
            assert_eq!(chosen.form.suffix_reading.as_deref(), Some("새이름끝"));
            assert!(chosen.mark.require_hanja);
            assert_eq!(chosen.anchor.alias.as_ref().unwrap().canonical, "國林");
            assert!(pool.diagnostics.iter().any(|diagnostic| matches!(
                diagnostic,
                SelectionDiagnostic::OpaqueLegacyProjection { .. }
            )));
        }
    }
}

#[test]
fn opaque_only_fallback_keeps_legacy_edges_without_gaining_phrase_authority() {
    let mut inner = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
    inner.push_with_role(particle_dictionary(true, true), SourceRole::Loanword);
    let mut outer = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
    outer.set_reference_only(true);
    outer.push_with_role(inner, SourceRole::Reference);
    let pool = outer.source_matches_at("字를").unwrap().next().unwrap();
    assert!(pool.ranked.is_empty());
    assert_eq!(pool.default_choice().form.reading, "글");
    assert_streamed(&outer, "무조건字를", "무조건글(字)를");
}

fn nested_particle_source() -> ChainDictionary<MapDictionary> {
    let mut child = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
    child.push_with_role(particle_dictionary(true, false), SourceRole::Reference);
    child
}

fn opaque_ranges(pool: &SourceMatch) -> Vec<(SourceId, usize)> {
    pool.diagnostics
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            SelectionDiagnostic::OpaqueLegacyProjection {
                source_id,
                source_count,
            } => Some((*source_id, *source_count)),
            _ => None,
        })
        .collect()
}

#[test]
fn discarded_nested_sources_do_not_emit_opaque_fallback_diagnostics() {
    for (policy, reference_only, first_role, second_role, retained) in [
        (
            SourcePolicy::Ordered,
            false,
            SourceRole::Reference,
            SourceRole::Reference,
            SourceId(0),
        ),
        (
            SourcePolicy::ReferenceFirst,
            true,
            SourceRole::Reference,
            SourceRole::Loanword,
            SourceId(0),
        ),
        (
            SourcePolicy::ReferenceAuthorized,
            false,
            SourceRole::Reference,
            SourceRole::Override,
            SourceId(1),
        ),
    ] {
        let mut outer = ChainDictionary::new().with_source_policy(policy);
        outer.set_reference_only(reference_only);
        outer.push_with_role(nested_particle_source(), first_role);
        outer.push_with_role(nested_particle_source(), second_role);
        let pool = outer.source_matches_at("字").unwrap().next().unwrap();
        assert_eq!(opaque_ranges(&pool), vec![(retained, 1)]);
        assert_eq!(pool.default_choice().anchor.source_id, retained);
        assert_eq!(pool.default_choice().form.reading, "글");
        let matches = outer.source_matches_at_spellings(&["字"]).unwrap();
        assert_eq!(opaque_ranges(&matches[0].1), vec![(retained, 1)]);
    }
}

#[test]
fn opaque_loanword_form_supplier_keeps_its_diagnostic_when_reference_only() {
    for extra_slot in [false, true] {
        let mut reference = particle_dictionary(true, false);
        reference
            .set_source_evidence(DictionaryEvidence {
                schema_version: 1,
                role: SourceRole::Reference,
                entries: std::collections::BTreeMap::from([(
                    "字".into(),
                    vec![CandidateEvidence {
                        reading: "글".into(),
                        suffix_reading: None,
                        contributors: vec![SourceContribution {
                            word_id: WordId(4),
                            sense_id: Some(SenseId(5)),
                            origin: WordOrigin::Foreign,
                        }],
                        authorized_forms: vec![AuthorizedForm {
                            form: ReadingForm {
                                reading: "사".into(),
                                suffix_reading: None,
                            },
                            kind: AuthorizationKind::Synonym,
                            witnesses: vec![RelationWitness {
                                source: SourceReference::Sense(SenseId(5)),
                                target: SourceReference::Sense(SenseId(6)),
                            }],
                        }],
                        diagnostics: vec![],
                    }],
                )]),
            })
            .unwrap();
        let mut loan = MapDictionary::new();
        loan.insert("字", "사");
        let mut child = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
        if extra_slot {
            child.push_with_role(MapDictionary::new(), SourceRole::Reference);
        }
        child.push_with_role(loan, SourceRole::Reference);
        let mut outer: ChainDictionary<Box<dyn HanjaDictionary>> =
            ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
        outer.set_reference_only(true);
        outer.push_with_role(Box::new(reference), SourceRole::Reference);
        outer.push_with_role(Box::new(child), SourceRole::Loanword);
        let pool = outer.source_matches_at("字").unwrap().next().unwrap();
        let spelling_pool = outer
            .source_matches_at_spellings(&["字"])
            .unwrap()
            .remove(0)
            .1;
        assert_eq!(
            opaque_ranges(&spelling_pool),
            vec![(SourceId(1), 1 + usize::from(extra_slot))]
        );
        assert_eq!(
            spelling_pool.default_choice().form_source,
            Some(SourceId(1))
        );
        assert!(
            pool.ranked
                .iter()
                .all(|choice| choice.anchor.source_id == SourceId(0))
        );
        assert_eq!(pool.default_choice().anchor.source_id, SourceId(0));
        assert_eq!(pool.default_choice().form_source, Some(SourceId(1)));
        assert_eq!(pool.default_choice().form.reading, "사");
        assert_eq!(
            opaque_ranges(&pool),
            vec![(SourceId(1), 1 + usize::from(extra_slot))]
        );
    }
}

#[test]
fn parent_opaque_projection_replaces_inherited_child_fallback_ranges() {
    for policy in [SourcePolicy::Ordered, SourcePolicy::ReferenceFirst] {
        let mut middle = ChainDictionary::new();
        middle.push_with_role(nested_particle_source(), SourceRole::Reference);
        middle.push_with_role(nested_particle_source(), SourceRole::Reference);
        let mut outer = ChainDictionary::new().with_source_policy(policy);
        outer.push_with_role(middle, SourceRole::Override);
        let pool = outer.source_matches_at("字").unwrap().next().unwrap();
        assert_eq!(opaque_ranges(&pool), vec![(SourceId(0), 2)]);
        assert_eq!(pool.default_choice().form.reading, "글");
        assert!(!pool.ranked.is_empty());
        let matches = outer.source_matches_at_spellings(&["字"]).unwrap();
        assert_eq!(opaque_ranges(&matches[0].1), vec![(SourceId(0), 2)]);
    }
}

#[test]
fn evidence_free_plain_leaves_are_not_opaque_projections() {
    for role in [SourceRole::Override, SourceRole::Reference] {
        let mut user = MapDictionary::new();
        user.insert("字", "정");
        let mut sources = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
        sources.push_with_role(user, role);
        sources.push_with_role(particle_dictionary(true, false), SourceRole::Reference);
        let pool = sources.source_matches_at("字").unwrap().next().unwrap();
        assert!(opaque_ranges(&pool).is_empty());
        assert_eq!(pool.default_choice().anchor.source_id, SourceId(0));
        assert!(
            pool.ranked
                .iter()
                .any(|choice| choice == pool.default_choice())
        );
        let matches = sources.source_matches_at_spellings(&["字"]).unwrap();
        assert!(opaque_ranges(&matches[0].1).is_empty());
    }
}

#[test]
fn identical_legacy_and_proof_diagnostics_are_reported_once() {
    for fallback_only in [false, true] {
        let mut first = particle_dictionary(true, false);
        // Keep just one candidate so each source supplies one unknown-evidence issue.
        first.insert("字", "글");
        first
            .set_phrase_evidence(
                particle_dictionary(true, false)
                    .phrase_evidence()
                    .unwrap()
                    .clone(),
            )
            .unwrap();
        let mut second = MapDictionary::new();
        second.insert("字", "사");
        let mut sources: ChainDictionary<Box<dyn HanjaDictionary>> =
            ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
        sources.set_reference_only(true);
        for dictionary in [first, second] {
            let source: Box<dyn HanjaDictionary> = if fallback_only {
                let mut inner =
                    ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
                inner.push_with_role(dictionary, SourceRole::Loanword);
                Box::new(inner)
            } else {
                Box::new(dictionary)
            };
            sources.push_with_role(source, SourceRole::Reference);
        }
        let pool = sources.source_matches_at("字").unwrap().next().unwrap();
        assert_eq!(pool.ranked.is_empty(), fallback_only);
        let expected = vec![
            SelectionDiagnostic::UnknownEvidence {
                source_id: SourceId(0),
                reading: "글".into(),
            },
            SelectionDiagnostic::UnknownEvidence {
                source_id: SourceId(1),
                reading: "사".into(),
            },
        ];
        for pool in [
            pool,
            sources
                .source_matches_at_spellings(&["字"])
                .unwrap()
                .remove(0)
                .1,
        ] {
            let unknown: Vec<_> = pool
                .diagnostics
                .iter()
                .filter(|diagnostic| {
                    matches!(diagnostic, SelectionDiagnostic::UnknownEvidence { .. })
                })
                .cloned()
                .collect();
            assert_eq!(unknown, expected);
        }
    }
}

#[test]
fn opaque_variant_fallback_keeps_the_queried_surface_and_width() {
    let leaf = || {
        let mut dictionary = MapDictionary::new();
        dictionary.insert("丽", "리");
        dictionary
    };
    let mut supplier = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
    supplier.push_with_role(leaf(), SourceRole::Reference);
    let mut hidden = ChainDictionary::new().with_source_policy(SourcePolicy::LoanwordFirst);
    hidden.push_with_role(leaf(), SourceRole::Loanword);
    let mut outer: ChainDictionary<Box<dyn HanjaDictionary>> =
        ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
    outer.set_reference_only(true);
    outer.push_with_role(Box::new(supplier), SourceRole::Loanword);
    outer.push_with_role(Box::new(hidden), SourceRole::Reference);
    outer.push_with_role(Box::new(dictionary(true)), SourceRole::Reference);

    let query = "丽";
    let spellings = outer.source_matches_at_spellings(&[query, "丽"]).unwrap();
    assert_eq!(spellings.len(), 1);
    assert!(spellings[0].1.ranked.is_empty());
    assert_eq!(spellings[0].1.hanja, query);
    assert_eq!(spellings[0].1.byte_len, query.len());
    assert_eq!(spellings[0].1.default_choice().form.reading, "리");
    let exact = outer.source_matches_at(query).unwrap().collect::<Vec<_>>();
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].hanja, query);
    assert_eq!(exact[0].byte_len, query.len());
}
