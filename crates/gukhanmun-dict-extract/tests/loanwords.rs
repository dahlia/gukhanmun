// Gukhanmun: Experimental source-independent loanword extraction tooling.
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

//! Independently authored synthetic cases, not copied from an official corpus.
use gukhanmun_dict_extract::loanwords::*;
use serde_json::{Value, json};
use std::{fs, path::Path};
fn record(id: &str, original: &str, reading: &str) -> Record {
    Record::from_fields(json!({"example_no":id,"srclang_mark":original,"korean_mark":reading,"regltn_code":"0003","synthetic":true})).unwrap()
}
fn snapshot(records: Vec<Record>) -> Snapshot {
    Snapshot::new(records, "synthetic-only".into()).unwrap()
}
fn conflict() -> Snapshot {
    snapshot(vec![
        record("a", "香港", "홍콩"),
        record("b", "香港(Xiānggǎng)", "샹강"),
    ])
}
fn decisions(s: &Snapshot) -> Decisions {
    let a = audit(s, &Decisions::default());
    Decisions {
        format_version: 1,
        decisions: vec![Decision {
            key: "香港".into(),
            chosen: "홍콩".into(),
            expected: a.candidates["香港"].clone(),
            rationale: "Synthetic test choice, no corpus policy".into(),
            references: vec!["synthetic fixture".into()],
        }],
    }
}
fn page(records: Vec<Value>, total: u64, number: u64) -> Vec<u8> {
    serde_json::to_vec(&json!({"resultCode":"00","resultMsg":"OK","pageNo":number,"numOfRows":2,"totalCount":total,"items":records})).unwrap()
}
#[test]
fn strict_duplicate_detection_and_canonical_json() {
    for raw in [
        r#"{"a":1,"a":2}"#,
        r#"{"unknown":{"x":0,"x":1}}"#,
        r#"[{"x":1,"x":1}]"#,
    ] {
        assert!(parse_json(raw.as_bytes()).is_err());
    }
    let a = parse_json(br#"{"z":{"b":2,"a":1},"a":[{"z":0,"b":1}]}"#).unwrap();
    let b = parse_json(br#"{"a":[{"b":1,"z":0}],"z":{"a":1,"b":2}}"#).unwrap();
    assert_eq!(
        canonical_json(&a),
        br#"{"a":[{"b":1,"z":0}],"z":{"a":1,"b":2}}"#
    );
    assert_eq!(canonical_json(&a), canonical_json(&b));
}
#[test]
fn ids_are_exact_safe_and_collisions_rejected() {
    for id in [
        json!(""),
        json!("a,b"),
        json!("a b"),
        json!("a\n"),
        json!(1.0),
        json!(1e20),
        json!(null),
    ] {
        let mut r = record("a", "漢", "한").fields;
        r["example_no"] = id;
        assert!(Record::from_fields(r).is_err());
    }
    let mut r = record("a", "漢", "한").fields;
    r["example_no"] = json!(18446744073709551615u64);
    assert_eq!(Record::from_fields(r).unwrap().id, "18446744073709551615");
    let mut r = record("a", "漢", "한").fields;
    r["example_no"] = json!(1);
    assert!(
        Snapshot::new(
            vec![Record::from_fields(r).unwrap(), record("1", "漢", "한")],
            "synthetic".into()
        )
        .is_err()
    );
}
#[test]
fn pages_reject_schema_aliases_and_inconsistent_pagination() {
    let r = record("a", "漢", "한").fields;
    let p = page(vec![r.clone()], 1, 1);
    assert_eq!(Page::parse(&p, 1, 2).unwrap().records.len(), 1);
    let mut lower: Value = serde_json::from_slice(&p).unwrap();
    for (a, b) in [
        ("resultCode", "resultcode"),
        ("resultMsg", "resultmsg"),
        ("totalCount", "totalcount"),
        ("numOfRows", "numofrows"),
        ("pageNo", "pageno"),
    ] {
        let v = lower.as_object_mut().unwrap().remove(a).unwrap();
        lower[b] = v;
    }
    assert!(Page::parse(&serde_json::to_vec(&lower).unwrap(), 1, 2).is_ok());
    for (field, value) in [
        ("resultcode", json!("00")),
        ("resultCode", json!(0)),
        ("resultMsg", json!(null)),
        ("totalCount", json!("1")),
        ("pageNo", json!(0)),
        ("numOfRows", json!(3)),
        ("items", json!(null)),
        ("totalCount", json!(3)),
    ] {
        let mut bad: Value = serde_json::from_slice(&p).unwrap();
        bad[field] = value;
        assert!(
            Page::parse(&serde_json::to_vec(&bad).unwrap(), 1, 2).is_err(),
            "{field}"
        );
    }
    assert!(Page::parse(&page(vec![r.clone(), r], 2, 1), 1, 2).is_err());
    assert!(Page::parse(&p, 2, 2).is_err());
    let mut bad = record("z", "漢", "한").fields;
    bad["regltn_code"] = json!("0004");
    assert!(Page::parse(&page(vec![bad], 1, 1), 1, 2).is_err());
    let p1 = Page::parse(
        &page(
            vec![
                record("a", "漢", "한").fields,
                record("b", "字", "자").fields,
            ],
            3,
            1,
        ),
        1,
        2,
    )
    .unwrap();
    let p2 = Page::parse(&page(vec![record("c", "山", "산").fields], 3, 2), 2, 2).unwrap();
    assert_eq!(
        Snapshot::from_pages(vec![p1.clone(), p2.clone()], "synthetic".into())
            .unwrap()
            .total_count,
        3
    );
    assert!(Snapshot::from_pages(vec![p1.clone()], "synthetic".into()).is_err());
    assert!(Snapshot::from_pages(vec![p2, p1], "synthetic".into()).is_err());
    assert!(Page::parse(&page(vec![], 0, 1), 1, 2).is_ok());
}
#[test]
fn snapshots_preserve_unknown_content_and_validate_integrity() {
    let mut r = record("a", " 金 ", " 금 ");
    r.fields["unknown"] = json!({"z":[true,null],"a":" unchanged "});
    let s = snapshot(vec![r]);
    assert_eq!(Snapshot::parse(&s.bytes()).unwrap(), s);
    let mut corrupt = json!(s);
    corrupt["total_count"] = json!(100);
    assert!(Snapshot::parse(&canonical_json(&corrupt)).is_err());
    corrupt = json!(s);
    corrupt["records"][0]["fields"]["unknown"]["a"] = json!("changed");
    assert!(Snapshot::parse(&canonical_json(&corrupt)).is_err());
}
#[test]
fn conservative_unicode_annotation_and_reading_grammar() {
    let originals = [
        (" 香港 (Xiānggǎng) ", true),
        ("香港（シャンガン）", true),
        ("香港(Xiānggǎng)", true),
        ("金", true),
        ("漢글", true),
        ("香 港", false),
        ("香港(note 漢)", false),
        ("香港()", false),
        ("香港(A)(B)", false),
        ("香港((A))", false),
        ("香港(X)", true),
        ("香港(注)", false),
        ("香港(123)", false),
        ("香港(A）", false),
        ("香カ港", false),
        ("漢-Latin", false),
        ("漢1", false),
        ("⼀", false),
        ("漢\u{fe00}", false),
        ("한글", false),
    ];
    for (i, (original, accepted)) in originals.iter().enumerate() {
        let s = snapshot(vec![record(&format!("r{i}"), original, " 한 글 ")]);
        let a = audit(&s, &Decisions::default());
        assert_eq!(!a.candidates.is_empty(), *accepted, "{original}");
        if *original == "金" {
            assert!(a.candidates.contains_key("金"));
            assert!(!a.candidates.contains_key("金"));
        }
    }
    for reading in [
        "",
        "한  글",
        "한\t글",
        "한/글",
        "한글1",
        "한",
        "한\n글",
        "한글-",
    ] {
        assert!(
            audit(
                &snapshot(vec![record("a", "漢", reading)]),
                &Decisions::default()
            )
            .candidates
            .is_empty(),
            "{reading:?}"
        );
    }
    let mut r = record("a", "Latin", "한");
    r.fields["relate_mark_o"] = json!("漢");
    r.fields["mean"] = json!("漢字");
    assert!(
        audit(&snapshot(vec![r]), &Decisions::default())
            .candidates
            .is_empty()
    );
}
#[test]
fn conflicts_evidence_corrections_and_deletions_fail_closed() {
    let s = conflict();
    assert!(matches!(
        extract(&s, &Decisions::default()),
        Err(Error::Decisions)
    ));
    let d = decisions(&s);
    let e = extract(&s, &d).unwrap();
    let t = String::from_utf8(e.tsv.clone()).unwrap();
    assert!(t.contains("香港\t홍콩\tfalse\tfalse\t1\t\ta\n"));
    assert!(t.contains("香港\t샹강\tfalse\tfalse\t0\t\tb\n"));
    let mut changed = s.records.clone();
    changed[0].fields["unknown-correction"] = json!(true);
    let new = snapshot(changed);
    assert!(extract(&new, &d).is_err());
    assert_eq!(new.diff(&s).changed, ["a"]);
    let new = snapshot(vec![s.records[0].clone()]);
    assert!(extract(&new, &d).is_err());
    assert_eq!(new.diff(&s).removed, ["b"]);
    let mut added = s.records.clone();
    added.push(record("c", "香港", "홍콩"));
    let new = snapshot(added);
    assert!(extract(&new, &d).is_err());
    assert_eq!(new.diff(&s).added, ["c"]);
    let mut duplicate = d.clone();
    duplicate.decisions.push(duplicate.decisions[0].clone());
    assert!(extract(&s, &duplicate).is_err());
    let mut wrong = d.clone();
    wrong.decisions[0].chosen = "틀림".into();
    assert!(extract(&s, &wrong).is_err());
    let mut reversed = s.records.clone();
    reversed.reverse();
    assert_eq!(extract(&snapshot(reversed), &d).unwrap().tsv, e.tsv);
}
#[test]
fn scalar_and_aggregate_backend_limits_are_checked() {
    let long = "한".repeat(21846);
    let s = snapshot(vec![record("a", "漢", &long)]);
    assert_eq!(
        audit(&s, &Decisions::default()).exclusions[0].reason,
        "reading-byte-limit"
    );
    let s = snapshot(vec![
        record("a", "漢", &"한".repeat(11000)),
        record("b", "漢", &"글".repeat(11000)),
    ]);
    let mut d = decisions_for_key(&s, "漢");
    d.decisions[0].chosen = "한".repeat(11000);
    assert!(extract(&s, &d).is_err());
    assert_eq!(
        audit(
            &snapshot(vec![record("a", &"漢".repeat(342), "한")]),
            &Decisions::default()
        )
        .exclusions[0]
            .reason,
        "key-byte-limit"
    );
}
fn decisions_for_key(s: &Snapshot, key: &str) -> Decisions {
    let c = audit(s, &Decisions::default()).candidates[key].clone();
    Decisions {
        format_version: 1,
        decisions: vec![Decision {
            key: key.into(),
            chosen: c[0].reading.clone(),
            expected: c,
            rationale: "Synthetic".into(),
            references: vec![],
        }],
    }
}
fn paths(dir: &Path, s: &Snapshot, d: &Decisions) -> ExtractionPaths {
    let p = ExtractionPaths {
        snapshot: dir.join("snapshot.json"),
        decisions: Some(dir.join("decisions.json")),
        prior_snapshot: None,
        tsv: dir.join("out.tsv"),
        report: dir.join("report.json"),
        diagnostic: Some(dir.join("failure.json")),
    };
    fs::write(&p.snapshot, s.bytes()).unwrap();
    fs::write(p.decisions.as_ref().unwrap(), canonical_json(&json!(d))).unwrap();
    p
}
#[test]
fn file_atomic_failures_check_and_pair_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let s = conflict();
    let mut p = paths(dir.path(), &s, &Decisions::default());
    fs::write(&p.tsv, "previous").unwrap();
    assert!(process_files(&p, false, false).is_err());
    assert_eq!(fs::read(&p.tsv).unwrap(), b"previous");
    assert_eq!(
        parse_json(&fs::read(p.diagnostic.as_ref().unwrap()).unwrap()).unwrap()["status"],
        "audit-only"
    );
    fs::write(
        p.decisions.as_ref().unwrap(),
        canonical_json(&json!(decisions(&s))),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&p.tsv, fs::Permissions::from_mode(0o640)).unwrap();
    }
    process_files(&p, false, false).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&p.tsv).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let ordinary = dir.path().join("ordinary-create");
        fs::write(&ordinary, b"umask reference").unwrap();
        assert_eq!(
            fs::metadata(&p.report).unwrap().permissions().mode() & 0o777,
            fs::metadata(ordinary).unwrap().permissions().mode() & 0o777,
        );
    }
    let before = fs::read(&p.report).unwrap();
    process_files(&p, true, false).unwrap();
    assert_eq!(fs::read(&p.report).unwrap(), before);
    fs::write(&p.tsv, "corrupt").unwrap();
    assert!(process_files(&p, true, false).is_err());
    assert_eq!(fs::read(&p.tsv).unwrap(), b"corrupt");
    p.report = dir.path().join("missing-parent/report.json");
    assert!(matches!(
        process_files(&p, false, false),
        Err(Error::IncompletePair)
    ));
    assert!(
        String::from_utf8(fs::read(&p.tsv).unwrap())
            .unwrap()
            .contains("香港")
    );
}
#[test]
fn all_collisions_fail_before_diagnostics_or_publication() {
    let dir = tempfile::tempdir().unwrap();
    let s = conflict();
    let p = paths(dir.path(), &s, &Decisions::default());
    let original = fs::read(&p.snapshot).unwrap();
    for dest in [&p.snapshot, p.decisions.as_ref().unwrap(), &p.tsv] {
        let mut bad = p.clone();
        bad.report = dest.clone();
        assert!(matches!(
            process_files(&bad, false, false),
            Err(Error::PathCollision)
        ));
    }
    // Reject absent destinations differing only in ASCII case on every platform.
    for (tsv, report) in [("out.tsv", "OUT.TSV"), ("Out.tsv", "out.tsv")] {
        let mut bad = p.clone();
        bad.tsv = dir.path().join(tsv);
        bad.report = dir.path().join(report);
        assert!(matches!(
            process_files(&bad, false, false),
            Err(Error::PathCollision)
        ));
        assert!(!bad.tsv.exists());
        assert!(!bad.report.exists());
    }
    let mut bad = p.clone();
    bad.diagnostic = Some(p.snapshot.clone());
    assert!(matches!(
        process_files(&bad, false, false),
        Err(Error::PathCollision)
    ));
    let mut bad = p.clone();
    bad.tsv = dir.path().join("./snapshot.json");
    assert!(matches!(
        process_files(&bad, false, false),
        Err(Error::PathCollision)
    ));
    let hardlink = dir.path().join("alias");
    fs::hard_link(&p.snapshot, &hardlink).unwrap();
    let mut bad = p.clone();
    bad.report = hardlink;
    assert!(matches!(
        process_files(&bad, false, false),
        Err(Error::PathCollision)
    ));
    #[cfg(unix)]
    {
        let alias = dir.path().join("linked");
        std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
        let mut bad = p.clone();
        bad.report = alias.join("snapshot.json");
        assert!(matches!(
            process_files(&bad, false, false),
            Err(Error::PathCollision)
        ));
    }
    assert_eq!(fs::read(&p.snapshot).unwrap(), original);
    assert!(!p.tsv.exists());
    assert!(!p.report.exists());
    assert!(!p.diagnostic.unwrap().exists());
}
#[test]
fn external_snapshots_are_atomic_and_check_ignores_time() {
    #[cfg(unix)]
    let dir = tempfile::tempdir_in("/var/tmp").unwrap();
    #[cfg(not(unix))]
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snapshot.json");
    let s = conflict();
    publish_snapshot(&path, &s, false).unwrap();
    let original = fs::read(&path).unwrap();
    let mut newer = s.clone();
    newer.collected_at = "later".into();
    publish_snapshot(&path, &newer, true).unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
    let different = snapshot(vec![]);
    assert!(publish_snapshot(&path, &different, true).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(collect_destination(Path::new("raw-snapshot.json")).is_err());
}
#[test]
fn candidate_backend_roundtrips_and_user_precedence() {
    use gukhanmun_core::{
        ChainDictionary, HanjaDictionary, MapDictionary, RenderMode, convert_plain_text,
    };
    use gukhanmun_mkdict::{BuildOptions, DictionaryFormat, build_dictionary};
    let s = conflict();
    let d = decisions(&s);
    let e = extract(&s, &d).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let tsv = dir.path().join("input.tsv");
    fs::write(&tsv, e.tsv).unwrap();
    for format in [DictionaryFormat::Fst, DictionaryFormat::Cdb] {
        let out = dir.path().join("dict.bin");
        build_dictionary(
            std::slice::from_ref(&tsv),
            &out,
            &BuildOptions {
                format,
                validate: true,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let dictionary: Box<dyn HanjaDictionary> = match format {
            DictionaryFormat::Fst => Box::new(gukhanmun_fst::FstDictionary::open(&out).unwrap()),
            DictionaryFormat::Cdb => Box::new(gukhanmun_cdb::CdbDictionary::open(&out).unwrap()),
        };
        let c = dictionary
            .candidate_matches_at("香港")
            .next()
            .unwrap()
            .candidates;
        assert_eq!(c.as_slice().len(), 2);
        assert_eq!(c.as_slice()[0].reading, "홍콩");
        assert!(c.as_slice()[0].preference > c.as_slice()[1].preference);
        assert_eq!(
            convert_plain_text("香港", &dictionary, RenderMode::HangulOnly),
            "홍콩"
        );
        let mut fallback = MapDictionary::new();
        fallback.insert("海港", "샹강");
        let effective: ChainDictionary<Box<dyn HanjaDictionary>> =
            [dictionary, Box::new(fallback) as Box<dyn HanjaDictionary>]
                .into_iter()
                .collect();
        assert!(!effective.has_homophone("海港", "샹강"));
        for segmentation in [
            gukhanmun_core::SegmentationStrategy::Lattice,
            gukhanmun_core::SegmentationStrategy::Eager,
        ] {
            use gukhanmun_core::{
                Engine, EngineOptions, InputToken, PlainScopeData, process_tokens_with_options,
                render_tokens, write_plain_text,
            };
            let options = EngineOptions {
                segmentation,
                ..EngineOptions::default()
            };
            let input = "香港과 海港";
            let expected = write_plain_text(render_tokens(
                process_tokens_with_options(
                    [InputToken::<PlainScopeData>::Text(input.into())],
                    &effective,
                    options,
                ),
                RenderMode::HangulHanjaParens,
            ));
            for split in input.char_indices().map(|(i, _)| i).chain([input.len()]) {
                let mut engine = Engine::<PlainScopeData, _>::with_options(&effective, options);
                let mut tokens = engine.push_token(InputToken::Text(input[..split].into()));
                tokens.extend(engine.push_token(InputToken::Text(input[split..].into())));
                tokens.extend(engine.finish());
                assert_eq!(
                    write_plain_text(render_tokens(tokens, RenderMode::HangulHanjaParens)),
                    expected
                );
            }
        }
        let dictionary: Box<dyn HanjaDictionary> = Box::new(effective);
        let mut user = MapDictionary::new();
        user.insert("香港", "사용자");
        let chain: ChainDictionary<Box<dyn HanjaDictionary>> =
            [Box::new(user) as Box<dyn HanjaDictionary>, dictionary]
                .into_iter()
                .collect();
        assert_eq!(
            chain
                .candidate_matches_at("香港")
                .next()
                .unwrap()
                .candidates
                .as_slice()
                .len(),
            1
        );
        assert_eq!(
            convert_plain_text("香港", &chain, RenderMode::HangulOnly),
            "사용자"
        );
    }
}

#[test]
fn live_response_envelope_preserves_nullable_records_without_operational_metadata() {
    let body = json!({"StatsVO":{"session":"private-session","field1":"private-ip"},"response":{"resultcode":0,"resultmsg":"NORMAL SERVICE","pageno":1,"numofrows":1,"totalcount":1,"items":[{"example_no":1,"srclang_mark":null,"korean_mark":null,"regltn_code":"0003"}]}});
    let p = Page::parse(&canonical_json(&body), 1, 1).unwrap();
    let snapshot = Snapshot::from_pages(vec![p], "synthetic".into()).unwrap();
    assert!(snapshot.records[0].fields["srclang_mark"].is_null());
    let bytes = String::from_utf8(snapshot.bytes()).unwrap();
    assert!(!bytes.contains("private-session"));
    assert!(!bytes.contains("private-ip"));
    assert_eq!(audit(&snapshot, &Decisions::default()).exclusions.len(), 1);
    for alias_code in [0, 99] {
        let mut ambiguous = body.clone();
        ambiguous["response"]["resultCode"] = json!(alias_code);
        assert!(Page::parse(&canonical_json(&ambiguous), 1, 1).is_err());
    }
    for invalid in [
        json!({"response":body["response"],"resultcode":0}),
        json!({"response":{"resultcode":"00"}}),
    ] {
        assert!(Page::parse(&canonical_json(&invalid), 1, 1).is_err());
    }
}

#[test]
fn unicode_destination_aliases_follow_the_actual_filesystem() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("out-é.tsv");
    let b = dir.path().join("out-e\u{301}.tsv");
    fs::write(&a, b"probe").unwrap();
    let aliases = b.exists() && same_file::is_same_file(&a, &b).unwrap();
    fs::remove_file(&a).unwrap();
    let result = preflight_paths(&[], &[&a, &b]);
    assert_eq!(matches!(result, Err(Error::PathCollision)), aliases);
    assert!(!a.exists());
    assert!(!b.exists());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[cfg(feature = "loanwords-collect")]
#[test]
fn named_service_key_and_legacy_alias_are_explicit_and_redacted() {
    #[cfg(unix)]
    let dir = tempfile::tempdir_in("/var/tmp").unwrap();
    #[cfg(not(unix))]
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("snapshot.json");
    let run = |current: Option<&str>, legacy: Option<&str>| {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_gukhanmun-loanwords"));
        command
            .env_remove("NIKL_LOANWORD_API_KEY")
            .env_remove("KOREAN_GO_KR_SERVICE_KEY");
        if let Some(key) = current {
            command.env("NIKL_LOANWORD_API_KEY", key);
        }
        if let Some(key) = legacy {
            command.env("KOREAN_GO_KR_SERVICE_KEY", key);
        }
        command
            .args(["collect", "--max-pages", "0", "--output"])
            .arg(&output)
            .output()
            .unwrap()
    };
    for (current, legacy) in [(Some("sentinel-key"), None), (None, Some("sentinel-key"))] {
        let result = run(current, legacy);
        assert!(!result.status.success());
        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(stderr.contains("collection limits"));
        assert!(!stderr.contains("sentinel-key"));
    }
    let result = run(Some("sentinel-key"), Some("sentinel-other"));
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains("conflicting service key environment variables"));
    assert!(!stderr.contains("sentinel"));
    assert!(!output.exists());
}
