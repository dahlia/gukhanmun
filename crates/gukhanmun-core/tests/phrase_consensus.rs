// Gukhanmun: Regression coverage for independently owned phrase consensus.
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

fn dictionary(attested: Option<&str>) -> MapDictionary {
    let mut d = MapDictionary::new();
    d.insert_candidates(
        "降服",
        ReadingCandidates::new(vec![
            ReadingCandidate {
                reading: "강복".into(),
                suffix_reading: None,
                mark: MatchMark::default(),
                preference: 10,
                alias: None,
            },
            ReadingCandidate {
                reading: "항복".into(),
                suffix_reading: None,
                mark: MatchMark::default(),
                preference: 0,
                alias: None,
            },
        ])
        .unwrap(),
    );
    if let Some(reading) = attested {
        d.set_phrase_evidence(PhraseEvidence::new(vec![PhraseRule::new(
            vec![WordId(1)],
            vec![
                PhraseChunk::new(
                    "무조건".into(),
                    vec![PhraseSpelling::new("無條件".into(), vec![])],
                ),
                PhraseChunk::new(
                    reading.into(),
                    vec![PhraseSpelling::new(
                        "降服".into(),
                        vec![PhraseCandidateLink::new(reading.into(), None)],
                    )],
                ),
            ],
        )]))
        .unwrap();
    }
    d
}
fn convert(d: &impl HanjaDictionary, chunks: &[&str]) -> (String, ResolutionReport) {
    let mut engine = Engine::<PlainScopeData, _>::new(d).with_resolution_reporting();
    let mut output = vec![];
    for chunk in chunks {
        output.extend(engine.push_token(InputToken::Text((*chunk).into())));
    }
    let (tail, report) = engine.finish_with_report();
    output.extend(tail);
    let text = output
        .into_iter()
        .filter_map(|t| match t {
            OutputToken::Text(t) | OutputToken::Verbatim(t) => Some(t),
            OutputToken::Annotated(a) => Some(a.reading),
            _ => None,
        })
        .collect();
    (text, report)
}
fn target(report: &ResolutionReport) -> &ResolutionEvent {
    report
        .events
        .iter()
        .find(|e| e.chosen.anchor.key == "降服")
        .unwrap()
}
#[test]
fn identical_independently_owned_attestations_select_the_same_semantic_choice() {
    let reference = dictionary(Some("항복"));
    let mut sources = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
    sources.push_with_role(reference.clone(), SourceRole::Reference);
    sources.push_with_role(reference, SourceRole::Reference);
    let input = "무조건降服";
    let (expected, expected_report) = convert(&sources, &[input]);
    assert_eq!(expected, "무조건항복");
    let event = target(&expected_report);
    assert_eq!(event.outcome, ResolutionOutcome::Selected);
    assert_eq!(event.rules.len(), 2);
    assert_eq!(event.chosen.anchor.source_id, SourceId(0));
    assert_eq!(event.remaining.len(), 2);
    for split in input.char_indices().map(|(at, _)| at).chain([input.len()]) {
        let (text, report) = convert(&sources, &[&input[..split], &input[split..]]);
        assert_eq!(text, expected, "split={split}");
        assert_eq!(report, expected_report, "split={split}");
    }
}
#[test]
fn independently_owned_disagreeing_attestations_still_conflict() {
    for agreeing_sources in [1, 2] {
        let mut sources = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
        for _ in 0..agreeing_sources {
            sources.push_with_role(dictionary(Some("항복")), SourceRole::Reference);
        }
        sources.push_with_role(dictionary(Some("강복")), SourceRole::Reference);
        let (text, report) = convert(&sources, &["무조건降服"]);
        assert_eq!(text, "무조건강복");
        assert_eq!(target(&report).outcome, ResolutionOutcome::Conflict);
        assert!(target(&report).remaining.is_empty());
    }
}

#[test]
fn equivalent_unwitnessed_higher_reference_never_borrows_a_lower_sources_proof() {
    let mut sources = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
    sources.push_with_role(dictionary(None), SourceRole::Reference);
    sources.push_with_role(dictionary(Some("항복")), SourceRole::Reference);
    sources.push_with_role(dictionary(Some("항복")), SourceRole::Reference);
    let (text, report) = convert(&sources, &["무조건降服"]);
    assert_eq!(text, "무조건항복");
    let event = target(&report);
    assert_eq!(event.outcome, ResolutionOutcome::Selected);
    assert_eq!(event.chosen.anchor.source_id, SourceId(1));
    assert!(
        event
            .remaining
            .iter()
            .all(|choice| choice.anchor.source_id != SourceId(0))
    );
}
#[test]
fn incompatible_override_invalidates_every_lower_phrase_rule() {
    let mut sources = ChainDictionary::new().with_source_policy(SourcePolicy::ReferenceFirst);
    let mut authored = MapDictionary::new();
    authored.insert("降服", "강복");
    sources.push(authored);
    sources.push_with_role(dictionary(Some("항복")), SourceRole::Reference);
    sources.push_with_role(dictionary(Some("항복")), SourceRole::Reference);
    let (text, report) = convert(&sources, &["무조건降服"]);
    assert_eq!(text, "무조건강복");
    assert!(report.events.iter().all(|event| event.rules.is_empty()));
}
