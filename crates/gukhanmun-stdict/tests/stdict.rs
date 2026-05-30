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

use std::fs;
use std::io::Write;

use assert_cmd::Command;
use gukhanmun_core::{HanjaDictionary, RenderMode, convert_plain_text};
use gukhanmun_stdict::extract::{ExtractStats, extract_json_reader_to_tsv};
use proptest::prelude::*;
use tempfile::tempdir;
use tracing_test::traced_test;
use zip::ZipWriter;
use zip::write::FileOptions;

#[traced_test]
#[test]
fn extraction_emits_diagnostic_events() {
    let mut output = Vec::new();
    extract_json_reader_to_tsv(synthetic_json().as_bytes(), &mut output).unwrap();

    assert!(
        logs_contain("wrote dictionary TSV"),
        "info event for write completion"
    );
    assert!(
        logs_contain("processed JSON dump"),
        "debug event for each JSON shard"
    );
}

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
        .stderr(predicates::str::contains("extraction complete"));

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
    // `引` carries its Sino-Korean reading 인 (so 引數 -> 인수, 引用 -> 인용);
    // it must not map to the 삐끼 (Japanese 引き) loanword headword, which would
    // otherwise shadow the correct reading for every 引 compound.
    assert_eq!(dict.lookup("引").unwrap().unwrap().reading(), "인");
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

#[test]
fn bundled_dictionary_applies_initial_sound_law_by_position() {
    let dict = gukhanmun_stdict::ko_kr();

    // `年` after a number keeps its original sound (single-hanja unihan path).
    assert_eq!(
        convert_plain_text("1998年", dict, RenderMode::HangulOnly),
        "1998년"
    );
    // Multi-syllable suffix overrides from the bundled `suffix.tsv`.
    assert_eq!(
        convert_plain_text("1990年代", dict, RenderMode::HangulOnly),
        "1990년대"
    );
}

#[test]
fn bundled_multi_syllable_matches_carry_suffix_reading() {
    let dict = gukhanmun_stdict::ko_kr();

    let year_decade = dict
        .matches_at("年代")
        .find(|matched| matched.byte_len == "年代".len())
        .expect("年代 matches in bundled dictionary");
    assert_eq!(year_decade.reading, "연대");
    assert_eq!(year_decade.suffix_reading.as_deref(), Some("년대"));

    // Single hanja carry no suffix override; the engine handles their initial
    // sound law from the unihan readings instead.
    let year = dict
        .matches_at("年")
        .find(|matched| matched.byte_len == "年".len())
        .expect("年 matches in bundled dictionary");
    assert_eq!(year.suffix_reading, None);
}

#[test]
fn bundled_suffix_table_rows_are_well_formed() {
    let table = include_str!("../data/suffix.tsv");
    let mut lines = table.lines();
    assert_eq!(lines.next(), Some("hanja\tinitial\tsuffix"));
    let mut count = 0;
    for line in lines.filter(|line| !line.is_empty()) {
        count += 1;
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 3, "row `{line}` must have three fields");
        let [hanja, initial, suffix] = [fields[0], fields[1], fields[2]];
        // Multi-syllable hanja key only (single hanja are handled by the engine).
        assert!(
            hanja.chars().count() >= 2 && hanja.chars().all(|c| !c.is_ascii()),
            "key `{hanja}` must be a multi-syllable hanja string"
        );
        // The two readings differ only in their first syllable.
        assert_eq!(initial.chars().count(), suffix.chars().count());
        assert_ne!(
            initial.chars().next(),
            suffix.chars().next(),
            "row `{line}` first syllables must differ"
        );
        assert!(
            initial.chars().skip(1).eq(suffix.chars().skip(1)),
            "row `{line}` must differ only in the first syllable"
        );
    }
    assert!(count > 0, "suffix.tsv should record at least one override");
}

#[test]
fn bundled_rules_set_require_hanja_marks() {
    let dict = gukhanmun_stdict::ko_kr();

    // Only `contains`-kind rules remain in the bundled rules file: they mark
    // entries that embed a rare or multi-reading hanja glyph regardless of
    // homophony.  Common-but-homophonous words such as `漢字`/`天地` are no
    // longer pinned with require_hanja; they are governed by the configurable
    // homophone detection strategy instead.
    let marked = ["馳驟", "勃鬱", "交龜"];
    for hanja in marked {
        let entry = dict
            .lookup(hanja)
            .unwrap()
            .unwrap_or_else(|| panic!("no exact match for `{hanja}` in bundled stdict"));
        assert!(
            entry.mark().require_hanja,
            "require_hanja not set for `{hanja}`"
        );
    }

    // Homophonous common words must stay unmarked so context-local detection
    // controls whether they are glossed.
    let unmarked = ["漢字", "天地", "史記", "詐欺", "會議", "懷疑"];
    for hanja in unmarked {
        let entry = dict
            .lookup(hanja)
            .unwrap()
            .unwrap_or_else(|| panic!("no exact match for `{hanja}` in bundled stdict"));
        assert!(
            !entry.mark().require_hanja,
            "require_hanja unexpectedly set for `{hanja}`"
        );
    }
}

#[test]
fn bundled_contains_rules_drive_hangul_only_rendering_with_disambiguating_hanja() {
    let dict = gukhanmun_stdict::ko_kr();

    // Words embedding a rare hanja are always glossed by the surviving
    // `contains` rules, even standalone.
    assert_eq!(
        convert_plain_text("馳驟", dict, RenderMode::HangulOnly),
        "치취(馳驟)"
    );
    assert_eq!(
        convert_plain_text("勃鬱", dict, RenderMode::HangulOnly),
        "발울(勃鬱)"
    );
}

#[test]
fn homophone_words_follow_context_local_detection() {
    let dict = gukhanmun_stdict::ko_kr();

    // Standalone homophonous words are no longer pinned to require_hanja, so
    // context-local detection leaves them as plain hangul.
    assert_eq!(
        convert_plain_text("漢字", dict, RenderMode::HangulOnly),
        "한자"
    );
    assert_eq!(
        convert_plain_text("史記", dict, RenderMode::HangulOnly),
        "사기"
    );

    // A genuine in-window collision is still glossed under context-local
    // detection (`漢字`, `漢子`, `韓子` all read 한자).
    assert_eq!(
        convert_plain_text("漢字와 漢子", dict, RenderMode::HangulOnly),
        "한자(漢字)와 한자(漢子)"
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
