// Gukhanmun: Standard dictionary source evidence extraction regressions.
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

use gukhanmun_core::{AuthorizationKind, DictionaryEvidence, EvidenceDiagnosticKind};
use gukhanmun_stdict::extract::{
    extract_json_reader_to_evidence_json, extract_json_reader_to_tsv, extract_path_to_evidence_json,
};
use serde_json::{Value, json};

fn record(id: u64, word: &str, key: Option<&str>, sense: u64) -> Value {
    json!({"target_code": id, "word_info": {
        "word": word, "word_type": "한자어", "word_unit": "단어",
        "original_language_info": key.map(|key| vec![json!({"original_language":key,"language_type":"한자"})]).unwrap_or_default(),
        "pos_info": [{"pos":"명사","comm_pattern_info":[{"sense_info":[{
            "sense_code":sense,"definition":"실질 풀이.","definition_original":"실질 풀이.",
            "cat_info":[{"cat":"인명"}]
        }]}]}]
    }})
}

fn sense(record: &mut Value) -> &mut Value {
    &mut record["word_info"]["pos_info"][0]["comm_pattern_info"][0]["sense_info"][0]
}

fn evidence(records: Vec<Value>) -> DictionaryEvidence {
    let bytes = serde_json::to_vec(&json!({"channel":{"item":records}})).unwrap();
    let mut output = Vec::new();
    extract_json_reader_to_evidence_json(bytes.as_slice(), &mut output).unwrap();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn excluded_non_hanja_phrase_is_retained_as_typed_redirect_witness() {
    let mut origin = record(1, "가온", Some("宙名"), 10);
    sense(&mut origin)["definition_original"] = json!("→ <sense_no>20</sense_no>공백없는표시.");
    let mut target = record(3, "새 이름02", None, 20);
    target["word_info"]["word_unit"] = json!("구");
    let records = vec![origin, record(20, "무관", None, 30), target];
    let result = evidence(records.clone());
    assert_eq!(result.entries.len(), 1);
    let candidate = &result.entries["宙名"][0];
    assert_eq!(candidate.reading, "가온");
    assert_eq!(candidate.authorized_forms[0].form.reading, "새 이름");
    assert_eq!(
        candidate.authorized_forms[0].kind,
        AuthorizationKind::Redirect
    );
    // Evidence is additive: the canonical TSV still contains only the original.
    let bytes = serde_json::to_vec(&json!({"channel":{"item":records}})).unwrap();
    let mut tsv = Vec::new();
    extract_json_reader_to_tsv(bytes.as_slice(), &mut tsv).unwrap();
    let tsv = String::from_utf8(tsv).unwrap();
    assert!(tsv.contains("宙名\t가온\t"));
    assert!(!tsv.contains("새 이름"));
}

#[test]
fn typed_synonym_requires_foreign_name_and_all_contributing_senses() {
    let mut origin = record(1, "가온", Some("宙名"), 10);
    origin["word_info"]["word_type"] = json!("외래어");
    sense(&mut origin)["lexical_info"] =
        json!([{"type":"동의어","unit":"의미","link_target_code":"20"}]);
    let result = evidence(vec![origin.clone(), record(2, "누리", None, 20)]);
    assert_eq!(
        result.entries["宙名"][0].authorized_forms[0].form.reading,
        "누리"
    );
    let mut other = sense(&mut origin).clone();
    other["sense_code"] = json!(11);
    other["lexical_info"] = json!([]);
    origin["word_info"]["pos_info"][0]["comm_pattern_info"][0]["sense_info"]
        .as_array_mut()
        .unwrap()
        .push(other);
    let result = evidence(vec![origin, record(2, "누리", None, 20)]);
    assert_eq!(result.entries["宙名"][0].contributors.len(), 2);
    assert!(result.entries["宙名"][0].authorized_forms.is_empty());
}

#[test]
fn visible_redirect_without_machine_evidence_is_not_a_terminal() {
    let mut origin = record(1, "가온", Some("宙名"), 10);
    sense(&mut origin)["definition"] = json!("→ 누리.");
    sense(&mut origin)["definition_original"] = Value::Null;
    let result = evidence(vec![origin]);
    let candidate = &result.entries["宙名"][0];
    assert!(candidate.authorized_forms.is_empty());
    assert_eq!(
        candidate.diagnostics[0].kind,
        EvidenceDiagnosticKind::UnsupportedRelation
    );
}

#[test]
fn missing_contributor_ids_leave_candidate_uncovered() {
    let origin = record(1, "가온", Some("宙名"), 10);
    let mut unidentified = record(2, "가온", Some("宙名"), 20);
    unidentified["target_code"] = Value::Null;
    assert!(evidence(vec![origin, unidentified]).entries.is_empty());
}

#[test]
fn complete_graph_precedes_admission_and_is_shard_order_independent() {
    let mut origin = record(1, "가온", Some("宙名"), 10);
    sense(&mut origin)["definition_original"] = json!("→ <sense_no>20</sense_no>새이름.");
    let target = record(2, "새 이름", None, 20);
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    for (dir, records) in [
        (a.path(), [origin.clone(), target.clone()]),
        (b.path(), [target, origin]),
    ] {
        for (index, record) in records.into_iter().enumerate() {
            std::fs::write(
                dir.join(format!("{index}.json")),
                serde_json::to_vec(&json!({"channel":{"item":[record]}})).unwrap(),
            )
            .unwrap();
        }
    }
    let mut first = Vec::new();
    let mut reversed = Vec::new();
    extract_path_to_evidence_json(a.path(), &mut first).unwrap();
    extract_path_to_evidence_json(b.path(), &mut reversed).unwrap();
    assert_eq!(first, reversed);
    let result: DictionaryEvidence = serde_json::from_slice(&first).unwrap();
    assert_eq!(
        result.entries["宙名"][0].authorized_forms[0].form.reading,
        "새 이름"
    );
}

#[test]
fn sino_korean_native_mixtures_are_not_mislabeled_as_foreign_mixtures() {
    let mut origin = record(1, "가온", Some("宙名"), 10);
    origin["word_info"]["word_type"] = json!("혼종어");
    origin["word_info"]["original_language_info"]
        .as_array_mut()
        .unwrap()
        .push(json!({"original_language":"이","language_type":"고유어"}));
    let result = evidence(vec![origin.clone()]);
    assert_eq!(
        result.entries["宙名이"][0].contributors[0].origin,
        gukhanmun_core::WordOrigin::Native
    );
    origin["word_info"]["original_language_info"]
        .as_array_mut()
        .unwrap()
        .push(json!({"original_language":"Ga[嘉]","language_type":"영어"}));
    let result = evidence(vec![origin]);
    assert_eq!(
        result.entries["宙名이嘉"][0].contributors[0].origin,
        gukhanmun_core::WordOrigin::Mixed
    );
}

#[test]
fn machine_word_redirect_keeps_the_word_namespace_and_checks_all_senses() {
    let mut origin = record(1, "가온", Some("宙名"), 10);
    sense(&mut origin)["definition_original"] = json!("→ <word_no>20</word_no>누리.");
    let target = record(20, "누리", None, 30);
    let unrelated = record(3, "무관", None, 20);
    let result = evidence(vec![origin.clone(), target.clone(), unrelated]);
    let grant = &result.entries["宙名"][0].authorized_forms[0];
    assert_eq!(grant.form.reading, "누리");
    assert_eq!(
        grant.witnesses[0].target,
        gukhanmun_core::SourceReference::Word(gukhanmun_core::WordId(20))
    );
    let mut target = target;
    let mut other = sense(&mut target).clone();
    other["sense_code"] = json!(31);
    other["definition_original"] = json!("→ <sense_no>40</sense_no>다솜.");
    target["word_info"]["pos_info"][0]["comm_pattern_info"][0]["sense_info"]
        .as_array_mut()
        .unwrap()
        .push(other);
    let result = evidence(vec![origin, target, record(4, "다솜", None, 40)]);
    assert!(result.entries["宙名"][0].authorized_forms.is_empty());
    assert_eq!(
        result.entries["宙名"][0].diagnostics[0].kind,
        EvidenceDiagnosticKind::AmbiguousTarget
    );
}

#[test]
fn source_word_synonym_is_not_rewritten_into_a_sense_edge() {
    let mut origin = record(1, "가온", Some("宙名"), 10);
    origin["word_info"]["word_type"] = json!("외래어");
    origin["word_info"]["lexical_info"] =
        json!([{"type":"동의어","unit":"의미","link_target_code":"20"}]);
    let result = evidence(vec![origin, record(2, "누리", None, 20)]);
    assert_eq!(
        result.entries["宙名"][0].authorized_forms[0].witnesses[0].source,
        gukhanmun_core::SourceReference::Word(gukhanmun_core::WordId(1))
    );
}

#[test]
fn unidentified_words_do_not_hide_ambiguous_sense_targets() {
    for definition in [
        "→ <sense_no>20</sense_no>표시.",
        "→ <word_no>2</word_no>표시.",
    ] {
        let mut origin = record(1, "가온", Some("宙名"), 10);
        sense(&mut origin)["definition_original"] = json!(definition);
        let target = record(2, "누리", None, 20);
        let mut unidentified = record(3, "다솜", None, 20);
        unidentified.as_object_mut().unwrap().remove("target_code");
        let result = evidence(vec![origin, target, unidentified]);
        let candidate = &result.entries["宙名"][0];
        assert!(candidate.authorized_forms.is_empty(), "{definition}");
        assert!(
            candidate
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.kind == EvidenceDiagnosticKind::AmbiguousTarget })
        );
    }
}

#[test]
fn unidentified_words_can_poison_a_contributing_sense_without_invented_word_ids() {
    let mut origin = record(1, "가온", Some("宙名"), 10);
    sense(&mut origin)["definition_original"] = json!("→ <sense_no>20</sense_no>표시.");
    let mut unidentified = record(3, "다솜", None, 10);
    unidentified.as_object_mut().unwrap().remove("target_code");
    let result = evidence(vec![origin, record(2, "누리", None, 20), unidentified]);
    let candidate = &result.entries["宙名"][0];
    assert!(candidate.authorized_forms.is_empty());
    assert_eq!(candidate.contributors.len(), 1);
    assert_eq!(candidate.contributors[0].word_id.0, 1);
    assert!(
        candidate
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.kind == EvidenceDiagnosticKind::AmbiguousTarget })
    );
}
