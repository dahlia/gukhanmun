// Gukhanmun: Regression coverage for fixed-path source phrase constraints.
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

fn candidate(reading: &str, preference: u32) -> ReadingCandidate {
    ReadingCandidate {
        reading: reading.into(),
        suffix_reading: None,
        mark: MatchMark::default(),
        preference,
        alias: None,
    }
}
fn chunk(key: &str, reading: &str) -> PhraseChunk {
    PhraseChunk::new(
        reading.into(),
        vec![PhraseSpelling::new(
            key.into(),
            vec![PhraseCandidateLink::new(reading.into(), None)],
        )],
    )
}
fn rule(id: u32, reading: &str) -> PhraseRule {
    PhraseRule::new(
        vec![WordId(u64::from(id))],
        vec![chunk("無條件", "무조건"), chunk("降服", reading)],
    )
}
fn dictionary(rules: Vec<PhraseRule>) -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("無條件", "무조건");
    dictionary.insert_candidates(
        "降服",
        ReadingCandidates::new(vec![candidate("강복", 10), candidate("항복", 0)]).unwrap(),
    );
    dictionary.insert_candidates(
        "降服하다",
        ReadingCandidates::new(vec![candidate("강복하다", 10), candidate("항복하다", 0)]).unwrap(),
    );
    dictionary
        .set_phrase_evidence(PhraseEvidence::new(rules))
        .unwrap();
    dictionary
}
fn render<S>(tokens: Vec<OutputToken<S>>) -> String {
    tokens
        .into_iter()
        .filter_map(|token| match token {
            OutputToken::Text(text) | OutputToken::Verbatim(text) => Some(text),
            OutputToken::Annotated(annotation) => Some(annotation.reading),
            _ => None,
        })
        .collect()
}
fn streamed(
    dictionary: &impl HanjaDictionary,
    chunks: &[&str],
    strategy: SegmentationStrategy,
) -> (String, ResolutionReport) {
    let mut engine = Engine::<PlainScopeData, _>::with_options(
        dictionary,
        EngineOptions {
            segmentation: strategy,
            ..Default::default()
        },
    )
    .with_resolution_reporting();
    let mut output = Vec::new();
    for chunk in chunks {
        output.extend(engine.push_token(InputToken::Text((*chunk).into())));
    }
    // A final textual witness lets callers inspect reports before consuming Engine.
    output.extend(engine.push_token(InputToken::Text(" ".into())));
    output.extend(engine.flush());
    let report = engine.take_resolution_report();
    output.extend(engine.finish());
    let mut text = render(output);
    text.pop();
    (text, report)
}
#[test]
fn source_phrase_changes_reading_without_changing_edges() {
    let dictionary = dictionary(vec![rule(1, "항복")]);
    for strategy in [SegmentationStrategy::Lattice, SegmentationStrategy::Eager] {
        for input in [
            "無條件降服",
            "無條件 降服",
            "무조건降服",
            "무조건 降服",
            "無條件降服을",
        ] {
            let (text, report) = streamed(&dictionary, &[input], strategy);
            assert!(text.contains("항복"), "{input}: {text}");
            assert!(
                report
                    .events
                    .iter()
                    .any(|event| event.outcome == ResolutionOutcome::Selected
                        && event.chosen.anchor.reading == "항복")
            );
        }
    }
}
#[test]
fn all_scalar_chunk_splits_keep_literal_context_and_decisions() {
    let dictionary = dictionary(vec![rule(1, "항복")]);
    let input = "앞 문장. 무조건 降服을 끝. 무조건降服 뒤 문장 길게 길게 길게.";
    let (expected, expected_report) =
        streamed(&dictionary, &[input], SegmentationStrategy::Lattice);
    assert_eq!(expected.matches("항복").count(), 2);
    for split in input
        .char_indices()
        .map(|(index, _)| index)
        .chain(core::iter::once(input.len()))
    {
        let (actual, report) = streamed(
            &dictionary,
            &[&input[..split], &input[split..]],
            SegmentationStrategy::Lattice,
        );
        assert_eq!(actual, expected, "split={split}");
        assert_eq!(report, expected_report, "report split={split}");
    }
    let scalars: Vec<&str> = input
        .char_indices()
        .map(|(index, ch)| &input[index..index + ch.len_utf8()])
        .collect();
    assert_eq!(
        streamed(&dictionary, &scalars, SegmentationStrategy::Lattice).0,
        expected
    );
}
#[test]
fn strict_surface_and_whole_edge_alignment_reject_accidental_matches() {
    let mut dictionary = dictionary(vec![rule(1, "항복")]);
    dictionary.insert("無條件降服", "긴말");
    for input in [
        "無條件降服",
        "무조건  降服",
        "무조건\t降服",
        "무조건\n降服",
        "불무조건降服",
        "무조건降服하다",
        "무조건降服되는",
    ] {
        let text = streamed(&dictionary, &[input], SegmentationStrategy::Lattice).0;
        assert!(!text.contains("항복"), "{input}: {text}");
    }
}
#[test]
fn witnessed_terminal_extension_is_atomic() {
    let extension = TerminalExtension::new(
        "하다".into(),
        vec![PhraseSpelling::new(
            "降服하다".into(),
            vec![PhraseCandidateLink::new("항복하다".into(), None)],
        )],
        vec![SubheadwordWitness::new(WordId(2), WordId(3))],
    );
    let dictionary = dictionary(vec![
        rule(1, "항복").with_terminal_extensions(vec![extension]),
    ]);
    assert_eq!(
        streamed(
            &dictionary,
            &["무조건 降服하다"],
            SegmentationStrategy::Lattice
        )
        .0,
        "무조건 항복하다"
    );
    assert_eq!(
        streamed(&dictionary, &["降服하다"], SegmentationStrategy::Lattice).0,
        "강복하다"
    );
}
#[test]
fn rule_conflicts_are_local_and_retain_legacy_choice() {
    let dictionary = dictionary(vec![rule(1, "항복"), rule(2, "강복")]);
    let (text, report) = streamed(&dictionary, &["無條件降服"], SegmentationStrategy::Lattice);
    assert_eq!(text, "무조건강복");
    let event = report
        .events
        .iter()
        .find(|event| event.chosen.anchor.key == "降服")
        .unwrap();
    assert_eq!(event.outcome, ResolutionOutcome::Conflict);
    assert_eq!(event.rules.len(), 2);
    assert!(event.remaining.is_empty());
}
#[test]
fn explicit_flush_and_markup_break_phrase_evidence() {
    let dictionary = dictionary(vec![rule(1, "항복")]);
    let mut engine = Engine::<PlainScopeData, _>::new(&dictionary);
    let mut output = engine.push_token(InputToken::Text("무조건 ".into()));
    output.extend(engine.flush());
    output.extend(engine.push_token(InputToken::Text("降服".into())));
    output.extend(engine.finish());
    assert_eq!(render(output), "무조건 강복");
    let mut engine = Engine::<PlainScopeData, _>::new(&dictionary);
    let mut output = engine.push_token(InputToken::Text("무조건 ".into()));
    output.extend(engine.push_token(InputToken::Open(Scope::new(PlainScopeData))));
    output.extend(engine.push_token(InputToken::Text("降服".into())));
    output.extend(engine.finish());
    assert_eq!(render(output), "무조건 강복");
}
#[test]
fn matcher_limit_never_applies_partial_constraints() {
    let rules = (1..=MAX_PHRASE_MATCHES + 1)
        .map(|id| rule(id as u32, "항복"))
        .collect();
    let dictionary = dictionary(rules);
    let (text, report) = streamed(&dictionary, &["무조건降服"], SegmentationStrategy::Lattice);
    assert_eq!(text, "무조건강복");
    assert!(
        report
            .events
            .iter()
            .any(|event| event.outcome == ResolutionOutcome::Limit)
    );
}

#[test]
fn particle_conflict_preserves_the_legacy_particle_choice() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("無條件", "무조건");
    dictionary.insert_candidates(
        "降服",
        ReadingCandidates::new(vec![candidate("가가", 10), candidate("가각", 0)]).unwrap(),
    );
    dictionary
        .set_phrase_evidence(PhraseEvidence::new(vec![rule(1, "가각")]))
        .unwrap();
    let (text, report) = streamed(
        &dictionary,
        &["무조건降服를"],
        SegmentationStrategy::Lattice,
    );
    assert_eq!(text, "무조건가가를");
    assert!(
        report
            .events
            .iter()
            .any(|event| event.outcome == ResolutionOutcome::Conflict
                && event.chosen.anchor.reading == "가가")
    );
}

#[test]
fn eof_proves_a_phrase_boundary_but_arbitrary_hangul_suffix_does_not() {
    let dictionary = dictionary(vec![rule(1, "항복")]);
    let mut engine = Engine::<PlainScopeData, _>::new(&dictionary).with_resolution_reporting();
    let mut output = engine.push_token(InputToken::Text("무조건降服".into()));
    let (tail, report) = engine.finish_with_report();
    output.extend(tail);
    assert!(
        report
            .events
            .iter()
            .any(|event| event.outcome == ResolutionOutcome::Selected)
    );
    assert_eq!(render(output), "무조건항복");
    assert_eq!(
        streamed(
            &dictionary,
            &["무조건降服하는"],
            SegmentationStrategy::Lattice
        )
        .0,
        "무조건강복하는"
    );
}

#[test]
fn phrase_lookahead_retains_only_a_finite_additional_plaintext_tail() {
    let dictionary = dictionary(vec![rule(1, "항복")]);
    let mut engine = Engine::<PlainScopeData, _>::new(&dictionary);
    let mut emitted = 0;
    for _ in 0..5000 {
        emitted += render(engine.push_token(InputToken::Text("가".into())))
            .chars()
            .count();
        assert!(engine.buffered_chars() < 40);
    }
    emitted += render(engine.finish()).chars().count();
    assert_eq!(emitted, 5000);
}

#[test]
fn selected_choice_keeps_its_own_marks_and_positional_metadata() {
    let mut dictionary = dictionary(vec![rule(1, "항복")]);
    let mut selected = candidate("항복", 0);
    selected.mark.require_hanja = true;
    dictionary.insert_candidates(
        "降服",
        ReadingCandidates::new(vec![candidate("강복", 10), selected]).unwrap(),
    );
    dictionary
        .set_phrase_evidence(PhraseEvidence::new(vec![rule(1, "항복")]))
        .unwrap();
    let (text, report) = streamed(&dictionary, &["무조건降服"], SegmentationStrategy::Lattice);
    assert_eq!(text, "무조건항복");
    let event = report
        .events
        .iter()
        .find(|event| event.chosen.anchor.key == "降服")
        .unwrap();
    assert!(event.chosen.mark.require_hanja);
    assert_eq!(event.chosen.form.reading, "항복");
}

#[test]
fn source_ownership_survives_nested_ordered_and_cooperative_chains() {
    let reference = dictionary(vec![rule(1, "항복")]);
    let mut inner = ChainDictionary::<MapDictionary>::new();
    inner.push_with_role(MapDictionary::new(), SourceRole::Override);
    inner.push_with_role(reference, SourceRole::Reference);
    let mut outer = ChainDictionary::<ChainDictionary<MapDictionary>>::new()
        .with_source_policy(SourcePolicy::ReferenceFirst);
    outer.push(inner);
    assert_eq!(
        streamed(&outer, &["무조건降服"], SegmentationStrategy::Lattice).0,
        "무조건항복"
    );
    let mut override_dictionary = MapDictionary::new();
    override_dictionary.insert("降服", "강복");
    let mut override_inner = ChainDictionary::<MapDictionary>::new()
        .with_source_policy(SourcePolicy::ReferenceAuthorized);
    override_inner.push_with_role(override_dictionary, SourceRole::Override);
    override_inner.push_with_role(dictionary(vec![rule(1, "항복")]), SourceRole::Reference);
    let mut outer = ChainDictionary::<ChainDictionary<MapDictionary>>::new()
        .with_source_policy(SourcePolicy::ReferenceFirst);
    outer.push(override_inner);
    assert_eq!(
        streamed(&outer, &["무조건降服"], SegmentationStrategy::Lattice).0,
        "무조건강복"
    );
}

#[test]
fn owned_phrase_presence_tracks_builtin_leaf_evidence() {
    assert!(!MapDictionary::new().has_owned_phrase_evidence());
    assert!(!dictionary(vec![]).has_owned_phrase_evidence());
    assert!(dictionary(vec![rule(1, "항복")]).has_owned_phrase_evidence());
    assert!(!UnihanCharDict.has_owned_phrase_evidence());
}

#[test]
fn owned_phrase_presence_short_circuits_through_wrappers_and_nested_chains() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Probe {
        dictionary: MapDictionary,
        checks: Arc<AtomicUsize>,
    }
    impl HanjaDictionary for Probe {
        fn has_owned_phrase_evidence(&self) -> bool {
            self.checks.fetch_add(1, Ordering::Relaxed);
            self.dictionary.has_owned_phrase_evidence()
        }
        fn owned_phrase_evidence(&self) -> Vec<OwnedPhraseEvidence<'_>> {
            panic!("presence checks and source lookups must not collect evidence")
        }
        fn phrase_evidence(&self) -> Option<&PhraseEvidence> {
            self.dictionary.phrase_evidence()
        }
        fn candidate_matches_at<'a>(
            &'a self,
            source: &'a str,
        ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
            self.dictionary.candidate_matches_at(source)
        }
        fn matches_at<'a>(&'a self, source: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            self.dictionary.matches_at(source)
        }
    }

    let first_checks = Arc::new(AtomicUsize::new(0));
    let first = Probe {
        dictionary: dictionary(vec![rule(1, "항복")]),
        checks: first_checks.clone(),
    };
    assert!(HanjaDictionary::has_owned_phrase_evidence(&&first));
    let boxed: Box<dyn HanjaDictionary + Send + Sync> = Box::new(first);
    assert!(HanjaDictionary::has_owned_phrase_evidence(&boxed));
    let shared = Arc::new(boxed);
    assert!(HanjaDictionary::has_owned_phrase_evidence(&shared));
    assert_eq!(first_checks.swap(0, Ordering::Relaxed), 3);

    let mut inner = ChainDictionary::<Box<dyn HanjaDictionary>>::new();
    inner.push(Box::new(shared));
    let later_checks = Arc::new(AtomicUsize::new(0));
    inner.push(Box::new(Probe {
        dictionary: dictionary(vec![rule(1, "항복")]),
        checks: later_checks.clone(),
    }));
    let mut outer = ChainDictionary::new();
    outer.push(inner);
    assert!(outer.has_owned_phrase_evidence());
    assert_eq!(first_checks.load(Ordering::Relaxed), 1);
    assert_eq!(later_checks.load(Ordering::Relaxed), 0);

    // Both hot lookup paths must use the cheap capability through every layer.
    assert!(outer.source_matches_at("無關").is_some());
    assert!(outer.source_matches_at_spellings(&["無關"]).is_some());
}

#[test]
fn owned_phrase_presence_preserves_nested_opaque_loanword_exclusion() {
    let reference = dictionary(vec![rule(1, "항복")]);
    let mut inner = ChainDictionary::new();
    inner.push_with_role(reference.clone(), SourceRole::Reference);
    assert!(inner.has_owned_phrase_evidence());
    let mut outer = ChainDictionary::new();
    outer.push_with_role(inner, SourceRole::Loanword);
    assert!(!outer.has_owned_phrase_evidence());
    assert!(outer.owned_phrase_evidence().is_empty());
    assert!(outer.source_matches_at("降服").is_none());
    assert!(outer.source_matches_at_spellings(&["降服"]).is_none());

    let mut leaf = ChainDictionary::new();
    leaf.push_with_role(reference, SourceRole::Loanword);
    assert!(leaf.has_owned_phrase_evidence());
    assert!(!leaf.owned_phrase_evidence().is_empty());
}

#[test]
fn owned_phrase_presence_default_preserves_custom_composite_evidence() {
    struct Composite(MapDictionary);
    impl HanjaDictionary for Composite {
        fn owned_phrase_evidence(&self) -> Vec<OwnedPhraseEvidence<'_>> {
            self.0.owned_phrase_evidence()
        }
        fn matches_at<'a>(&'a self, source: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            self.0.matches_at(source)
        }
    }
    let composite = Composite(dictionary(vec![rule(1, "항복")]));
    assert!(composite.phrase_evidence().is_none());
    assert!(composite.has_owned_phrase_evidence());
    let mut chain = ChainDictionary::new();
    chain.push_with_role(composite, SourceRole::Reference);
    assert!(chain.has_owned_phrase_evidence());
    assert!(chain.source_matches_at("無關").is_some());
    assert!(chain.source_matches_at_spellings(&["無關"]).is_some());
    assert!(!Composite(MapDictionary::new()).has_owned_phrase_evidence());
}

#[test]
fn variants_keep_source_owner_and_full_edge_alignment() {
    let dictionary = dictionary(vec![rule(1, "항복")]);
    // Compatibility glyph FA09 normalizes to the complete source key 降服.
    assert_eq!(
        streamed(&dictionary, &["무조건降服"], SegmentationStrategy::Lattice).0,
        "무조건항복"
    );
}

#[test]
fn clipped_committed_plaintext_still_matches_a_literal_chunk() {
    let dictionary = dictionary(vec![rule(1, "항복")]);
    let first = format!("{}。무조건 降服{}", "가".repeat(100), " 나".repeat(9));
    let last = " 끝".repeat(20);
    let full = format!("{first}{last}");
    let (expected, expected_report) =
        streamed(&dictionary, &[&full], SegmentationStrategy::Lattice);
    assert!(expected.contains("항복"));
    let (actual, report) = streamed(&dictionary, &[&first, &last], SegmentationStrategy::Lattice);
    assert_eq!(actual, expected);
    assert_eq!(report, expected_report);
}

#[test]
fn growing_fallback_run_uses_bounded_lookup_probes() {
    struct Counting {
        inner: MapDictionary,
        queries: std::cell::Cell<usize>,
    }
    impl HanjaDictionary for Counting {
        fn matches_at<'a>(&'a self, source: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            self.queries.set(self.queries.get() + 1);
            self.inner.matches_at(source)
        }
        fn phrase_evidence(&self) -> Option<&PhraseEvidence> {
            self.inner.phrase_evidence()
        }
        fn max_word_chars(&self) -> Option<usize> {
            self.inner.max_word_chars()
        }
        // Exercise the legacy/custom path without source-pool memoization,
        // making an accidental full resegmentation observable as extra lookups.
        fn source_matches_at<'a>(
            &'a self,
            _: &'a str,
        ) -> Option<Box<dyn Iterator<Item = SourceMatch> + 'a>> {
            None
        }
    }
    let dictionary = Counting {
        inner: dictionary(vec![rule(1, "항복")]),
        queries: std::cell::Cell::new(0),
    };
    let mut engine = Engine::<PlainScopeData, _>::new(&dictionary);
    for _ in 0..1000 {
        assert!(engine.push_token(InputToken::Text("龘".into())).is_empty());
    }
    assert!(
        dictionary.queries.get() < 20000,
        "{} lookups",
        dictionary.queries.get()
    );
    assert_eq!(render(engine.finish()).chars().count(), 1000);
}

fn authorize(
    dictionary: &mut MapDictionary,
    key: &str,
    anchor_reading: &str,
    anchor_suffix: Option<&str>,
    form: ReadingForm,
    kind: AuthorizationKind,
) {
    dictionary
        .set_source_evidence(DictionaryEvidence {
            schema_version: 1,
            role: SourceRole::Reference,
            entries: std::collections::BTreeMap::from([(
                key.into(),
                vec![CandidateEvidence {
                    reading: anchor_reading.into(),
                    suffix_reading: anchor_suffix.map(Into::into),
                    contributors: vec![SourceContribution {
                        word_id: WordId(10),
                        sense_id: None,
                        origin: WordOrigin::Foreign,
                    }],
                    authorized_forms: vec![AuthorizedForm {
                        form,
                        kind,
                        witnesses: vec![RelationWitness {
                            source: SourceReference::Word(WordId(10)),
                            target: SourceReference::Word(WordId(11)),
                        }],
                    }],
                    diagnostics: vec![],
                }],
            )]),
        })
        .unwrap();
}

#[test]
fn phrase_rejects_a_redirect_form_but_keeps_it_as_the_bare_default() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("車間", "차간");
    dictionary.insert("距離", "거리");
    authorize(
        &mut dictionary,
        "車間",
        "차간",
        None,
        ReadingForm {
            reading: "찻간".into(),
            suffix_reading: None,
        },
        AuthorizationKind::Redirect,
    );
    dictionary
        .set_phrase_evidence(PhraseEvidence::new(vec![PhraseRule::new(
            vec![WordId(1)],
            vec![chunk("車間", "차간"), chunk("距離", "거리")],
        )]))
        .unwrap();
    let mut chain = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    chain.push_with_role(dictionary, SourceRole::Reference);
    let dictionary = chain;
    assert_eq!(
        streamed(&dictionary, &["車間"], SegmentationStrategy::Lattice).0,
        "찻간"
    );
    for input in ["車間距離", "車間 距離"] {
        let (text, report) = streamed(&dictionary, &[input], SegmentationStrategy::Lattice);
        assert_eq!(
            text,
            if input.contains(' ') {
                "차간 거리"
            } else {
                "차간거리"
            }
        );
        assert!(
            report
                .events
                .iter()
                .any(|event| event.chosen.anchor.key == "車間"
                    && event.outcome == ResolutionOutcome::Selected
                    && event.chosen.form.reading == "차간")
        );
    }
}

#[test]
fn phrase_rejects_an_authorized_loan_form_without_changing_source_policy() {
    let mut reference = MapDictionary::new();
    reference.insert("江門", "강문");
    reference.insert("八學士", "팔학사");
    authorize(
        &mut reference,
        "江門",
        "강문",
        None,
        ReadingForm {
            reading: "장먼".into(),
            suffix_reading: None,
        },
        AuthorizationKind::Synonym,
    );
    reference
        .set_phrase_evidence(PhraseEvidence::new(vec![PhraseRule::new(
            vec![WordId(1)],
            vec![chunk("江門", "강문"), chunk("八學士", "팔학사")],
        )]))
        .unwrap();
    let mut loan = MapDictionary::new();
    loan.insert("江門", "장먼");
    let mut dictionary =
        ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    dictionary.push_with_role(reference, SourceRole::Reference);
    dictionary.push_with_role(loan, SourceRole::Loanword);
    assert_eq!(
        streamed(&dictionary, &["江門"], SegmentationStrategy::Lattice).0,
        "장먼"
    );
    assert_eq!(
        streamed(&dictionary, &["江門八學士"], SegmentationStrategy::Lattice).0,
        "강문팔학사"
    );
}

#[test]
fn phrase_requires_the_complete_attested_suffix_pair_before_positional_realization() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("甲", "갑");
    dictionary.insert_with_suffix("列", "열", "렬");
    authorize(
        &mut dictionary,
        "列",
        "열",
        Some("렬"),
        ReadingForm {
            reading: "열".into(),
            suffix_reading: Some("연".into()),
        },
        AuthorizationKind::Redirect,
    );
    let terminal = PhraseChunk::new(
        "렬".into(),
        vec![PhraseSpelling::new(
            "列".into(),
            vec![PhraseCandidateLink::new("열".into(), Some("렬".into()))],
        )],
    );
    dictionary
        .set_phrase_evidence(PhraseEvidence::new(vec![PhraseRule::new(
            vec![WordId(1)],
            vec![chunk("甲", "갑"), terminal],
        )]))
        .unwrap();
    let mut chain = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    chain.push_with_role(dictionary, SourceRole::Reference);
    let dictionary = chain;
    let (text, report) = streamed(&dictionary, &["甲列"], SegmentationStrategy::Lattice);
    assert_eq!(text, "갑렬");
    let event = report
        .events
        .iter()
        .find(|event| event.chosen.anchor.key == "列")
        .unwrap();
    assert_eq!(event.outcome, ResolutionOutcome::Selected);
    assert_eq!(event.chosen.form.reading, "열");
    assert_eq!(event.chosen.form.suffix_reading.as_deref(), Some("렬"));
    assert_eq!(
        convert_plain_text_with_options(
            "甲列",
            &dictionary,
            RenderMode::HangulOnly,
            EngineOptions {
                initial_sound_law: false,
                ..Default::default()
            }
        ),
        "갑렬"
    );
    assert_eq!(
        streamed(&dictionary, &["甲 列"], SegmentationStrategy::Lattice).0,
        "갑 열"
    );
}

#[test]
fn equivalent_authorized_suppliers_resolve_one_semantic_reading() {
    let mut reference = dictionary(vec![]);
    authorize(
        &mut reference,
        "降服",
        "항복",
        None,
        ReadingForm {
            reading: "항복".into(),
            suffix_reading: None,
        },
        AuthorizationKind::Synonym,
    );
    reference
        .set_phrase_evidence(PhraseEvidence::new(vec![rule(1, "항복")]))
        .unwrap();
    let mut loan = MapDictionary::new();
    loan.insert("降服", "항복");
    let mut dictionary =
        ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    dictionary.push_with_role(reference, SourceRole::Reference);
    dictionary.push_with_role(loan.clone(), SourceRole::Loanword);
    dictionary.push_with_role(loan, SourceRole::Loanword);
    assert_eq!(
        streamed(&dictionary, &["降服"], SegmentationStrategy::Lattice).0,
        "강복"
    );
    let (text, report) = streamed(&dictionary, &["무조건降服"], SegmentationStrategy::Lattice);
    assert_eq!(text, "무조건항복");
    let event = report
        .events
        .iter()
        .find(|event| event.chosen.anchor.key == "降服")
        .unwrap();
    assert_eq!(event.outcome, ResolutionOutcome::Selected);
    assert_eq!(event.chosen.reason, ChoiceReason::AuthorizedLoanword);
    assert_eq!(event.chosen.form_source, Some(SourceId(1)));
    assert_eq!(event.remaining.len(), 3);
}

#[test]
fn unconstrained_source_ambiguity_reports_the_unchanged_policy_default() {
    let dictionary = dictionary(vec![rule(1, "항복")]);
    let (text, report) = streamed(&dictionary, &["降服"], SegmentationStrategy::Lattice);
    assert_eq!(text, "강복");
    let event = report
        .events
        .iter()
        .find(|event| event.outcome == ResolutionOutcome::PolicyDefault)
        .unwrap();
    assert!(event.rules.is_empty());
    assert_eq!(event.chosen.anchor.reading, "강복");
    assert_eq!(event.remaining.len(), 2);
    let matched = dictionary
        .source_matches_at("降服")
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(event.chosen, *matched.select(" ", false).1.choice);
}

#[test]
fn particle_unique_source_pool_needs_no_policy_default_or_conflict_report() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert_candidates(
        "東京",
        ReadingCandidates::new(vec![candidate("도쿄", 10), candidate("동경", 0)]).unwrap(),
    );
    let mut chain = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    chain.push_with_role(dictionary, SourceRole::Reference);
    let (text, report) = streamed(&chain, &["東京을"], SegmentationStrategy::Lattice);
    assert_eq!(text, "동경을");
    assert!(report.events.is_empty());
    let (text, report) = streamed(&chain, &["東京"], SegmentationStrategy::Lattice);
    assert_eq!(text, "도쿄");
    assert_eq!(report.events.len(), 1);
    assert_eq!(report.events[0].outcome, ResolutionOutcome::PolicyDefault);
    assert!(report.events[0].rules.is_empty());
    let (_, whole_report) = streamed(&chain, &["앞말 東京"], SegmentationStrategy::Lattice);
    let (_, split_report) = streamed(&chain, &["앞말 ", "東京"], SegmentationStrategy::Lattice);
    assert_eq!(split_report, whole_report);
}

#[test]
fn unconstrained_empty_particle_pool_reports_policy_default_not_conflict() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert_candidates(
        "東京",
        ReadingCandidates::new(vec![candidate("동경", 10), candidate("경성", 0)]).unwrap(),
    );
    let mut chain = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    chain.push_with_role(dictionary, SourceRole::Reference);
    let (text, report) = streamed(&chain, &["東京를"], SegmentationStrategy::Lattice);
    assert_eq!(text, "동경를");
    assert_eq!(report.events.len(), 1);
    assert_eq!(report.events[0].outcome, ResolutionOutcome::PolicyDefault);
    assert!(report.events[0].remaining.is_empty());
    assert!(report.events[0].rules.is_empty());
}

#[test]
fn explicit_source_gloss_can_override_a_reported_phrase_resolution() {
    let dictionary = dictionary(vec![rule(1, "항복")]);
    let mut engine = Engine::<PlainScopeData, _>::new(&dictionary).with_resolution_reporting();
    let mut tokens = engine.push_token(InputToken::Text("無條件降服(강복)".into()));
    let (tail, report) = engine.finish_with_report();
    tokens.extend(tail);
    let tokens = collapse_redundant_parens(tokens, true);
    let annotation = tokens
        .iter()
        .find_map(|token| match token {
            OutputToken::Annotated(annotation) if annotation.hanja == "降服" => Some(annotation),
            _ => None,
        })
        .unwrap();
    assert_eq!(annotation.reading, "강복");
    assert!(annotation.from_source_gloss);
    assert_eq!(
        annotation.selection.as_ref().unwrap().choice.anchor.reading,
        "강복"
    );
    let event = report
        .events
        .iter()
        .find(|event| event.chosen.anchor.key == "降服")
        .unwrap();
    assert_eq!(event.outcome, ResolutionOutcome::Selected);
    assert_eq!(event.chosen.form.reading, "항복");
}

fn oversized_readings() -> ReadingCandidates {
    let mut readings: Vec<ReadingCandidate> = (0..=MAX_PHRASE_CANDIDATES)
        .map(|index| {
            let syllable = char::from_u32(0xac00 + index as u32).unwrap();
            candidate(&format!("{syllable}복"), 0)
        })
        .collect();
    readings.push(candidate("강복", 10000));
    readings.push(candidate("항복", 0));
    ReadingCandidates::new(readings).unwrap()
}

#[test]
fn oversized_pool_withholds_overlapping_rules_atomically_without_truncating_policy() {
    let mut dictionary = dictionary(vec![]);
    dictionary.insert_candidates("降服", oversized_readings());
    dictionary.insert_candidates(
        "行列",
        ReadingCandidates::new(vec![candidate("행렬", 10), candidate("항렬", 0)]).unwrap(),
    );
    dictionary
        .set_phrase_evidence(PhraseEvidence::new(vec![
            rule(1, "항복"),
            PhraseRule::new(
                vec![WordId(2)],
                vec![
                    chunk("無條件", "무조건"),
                    chunk("降服", "항복"),
                    chunk("行列", "항렬"),
                ],
            ),
        ]))
        .unwrap();
    let stored = dictionary.candidate_matches_at("降服").next().unwrap();
    assert!(stored.candidates.as_slice().len() > MAX_PHRASE_CANDIDATES);
    let (text, report) = streamed(
        &dictionary,
        &["無條件降服行列"],
        SegmentationStrategy::Lattice,
    );
    assert_eq!(text, "무조건강복행렬");
    assert!(!report.events.is_empty());
    assert!(
        report
            .events
            .iter()
            .all(|event| event.outcome == ResolutionOutcome::Limit && event.remaining.is_empty())
    );
    assert!(
        report
            .events
            .iter()
            .any(|event| event.chosen.anchor.key == "行列")
    );
    let mut engine = Engine::<PlainScopeData, _>::new(&dictionary);
    let mut output = engine.push_token(InputToken::Text("無條件降服行列".into()));
    let (tail, report) = engine.finish_with_report();
    output.extend(tail);
    assert_eq!(render(output), text);
    assert!(report.events.is_empty());
    assert_eq!(
        dictionary
            .candidate_matches_at("降服")
            .next()
            .unwrap()
            .candidates,
        stored.candidates
    );
}

#[test]
fn policy_reporting_alone_limits_oversized_pool_without_enumerating_remaining_choices() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert_candidates("降服", oversized_readings());
    let mut chain = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceAuthorized);
    chain.push_with_role(dictionary, SourceRole::Reference);
    let (text, report) = streamed(&chain, &["降服"], SegmentationStrategy::Lattice);
    assert_eq!(text, "강복");
    assert_eq!(report.events.len(), 1);
    assert_eq!(report.events[0].outcome, ResolutionOutcome::Limit);
    assert!(report.events[0].remaining.is_empty());
    assert!(report.events[0].rules.is_empty());
    assert_eq!(report.events[0].chosen.anchor.reading, "강복");
}

struct NoMax(MapDictionary);
impl HanjaDictionary for NoMax {
    fn matches_at<'a>(&'a self, source: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        self.0.matches_at(source)
    }
    fn candidate_matches_at<'a>(
        &'a self,
        source: &'a str,
    ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
        self.0.candidate_matches_at(source)
    }
    fn source_matches_at<'a>(
        &'a self,
        source: &'a str,
    ) -> Option<Box<dyn Iterator<Item = SourceMatch> + 'a>> {
        self.0.source_matches_at(source)
    }
    fn phrase_evidence(&self) -> Option<&PhraseEvidence> {
        self.0.phrase_evidence()
    }
    fn phrase_index(&self) -> Option<&PhraseIndex> {
        self.0.phrase_index()
    }
    fn max_word_chars(&self) -> Option<usize> {
        None
    }
}

#[test]
fn unknown_word_maximum_still_emits_delimited_plaintext_before_a_late_phrase() {
    let dictionary = NoMax(dictionary(vec![rule(1, "항복")]));
    let prefix = "평문 ".repeat(2000);
    let phrase = "무조건 降服";
    let expected = format!("{prefix}무조건 항복");
    for split in phrase
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(core::iter::once(phrase.len()))
    {
        let mut engine = Engine::<PlainScopeData, _>::new(&dictionary).with_resolution_reporting();
        let mut output = Vec::new();
        for _ in 0..2000 {
            output.extend(engine.push_token(InputToken::Text("평문 ".into())));
            assert!(engine.buffered_chars() < 40);
        }
        assert!(!output.is_empty());
        output.extend(engine.push_token(InputToken::Text(phrase[..split].into())));
        output.extend(engine.push_token(InputToken::Text(phrase[split..].into())));
        let (tail, report) = engine.finish_with_report();
        output.extend(tail);
        assert_eq!(render(output), expected, "split={split}");
        assert!(
            report
                .events
                .iter()
                .any(|event| event.outcome == ResolutionOutcome::Selected
                    && event.chosen.anchor.reading == "항복")
        );
    }
}

#[test]
fn unknown_word_maximum_does_not_retain_all_delimited_hanja_phrases() {
    let dictionary = NoMax(dictionary(vec![rule(1, "항복")]));
    let mut engine = Engine::<PlainScopeData, _>::new(&dictionary);
    let mut output = Vec::new();
    for _ in 0..1000 {
        output.extend(engine.push_token(InputToken::Text("無條件 降服 ".into())));
        assert!(engine.buffered_chars() < 40);
    }
    assert!(!output.is_empty());
    output.extend(engine.finish());
    assert_eq!(render(output), "무조건 항복 ".repeat(1000));
}

#[test]
fn unknown_word_maximum_preserves_unfinished_long_hanja_keys() {
    let key = format!("降服{}末", "龘".repeat(40));
    let mut dictionary = dictionary(vec![]);
    dictionary.insert(&key, "긴말");
    dictionary
        .set_phrase_evidence(PhraseEvidence::new(vec![rule(1, "항복")]))
        .unwrap();
    let dictionary = NoMax(dictionary);
    let mut engine = Engine::<PlainScopeData, _>::new(&dictionary);
    let split = key.char_indices().nth(25).unwrap().0;
    assert!(
        engine
            .push_token(InputToken::Text(key[..split].into()))
            .is_empty()
    );
    assert!(engine.buffered_chars() > 20);
    let mut output = engine.push_token(InputToken::Text(key[split..].into()));
    output.extend(engine.finish());
    assert_eq!(render(output), "긴말");
}

#[test]
fn phrase_sources_preserve_exact_first_and_reject_ambiguous_variants() {
    for (input, first, first_reading, second, second_reading) in [
        ("弁", "辨", "변", "瓣", "판"),
        ("发", "發", "발", "髮", "발"),
    ] {
        let mut dictionary = dictionary(vec![rule(1, "항복")]);
        dictionary.insert(first, first_reading);
        dictionary.insert(second, second_reading);
        dictionary
            .set_phrase_evidence(PhraseEvidence::new(vec![rule(1, "항복")]))
            .unwrap();
        assert!(
            dictionary
                .source_matches_at_spellings(&[input, first, second])
                .unwrap()
                .is_empty()
        );
        assert!(
            dictionary
                .source_matches_at(input)
                .unwrap()
                .next()
                .is_none()
        );
        for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
            let tokens = process_tokens_with_options(
                vec![InputToken::<PlainScopeData>::Text(input.into())],
                &dictionary,
                EngineOptions {
                    segmentation: strategy,
                    ..Default::default()
                },
            );
            if input == "弁" {
                let OutputToken::Annotated(annotation) = &tokens[0] else {
                    panic!("expected fallback annotation")
                };
                assert_eq!(annotation.reading, "변");
                assert!(!annotation.from_dictionary);
                assert_eq!(annotation.dictionary_hanja, None);
                assert_eq!(annotation.selection, None);
            } else {
                assert_eq!(tokens, vec![OutputToken::Text(input.into())]);
            }
        }
        dictionary.insert(input, "정");
        dictionary
            .set_phrase_evidence(PhraseEvidence::new(vec![rule(1, "항복")]))
            .unwrap();
        let matches = dictionary
            .source_matches_at_spellings(&[input, first, second])
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, 0);
        assert_eq!(matches[0].1.byte_len, input.len());
        assert_eq!(matches[0].1.default_choice().anchor.key, input);
        for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
            let tokens = process_tokens_with_options(
                vec![InputToken::<PlainScopeData>::Text(input.into())],
                &dictionary,
                EngineOptions {
                    segmentation: strategy,
                    ..Default::default()
                },
            );
            let OutputToken::Annotated(annotation) = &tokens[0] else {
                panic!("expected dictionary annotation")
            };
            assert_eq!(annotation.reading, "정");
            assert!(annotation.from_dictionary);
            assert_eq!(annotation.dictionary_hanja.as_deref(), Some(input));
            assert_eq!(
                annotation.selection.as_ref().unwrap().choice.anchor.key,
                input
            );
        }
    }
}

#[test]
fn forced_and_inline_boundaries_do_not_prove_a_terminal_phrase_word_boundary() {
    #[derive(Clone)]
    struct InlineScope(bool);
    impl ScopeData for InlineScope {
        fn is_preserve(&self) -> bool {
            self.0
        }
    }
    let dictionary = dictionary(vec![rule(1, "항복")]);
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        assert_eq!(
            streamed(&dictionary, &["無條件降服하다"], strategy).0,
            "무조건강복하다"
        );
        for boundary in ["open", "close", "verbatim", "preserve", "flush"] {
            let mut engine = Engine::<InlineScope, _>::with_options(
                &dictionary,
                EngineOptions {
                    segmentation: strategy,
                    ..Default::default()
                },
            );
            let mut output = Vec::new();
            if boundary == "close" {
                output.extend(engine.push_token(InputToken::Open(Scope::new(InlineScope(false)))));
            }
            output.extend(engine.push_token(InputToken::Text("無條件降服".into())));
            match boundary {
                "open" | "preserve" => {
                    output.extend(engine.push_token(InputToken::Open(Scope::new(InlineScope(
                        boundary == "preserve",
                    )))));
                    output.extend(engine.push_token(InputToken::Text("하다".into())));
                    output.extend(engine.push_token(InputToken::Close));
                }
                "close" => {
                    output.extend(engine.push_token(InputToken::Close));
                    output.extend(engine.push_token(InputToken::Text("하다".into())));
                }
                "verbatim" => output.extend(engine.push_token(InputToken::Verbatim("하다".into()))),
                "flush" => {
                    output.extend(engine.flush());
                    output.extend(engine.push_token(InputToken::Text("하다".into())));
                }
                _ => unreachable!(),
            }
            output.extend(engine.finish());
            assert_eq!(render(output), "무조건강복하다", "{strategy:?}, {boundary}");
        }
    }
}

#[test]
fn eof_or_a_textual_space_still_proves_a_terminal_phrase_boundary() {
    let dictionary = dictionary(vec![rule(1, "항복")]);
    for strategy in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
        let options = EngineOptions {
            segmentation: strategy,
            ..Default::default()
        };
        let mut engine = Engine::<PlainScopeData, _>::with_options(&dictionary, options);
        let mut output = engine.push_token(InputToken::Text("無條件降服".into()));
        output.extend(engine.finish());
        assert_eq!(render(output), "무조건항복");
        let mut engine = Engine::<PlainScopeData, _>::with_options(&dictionary, options);
        let mut output = engine.push_token(InputToken::Text("無條件降服 ".into()));
        output.extend(engine.flush());
        output.extend(engine.push_token(InputToken::Text("하다".into())));
        output.extend(engine.finish());
        assert_eq!(render(output), "무조건항복 하다");
        let mut engine = Engine::<PlainScopeData, _>::with_options(&dictionary, options);
        let mut output = engine.push_token(InputToken::Open(Scope::new(PlainScopeData)));
        output.extend(engine.push_token(InputToken::Text("無條件降服".into())));
        output.extend(engine.push_token(InputToken::Close));
        output.extend(engine.finish());
        assert_eq!(render(output), "무조건항복");
    }
}
