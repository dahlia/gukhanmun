use std::collections::BTreeMap;
use std::fs;

use assert_cmd::Command;
use gukhanmun_mkdict::FstDictionary;
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
            "ignoring unsupported TSV column `category`",
        ));

    let dictionary = FstDictionary::open(&output).unwrap();
    assert_eq!(dictionary.metadata().get("source").unwrap(), "fixture");
    assert_eq!(dictionary.metadata().get("license").unwrap(), "CC0-1.0");
    assert_eq!(
        dictionary.metadata().get("build_date").unwrap(),
        "1970-01-01T00:00:00Z"
    );
    assert_eq!(dictionary.entry_count(), 3);

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
