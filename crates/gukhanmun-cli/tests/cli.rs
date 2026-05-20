// Gukhanmun: Command-line interface for Gukhanmun plain-text conversion.
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
use gukhanmun_mkdict::{BuildOptions, DictionaryFormat, MergePolicy, build_dictionary};
use predicates::prelude::*;
use proptest::prelude::*;
use tempfile::tempdir;

#[test]
fn converts_stdin_to_stdout_with_bundled_stdict_by_default() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .write_stdin("漢字 北京 標識 一分錢 布告하다 佈告하다\n")
        .assert()
        .success()
        .stdout("한자 베이징 표지 일푼전 포고하다(布告하다) 포고하다(佈告하다)\n");
}

#[test]
fn rendering_option_selects_parenthesized_output() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--rendering", "hangul-hanja-parens"])
        .write_stdin("漢字\n")
        .assert()
        .success()
        .stdout("한자(漢字)\n");
}

#[test]
fn no_stdict_uses_fallback_only_conversion() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .arg("--no-stdict")
        .write_stdin("北京\n")
        .assert()
        .success()
        .stdout("북경\n");
}

#[test]
fn ko_kp_preset_disables_bundled_stdict_and_initial_sound_law() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--preset", "ko-kp"])
        .write_stdin("北京 來日\n")
        .assert()
        .success()
        .stdout("북경 래일\n");
}

#[test]
fn reads_from_file_and_writes_to_output_file() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("input.txt");
    let output = temp.path().join("output.txt");
    fs::write(&input, "漢字\n").unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([input.to_str().unwrap(), "-o", output.to_str().unwrap()])
        .assert()
        .success()
        .stdout("");

    assert_eq!(fs::read_to_string(output).unwrap(), "한자\n");
}

#[test]
fn same_input_and_output_path_is_replaced_after_successful_conversion() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("text.txt");
    fs::write(&path, "漢字\n北京\n").unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([path.to_str().unwrap(), "-o", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout("");

    assert_eq!(fs::read_to_string(path).unwrap(), "한자\n베이징\n");
}

#[cfg(unix)]
#[test]
fn same_input_and_output_path_preserves_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let path = temp.path().join("private.txt");
    fs::write(&path, "漢字\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([path.to_str().unwrap(), "-o", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout("");

    assert_eq!(fs::read_to_string(&path).unwrap(), "한자\n");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn user_dictionary_overrides_bundled_stdict() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture(
        temp.path().join("user.tsv"),
        temp.path().join("user.gukfst"),
        "hanja\thangul\n漢字\t사용자\n",
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--dictionary", dictionary.to_str().unwrap()])
        .write_stdin("漢字 北京\n")
        .assert()
        .success()
        .stdout("사용자 베이징\n");
}

#[test]
fn cdb_user_dictionary_overrides_bundled_stdict() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture_with_format(
        temp.path().join("user.tsv"),
        temp.path().join("user.gukcdb"),
        "hanja\thangul\n漢字\t사용자\n",
        DictionaryFormat::Cdb,
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--dictionary", dictionary.to_str().unwrap()])
        .write_stdin("漢字 北京\n")
        .assert()
        .success()
        .stdout("사용자 베이징\n");
}

#[test]
fn extensionless_cdb_user_dictionary_loads_from_file_contents() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture_with_format(
        temp.path().join("user.tsv"),
        temp.path().join("user-dict"),
        "hanja\thangul\n漢字\t사용자\n",
        DictionaryFormat::Cdb,
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--dictionary", dictionary.to_str().unwrap()])
        .write_stdin("漢字 北京\n")
        .assert()
        .success()
        .stdout("사용자 베이징\n");
}

#[test]
fn later_user_dictionary_has_higher_priority_than_earlier_dictionary() {
    let temp = tempdir().unwrap();
    let first = build_dictionary_fixture(
        temp.path().join("first.tsv"),
        temp.path().join("first.gukfst"),
        "hanja\thangul\n漢字\t첫째\n",
    );
    let second = build_dictionary_fixture(
        temp.path().join("second.tsv"),
        temp.path().join("second.gukfst"),
        "hanja\thangul\n漢字\t둘째\n",
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--no-stdict",
            "--dictionary",
            first.to_str().unwrap(),
            "--dictionary",
            second.to_str().unwrap(),
        ])
        .write_stdin("漢字\n")
        .assert()
        .success()
        .stdout("둘째\n");
}

#[test]
fn missing_dictionary_path_reports_a_human_readable_error() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--dictionary", "missing.gukfst"])
        .write_stdin("漢字\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to load dictionary"));
}

#[test]
fn format_text_html_converts_html_input() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--format", "text/html"])
        .write_stdin("<p>漢字</p>")
        .assert()
        .success()
        .stdout("<p>한자</p>");
}

#[test]
fn format_text_markdown_converts_markdown_input() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--format", "text/markdown"])
        .write_stdin("# 漢字\n")
        .assert()
        .success()
        .stdout("한자\n====\n");
}

#[test]
fn format_text_markdown_does_not_transform_punctuation() {
    // Hongdown's punctuation options (curly quotes, em dash, ellipsis) must be
    // disabled so non-hanja text content is not unexpectedly mutated.
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--format", "text/markdown"])
        .write_stdin("\"Hello\" -- world...\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"Hello\""))
        .stdout(predicates::str::contains("--"))
        .stdout(predicates::str::contains("..."));
}

#[test]
fn format_text_markdown_gfm_variant_with_space_enables_gfm_extensions() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--format", "text/markdown; variant=GFM"])
        .write_stdin("~~漢字~~\n")
        .assert()
        .success()
        .stdout("~~한자~~\n");
}

#[test]
fn format_text_markdown_gfm_variant_no_space_enables_gfm_extensions() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--format", "text/markdown;variant=GFM"])
        .write_stdin("~~漢字~~\n")
        .assert()
        .success()
        .stdout("~~한자~~\n");
}

#[test]
fn format_text_markdown_gfm_variant_extra_whitespace_and_unknown_param() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--format", "text/markdown;   foo=bar;   variant=GFM"])
        .write_stdin("~~漢字~~\n")
        .assert()
        .success()
        .stdout("~~한자~~\n");
}

#[test]
fn html_extension_infers_html_format() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("doc.html");
    let output = temp.path().join("out.html");
    fs::write(&input, "<p>漢字</p>").unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([input.to_str().unwrap(), "-o", output.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(output).unwrap(), "<p>한자</p>");
}

#[test]
fn md_extension_infers_markdown_format() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("doc.md");
    let output = temp.path().join("out.md");
    fs::write(&input, "# 漢字\n").unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([input.to_str().unwrap(), "-o", output.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(output).unwrap(), "한자\n====\n");
}

#[test]
fn unknown_extension_falls_back_to_plain_text() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("doc.txt");
    let output = temp.path().join("out.txt");
    fs::write(&input, "漢字\n").unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([input.to_str().unwrap(), "-o", output.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(output).unwrap(), "한자\n");
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 32,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    #[test]
    fn non_hanja_multiline_input_is_preserved(input in "[A-Za-z0-9가-힣 .,!?()\\n]{0,256}") {
        let assert = Command::cargo_bin("gukhanmun")
            .unwrap()
            .write_stdin(input.clone())
            .assert()
            .success();

        let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        prop_assert_eq!(output, input);
    }
}

fn build_dictionary_fixture(
    input: std::path::PathBuf,
    output: std::path::PathBuf,
    tsv: &str,
) -> std::path::PathBuf {
    build_dictionary_fixture_with_format(input, output, tsv, DictionaryFormat::Fst)
}

fn build_dictionary_fixture_with_format(
    input: std::path::PathBuf,
    output: std::path::PathBuf,
    tsv: &str,
    format: DictionaryFormat,
) -> std::path::PathBuf {
    fs::write(&input, tsv).unwrap();
    build_dictionary(
        &[input],
        &output,
        &BuildOptions {
            format,
            merge: MergePolicy::Error,
            validate: true,
            max_key_bytes: gukhanmun_mkdict::DEFAULT_MAX_KEY_BYTES,
            metadata: BTreeMap::new(),
        },
    )
    .unwrap();
    output
}
