// Gukhanmun: Bundled Standard Korean Language Dictionary for Gukhanmun.
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

use gukhanmun_core::PhraseEvidence;
use gukhanmun_stdict::extract::extract_json_reader_to_phrase_files;
use serde_json::{Value, json};

fn row(id: u64, word: &str, unit: &str, originals: &[&str]) -> Value {
    json!({"target_code":id,"word_info":{"word":word,"word_unit":unit,"word_type":"한자어","original_language_info":originals.iter().enumerate().flat_map(|(i,key)| {
        let mut rows=Vec::new();
        if i>0 {rows.push(json!({"original_language":"/","language_type":"/(병기)"}));}
        let parts=key.split('+').collect::<Vec<_>>();
        for part in parts { rows.push(json!({"original_language":part,"language_type": if part.chars().all(|c| ('가'..='힣').contains(&c)) {"고유어"} else {"한자"}})); }
        rows
    }).collect::<Vec<_>>()}})
}
fn fixture() -> Vec<Value> {
    let mut parent = row(368000, "항복", "단어", &["降伏", "降服"]);
    parent["word_info"]["relation_info"] = json!([{"type":"부표제어","link_target_code":"502316"}]);
    vec![
        row(115990, "무조건^항복", "구", &["無條件降伏"]),
        row(425066, "무-조건", "단어", &["無條件"]),
        parent,
        row(502316, "항복-하다", "단어", &["降伏+하다", "降服+하다"]),
    ]
}
fn compile(rows: Vec<Value>) -> (PhraseEvidence, Value, Vec<u8>, Vec<u8>) {
    let input = serde_json::to_vec(&json!({"channel":{"item":rows}})).unwrap();
    let (mut evidence, mut audit) = (Vec::new(), Vec::new());
    extract_json_reader_to_phrase_files(input.as_slice(), &mut evidence, &mut audit).unwrap();
    let parsed: PhraseEvidence = serde_json::from_slice(&evidence).unwrap();
    parsed.validate().unwrap();
    (
        parsed,
        serde_json::from_slice(&audit).unwrap(),
        evidence,
        audit,
    )
}
#[test]
fn attested_sibling_and_typed_subheadword_extension() {
    let (evidence, audit, _, _) = compile(fixture());
    assert_eq!(evidence.rules.len(), 1);
    let rule = &evidence.rules[0];
    assert_eq!(rule.source_ids[0].0, 115990);
    let terminal = &rule.chunks[1];
    assert_eq!(terminal.spellings[0].key, "降伏");
    assert_eq!(terminal.spellings[1].key, "降服");
    assert_eq!(terminal.spellings[1].sibling_witnesses[0].0, 368000);
    assert_eq!(terminal.spellings[1].candidates[0].reading, "항복");
    assert_eq!(rule.terminal_extensions[0].literal_suffix, "하다");
    assert_eq!(rule.terminal_extensions[0].spellings[1].key, "降服하다");
    assert_eq!(rule.terminal_extensions[0].witnesses[0].child.0, 502316);
    assert_eq!(audit["records"][0]["terminal_outcome"], "validated");
}
#[test]
fn all_potential_parents_must_agree_and_sense_targets_do_not_license() {
    let mut rows = fixture();
    rows.push(row(900, "항복02", "단어", &["降伏", "降服"]));
    let (evidence, _, _, _) = compile(rows);
    assert!(evidence.rules[0].terminal_extensions.is_empty());
    let mut rows = fixture();
    rows[2]["word_info"]["relation_info"][0]["unit"] = json!("의미");
    let (evidence, audit, _, _) = compile(rows);
    assert!(evidence.rules[0].terminal_extensions.is_empty());
    assert_eq!(
        audit["records"][0]["terminal_outcome"],
        "invalid-word-target"
    );
}
#[test]
fn malformed_targets_withhold_the_whole_extension() {
    for target in [json!("502316x"), json!(999), json!(null)] {
        let mut rows = fixture();
        rows[2]["word_info"]["relation_info"][0]["link_target_code"] = target;
        assert!(compile(rows).0.rules[0].terminal_extensions.is_empty());
    }
    let mut rows = fixture();
    rows.push(rows[3].clone());
    assert!(compile(rows).0.rules[0].terminal_extensions.is_empty());
}
#[test]
fn strict_alignment_categories_and_all_raw_spelling_collisions() {
    for (word, key, reason) in [
        ("무조건-항복", "無條件降伏", "chunk-alignment"),
        ("무조건^^항복", "無條件降伏", "chunk-alignment"),
        ("무조건^항복", "無條件", "chunk-alignment"),
    ] {
        let (evidence, audit, _, _) = compile(vec![row(1, word, "구", &[key])]);
        assert!(evidence.rules.is_empty());
        assert_eq!(audit["records"][0]["outcome"], format!("withheld:{reason}"));
    }
    let mut rows = fixture();
    rows.push(row(2, "무조건^강복", "구", &["無條件降伏"]));
    assert!(compile(rows).0.rules.is_empty());
    let mut rows = fixture();
    rows.push(row(2, "무조건강복", "단어", &["無條件降伏"]));
    assert!(compile(rows).0.rules.is_empty());
    let mut rows = fixture();
    rows[0]["word_info"]["pos_info"] =
        json!([{"comm_pattern_info":[{"sense_info":[{"cat_info":[{"cat":"역사"}]}]}]}]);
    assert!(compile(rows).0.rules.is_empty());
}
#[test]
fn missing_candidates_remain_literal_only_and_other_families_compile() {
    let rows = vec![
        row(15317, "건조^경보", "구", &["乾燥警報"]),
        row(78461, "대기^행렬", "구", &["待機行列"]),
        row(209762, "습성^해수", "구", &["濕性咳嗽"]),
        row(10, "행렬", "단어", &["行列"]),
    ];
    let (evidence, _, _, _) = compile(rows);
    assert_eq!(evidence.rules.len(), 3);
    for rule in &evidence.rules {
        assert!(rule.chunks[0].spellings[0].candidates.is_empty());
    }
    assert!(
        evidence
            .rules
            .iter()
            .find(|r| r.source_ids[0].0 == 78461)
            .unwrap()
            .chunks[1]
            .spellings[0]
            .candidates
            .len()
            == 1
    );
}
#[test]
fn output_is_independent_of_input_order_and_ambiguous_donors_are_quarantined() {
    let rows = fixture();
    let (_, _, a, b) = compile(rows.clone());
    let mut reversed = rows.clone();
    reversed.reverse();
    let (_, _, c, d) = compile(reversed);
    assert_eq!(a, c);
    assert_eq!(b, d);
    let mut rows = rows;
    rows.push(row(900, "항복02", "단어", &["降伏"]));
    let evidence = compile(rows).0;
    assert_eq!(evidence.rules[0].chunks[1].spellings.len(), 1);
}
#[test]
fn explicit_limits_are_audited_without_partial_admission() {
    let word = std::iter::repeat_n("가", 17).collect::<Vec<_>>().join("^");
    let (evidence, audit, _, _) = compile(vec![row(1, &word, "구", &[&"家".repeat(17)])]);
    assert!(evidence.rules.is_empty());
    assert_eq!(audit["records"][0]["outcome"], "withheld:chunk-limit");
    assert_eq!(audit["limits"]["max_chunks"], 16);
}
#[test]
fn colliding_derived_and_explicit_phrase_spellings_are_both_quarantined() {
    let mut rows = fixture();
    // The explicit phrase keys differ, but both donor groups derive 降服.
    rows.push(row(12, "무조건^강복", "구", &["無條件江伏"]));
    rows.push(row(13, "강복", "단어", &["江伏", "降服"]));
    let (evidence, audit, _, _) = compile(rows);
    assert!(evidence.rules.is_empty());
    assert!(
        audit["records"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["outcome"].as_str().unwrap().starts_with("withheld:"))
    );
    let record = audit["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["source_id"] == 115990)
        .unwrap();
    assert_eq!(record["outcome"], "withheld:derived-phrase-collision");
    assert_eq!(record["terminal_extensions"], 0);
    assert_eq!(record["terminal_outcome"], "withheld");
}
#[test]
fn cli_checks_both_outputs_without_writing_and_requires_the_pair() {
    use assert_cmd::cargo::cargo_bin_cmd;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.json");
    let tsv = dir.path().join("words.tsv");
    let evidence = dir.path().join("phrases.json");
    let manifest = dir.path().join("audit.json");
    std::fs::write(
        &input,
        serde_json::to_vec(&json!({"channel":{"item":fixture()}})).unwrap(),
    )
    .unwrap();
    let args = [
        input.as_os_str(),
        "--output".as_ref(),
        tsv.as_os_str(),
        "--phrase-evidence-output".as_ref(),
        evidence.as_os_str(),
        "--phrase-manifest-output".as_ref(),
        manifest.as_os_str(),
    ];
    cargo_bin_cmd!("gukhanmun-stdict-extract")
        .args(args)
        .assert()
        .success();
    cargo_bin_cmd!("gukhanmun-stdict-extract")
        .args(args)
        .arg("--check")
        .assert()
        .success();
    std::fs::write(&manifest, b"changed").unwrap();
    cargo_bin_cmd!("gukhanmun-stdict-extract")
        .args(args)
        .arg("--check")
        .assert()
        .failure();
    assert_eq!(std::fs::read(&manifest).unwrap(), b"changed");
    cargo_bin_cmd!("gukhanmun-stdict-extract")
        .arg(&input)
        .arg("--phrase-evidence-output")
        .arg(&evidence)
        .assert()
        .failure();
}

#[test]
fn morpheme_markers_are_inside_chunks_and_homograph_digits_are_metadata() {
    let (evidence, _, _, _) = compile(vec![row(1, "급-강하^폭격01", "구", &["急降下爆擊"])]);
    assert_eq!(evidence.rules[0].chunks.len(), 2);
    assert_eq!(evidence.rules[0].chunks[0].reading, "급강하");
    assert_eq!(evidence.rules[0].chunks[0].spellings[0].key, "急降下");
    assert_eq!(evidence.rules[0].chunks[1].reading, "폭격");
}

#[test]
fn duplicate_source_id_audits_are_also_order_independent() {
    let rows = vec![
        row(1, "가^나", "구", &["家那"]),
        row(1, "가^나", "구", &["假那"]),
    ];
    let (_, _, a, b) = compile(rows.clone());
    let mut reverse = rows;
    reverse.reverse();
    let (_, _, c, d) = compile(reverse);
    assert_eq!(a, c);
    assert_eq!(b, d);
}
#[test]
fn donors_never_create_recognition_grids_or_transitive_alternatives() {
    let rows = vec![
        row(1, "무조건^항복", "구", &["無條件降伏"]),
        row(2, "항복", "단어", &["降伏", "江服"]),
        row(3, "강복", "단어", &["江服", "降福"]),
    ];
    let evidence = compile(rows).0;
    let keys = evidence.rules[0].chunks[1]
        .spellings
        .iter()
        .map(|s| s.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(keys, vec!["降伏", "江服"]);
    assert!(!keys.contains(&"降服"));
    assert!(!keys.contains(&"江伏"));
    assert!(!keys.contains(&"降福"));
}
#[test]
fn sibling_and_product_limits_withhold_complete_rules() {
    let alternatives = (0..33)
        .map(|i| format!("{}伏", char::from_u32(0x4e00 + i).unwrap()))
        .collect::<Vec<_>>();
    let refs = alternatives.iter().map(String::as_str).collect::<Vec<_>>();
    let original = format!("無條件{}", refs[0]);
    let (evidence, audit, _, _) = compile(vec![
        row(1, "무조건^항복", "구", &[&original]),
        row(2, "항복", "단어", &refs),
    ]);
    assert!(evidence.rules.is_empty());
    assert_eq!(audit["records"][0]["outcome"], "withheld:sibling-limit");
    let mut rows = vec![row(1, "가나^다라^마바", "구", &["家那多羅摩波"])];
    for (id, reading, original) in [
        (2, "가나", "家那"),
        (3, "다라", "多羅"),
        (4, "마바", "摩波"),
    ] {
        let mut spellings = vec![original.to_owned()];
        spellings.extend((0..16).map(|i| {
            format!(
                "{}{}",
                char::from_u32(0x4f00 + id as u32 * 32 + i).unwrap(),
                original.chars().nth(1).unwrap()
            )
        }));
        rows.push(row(
            id,
            reading,
            "단어",
            &spellings.iter().map(String::as_str).collect::<Vec<_>>(),
        ));
    }
    let (evidence, audit, _, _) = compile(rows);
    assert!(evidence.rules.is_empty());
    assert_eq!(audit["records"][0]["outcome"], "withheld:combination-limit");
}

#[test]
fn raw_mixed_native_phrase_collision_withholds_terminal_projection_only() {
    for spelling in ["降伏", "降服"] {
        let mut rows = fixture();
        rows.push(row(
            5,
            "무조건^강복하다",
            "구",
            &[&format!("無條件{spelling}+하다")],
        ));
        let (evidence, audit, _, _) = compile(rows);
        assert_eq!(evidence.rules.len(), 1);
        assert!(evidence.rules[0].terminal_extensions.is_empty());
        let record = audit["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source_id"] == 115990)
            .unwrap();
        assert_eq!(record["outcome"], "admitted");
        assert_eq!(record["terminal_extensions"], 0);
        assert_eq!(record["terminal_withheld"][0]["literal_suffix"], "하다");
        assert_eq!(
            record["terminal_withheld"][0]["reason"],
            "raw-terminal-spelling-reading-collision"
        );
    }
}

#[test]
fn subheadword_homograph_digits_do_not_become_literal_suffix_text() {
    let mut rows = fixture();
    rows[3]["word_info"]["word"] = json!("항복-하다01");
    let (evidence, audit, _, _) = compile(rows);
    assert_eq!(evidence.rules[0].terminal_extensions.len(), 1);
    assert_eq!(
        evidence.rules[0].terminal_extensions[0].literal_suffix,
        "하다"
    );
    assert_eq!(audit["records"][0]["terminal_outcome"], "validated");
}

#[test]
fn raw_terminal_collision_checks_prefix_siblings_and_retains_other_suffixes() {
    let mut rows = fixture();
    rows[1] = row(425066, "무-조건", "단어", &["無條件", "无條件"]);
    rows[2]["word_info"]["relation_info"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"부표제어","link_target_code":"502317"}));
    rows.push(row(
        502317,
        "항복-되다",
        "단어",
        &["降伏+되다", "降服+되다"],
    ));
    rows.push(row(5, "무조건^강복하다", "구", &["无條件降伏+하다"]));
    let (evidence, audit, _, _) = compile(rows);
    assert_eq!(evidence.rules.len(), 1);
    assert_eq!(evidence.rules[0].terminal_extensions.len(), 1);
    assert_eq!(
        evidence.rules[0].terminal_extensions[0].literal_suffix,
        "되다"
    );
    let record = audit["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source_id"] == 115990)
        .unwrap();
    assert_eq!(record["terminal_extensions"], 1);
    assert_eq!(record["terminal_outcome"], "partially-withheld");
    assert_eq!(record["terminal_withheld"][0]["literal_suffix"], "하다");
}
