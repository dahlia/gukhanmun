// Gukhanmun: Builds Gukhanmun dictionary backend files from canonical TSV input.
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

use assert_cmd::Command;
use gukhanmun_cdb::CdbDictionary;
use gukhanmun_core::HanjaDictionary;
use gukhanmun_fst::FstDictionary;
use proptest::prop_assert_eq;
use tempfile::tempdir;

#[test]
fn builds_fst_with_metadata_and_lookup() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let output = temp.path().join("dict.gukfst");
    fs::write(
        &input,
        "hanja\thangul\trequire_hanja\trequire_hangul\tcategory\n天地\t천지\tfalse\t0\tbasic\n漢字\t한자\t1\tfalse\tbasic\n色깔論\t색깔론\tfalse\ttrue\tmixed\n",
    )
    .unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .env_remove("SOURCE_DATE_EPOCH")
        .args([
            "-o",
            output.to_str().unwrap(),
            "--validate",
            "--metadata",
            "source=fixture",
            "--metadata",
            "license=CC0-1.0",
            input.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "ignoring unsupported input column",
        ))
        .stderr(predicates::str::contains("category"));

    let dictionary = FstDictionary::open(&output).unwrap();
    assert_eq!(dictionary.metadata().get("source").unwrap(), "fixture");
    assert_eq!(dictionary.metadata().get("license").unwrap(), "CC0-1.0");
    assert_eq!(
        dictionary.metadata().get("build_date").unwrap(),
        "1970-01-01T00:00:00Z"
    );
    assert_eq!(dictionary.entry_count(), 3);
    assert_eq!(dictionary.metadata().get("max_word_chars").unwrap(), "3");
    assert_eq!(dictionary.metadata().get("max_key_bytes").unwrap(), "9");
    assert_eq!(dictionary.max_word_chars(), Some(3));

    let hanja = dictionary.lookup("漢字").unwrap().unwrap();
    assert_eq!(hanja.reading(), "한자");
    assert!(hanja.mark().require_hanja);
    assert!(!hanja.mark().require_hangul);

    let mixed = dictionary.lookup("色깔論").unwrap().unwrap();
    assert_eq!(mixed.reading(), "색깔론");
    assert!(!mixed.mark().require_hanja);
    assert!(mixed.mark().require_hangul);
}

#[test]
fn builds_cdb_with_metadata_prefix_matches_and_lookup() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let output = temp.path().join("dict.gukcdb");
    fs::write(
        &input,
        "hanja\thangul\trequire_hanja\trequire_hangul\n行事\t행사\tfalse\tfalse\n行事場\t행사장\ttrue\tfalse\n場所\t장소\tfalse\ttrue\n漢字\t한자\tfalse\tfalse\n翰字\t한자\tfalse\tfalse\n",
    )
    .unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            output.to_str().unwrap(),
            "--format",
            "cdb",
            "--validate",
            "--metadata",
            "source=fixture",
            input.to_str().unwrap(),
        ])
        .assert()
        .success();

    let dictionary = CdbDictionary::open(&output).unwrap();
    assert_eq!(dictionary.metadata().get("source").unwrap(), "fixture");
    assert_eq!(dictionary.entry_count(), 5);
    assert_eq!(dictionary.max_word_chars(), Some(3));
    let matches = dictionary.matches_at("行事場入口").collect::<Vec<_>>();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].byte_len, "行事".len());
    assert_eq!(matches[0].reading, "행사");
    assert_eq!(matches[1].byte_len, "行事場".len());
    assert_eq!(matches[1].reading, "행사장");
    assert!(matches[1].mark.require_hanja);
    assert!(dictionary.lookup("漢字").unwrap().unwrap().reading() == "한자");
    assert!(dictionary.has_homophone("漢字", "한자"));
}

#[test]
fn cdb_rejects_entries_that_collide_with_metadata_key() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let output = temp.path().join("dict.gukcdb");
    fs::write(&input, "hanja\thangul\n__gukhanmun_meta__\t예약어\n").unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            output.to_str().unwrap(),
            "--format",
            "cdb",
            input.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "`__gukhanmun_meta__` is reserved for CDB metadata",
        ));

    assert!(!output.exists());
}

#[test]
fn parses_csv_and_jsonl_inputs_by_extension() {
    let temp = tempdir().unwrap();
    let csv_input = temp.path().join("dict.csv");
    let jsonl_input = temp.path().join("extra.jsonl");
    let output = temp.path().join("dict.gukfst");
    fs::write(
        &csv_input,
        "hanja,hangul,require_hanja,require_hangul\n天地,천지,true,false\n",
    )
    .unwrap();
    fs::write(
        &jsonl_input,
        "{\"hanja\":\"漢字\",\"hangul\":\"한자\",\"requireHanja\":false,\"requireHangul\":true}\n",
    )
    .unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            output.to_str().unwrap(),
            "--validate",
            csv_input.to_str().unwrap(),
            jsonl_input.to_str().unwrap(),
        ])
        .assert()
        .success();

    let dictionary = FstDictionary::open(&output).unwrap();
    let csv_entry = dictionary.lookup("天地").unwrap().unwrap();
    assert_eq!(csv_entry.reading(), "천지");
    assert!(csv_entry.mark().require_hanja);
    assert!(!csv_entry.mark().require_hangul);
    let json_entry = dictionary.lookup("漢字").unwrap().unwrap();
    assert_eq!(json_entry.reading(), "한자");
    assert!(!json_entry.mark().require_hanja);
    assert!(json_entry.mark().require_hangul);
}

#[test]
fn duplicate_keys_fail_by_default() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let output = temp.path().join("dict.gukfst");
    fs::write(&input, "hanja\thangul\n天地\t천지\n天地\t천디\n").unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args(["-o", output.to_str().unwrap(), input.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::str::contains("duplicate entry for `天地`"));
}

#[test]
fn rejects_tsv_without_required_header() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let output = temp.path().join("dict.gukfst");
    fs::write(&input, "天地\t천지\n").unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args(["-o", output.to_str().unwrap(), input.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::str::contains("missing required `hanja` column"));
}

#[test]
fn rejects_keys_over_the_configured_byte_limit() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let output = temp.path().join("dict.gukfst");
    fs::write(&input, "hanja\thangul\n天地\t천지\n").unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            output.to_str().unwrap(),
            "--max-key-bytes",
            "3",
            input.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("exceeds --max-key-bytes=3"));
}

#[cfg(unix)]
#[test]
fn cdb_output_rejects_non_utf8_paths() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    fs::write(&input, "hanja\thangul\n天地\t천지\n").unwrap();
    let output = temp
        .path()
        .join(OsString::from_vec(b"dict-\xff.gukcdb".to_vec()));
    let lossy_output = std::path::PathBuf::from(output.as_os_str().to_string_lossy().into_owned());

    let error = gukhanmun_mkdict::build_dictionary(
        &[input],
        &output,
        &gukhanmun_mkdict::BuildOptions {
            format: gukhanmun_mkdict::DictionaryFormat::Cdb,
            merge: gukhanmun_mkdict::MergePolicy::Error,
            validate: true,
            max_key_bytes: gukhanmun_mkdict::DEFAULT_MAX_KEY_BYTES,
            metadata: BTreeMap::new(),
            rules: Vec::new(),
            allow_unmatched_rules: false,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("valid UTF-8"));
    assert!(!lossy_output.exists());
}

#[test]
fn source_date_epoch_controls_build_date_metadata() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let output = temp.path().join("dict.gukfst");
    fs::write(&input, "hanja\thangul\n天地\t천지\n").unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .env("SOURCE_DATE_EPOCH", "946684800")
        .args(["-o", output.to_str().unwrap(), input.to_str().unwrap()])
        .assert()
        .success();

    let dictionary = FstDictionary::open(&output).unwrap();
    assert_eq!(
        dictionary.metadata().get("build_date").unwrap(),
        "2000-01-01T00:00:00Z"
    );
}

#[test]
fn first_and_last_wins_merge_policies_are_explicit() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let first = temp.path().join("first.gukfst");
    let last = temp.path().join("last.gukfst");
    fs::write(&input, "hanja\thangul\n天地\t천지\n天地\t천디\n").unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            first.to_str().unwrap(),
            "--merge",
            "first-wins",
            input.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            last.to_str().unwrap(),
            "--merge",
            "last-wins",
            input.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(
        FstDictionary::open(&first)
            .unwrap()
            .lookup("天地")
            .unwrap()
            .unwrap()
            .reading(),
        "천지"
    );
    assert_eq!(
        FstDictionary::open(&last)
            .unwrap()
            .lookup("天地")
            .unwrap()
            .unwrap()
            .reading(),
        "천디"
    );
}

#[test]
fn rules_flag_applies_marks_to_fst_and_cdb_builds() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let rules = temp.path().join("rules.tsv");
    let fst_output = temp.path().join("dict.gukfst");
    let cdb_output = temp.path().join("dict.gukcdb");
    fs::write(
        &input,
        "hanja\thangul\n漢字\t한자\n天地\t천지\n史記\t사기\n詐欺\t사기\n書冊\t서책\n",
    )
    .unwrap();
    fs::write(
        &rules,
        "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
         entry\t漢字\ttrue\tfalse\thomophone-heavy\n\
         contains\t天\ttrue\tfalse\trare hanja\n\
         reading\t사기\ttrue\tfalse\tambiguous reading\n",
    )
    .unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            fst_output.to_str().unwrap(),
            "--rules",
            rules.to_str().unwrap(),
            "--validate",
            input.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            cdb_output.to_str().unwrap(),
            "--format",
            "cdb",
            "--rules",
            rules.to_str().unwrap(),
            "--validate",
            input.to_str().unwrap(),
        ])
        .assert()
        .success();

    let fst = FstDictionary::open(&fst_output).unwrap();
    assert!(fst.lookup("漢字").unwrap().unwrap().mark().require_hanja);
    assert!(fst.lookup("天地").unwrap().unwrap().mark().require_hanja);
    assert!(fst.lookup("史記").unwrap().unwrap().mark().require_hanja);
    assert!(fst.lookup("詐欺").unwrap().unwrap().mark().require_hanja);
    assert!(!fst.lookup("書冊").unwrap().unwrap().mark().require_hanja);

    let cdb = CdbDictionary::open(&cdb_output).unwrap();
    assert!(cdb.lookup("漢字").unwrap().unwrap().mark().require_hanja);
    assert!(cdb.lookup("天地").unwrap().unwrap().mark().require_hanja);
    assert!(cdb.lookup("史記").unwrap().unwrap().mark().require_hanja);
    assert!(cdb.lookup("詐欺").unwrap().unwrap().mark().require_hanja);
    assert!(!cdb.lookup("書冊").unwrap().unwrap().mark().require_hanja);
}

#[test]
fn rules_flag_rejects_unmatched_rules_by_default() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let rules = temp.path().join("rules.tsv");
    let output = temp.path().join("dict.gukfst");
    fs::write(&input, "hanja\thangul\n漢字\t한자\n").unwrap();
    fs::write(
        &rules,
        "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
         entry\t天地\ttrue\tfalse\tmissing\n\
         contains\t驟\ttrue\tfalse\tmissing\n",
    )
    .unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            output.to_str().unwrap(),
            "--rules",
            rules.to_str().unwrap(),
            input.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("2 unmatched"));

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            output.to_str().unwrap(),
            "--rules",
            rules.to_str().unwrap(),
            "--allow-unmatched-rules",
            input.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn rules_flag_rejects_duplicate_rules_across_files() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("dict.tsv");
    let rules_a = temp.path().join("rules-a.tsv");
    let rules_b = temp.path().join("rules-b.tsv");
    let output = temp.path().join("dict.gukfst");
    fs::write(&input, "hanja\thangul\n漢字\t한자\n").unwrap();
    fs::write(
        &rules_a,
        "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
         entry\t漢字\ttrue\tfalse\tfirst\n",
    )
    .unwrap();
    fs::write(
        &rules_b,
        "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
         entry\t漢字\tfalse\ttrue\tsecond\n",
    )
    .unwrap();

    Command::cargo_bin("gukhanmun-mkdict")
        .unwrap()
        .args([
            "-o",
            output.to_str().unwrap(),
            "--rules",
            rules_a.to_str().unwrap(),
            "--rules",
            rules_b.to_str().unwrap(),
            input.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("duplicate rule"));
}

proptest::proptest! {
    #[test]
    fn generated_entries_round_trip(entries in unique_entries()) {
        let temp = tempdir().unwrap();
        let input = temp.path().join("dict.tsv");
        let output = temp.path().join("dict.gukfst");
        let mut tsv = String::from("hanja\thangul\trequire_hanja\trequire_hangul\n");
        let mut expected = BTreeMap::new();
        for (hanja, hangul, require_hanja, require_hangul) in entries {
            tsv.push_str(&format!("{hanja}\t{hangul}\t{require_hanja}\t{require_hangul}\n"));
            expected.insert(hanja, (hangul, require_hanja, require_hangul));
        }
        fs::write(&input, tsv).unwrap();

        Command::cargo_bin("gukhanmun-mkdict")
            .unwrap()
            .args(["-o", output.to_str().unwrap(), "--validate", input.to_str().unwrap()])
            .assert()
            .success();

        let dictionary = FstDictionary::open(&output).unwrap();
        assert_eq!(dictionary.entry_count(), expected.len() as u64);
        for (hanja, (hangul, require_hanja, require_hangul)) in expected {
            let entry = dictionary.lookup(&hanja).unwrap().unwrap();
            prop_assert_eq!(entry.reading(), hangul);
            prop_assert_eq!(entry.mark().require_hanja, require_hanja);
            prop_assert_eq!(entry.mark().require_hangul, require_hangul);
        }
    }

    #[test]
    fn generated_entries_round_trip_through_cdb(entries in unique_entries()) {
        let temp = tempdir().unwrap();
        let input = temp.path().join("dict.tsv");
        let output = temp.path().join("dict.gukcdb");
        let mut tsv = String::from("hanja\thangul\trequire_hanja\trequire_hangul\n");
        let mut expected = BTreeMap::new();
        for (hanja, hangul, require_hanja, require_hangul) in entries {
            tsv.push_str(&format!("{hanja}\t{hangul}\t{require_hanja}\t{require_hangul}\n"));
            expected.insert(hanja, (hangul, require_hanja, require_hangul));
        }
        fs::write(&input, tsv).unwrap();

        Command::cargo_bin("gukhanmun-mkdict")
            .unwrap()
            .args([
                "-o",
                output.to_str().unwrap(),
                "--format",
                "cdb",
                "--validate",
                input.to_str().unwrap(),
            ])
            .assert()
            .success();

        let dictionary = CdbDictionary::open(&output).unwrap();
        assert_eq!(dictionary.entry_count(), expected.len() as u64);
        for (hanja, (hangul, require_hanja, require_hangul)) in expected {
            let entry = dictionary.lookup(&hanja).unwrap().unwrap();
            prop_assert_eq!(entry.reading(), hangul);
            prop_assert_eq!(entry.mark().require_hanja, require_hanja);
            prop_assert_eq!(entry.mark().require_hangul, require_hangul);
        }
    }
}

fn unique_entries() -> impl proptest::strategy::Strategy<Value = Vec<(String, String, bool, bool)>>
{
    use proptest::prelude::*;
    proptest::collection::btree_map(
        "[一-龥]{1,3}",
        ("[가-힣]{1,4}", any::<bool>(), any::<bool>()),
        1..16,
    )
    .prop_map(|entries| {
        entries
            .into_iter()
            .map(|(hanja, (hangul, require_hanja, require_hangul))| {
                (hanja, hangul, require_hanja, require_hangul)
            })
            .collect()
    })
}
