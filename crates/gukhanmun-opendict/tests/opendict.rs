// Gukhanmun: Bundled Open Korean Dictionary data for Gukhanmun.
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

use std::fs;
use std::io::Write;

use assert_cmd::Command;
use gukhanmun_core::HanjaDictionary;
use gukhanmun_opendict::extract::{CategoryWriters, ExtractStats, extract_json_reader_to_files};
use predicates::str::contains;
use tempfile::tempdir;
use zip::ZipWriter;
use zip::write::FileOptions;

#[test]
fn extracts_category_tsvs_from_synthetic_json() {
    let mut general = Vec::new();
    let mut north_korean = Vec::new();
    let mut dialect = Vec::new();
    let mut archaic = Vec::new();
    let stats = extract_json_reader_to_files(
        synthetic_json().as_bytes(),
        CategoryWriters {
            general: &mut general,
            north_korean: &mut north_korean,
            dialect: &mut dialect,
            archaic: &mut archaic,
        },
    )
    .unwrap();

    assert_eq!(
        stats.general,
        ExtractStats {
            items_seen: 7,
            entries_written: 5,
            duplicate_keys: 1,
            skipped_items: 2,
            skipped_default_pins: include_str!("../data/defaults.tsv")
                .lines()
                .skip(1)
                .filter(|line| line.starts_with("general\t"))
                .count(),
        }
    );
    let general = String::from_utf8(general).unwrap();
    assert_eq!(
        general,
        "hanja\thangul\trequire_hanja\trequire_hangul\tpreference\tsuffix_reading\tsource_ids\n\
         勞動\t노동\tfalse\tfalse\t\t\t\n\
         北京\t베이징\tfalse\tfalse\t\t\t\n\
         歷史\t다른역사\tfalse\tfalse\t1\t\t\n\
         歷史\t역사\tfalse\tfalse\t0\t\t\n\
         색깔論\t색깔론\tfalse\tfalse\t\t\t\n"
    );
    assert!(!general.contains("引\t히끼"));
    assert_eq!(
        String::from_utf8(north_korean).unwrap(),
        "hanja\thangul\trequire_hanja\trequire_hangul\tpreference\tsuffix_reading\tsource_ids\n\
         勞動\t로동\tfalse\tfalse\t\t\t\n\
         歷史\t력사\tfalse\tfalse\t\t\t\n"
    );
    assert_eq!(
        String::from_utf8(dialect).unwrap(),
        "hanja\thangul\trequire_hanja\trequire_hangul\tpreference\tsuffix_reading\tsource_ids\n\
         濟州\t제주\tfalse\tfalse\t\t\t\n"
    );
    assert_eq!(
        String::from_utf8(archaic).unwrap(),
        "hanja\thangul\trequire_hanja\trequire_hangul\tpreference\tsuffix_reading\tsource_ids\n\
         古語\t고어\tfalse\tfalse\t\t\t\n"
    );
}

#[test]
fn cli_ignores_json_named_subdirectories() {
    let temp = tempdir().unwrap();
    let input_dir = temp.path().join("input");
    let general_path = temp.path().join("general.tsv");
    let north_korean_path = temp.path().join("north-korean.tsv");
    let dialect_path = temp.path().join("dialect.tsv");
    let archaic_path = temp.path().join("archaic.tsv");
    fs::create_dir(&input_dir).unwrap();
    fs::write(input_dir.join("part.json"), synthetic_json()).unwrap();
    fs::create_dir(input_dir.join("nested.json")).unwrap();

    Command::cargo_bin("gukhanmun-opendict-extract")
        .unwrap()
        .args([
            input_dir.to_str().unwrap(),
            "--general-output",
            general_path.to_str().unwrap(),
            "--north-korean-output",
            north_korean_path.to_str().unwrap(),
            "--dialect-output",
            dialect_path.to_str().unwrap(),
            "--archaic-output",
            archaic_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(
        fs::read_to_string(general_path)
            .unwrap()
            .contains("歷史\t역사")
    );
}

#[test]
fn cli_reads_uppercase_json_extensions_in_directories() {
    let temp = tempdir().unwrap();
    let input_dir = temp.path().join("input");
    let general_path = temp.path().join("general.tsv");
    let north_korean_path = temp.path().join("north-korean.tsv");
    let dialect_path = temp.path().join("dialect.tsv");
    let archaic_path = temp.path().join("archaic.tsv");
    fs::create_dir(&input_dir).unwrap();
    fs::write(input_dir.join("part.JSON"), synthetic_json()).unwrap();

    Command::cargo_bin("gukhanmun-opendict-extract")
        .unwrap()
        .args([
            input_dir.to_str().unwrap(),
            "--general-output",
            general_path.to_str().unwrap(),
            "--north-korean-output",
            north_korean_path.to_str().unwrap(),
            "--dialect-output",
            dialect_path.to_str().unwrap(),
            "--archaic-output",
            archaic_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(
        fs::read_to_string(general_path)
            .unwrap()
            .contains("歷史\t역사")
    );
}

#[test]
fn cli_extracts_from_zip_archives() {
    let temp = tempdir().unwrap();
    let zip_path = temp.path().join("opendict.zip");
    let general_path = temp.path().join("general.tsv");
    let north_korean_path = temp.path().join("north-korean.tsv");
    let dialect_path = temp.path().join("dialect.tsv");
    let archaic_path = temp.path().join("archaic.tsv");
    let zip_file = fs::File::create(&zip_path).unwrap();
    let mut zip = ZipWriter::new(zip_file);
    zip.start_file::<_, ()>("part.json", FileOptions::default())
        .unwrap();
    zip.write_all(synthetic_json().as_bytes()).unwrap();
    zip.finish().unwrap();

    Command::cargo_bin("gukhanmun-opendict-extract")
        .unwrap()
        .args([
            zip_path.to_str().unwrap(),
            "--general-output",
            general_path.to_str().unwrap(),
            "--north-korean-output",
            north_korean_path.to_str().unwrap(),
            "--dialect-output",
            dialect_path.to_str().unwrap(),
            "--archaic-output",
            archaic_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stderr(contains("extraction complete"));

    assert!(
        fs::read_to_string(general_path)
            .unwrap()
            .contains("歷史\t역사")
    );
    assert!(
        fs::read_to_string(north_korean_path)
            .unwrap()
            .contains("歷史\t력사")
    );
    assert!(
        fs::read_to_string(dialect_path)
            .unwrap()
            .contains("濟州\t제주")
    );
    assert!(
        fs::read_to_string(archaic_path)
            .unwrap()
            .contains("古語\t고어")
    );
}

#[test]
fn cli_extracts_from_uppercase_zip_archive_extensions() {
    let temp = tempdir().unwrap();
    let zip_path = temp.path().join("opendict.ZIP");
    let general_path = temp.path().join("general.tsv");
    let north_korean_path = temp.path().join("north-korean.tsv");
    let dialect_path = temp.path().join("dialect.tsv");
    let archaic_path = temp.path().join("archaic.tsv");
    let zip_file = fs::File::create(&zip_path).unwrap();
    let mut zip = ZipWriter::new(zip_file);
    zip.start_file::<_, ()>("part.JSON", FileOptions::default())
        .unwrap();
    zip.write_all(synthetic_json().as_bytes()).unwrap();
    zip.finish().unwrap();

    Command::cargo_bin("gukhanmun-opendict-extract")
        .unwrap()
        .args([
            zip_path.to_str().unwrap(),
            "--general-output",
            general_path.to_str().unwrap(),
            "--north-korean-output",
            north_korean_path.to_str().unwrap(),
            "--dialect-output",
            dialect_path.to_str().unwrap(),
            "--archaic-output",
            archaic_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(
        fs::read_to_string(general_path)
            .unwrap()
            .contains("歷史\t역사")
    );
}

#[test]
fn bundled_dictionaries_are_partitioned_by_category() {
    let general = gukhanmun_opendict::general();
    let north_korean = gukhanmun_opendict::north_korean();
    let dialect = gukhanmun_opendict::dialect();
    let archaic = gukhanmun_opendict::archaic();

    assert_eq!(general.entry_count(), 350_330);
    assert_eq!(north_korean.entry_count(), 34_093);
    assert_eq!(dialect.entry_count(), 5_714);
    assert_eq!(archaic.entry_count(), 16);

    assert_eq!(general.lookup("歷史").unwrap().unwrap().reading(), "역사");
    assert_eq!(general.lookup("來日").unwrap().unwrap().reading(), "내일");
    assert_eq!(general.lookup("勞動").unwrap().unwrap().reading(), "노동");

    assert_eq!(
        north_korean.lookup("歷史").unwrap().unwrap().reading(),
        "력사"
    );
    assert_eq!(
        north_korean.lookup("來日").unwrap().unwrap().reading(),
        "래일"
    );
    assert_eq!(
        north_korean.lookup("勞動").unwrap().unwrap().reading(),
        "로동"
    );
    assert_eq!(north_korean.max_word_chars(), Some(17));

    assert_eq!(
        dialect.lookup("一家방답").unwrap().unwrap().reading(),
        "일가방답"
    );
    assert_eq!(archaic.lookup("禮數").unwrap().unwrap().reading(), "례수");
}

fn synthetic_json() -> &'static str {
    r#"
{
  "channel": {
    "item": [
      {
        "wordinfo": {
          "word": "역사",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "歷史", "language_type": "한자" }
          ]
        },
        "senseinfo": { "type": "일반어" }
      },
      {
        "wordinfo": {
          "word": "다른역사",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "歷史", "language_type": "한자" }
          ]
        },
        "senseinfo": { "type": "일반어" }
      },
      {
        "wordinfo": {
          "word": "베이징",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "Beijing[北京]", "language_type": "안밝힘" }
          ]
        },
        "senseinfo": { "type": " 일반어 " }
      },
      {
        "wordinfo": {
          "word": "색깔-론",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "색깔", "language_type": "고유어" },
            { "original_language": "論", "language_type": "한자" }
          ]
        },
        "senseinfo": { "type": "일반어" }
      },
      {
        "wordinfo": {
          "word": "히끼",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "hiki[引]", "language_type": "안밝힘" }
          ]
        },
        "senseinfo": { "type": "일반어" }
      },
      {
        "wordinfo": {
          "word": "전문어",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "專門語", "language_type": "한자" }
          ]
        },
        "senseinfo": { "type": "전문어" }
      },
      {
        "wordinfo": {
          "word": "로동",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "勞動", "language_type": "한자" }
          ]
        },
        "senseinfo": { "type": "북한어" }
      },
      {
        "wordinfo": {
          "word": "력사",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "歷史", "language_type": "한자" }
          ]
        },
        "senseinfo": { "type": "북한어" }
      },
      {
        "wordinfo": {
          "word": "노동",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "勞動", "language_type": "한자" }
          ]
        },
        "senseinfo": { "type": "일반어" }
      },
      {
        "wordinfo": {
          "word": "제주",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "濟州", "language_type": "한자" }
          ]
        },
        "senseinfo": { "type": "방언" }
      },
      {
        "wordinfo": {
          "word": "고어",
          "word_unit": "어휘",
          "original_language_info": [
            { "original_language": "古語", "language_type": "한자" }
          ]
        },
        "senseinfo": { "type": "옛말" }
      },
      {
        "wordinfo": {
          "word": "관용구",
          "word_unit": "관용구",
          "original_language_info": [
            { "original_language": "慣用句", "language_type": "한자" }
          ]
        },
        "senseinfo": { "type": "일반어" }
      }
    ]
  }
}
"#
}

#[test]
fn cli_check_reports_all_drift_without_creating_or_overwriting_outputs() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("input.json");
    fs::write(&input, synthetic_json()).unwrap();
    let paths = ["general.tsv", "north.tsv", "dialect.tsv", "archaic.tsv"]
        .map(|name| temp.path().join(name));
    fs::write(&paths[0], "sentinel").unwrap();
    let mut command = Command::cargo_bin("gukhanmun-opendict-extract").unwrap();
    command
        .arg(&input)
        .args(["--general-output"])
        .arg(&paths[0])
        .arg("--north-korean-output")
        .arg(&paths[1])
        .arg("--dialect-output")
        .arg(&paths[2])
        .arg("--archaic-output")
        .arg(&paths[3])
        .arg("--check")
        .assert()
        .failure()
        .stderr(contains("general.tsv"))
        .stderr(contains("north.tsv"))
        .stderr(contains("dialect.tsv"))
        .stderr(contains("archaic.tsv"));
    assert_eq!(fs::read(&paths[0]).unwrap(), b"sentinel");
    assert!(paths[1..].iter().all(|path| !path.exists()));
    // Malformed source must also leave existing snapshots untouched in write mode.
    fs::write(&input, "{").unwrap();
    Command::cargo_bin("gukhanmun-opendict-extract")
        .unwrap()
        .arg(&input)
        .arg("--general-output")
        .arg(&paths[0])
        .arg("--north-korean-output")
        .arg(&paths[1])
        .arg("--dialect-output")
        .arg(&paths[2])
        .arg("--archaic-output")
        .arg(&paths[3])
        .assert()
        .failure();
    assert_eq!(fs::read(&paths[0]).unwrap(), b"sentinel");
    assert!(paths[1..].iter().all(|path| !path.exists()));
}

#[test]
fn reordered_shards_and_items_produce_identical_outputs() {
    use gukhanmun_opendict::extract::extract_path_to_files;
    let temp = tempdir().unwrap();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    let value: serde_json::Value = serde_json::from_str(synthetic_json()).unwrap();
    let items = value["channel"]["item"].as_array().unwrap();
    for (index, item) in items.iter().enumerate() {
        let dump = serde_json::json!({"channel": {"item": [item]}}).to_string();
        fs::write(first.join(format!("{index:03}.json")), &dump).unwrap();
        fs::write(
            second.join(format!("{:03}.json", items.len() - index)),
            &dump,
        )
        .unwrap();
    }
    let extract = |path: &std::path::Path| {
        let mut outputs = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let [general, north_korean, dialect, archaic] = &mut outputs;
        extract_path_to_files(
            path,
            CategoryWriters {
                general,
                north_korean,
                dialect,
                archaic,
            },
        )
        .unwrap();
        outputs
    };
    assert_eq!(extract(&first), extract(&second));
}

#[test]
fn all_category_candidates_roundtrip_through_fst_and_cdb() {
    use gukhanmun_mkdict::{BuildOptions, DictionaryFormat, MergePolicy, build_dictionary};
    use std::collections::BTreeMap;
    let temp = tempdir().unwrap();
    for (category, fst) in [
        ("general", gukhanmun_opendict::general()),
        ("north-korean", gukhanmun_opendict::north_korean()),
        ("dialect", gukhanmun_opendict::dialect()),
        ("archaic", gukhanmun_opendict::archaic()),
    ] {
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join(format!("{category}.tsv"));
        let output = temp.path().join(format!("{category}.cdb"));
        build_dictionary(
            &[input],
            &output,
            &BuildOptions {
                format: DictionaryFormat::Cdb,
                merge: MergePolicy::Error,
                validate: true,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        let cdb = gukhanmun_cdb::CdbDictionary::open(&output).unwrap();
        let entries = |dictionary: &dyn HanjaDictionary| {
            dictionary
                .candidate_entries()
                .unwrap()
                .map(|entry| (entry.hanja, entry.candidates))
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(entries(fst), entries(&cdb), "{category}");
    }
}

#[test]
fn actual_candidates_preserve_defaults_priority_homophones_and_streaming() {
    use gukhanmun_core::{
        ChainDictionary, ContextWindow, Engine, EngineOptions, HomophoneDetection, InputToken,
        MapDictionary, OutputToken, PlainScopeData, RenderMode, SegmentationStrategy,
        mark_homophones_with_detection, process_tokens_with_options, read_plain_text,
        render_tokens, write_plain_text,
    };
    let general = gukhanmun_opendict::general();
    let north = gukhanmun_opendict::north_korean();
    assert_eq!(
        general
            .lookup("俳句")
            .unwrap()
            .unwrap()
            .candidates()
            .as_slice()
            .len(),
        2
    );
    assert_eq!(general.lookup("俳句").unwrap().unwrap().reading(), "하이쿠");
    // The real unselected 丁髷 → 존마게 must not introduce a homophone for
    // a separate user entry. Isolate it from genuine homophones in the corpus.
    assert!(!general.has_homophone("假語", "존마게"));
    let mut probe = MapDictionary::new();
    probe.insert("假語", "존마게");
    let mut homophones = ChainDictionary::new();
    homophones.push(&probe as &dyn HanjaDictionary);
    homophones.push(general as &dyn HanjaDictionary);
    let marked = mark_homophones_with_detection(
        gukhanmun_core::process_tokens(read_plain_text("假語"), &homophones),
        &homophones,
        ContextWindow::PerDocument,
        HomophoneDetection::DictionaryWide,
    );
    assert!(
        !marked
            .iter()
            .any(|t| matches!(t,OutputToken::Annotated(a) if a.homophone))
    );
    for (dictionaries, expected, count) in [
        (vec![north as &dyn HanjaDictionary, general], "계치", 2),
        (vec![general as &dyn HanjaDictionary, north], "해치", 1),
    ] {
        let mut chain = ChainDictionary::new();
        for dictionary in dictionaries {
            chain.push(dictionary);
        }
        let selected = chain.candidate_matches_at("齘齒").next().unwrap();
        assert_eq!(selected.candidates.as_slice().len(), count);
        assert_eq!(selected.select_default().reading, expected);
    }
    for input in ["俳句 排球", "1990年代 1998年度", "비率 年率 年"] {
        let mut baseline = None;
        for segmentation in [SegmentationStrategy::Eager, SegmentationStrategy::Lattice] {
            let options = EngineOptions {
                segmentation,
                ..EngineOptions::default()
            };
            let expected = write_plain_text(render_tokens(
                process_tokens_with_options(
                    [InputToken::<PlainScopeData>::Text(input.into())],
                    general,
                    options,
                ),
                RenderMode::HangulHanjaParens,
            ));
            if let Some(ref baseline) = baseline {
                assert_eq!(&expected, baseline);
            } else {
                baseline = Some(expected.clone());
            }
            for split in input.char_indices().map(|(i, _)| i).chain([input.len()]) {
                let mut engine = Engine::<PlainScopeData, _>::with_options(general, options);
                let mut tokens = engine.push_token(InputToken::Text(input[..split].into()));
                tokens.extend(engine.push_token(InputToken::Text(input[split..].into())));
                tokens.extend(engine.finish());
                assert_eq!(
                    write_plain_text(render_tokens(tokens, RenderMode::HangulHanjaParens)),
                    expected,
                    "{input} split {split} {segmentation:?}"
                );
            }
        }
    }
}

#[test]
fn pin_drift_validation_leaves_every_library_writer_untouched() {
    for (key, category) in [("俳句", "일반어"), ("齘齒", "북한어")] {
        let input = serde_json::json!({"channel": {"item": [{
            "target_code": 1,
            "wordinfo": {"word":"새로운독음", "word_unit":"어휘", "original_language_info": [
                {"original_language":key, "language_type":"한자"}]},
            "senseinfo": {"type":category}
        }, {
            "target_code": 2,
            "wordinfo": {"word":"갑을", "word_unit":"어휘", "original_language_info": [
                {"original_language":"甲乙", "language_type":"한자"}]},
            "senseinfo": {"type":"일반어"}
        }]}})
        .to_string();
        let mut outputs = [
            b"first".to_vec(),
            b"second".to_vec(),
            b"third".to_vec(),
            b"fourth".to_vec(),
        ];
        let before = outputs.clone();
        let [general, north_korean, dialect, archaic] = &mut outputs;
        let error = extract_json_reader_to_files(
            input.as_bytes(),
            CategoryWriters {
                general,
                north_korean,
                dialect,
                archaic,
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("candidate set changed"));
        assert_eq!(outputs, before, "{category}/{key}");
    }
}

#[test]
fn partial_pinned_keys_allow_known_readings_but_full_inputs_require_signatures() {
    use gukhanmun_opendict::extract::extract_path_to_files;

    let dump = |readings: &[&str]| {
        serde_json::json!({"channel": {"item": readings.iter().enumerate().map(|(id, word)| {
            serde_json::json!({
                "target_code": id + 1,
                "wordinfo": {"word": word, "word_unit": "어휘", "original_language_info": [
                    {"original_language": "七分", "language_type": "한자"}]},
                "senseinfo": {"type": "일반어"}
            })
        }).collect::<Vec<_>>()}})
        .to_string()
    };
    let temp = tempdir().unwrap();
    let path = temp.path().join("shard.json");
    for reading in ["시치부", "칠분"] {
        let input = dump(&[reading]);
        fs::write(&path, &input).unwrap();
        for from_path in [false, true] {
            let mut general = Vec::new();
            let writers = CategoryWriters {
                general: &mut general,
                north_korean: Vec::new(),
                dialect: Vec::new(),
                archaic: Vec::new(),
            };
            let stats = if from_path {
                extract_path_to_files(&path, writers)
            } else {
                extract_json_reader_to_files(input.as_bytes(), writers)
            }
            .unwrap();
            assert_eq!(stats.general.skipped_default_pins, 349);
            assert!(
                String::from_utf8(general)
                    .unwrap()
                    .contains(&format!("七分\t{reading}\tfalse\tfalse\t\t\t1\n"))
            );
        }
        let mut general = b"sentinel".to_vec();
        let error = extract_path_to_files(
            temp.path(),
            CategoryWriters {
                general: &mut general,
                north_korean: Vec::new(),
                dialect: Vec::new(),
                archaic: Vec::new(),
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("candidate set changed"));
        assert_eq!(general, b"sentinel");
    }
    let input = dump(&["시치부", "칠분"]);
    let mut general = Vec::new();
    let stats = extract_json_reader_to_files(
        input.as_bytes(),
        CategoryWriters {
            general: &mut general,
            north_korean: Vec::new(),
            dialect: Vec::new(),
            archaic: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(stats.general.skipped_default_pins, 348);
    let output = String::from_utf8(general).unwrap();
    assert!(output.contains("七分\t시치부\tfalse\tfalse\t0\t\t1\n"));
    assert!(output.contains("七分\t칠분\tfalse\tfalse\t1\t\t2\n"));
}

#[test]
fn partial_pinned_positional_key_accepts_initial_without_its_suffix_record() {
    let input = serde_json::json!({"channel": {"item": [{
        "target_code": 1,
        "wordinfo": {"word": "연대", "word_unit": "어휘", "original_language_info": [
            {"original_language": "年代", "language_type": "한자"}]},
        "senseinfo": {"type": "일반어"}
    }]}})
    .to_string();
    let mut general = Vec::new();
    let stats = extract_json_reader_to_files(
        input.as_bytes(),
        CategoryWriters {
            general: &mut general,
            north_korean: Vec::new(),
            dialect: Vec::new(),
            archaic: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(stats.general.skipped_default_pins, 349);
    assert!(
        String::from_utf8(general)
            .unwrap()
            .contains("年代\t연대\tfalse\tfalse\t\t\t1\n")
    );
}
