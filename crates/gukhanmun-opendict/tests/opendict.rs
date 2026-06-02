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
            items_seen: 6,
            entries_written: 4,
            duplicate_keys: 1,
            skipped_items: 1,
        }
    );
    assert_eq!(
        String::from_utf8(general).unwrap(),
        "hanja\thangul\trequire_hanja\trequire_hangul\n\
         勞動\t노동\tfalse\tfalse\n\
         北京\t베이징\tfalse\tfalse\n\
         歷史\t역사\tfalse\tfalse\n\
         색깔論\t색깔론\tfalse\tfalse\n"
    );
    assert_eq!(
        String::from_utf8(north_korean).unwrap(),
        "hanja\thangul\trequire_hanja\trequire_hangul\n\
         勞動\t로동\tfalse\tfalse\n\
         歷史\t력사\tfalse\tfalse\n"
    );
    assert_eq!(
        String::from_utf8(dialect).unwrap(),
        "hanja\thangul\trequire_hanja\trequire_hangul\n\
         濟州\t제주\tfalse\tfalse\n"
    );
    assert_eq!(
        String::from_utf8(archaic).unwrap(),
        "hanja\thangul\trequire_hanja\trequire_hangul\n\
         古語\t고어\tfalse\tfalse\n"
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
        .stderr(predicates::str::contains("extraction complete"));

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
fn bundled_dictionaries_are_partitioned_by_category() {
    let general = gukhanmun_opendict::general();
    let north_korean = gukhanmun_opendict::north_korean();
    let dialect = gukhanmun_opendict::dialect();
    let archaic = gukhanmun_opendict::archaic();

    assert_eq!(general.entry_count(), 350_383);
    assert_eq!(north_korean.entry_count(), 34_093);
    assert_eq!(dialect.entry_count(), 5_715);
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
