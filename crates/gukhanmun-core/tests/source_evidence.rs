// Gukhanmun: Regression tests for typed candidate source evidence.
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

use std::collections::BTreeMap;
use std::sync::Arc;

use gukhanmun_core::{
    AuthorizationKind, AuthorizedForm, CandidateEvidence, DictionaryEvidence, EvidenceDiagnostic,
    EvidenceDiagnosticKind, EvidenceError, HanjaDictionary, MapDictionary, ReadingForm,
    RelationWitness, SenseId, SourceContribution, SourceReference, SourceRole, WordId, WordOrigin,
};

fn dictionary() -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    dictionary.insert_with_suffix("霧川", "무천", "무쳔");
    dictionary.insert("靑圖", "청도");
    dictionary
}

fn evidence() -> DictionaryEvidence {
    DictionaryEvidence {
        schema_version: 1,
        role: SourceRole::Reference,
        entries: BTreeMap::from([(
            "霧川".into(),
            vec![CandidateEvidence {
                reading: "무천".into(),
                suffix_reading: Some("무쳔".into()),
                contributors: vec![SourceContribution {
                    word_id: WordId(7),
                    sense_id: Some(SenseId(7)),
                    origin: WordOrigin::Foreign,
                }],
                authorized_forms: vec![AuthorizedForm {
                    form: ReadingForm {
                        reading: "누벨 리브".into(),
                        suffix_reading: None,
                    },
                    kind: AuthorizationKind::Redirect,
                    witnesses: vec![RelationWitness {
                        source: SourceReference::Sense(SenseId(7)),
                        target: SourceReference::Word(WordId(7)),
                    }],
                }],
                diagnostics: vec![EvidenceDiagnostic {
                    kind: EvidenceDiagnosticKind::MissingTarget,
                    source: Some(SourceReference::Sense(SenseId(8))),
                    message: "Another source relation has no target".into(),
                }],
            }],
        )]),
    }
}

#[test]
fn typed_ids_preserve_distinct_namespaces() {
    assert_ne!(
        SourceReference::Word(WordId(7)),
        SourceReference::Sense(SenseId(7))
    );
    assert_eq!(
        evidence().entries["霧川"][0].authorized_forms[0]
            .form
            .reading,
        "누벨 리브"
    );
}

#[test]
fn partial_evidence_does_not_change_legacy_candidates() {
    let mut dictionary = dictionary();
    let before: Vec<_> = dictionary.candidate_entries().unwrap().collect();
    let before_matches: Vec<_> = dictionary.matches_at("霧川을").collect();
    dictionary.set_source_evidence(evidence()).unwrap();
    assert_eq!(
        dictionary.candidate_entries().unwrap().collect::<Vec<_>>(),
        before
    );
    assert_eq!(
        dictionary.matches_at("霧川을").collect::<Vec<_>>(),
        before_matches
    );
    assert_eq!(dictionary.source_evidence(), Some(&evidence()));
}

#[test]
fn validation_requires_exact_key_reading_and_suffix() {
    let dictionary = dictionary();
    let mut wrong_key = evidence();
    let candidates = wrong_key.entries.remove("霧川").unwrap();
    wrong_key.entries.insert("霧河".into(), candidates);
    assert_eq!(
        wrong_key.validate_records(dictionary.candidate_entries().unwrap()),
        Err(EvidenceError::UnknownKey("霧河".into()))
    );
    for field in ["reading", "suffix", "absent_suffix"] {
        let mut wrong = evidence();
        let candidate = &mut wrong.entries.get_mut("霧川").unwrap()[0];
        match field {
            "reading" => candidate.reading = "무쳔".into(),
            "suffix" => candidate.suffix_reading = Some("무천".into()),
            _ => candidate.suffix_reading = None,
        }
        assert_eq!(
            wrong.validate_records(dictionary.candidate_entries().unwrap()),
            Err(EvidenceError::UnknownCandidate("霧川".into()))
        );
    }
}

#[test]
fn rejected_replacement_preserves_valid_evidence() {
    let mut dictionary = dictionary();
    dictionary.set_source_evidence(evidence()).unwrap();
    let mut rejected = evidence();
    rejected.schema_version = 2;
    assert_eq!(
        dictionary.set_source_evidence(rejected),
        Err(EvidenceError::UnsupportedSchemaVersion(2))
    );
    assert_eq!(dictionary.source_evidence(), Some(&evidence()));
}

#[test]
fn edits_invalidate_only_evidence_of_the_edited_key() {
    let mut dictionary = dictionary();
    dictionary.set_source_evidence(evidence()).unwrap();
    dictionary.insert("靑圖", "청토");
    assert_eq!(dictionary.source_evidence(), Some(&evidence()));
    dictionary.insert("霧川", "무하");
    let retained = dictionary.source_evidence().unwrap();
    assert_eq!(retained.role, SourceRole::Reference);
    assert!(retained.entries.is_empty());
    retained
        .validate_records(dictionary.candidate_entries().unwrap())
        .unwrap();
}

#[test]
fn malformed_evidence_is_rejected() {
    let mut missing_contributors = evidence();
    missing_contributors.entries.get_mut("霧川").unwrap()[0]
        .contributors
        .clear();
    assert_eq!(
        missing_contributors.validate(),
        Err(EvidenceError::EmptyContributors)
    );

    let mut missing_witness = evidence();
    missing_witness.entries.get_mut("霧川").unwrap()[0].authorized_forms[0]
        .witnesses
        .clear();
    assert_eq!(
        missing_witness.validate(),
        Err(EvidenceError::EmptyWitnesses)
    );

    let mut duplicate = evidence();
    let candidate = duplicate.entries["霧川"][0].clone();
    duplicate.entries.get_mut("霧川").unwrap().push(candidate);
    assert_eq!(duplicate.validate(), Err(EvidenceError::Duplicate));

    let mut conflicting_origin = evidence();
    let candidate = &mut conflicting_origin.entries.get_mut("霧川").unwrap()[0];
    let mut contributor = candidate.contributors[0].clone();
    contributor.origin = WordOrigin::Native;
    candidate.contributors.push(contributor);
    assert_eq!(conflicting_origin.validate(), Err(EvidenceError::Duplicate));
}

#[test]
fn wrappers_forward_borrowed_evidence_and_full_candidates() {
    fn check(dictionary: &dyn HanjaDictionary) {
        assert_eq!(dictionary.source_evidence(), Some(&evidence()));
        assert_eq!(
            dictionary
                .candidate_matches_at("霧川")
                .next()
                .unwrap()
                .candidates
                .as_slice()[0]
                .suffix_reading
                .as_deref(),
            Some("무쳔")
        );
    }
    let mut dictionary = dictionary();
    dictionary.set_source_evidence(evidence()).unwrap();
    let reference = &dictionary;
    check(&reference);
    let boxed: Box<dyn HanjaDictionary> = Box::new(dictionary);
    check(&boxed);
    let shared: Arc<dyn HanjaDictionary> = Arc::from(boxed);
    check(&shared);
    assert!(std::ptr::eq(
        shared.source_evidence().unwrap(),
        shared.as_ref().source_evidence().unwrap()
    ));
}

#[cfg(feature = "serde")]
#[test]
fn serialized_evidence_preserves_typed_witnesses_and_requires_link_validation() {
    let dictionary = dictionary();
    let encoded = serde_json::to_string(&evidence()).unwrap();
    let decoded: DictionaryEvidence = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, evidence());
    decoded
        .validate_records(dictionary.candidate_entries().unwrap())
        .unwrap();
    assert_eq!(serde_json::to_string(&decoded).unwrap(), encoded);
    let mut malformed: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    malformed["entries"]["霧川"][0]["suffix_reading"] = serde_json::Value::Null;
    let decoded: DictionaryEvidence = serde_json::from_value(malformed).unwrap();
    assert_eq!(
        decoded.validate_records(dictionary.candidate_entries().unwrap()),
        Err(EvidenceError::UnknownCandidate("霧川".into()))
    );
}

#[cfg(feature = "serde")]
#[test]
fn serialized_evidence_fails_closed_on_unknown_fields_and_enum_variants() {
    let value = serde_json::to_value(evidence()).unwrap();
    assert_eq!(value["role"], "reference");
    let witness = &value["entries"]["霧川"][0]["authorized_forms"][0]["witnesses"][0];
    assert_eq!(witness["source"], serde_json::json!({ "sense": 7 }));
    assert_eq!(witness["target"], serde_json::json!({ "word": 7 }));
    let mut extra_field = value.clone();
    extra_field["new_critical_evidence"] = serde_json::json!(true);
    assert!(serde_json::from_value::<DictionaryEvidence>(extra_field).is_err());
    let mut unknown_role = value;
    unknown_role["role"] = serde_json::json!("guessed");
    assert!(serde_json::from_value::<DictionaryEvidence>(unknown_role).is_err());
}

#[cfg(feature = "serde")]
#[test]
fn compact_transport_roundtrips_and_rejects_ambiguous_structure() {
    struct Payload(DictionaryEvidence);
    impl serde::Serialize for Payload {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            gukhanmun_core::serialize_dictionary_evidence(&self.0, serializer)
        }
    }
    impl<'de> serde::Deserialize<'de> for Payload {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            gukhanmun_core::deserialize_dictionary_evidence(deserializer).map(Self)
        }
    }
    let encoded = serde_json::to_string(&Payload(evidence())).unwrap();
    let decoded: Payload = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.0, evidence());
    assert!(encoded.starts_with("[1,\"reference\",{"));
    let mut value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    value.as_array_mut().unwrap().push(serde_json::Value::Null);
    assert!(serde_json::from_value::<Payload>(value).is_err());
    assert!(serde_json::from_str::<Payload>("[1,\"reference\",{\"字\":[],\"字\":[]}]").is_err());
    assert!(serde_json::from_str::<Payload>("[1,\"guessed\",{}]").is_err());
}
