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
            items_seen: 16,
            entries_written: 15,
            duplicate_keys: 3,
            skipped_items: 2,
            skipped_default_pins: 46,
        }
    );
    // `北京` (multi-character) keeps its foreign reading 베이징, but the
    // single-character foreign loanword 삐끼 (Hiki[引]) does not emit a 引 key:
    // single hanja borrowed for a foreign reading are dropped so they cannot
    // shadow the Sino-Korean reading (引 → 인) recovered from unihan.
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "hanja\thangul\trequire_hanja\trequire_hangul\tpreference\tsuffix_reading\tsource_ids\n一分錢\t일푼전\tfalse\tfalse\t\t\t\n一點鎖線\t일점쇄선\tfalse\tfalse\t\t\t\n佈告하다\t포고하다\tfalse\tfalse\t\t\t\n北京\t베이징\tfalse\tfalse\t30\t\t\n北京\t북경\tfalse\tfalse\t10\t\t\n嚴戒\t엄계\tfalse\tfalse\t\t\t\n天地\t천지\tfalse\tfalse\t\t\t\n布告하다\t포고하다\tfalse\tfalse\t\t\t\n標識\t표식\tfalse\tfalse\t0\t\t\n標識\t표지\tfalse\tfalse\t10\t\t\n溫暖\t온난\tfalse\tfalse\t\t\t\n溫煖\t온난\tfalse\tfalse\t\t\t\n漢字\t중복한자\tfalse\tfalse\t10\t\t\n漢字\t한자\tfalse\tfalse\t10\t\t\n입口字집\t입구자집\tfalse\tfalse\t\t\t\n"
    );
}

#[test]
fn saisiot_prescribed_reading_wins_regardless_of_dump_order() {
    // Standard Korean Orthography §30 prescribes 數字 → 숫자. Both 수자 and 숫자
    // are dictionary head words keyed on 數字 with otherwise equal priority, so
    // before the §30 promotion the winner depended on which appeared first in
    // the dump. The prescribed reading must now win either way.
    for (first, second) in [("수자", "숫자"), ("숫자", "수자")] {
        let json = saisiot_collision_json(first, second);
        let mut output = Vec::new();
        extract_json_reader_to_tsv(json.as_bytes(), &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(
            output.contains("數字\t숫자\tfalse\tfalse\t"),
            "expected 數字 → 숫자 with dump order {first} then {second}, got:\n{output}"
        );
        assert!(output.contains("數字\t숫자\tfalse\tfalse\t20\t"));
        assert!(output.contains("數字\t수자\tfalse\tfalse\t10\t"));
        assert!(
            output.contains("數字\t수자\t"),
            "saisiot-free 수자 must be preserved for 數字 with dump order {first} then {second}"
        );
    }
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
    assert!(output.contains("漢字\t한자\tfalse\tfalse\t"));
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

#[test]
fn bundled_dictionary_prefers_saisiot_prescribed_readings() {
    let dict = gukhanmun_stdict::ko_kr();

    // The six Standard Korean Orthography §30 saisiot compounds resolve to their
    // prescribed saisiot reading, not the saisiot-free homograph.
    for (hanja, reading) in [
        ("庫間", "곳간"),
        ("貰房", "셋방"),
        ("數字", "숫자"),
        ("車間", "찻간"),
        ("退間", "툇간"),
        ("回數", "횟수"),
    ] {
        assert_eq!(
            dict.lookup(hanja).unwrap().unwrap().reading(),
            reading,
            "{hanja} should read {reading}"
        );
    }

    // The reported regression: 數字 must convert to 숫자, not 수자.
    assert_eq!(
        convert_plain_text("數字", dict, RenderMode::HangulOnly),
        "숫자"
    );
}

proptest! {
    #[test]
    fn generated_single_hanja_items_extract_deterministically(
        hanja in "[一-龥]{1,3}",
        reading in "[가-힣]{1,5}",
    ) {
        proptest::prop_assume!(!include_str!("../data/defaults.tsv").lines().skip(1).any(|line| line.split('\t').next()==Some(hanja.as_str())));
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
        let expected = format!("{hanja}\t{reading}\tfalse\tfalse\t");
        prop_assert!(output.contains(&expected));
    }

    #[test]
    fn generated_bracketed_foreign_hanja_items_use_the_hanja_spelling(
        romanized in "[A-Za-z]{1,8}",
        hanja in "[一-龥]{1,3}",
        reading in "[가-힣]{1,5}",
    ) {
        proptest::prop_assume!(!include_str!("../data/defaults.tsv").lines().skip(1).any(|line| line.split('\t').next()==Some(hanja.as_str())));
        let json = format!(
            r#"{{"channel":{{"item":[{{"word_info":{{"word":"{reading}","word_unit":"단어","word_type":"외래어","original_language_info":[{{"original_language":"{romanized}[{hanja}]","language_type":"안 밝힘"}}]}}}}]}}}}"#
        );
        let mut output = Vec::new();

        let stats = extract_json_reader_to_tsv(json.as_bytes(), &mut output).unwrap();

        let output = String::from_utf8(output).unwrap();
        let discarded_prefix = format!("{romanized}[");
        prop_assert!(!output.contains(&discarded_prefix));
        if hanja.chars().count() == 1 {
            // A single hanja borrowed for a foreign reading is dropped so it
            // cannot shadow the character's Sino-Korean reading, which is
            // recovered from the engine's unihan fallback instead.
            prop_assert_eq!(
                stats,
                ExtractStats {
                    items_seen: 1,
                    entries_written: 0,
                    duplicate_keys: 0,
                    skipped_items: 0,
                    skipped_default_pins: 46,
                }
            );
            let dropped_key = format!("{hanja}\t");
            prop_assert!(!output.contains(&dropped_key));
        } else {
            // Multi-character foreign units keep their hanja spelling.
            prop_assert_eq!(
                stats,
                ExtractStats {
                    items_seen: 1,
                    entries_written: 1,
                    duplicate_keys: 0,
                    skipped_items: 0,
                    skipped_default_pins: 46,
                }
            );
            let expected = format!("{hanja}\t{reading}\tfalse\tfalse\t");
            prop_assert!(output.contains(&expected));
        }
    }
}

/// Builds a dump where two head words (`first` then `second`) both key on 數字,
/// modelling the 수자/숫자 homograph collision the §30 promotion resolves.
fn saisiot_collision_json(first: &str, second: &str) -> String {
    format!(
        r#"{{"channel":{{"item":[{{"word_info":{{"word":"{first}","word_unit":"단어","word_type":"한자어","original_language_info":[{{"original_language":"數字","language_type":"한자"}}]}}}},{{"word_info":{{"word":"{second}","word_unit":"단어","word_type":"한자어","original_language_info":[{{"original_language":"數字","language_type":"한자"}}]}}}}]}}}}"#
    )
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
          "word": "삐끼",
          "word_unit": "단어",
          "word_type": "외래어",
          "original_language_info": [
            { "original_language": "Hiki[引]", "language_type": "안 밝힘" }
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

#[test]
fn candidate_extraction_is_order_independent_and_unions_source_records() {
    let item = |id: u64, word: &str| serde_json::json!({"target_code":id,"word_info":{"word":word,"word_unit":"단어","original_language_info":[{"original_language":"東京","language_type":"한자"}]}});
    let items = vec![item(100, "동경"), item(101, "도쿄"), item(102, "동경")];
    let extract = |items: Vec<serde_json::Value>| {
        let mut output = Vec::new();
        extract_json_reader_to_tsv(
            serde_json::json!({"channel":{"item":items}})
                .to_string()
                .as_bytes(),
            &mut output,
        )
        .unwrap();
        String::from_utf8(output).unwrap()
    };
    let output = extract(items.clone());
    assert_eq!(output, extract(items.into_iter().rev().collect()));
    assert!(output.contains("東京\t동경\tfalse\tfalse\t10\t\t100,102\n"));
    assert!(output.contains("東京\t도쿄\tfalse\tfalse\t10\t\t101\n"));
}

#[test]
fn single_hanja_source_readings_stay_separate_from_positional_forms() {
    let item = |id: u64, key: &str, word: &str| serde_json::json!({"target_code":id,"word_info":{"word":word,"word_unit":"단어","original_language_info":[{"original_language":key,"language_type":"한자"}]}});
    // Real source identifiers for 率; automatic suffix pairing must not consume 률.
    let json = serde_json::json!({"channel":{"item":[item(105043,"率","-률02"),item(256492,"率","율02"),item(1,"年","연"),item(2,"年","-년")]}});
    let mut output = Vec::new();
    extract_json_reader_to_tsv(json.to_string().as_bytes(), &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    for reading in ["률", "율"] {
        assert!(output.lines().any(|line| {
            let f: Vec<_> = line.split('\t').collect();
            f[0] == "率" && f[1] == reading && f[5].is_empty()
        }));
    }
    for key in ["年", "率"] {
        let matched = gukhanmun_stdict::ko_kr()
            .candidate_matches_at(key)
            .next()
            .unwrap();
        assert!(matched.candidates.as_slice().len() > 1);
        assert!(
            matched
                .candidates
                .as_slice()
                .iter()
                .all(|c| c.suffix_reading.is_none())
        );
    }
    assert_eq!(
        convert_plain_text(
            "비率",
            gukhanmun_stdict::ko_kr(),
            RenderMode::HangulHanjaParens
        ),
        "비율(率)"
    );
}

#[test]
fn bundled_candidates_survive_boxed_chains_and_every_curated_pin_is_present() {
    let dictionary: Box<dyn HanjaDictionary> = Box::new(gukhanmun_stdict::ko_kr());
    let chain: gukhanmun_core::ChainDictionary<_> = [dictionary].into_iter().collect();
    let matched = chain.candidate_matches_at("率").next().unwrap();
    assert!(matched.candidates.as_slice().len() > 1);
    assert_eq!(matched.select_default().reading, "률");
    for line in include_str!("../data/defaults.tsv").lines().skip(1) {
        let f: Vec<_> = line.split('\t').collect();
        let matched = chain
            .candidate_matches_at(f[0])
            .find(|m| m.byte_len == f[0].len())
            .unwrap()
            .select_default();
        assert_eq!(matched.reading, f[1], "{}", f[0]);
        assert_eq!(matched.suffix_reading.as_deref().unwrap_or(""), f[2]);
    }
    assert_eq!(
        gukhanmun_stdict::ko_kr()
            .lookup("年代")
            .unwrap()
            .unwrap()
            .candidates()
            .as_slice()[0]
            .suffix_reading
            .as_deref(),
        Some("년대")
    );
}

#[test]
fn changed_source_candidates_invalidate_a_curated_default() {
    // 女 has a reviewed 여/녀 spelling decision in the pinned source;
    // an unrelated replacement must never inherit its preference silently.
    let json = serde_json::json!({"channel":{"item":[{"word_info":{"word":"다른독음","word_unit":"단어","original_language_info":[{"original_language":"女","language_type":"한자"}]}}]}});
    let error = extract_json_reader_to_tsv(json.to_string().as_bytes(), Vec::new()).unwrap_err();
    assert!(error.to_string().contains("candidate set changed"));
}

#[test]
fn shared_initial_and_suffix_readings_keep_both_roles_and_their_own_sources() {
    let item = |id: u64, word: &str| serde_json::json!({"target_code":id,"word_info":{"word":word,"word_unit":"단어","original_language_info":[{"original_language":"年度","language_type":"한자"}]}});
    let extract = |items: Vec<serde_json::Value>| {
        let mut output = Vec::new();
        extract_json_reader_to_tsv(
            serde_json::json!({"channel":{"item":items}})
                .to_string()
                .as_bytes(),
            &mut output,
        )
        .unwrap();
        String::from_utf8(output).unwrap()
    };
    let items = vec![item(1, "연도"), item(2, "-년도"), item(3, "년도")];
    let output = extract(items.clone());
    assert_eq!(output, extract(items.into_iter().rev().collect()));
    assert!(output.contains("年度\t연도\tfalse\tfalse\t10\t년도\t1,2\n"));
    assert!(output.contains("年度\t년도\tfalse\tfalse\t10\t\t2,3\n"));

    // Independent initial provenance must not promote a different candidate's
    // suffix pairing, even if its own source has higher priority.
    let foreign = serde_json::json!({"target_code":4,"word_info":{"word":"년도","word_unit":"단어","original_language_info":[{"original_language":"Nyeondo[年度]","language_type":"안 밝힘"}]}});
    let output = extract(vec![item(1, "연도"), item(2, "-년도"), foreign]);
    assert!(output.contains("年度\t연도\tfalse\tfalse\t10\t년도\t1,2\n"));
    assert!(output.contains("年度\t년도\tfalse\tfalse\t30\t\t2,4\n"));
}

#[test]
fn one_initial_reading_can_keep_several_suffix_forms() {
    let items=[(1,"여인"),(2,"-려인"),(3,"-녀인"),(4,"려인")].map(|(id,word)|serde_json::json!({"target_code":id,"word_info":{"word":word,"word_unit":"단어","original_language_info":[{"original_language":"旅人","language_type":"한자"}]}}));
    let mut output = Vec::new();
    extract_json_reader_to_tsv(
        serde_json::json!({"channel":{"item":items}})
            .to_string()
            .as_bytes(),
        &mut output,
    )
    .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("旅人\t여인\tfalse\tfalse\t10\t려인\t1,2\n"));
    assert!(output.contains("旅人\t여인\tfalse\tfalse\t10\t녀인\t1,3\n"));
    assert!(output.contains("旅人\t려인\tfalse\tfalse\t10\t\t2,4\n"));
}
