// Gukhanmun: Tests critical dictionary evidence transport and candidate linkage.
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
use std::fs;

use ciborium::value::Value;
use gukhanmun_core::{
    AuthorizationKind, AuthorizedForm, CandidateEvidence, DictionaryEvidence, HanjaDictionary,
    ReadingForm, RelationWitness, SenseId, SourceContribution, SourceReference, SourceRole, WordId,
    WordOrigin,
};
use gukhanmun_mkdict::{
    BuildOptions, DictionaryFormat, build_dictionary, build_dictionary_with_evidence,
};
use tempfile::TempDir;

fn evidence() -> DictionaryEvidence {
    DictionaryEvidence {
        schema_version: 1,
        role: SourceRole::Reference,
        entries: BTreeMap::from([(
            "玄林".into(),
            vec![CandidateEvidence {
                reading: "현림".into(),
                suffix_reading: None,
                contributors: vec![SourceContribution {
                    word_id: WordId(7),
                    sense_id: Some(SenseId(8)),
                    origin: WordOrigin::Foreign,
                }],
                authorized_forms: vec![AuthorizedForm {
                    form: ReadingForm {
                        reading: "새이름".into(),
                        suffix_reading: None,
                    },
                    kind: AuthorizationKind::Synonym,
                    witnesses: vec![RelationWitness {
                        source: SourceReference::Sense(SenseId(8)),
                        target: SourceReference::Sense(SenseId(7)),
                    }],
                }],
                diagnostics: vec![],
            }],
        )]),
    }
}

fn build(
    format: DictionaryFormat,
    source: Option<&DictionaryEvidence>,
) -> (TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.tsv");
    let output = temp.path().join("output.dict");
    fs::write(&input, "hanja\thangul\trequire_hanja\trequire_hangul\tpreference\n玄林\t현림\ttrue\tfalse\t2\n玄林\t검은숲\tfalse\ttrue\t0\n").unwrap();
    let options = BuildOptions {
        format,
        validate: true,
        metadata: BTreeMap::from([("build_date".into(), "1970-01-01T00:00:00Z".into())]),
        ..BuildOptions::default()
    };
    if let Some(source) = source {
        build_dictionary_with_evidence(&[input], &output, &options, source).unwrap();
    } else {
        build_dictionary(&[input], &output, &options).unwrap();
    }
    (temp, output)
}

#[test]
fn both_containers_preserve_critical_evidence_and_unchanged_candidates() {
    let source = evidence();
    for format in [DictionaryFormat::Fst, DictionaryFormat::Cdb] {
        let (_temp, output) = build(format, Some(&source));
        let dictionary: Box<dyn HanjaDictionary> = match format {
            DictionaryFormat::Fst => {
                let d = gukhanmun_fst::FstDictionary::open(&output).unwrap();
                assert_eq!(d.metadata()["version"], "4");
                Box::new(d)
            }
            DictionaryFormat::Cdb => {
                let d = gukhanmun_cdb::CdbDictionary::open(&output).unwrap();
                assert_eq!(d.metadata()["version"], "4");
                Box::new(d)
            }
        };
        assert_eq!(dictionary.source_evidence(), Some(&source));
        let actual = dictionary
            .candidate_matches_at("玄林")
            .next()
            .unwrap()
            .candidates;
        assert_eq!(actual.as_slice().len(), 2);
        assert_eq!(actual.as_slice()[0].reading, "현림");
        assert!(actual.as_slice()[0].mark.require_hanja);
        assert!(actual.as_slice()[1].mark.require_hangul);
    }
}

#[test]
fn evidence_free_builds_remain_old_containers_with_unknown_evidence() {
    for format in [DictionaryFormat::Fst, DictionaryFormat::Cdb] {
        let (_temp, output) = build(format, None);
        match format {
            DictionaryFormat::Fst => {
                let d = gukhanmun_fst::FstDictionary::open(output).unwrap();
                assert_eq!(d.metadata()["version"], "2");
                assert!(d.source_evidence().is_none());
            }
            DictionaryFormat::Cdb => {
                let d = gukhanmun_cdb::CdbDictionary::open(output).unwrap();
                assert_eq!(d.metadata()["version"], "2");
                assert!(d.source_evidence().is_none());
            }
        }
    }
}

// Rewrite only FST metadata, preserving the map and canonical reading payload.
fn rewrite_fst(bytes: &[u8], edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let offset = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize;
    let metadata_start = offset(16);
    let metadata_len = offset(24);
    let old_fst = offset(32);
    let fst_len = offset(40);
    let old_readings = offset(48);
    let mut value: Value =
        ciborium::de::from_reader(&bytes[metadata_start..metadata_start + metadata_len]).unwrap();
    edit(&mut value);
    let mut encoded = vec![];
    ciborium::ser::into_writer(&value, &mut encoded).unwrap();
    let mut result = bytes[..64].to_vec();
    result[24..32].copy_from_slice(&(encoded.len() as u64).to_le_bytes());
    result[32..40].copy_from_slice(&((64 + encoded.len()) as u64).to_le_bytes());
    result[48..56].copy_from_slice(&((64 + encoded.len() + fst_len) as u64).to_le_bytes());
    result.extend(encoded);
    result.extend_from_slice(&bytes[old_fst..old_fst + fst_len]);
    result.extend_from_slice(&bytes[old_readings..]);
    result
}

#[test]
fn readers_reject_missing_critical_evidence_and_fst_dangling_references() {
    let source = evidence();
    let (_temp, output) = build(DictionaryFormat::Fst, Some(&source));
    let bytes = fs::read(output).unwrap();
    let missing = rewrite_fst(&bytes, |value| {
        if let Value::Map(fields) = value {
            fields.retain(|(key, _)| key != &Value::Text("candidate_evidence".into()));
        }
    });
    assert!(matches!(
        gukhanmun_fst::FstDictionary::from_bytes(&missing),
        Err(gukhanmun_fst::Error::SourceEvidence { .. })
    ));
    let dangling = rewrite_fst(&bytes, |value| {
        let mut changed = source.clone();
        changed.entries.get_mut("玄林").unwrap()[0].reading = "未登載".into();
        if let Value::Map(fields) = value {
            for (key, v) in fields {
                if key == &Value::Text("candidate_evidence".into()) {
                    *v = encoded_evidence(&changed);
                }
            }
        }
    });
    assert!(matches!(
        gukhanmun_fst::FstDictionary::from_bytes(&dangling),
        Err(gukhanmun_fst::Error::SourceEvidence { .. })
    ));
    // Changing the old format tag to 4 must not silently fabricate evidence.
    let (_temp, output) = build(DictionaryFormat::Cdb, None);
    let mut old = fs::read(output).unwrap();
    let marker = b"gversiona2";
    let at = old
        .windows(marker.len())
        .position(|part| part == marker)
        .unwrap();
    old[at + marker.len() - 1] = b'4';
    assert!(matches!(
        gukhanmun_cdb::CdbDictionary::from_bytes(&old),
        Err(gukhanmun_cdb::Error::SourceEvidence { .. })
    ));
}

#[test]
fn builder_rejects_stale_evidence_before_creating_output() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.tsv");
    fs::write(&input, "hanja\thangul\n玄林\t다른독음\n").unwrap();
    for format in [DictionaryFormat::Fst, DictionaryFormat::Cdb] {
        let output = temp.path().join("must-not-exist.dict");
        assert!(
            build_dictionary_with_evidence(
                std::slice::from_ref(&input),
                &output,
                &BuildOptions {
                    format,
                    ..BuildOptions::default()
                },
                &evidence()
            )
            .is_err()
        );
        assert!(!output.exists());
    }
}

#[test]
fn cli_reads_evidence_json_and_validates_the_result() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.tsv");
    let source = temp.path().join("evidence.json");
    let output = temp.path().join("output.fst");
    fs::write(&input, "hanja\thangul\n玄林\t현림\n").unwrap();
    fs::write(&source, serde_json::to_vec(&evidence()).unwrap()).unwrap();
    assert_cmd::Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .arg("--evidence")
        .arg(&source)
        .arg("--output")
        .arg(&output)
        .arg("--validate")
        .arg(&input)
        .assert()
        .success();
    let dictionary = gukhanmun_fst::FstDictionary::open(output).unwrap();
    assert_eq!(dictionary.source_evidence(), Some(&evidence()));
}

#[test]
fn cdb_rejects_stale_or_unknown_schema_evidence() {
    let (_temp, output) = build(DictionaryFormat::Cdb, Some(&evidence()));
    let database = cdb::CDB::open(&output).unwrap();
    let records = database.iter().collect::<Result<Vec<_>, _>>().unwrap();
    for bad_schema in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let malformed = temp.path().join("malformed.cdb");
        let mut writer = cdb::CDBWriter::create(malformed.to_str().unwrap()).unwrap();
        for (key, value) in &records {
            let value = if key == b"__gukhanmun_meta__" {
                let mut metadata: Value = ciborium::de::from_reader(value.as_slice()).unwrap();
                if let Value::Map(fields) = &mut metadata {
                    let (_, encoded) = fields
                        .iter_mut()
                        .find(|(key, _)| key == &Value::Text("candidate_evidence".into()))
                        .unwrap();
                    let mut changed = evidence();
                    if bad_schema {
                        changed.schema_version = 99;
                    } else {
                        changed.entries.get_mut("玄林").unwrap()[0].suffix_reading =
                            Some("다름".into());
                    }
                    *encoded = encoded_evidence(&changed);
                }
                let mut encoded = vec![];
                ciborium::ser::into_writer(&metadata, &mut encoded).unwrap();
                encoded
            } else {
                value.clone()
            };
            writer.add(key, &value).unwrap();
        }
        writer.finish().unwrap();
        assert!(matches!(
            gukhanmun_cdb::CdbDictionary::open(malformed),
            Err(gukhanmun_cdb::Error::SourceEvidence { .. })
        ));
    }
}

struct EvidencePayload<'a>(&'a DictionaryEvidence);
impl serde::Serialize for EvidencePayload<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        gukhanmun_core::serialize_dictionary_evidence(self.0, serializer)
    }
}
fn encoded_evidence(evidence: &DictionaryEvidence) -> Value {
    Value::serialized(&EvidencePayload(evidence)).unwrap()
}
