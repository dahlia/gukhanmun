use std::fs;
use std::io::Write;

use assert_cmd::Command;
use gukhanmun_core::{RenderMode, convert_plain_text};
use gukhanmun_stdict::extract::{ExtractStats, extract_json_reader_to_tsv};
use proptest::prelude::*;
use tempfile::tempdir;
use zip::ZipWriter;
use zip::write::FileOptions;

#[test]
fn extracts_canonical_tsv_from_synthetic_json() {
    let mut output = Vec::new();
    let stats = extract_json_reader_to_tsv(synthetic_json().as_bytes(), &mut output).unwrap();

    assert_eq!(
        stats,
        ExtractStats {
            items_seen: 15,
            entries_written: 12,
            duplicate_keys: 3,
            skipped_items: 2,
        }
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "hanja\thangul\trequire_hanja\trequire_hangul\n\
         一分錢\t일푼전\tfalse\tfalse\n\
         一點鎖線\t일점쇄선\tfalse\tfalse\n\
         佈告하다\t포고하다\tfalse\tfalse\n\
         北京\t베이징\tfalse\tfalse\n\
         嚴戒\t엄계\tfalse\tfalse\n\
         天地\t천지\tfalse\tfalse\n\
         布告하다\t포고하다\tfalse\tfalse\n\
         標識\t표지\tfalse\tfalse\n\
         溫暖\t온난\tfalse\tfalse\n\
         溫煖\t온난\tfalse\tfalse\n\
         漢字\t한자\tfalse\tfalse\n\
         입口字집\t입구자집\tfalse\tfalse\n"
    );
}

#[test]
fn cli_extracts_from_zip_archives() {
    let temp = tempdir().unwrap();
    let zip_path = temp.path().join("stdict.zip");
    let output_path = temp.path().join("stdict.tsv");
    let zip_file = fs::File::create(&zip_path).unwrap();
    let mut zip = ZipWriter::new(zip_file);
    zip.start_file::<_, ()>("part.json", FileOptions::default())
        .unwrap();
    zip.write_all(synthetic_json().as_bytes()).unwrap();
    zip.finish().unwrap();

    Command::cargo_bin("gukhanmun-stdict-extract")
        .unwrap()
        .args([
            "-o",
            output_path.to_str().unwrap(),
            zip_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stderr(predicates::str::contains("entries_written=12"));

    let output = fs::read_to_string(output_path).unwrap();
    assert!(output.contains("漢字\t한자\tfalse\tfalse\n"));
}

#[test]
fn bundled_dictionary_converts_stdict_entries() {
    let dict = gukhanmun_stdict::ko_kr();

    assert!(dict.entry_count() > 200_000);
    assert_eq!(dict.lookup("漢字").unwrap().unwrap().reading(), "한자");
    assert_eq!(dict.lookup("北京").unwrap().unwrap().reading(), "베이징");
    assert_eq!(dict.lookup("標識").unwrap().unwrap().reading(), "표지");
    assert_eq!(dict.lookup("一分錢").unwrap().unwrap().reading(), "일푼전");
    assert_eq!(
        dict.lookup("布告하다").unwrap().unwrap().reading(),
        "포고하다"
    );
    assert_eq!(
        dict.lookup("佈告하다").unwrap().unwrap().reading(),
        "포고하다"
    );
    assert_eq!(
        convert_plain_text("行事場入口", dict, RenderMode::HangulHanjaParens),
        "행사장(行事場)입구(入口)"
    );
    assert_eq!(
        convert_plain_text("行事場所", dict, RenderMode::HangulHanjaParens),
        "행사(行事)장소(場所)"
    );
    assert_eq!(
        convert_plain_text("입口字집", dict, RenderMode::HangulOnly),
        "입구자집"
    );
}

proptest! {
    #[test]
    fn generated_single_hanja_items_extract_deterministically(
        hanja in "[一-龥]{1,3}",
        reading in "[가-힣]{1,5}",
    ) {
        let json = format!(
            r#"{{"channel":{{"item":[{{"word_info":{{"word":"{reading}","word_unit":"단어","word_type":"한자어","pronunciation_info":[{{"pronunciation":"{reading}"}}],"original_language_info":[{{"original_language":"{hanja}","language_type":"한자"}}]}}}}]}}}}"#
        );
        let mut first = Vec::new();
        let mut second = Vec::new();

        let first_stats = extract_json_reader_to_tsv(json.as_bytes(), &mut first).unwrap();
        let second_stats = extract_json_reader_to_tsv(json.as_bytes(), &mut second).unwrap();

        prop_assert_eq!(first_stats, second_stats);
        prop_assert_eq!(&first, &second);
        let output = String::from_utf8(first).unwrap();
        let expected = format!("{hanja}\t{reading}\tfalse\tfalse\n");
        prop_assert!(output.contains(&expected));
    }

    #[test]
    fn generated_bracketed_foreign_hanja_items_use_the_hanja_spelling(
        romanized in "[A-Za-z]{1,8}",
        hanja in "[一-龥]{1,3}",
        reading in "[가-힣]{1,5}",
    ) {
        let json = format!(
            r#"{{"channel":{{"item":[{{"word_info":{{"word":"{reading}","word_unit":"단어","word_type":"외래어","original_language_info":[{{"original_language":"{romanized}[{hanja}]","language_type":"안 밝힘"}}]}}}}]}}}}"#
        );
        let mut output = Vec::new();

        let stats = extract_json_reader_to_tsv(json.as_bytes(), &mut output).unwrap();

        prop_assert_eq!(
            stats,
            ExtractStats {
                items_seen: 1,
                entries_written: 1,
                duplicate_keys: 0,
                skipped_items: 0,
            }
        );
        let output = String::from_utf8(output).unwrap();
        let expected = format!("{hanja}\t{reading}\tfalse\tfalse\n");
        let discarded_prefix = format!("{romanized}[");
        prop_assert!(output.contains(&expected));
        prop_assert!(!output.contains(&discarded_prefix));
    }
}

fn synthetic_json() -> &'static str {
    r#"
{
  "channel": {
    "item": [
      {
        "word_info": {
          "word": "한자",
          "word_unit": "단어",
          "word_type": "한자어",
          "pronunciation_info": [{ "pronunciation": "한자" }],
          "original_language_info": [
            { "original_language": "漢字", "language_type": "한자" }
          ]
        }
      },
      {
        "word_info": {
          "word": "엄계",
          "word_unit": "단어",
          "word_type": "한자어",
          "pronunciation_info": [
            { "pronunciation": "엄계" },
            { "pronunciation": "엄게" }
          ],
          "original_language_info": [
            { "original_language": "嚴戒", "language_type": "한자" }
          ]
        }
      },
      {
        "word_info": {
          "word": "온난",
          "word_unit": "단어",
          "word_type": "한자어",
          "pronunciation_info": [{ "pronunciation": "온난" }],
          "original_language_info": [
            { "original_language": "溫暖", "language_type": "한자" },
            { "original_language": "/", "language_type": "/(병기)" },
            { "original_language": "溫煖", "language_type": "한자" }
          ]
        }
      },
      {
        "word_info": {
          "word": "입구자-집",
          "word_unit": "단어",
          "word_type": "혼종어",
          "pronunciation_info": [{ "pronunciation": "입꾸짜집" }],
          "original_language_info": [
            { "original_language": "입", "language_type": "고유어" },
            { "original_language": "口字", "language_type": "한자" },
            { "original_language": "집", "language_type": "고유어" }
          ]
        }
      },
      {
        "word_info": {
          "word": "북경",
          "word_unit": "단어",
          "word_type": "한자어",
          "original_language_info": [
            { "original_language": "北京", "language_type": "한자" }
          ]
        }
      },
      {
        "word_info": {
          "word": "베이징",
          "word_unit": "단어",
          "word_type": "외래어",
          "original_language_info": [
            { "original_language": "Beijing[北京]", "language_type": "안 밝힘" }
          ]
        }
      },
      {
        "word_info": {
          "word": "일점쇄선",
          "word_unit": "단어",
          "word_type": "한자어",
          "pronunciation_info": [{ "pronunciation": "일쩜쇄선" }],
          "original_language_info": [
            { "original_language": "一點<equ>&#x9396;</equ>線", "language_type": "한자" }
          ]
        }
      },
      {
        "word_info": {
          "word": "일푼전",
          "word_unit": "단어",
          "word_type": "한자어",
          "original_language_info": [
            { "original_language": "一分▽錢", "language_type": "한자" }
          ]
        }
      },
      {
        "word_info": {
          "word": "포고-하다",
          "word_unit": "단어",
          "word_type": "한자어",
          "original_language_info": [
            { "original_language": "布告하다/佈告하다", "language_type": "한자" }
          ]
        }
      },
      {
        "word_info": {
          "word": "표식03",
          "word_unit": "단어",
          "word_type": "한자어",
          "original_language_info": [
            { "original_language": "標識", "language_type": "한자" }
          ],
          "pos_info": [
            {
              "comm_pattern_info": [
                {
                  "sense_info": [
                    { "definition": "→ 표지03.", "definition_original": "→ <word_no>500357</word_no>표지_." }
                  ]
                }
              ]
            }
          ]
        }
      },
      {
        "word_info": {
          "word": "표지03",
          "word_unit": "단어",
          "word_type": "한자어",
          "original_language_info": [
            { "original_language": "標識", "language_type": "한자" }
          ],
          "pos_info": [
            {
              "comm_pattern_info": [
                {
                  "sense_info": [
                    { "definition": "표시나 특징으로 어떤 사물을 다른 것과 구별하게 함." }
                  ]
                }
              ]
            }
          ]
        }
      },
      {
        "word_info": {
          "word": "중복한자",
          "word_unit": "단어",
          "word_type": "한자어",
          "pronunciation_info": [{ "pronunciation": "다른한자" }],
          "original_language_info": [
            { "original_language": "漢字", "language_type": "한자" }
          ]
        }
      },
      {
        "word_info": {
          "word": "천지01",
          "word_unit": "단어",
          "word_type": "한자어",
          "original_language_info": [
            { "original_language": "天地", "language_type": "한자" }
          ]
        }
      },
      {
        "word_info": {
          "word": "리포-산",
          "word_unit": "단어",
          "word_type": "혼종어",
          "original_language_info": [
            { "original_language": "←lipoic", "language_type": "영어" },
            { "original_language": "酸", "language_type": "한자" }
          ]
        }
      },
      {
        "word_info": {
          "word": "속담",
          "word_unit": "속담",
          "word_type": "",
          "pronunciation_info": [],
          "original_language_info": null
        }
      }
    ]
  }
}
"#
}
