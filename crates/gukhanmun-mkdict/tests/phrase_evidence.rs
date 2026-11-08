// Gukhanmun: Tests embedded phrase evidence and conservative transport validation.
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

use ciborium::value::Value;
use gukhanmun_core::{
    HanjaDictionary, PhraseCandidateLink, PhraseChunk, PhraseEvidence, PhraseRule, PhraseSpelling,
    WordId,
};
use gukhanmun_mkdict::{BuildOptions, DictionaryFormat, build_dictionary_with_all_evidence};
use std::fs;

fn evidence() -> PhraseEvidence {
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
                    "降伏".into(),
                    vec![PhraseCandidateLink::new("항복".into(), None)],
                )],
            ),
        ],
    )])
}
fn build(format: DictionaryFormat) -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.tsv");
    let output = temp.path().join("output.dict");
    fs::write(&input, "hanja\thangul\n降伏\t항복\n").unwrap();
    build_dictionary_with_all_evidence(
        &[input],
        &output,
        &BuildOptions {
            format,
            validate: true,
            ..BuildOptions::default()
        },
        None,
        Some(&evidence()),
    )
    .unwrap();
    (temp, output)
}
fn open(
    format: DictionaryFormat,
    path: &std::path::Path,
) -> Result<Box<dyn HanjaDictionary>, String> {
    match format {
        DictionaryFormat::Fst => gukhanmun_fst::FstDictionary::open(path)
            .map(|d| Box::new(d) as Box<dyn HanjaDictionary>)
            .map_err(|e| e.to_string()),
        DictionaryFormat::Cdb => gukhanmun_cdb::CdbDictionary::open(path)
            .map(|d| Box::new(d) as Box<dyn HanjaDictionary>)
            .map_err(|e| e.to_string()),
    }
}
fn rewrite(
    format: DictionaryFormat,
    path: &std::path::Path,
    edit: impl FnOnce(&mut Value),
    trailing: bool,
) {
    let transform = |bytes: &[u8]| {
        let mut value: Value = ciborium::de::from_reader(bytes).unwrap();
        edit(&mut value);
        let mut encoded = vec![];
        ciborium::ser::into_writer(&value, &mut encoded).unwrap();
        if trailing {
            encoded.push(0);
        }
        encoded
    };
    match format {
        DictionaryFormat::Fst => {
            let bytes = fs::read(path).unwrap();
            let offset =
                |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize;
            let (start, len, fst, fst_len, readings) =
                (offset(16), offset(24), offset(32), offset(40), offset(48));
            let encoded = transform(&bytes[start..start + len]);
            let mut result = bytes[..64].to_vec();
            result[24..32].copy_from_slice(&(encoded.len() as u64).to_le_bytes());
            result[32..40].copy_from_slice(&((64 + encoded.len()) as u64).to_le_bytes());
            result[48..56].copy_from_slice(&((64 + encoded.len() + fst_len) as u64).to_le_bytes());
            result.extend(encoded);
            result.extend_from_slice(&bytes[fst..fst + fst_len]);
            result.extend_from_slice(&bytes[readings..]);
            fs::write(path, result).unwrap();
        }
        DictionaryFormat::Cdb => {
            let records = cdb::CDB::open(path)
                .unwrap()
                .iter()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            let mut transform = Some(transform);
            let mut writer = cdb::CDBWriter::create(path.to_str().unwrap()).unwrap();
            for (key, value) in records {
                let value = if key == b"__gukhanmun_meta__" {
                    transform.take().unwrap()(&value)
                } else {
                    value
                };
                writer.add(&key, &value).unwrap();
            }
            writer.finish().unwrap();
        }
    }
}
#[test]
fn both_containers_preserve_phrase_only_evidence_and_candidate_payload() {
    for format in [DictionaryFormat::Fst, DictionaryFormat::Cdb] {
        let (_temp, path) = build(format);
        let dictionary = open(format, &path).unwrap();
        assert_eq!(dictionary.phrase_evidence(), Some(&evidence()));
        assert!(dictionary.source_evidence().is_none());
        assert_eq!(
            dictionary.matches_at("降伏").next().unwrap().reading,
            "항복"
        );
    }
}
#[test]
fn both_readers_reject_future_schema_missing_keys_duplicate_fields_and_trailing_bytes() {
    for format in [DictionaryFormat::Fst, DictionaryFormat::Cdb] {
        for defect in 0..6 {
            let (_temp, path) = build(format);
            rewrite(
                format,
                &path,
                |value| {
                    let Value::Map(fields) = value else { panic!() };
                    let index = fields
                        .iter()
                        .position(|(key, _)| key == &Value::Text("phrase_evidence".into()))
                        .unwrap();
                    match defect {
                        0..=2 => {
                            let mut changed = evidence();
                            if defect == 0 {
                                changed.schema_version = 99;
                            }
                            if defect == 1 {
                                changed.rules[0].chunks[1].spellings[0].key = "降服".into();
                            }
                            if defect == 2 {
                                changed.rules[0].chunks[1].spellings[0].candidates[0]
                                    .suffix_reading = Some("강복".into());
                            }
                            fields[index].1 = Value::serialized(&changed).unwrap();
                        }
                        3 => fields.push(fields[index].clone()),
                        4 => {
                            fields.remove(index);
                        }
                        _ => {}
                    }
                },
                defect == 5,
            );
            assert!(open(format, &path).is_err(), "accepted defect {defect}");
        }
    }
}
#[test]
fn builder_rejects_stale_phrase_links_before_writing_either_backend() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.tsv");
    fs::write(&input, "hanja\thangul\n降伏\t강복\n").unwrap();
    for format in [DictionaryFormat::Fst, DictionaryFormat::Cdb] {
        let output = temp.path().join("must-not-exist");
        assert!(
            build_dictionary_with_all_evidence(
                std::slice::from_ref(&input),
                &output,
                &BuildOptions {
                    format,
                    ..BuildOptions::default()
                },
                None,
                Some(&evidence())
            )
            .is_err()
        );
        assert!(!output.exists());
    }
}
#[test]
fn cli_accepts_explicit_phrase_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.tsv");
    let source = temp.path().join("phrases.json");
    let output = temp.path().join("output.fst");
    fs::write(&input, "hanja\thangul\n降伏\t항복\n").unwrap();
    fs::write(&source, serde_json::to_vec(&evidence()).unwrap()).unwrap();
    assert_cmd::Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .arg("--phrase-evidence")
        .arg(source)
        .arg("--output")
        .arg(&output)
        .arg(input)
        .assert()
        .success();
    assert_eq!(
        open(DictionaryFormat::Fst, &output)
            .unwrap()
            .phrase_evidence(),
        Some(&evidence())
    );
}

#[test]
fn candidate_v1_and_phrase_evidence_coexist_in_both_containers() {
    let source = gukhanmun_core::DictionaryEvidence {
        schema_version: 1,
        role: gukhanmun_core::SourceRole::Reference,
        entries: std::collections::BTreeMap::from([(
            "降伏".into(),
            vec![gukhanmun_core::CandidateEvidence {
                reading: "항복".into(),
                suffix_reading: None,
                contributors: vec![gukhanmun_core::SourceContribution {
                    word_id: WordId(2),
                    sense_id: None,
                    origin: gukhanmun_core::WordOrigin::Native,
                }],
                authorized_forms: vec![],
                diagnostics: vec![],
            }],
        )]),
    };
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.tsv");
    fs::write(&input, "hanja\thangul\n降伏\t항복\n").unwrap();
    for format in [DictionaryFormat::Fst, DictionaryFormat::Cdb] {
        let output = temp.path().join("output.dict");
        build_dictionary_with_all_evidence(
            std::slice::from_ref(&input),
            &output,
            &BuildOptions {
                format,
                validate: true,
                ..BuildOptions::default()
            },
            Some(&source),
            Some(&evidence()),
        )
        .unwrap();
        let dictionary = open(format, &output).unwrap();
        assert_eq!(dictionary.source_evidence(), Some(&source));
        assert_eq!(dictionary.phrase_evidence(), Some(&evidence()));
    }
}
