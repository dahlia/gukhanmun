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
use std::io::{Read, Write};
use std::process::{Command as StdCommand, Stdio};
use std::sync::mpsc;
use std::time::Duration;

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
        // 漢字 keeps its gloss via the bundled require-hanja rule; 布告하다 and
        // 佈告하다 share 포고하다 within the line, so the default context-local
        // detection glosses both.  標識 has no in-text homophone here and is
        // therefore left as plain hangul.
        .stdout("한자(漢字) 베이징 표지 일푼전 포고하다(布告하다) 포고하다(佈告하다)\n");
}

#[test]
fn help_groups_options_by_pipeline_area() {
    let assert = Command::cargo_bin("gukhanmun")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    for heading in [
        "Input/output:",
        "Language and dictionaries:",
        "Conversion:",
        "Rendering policy:",
        "User directives:",
    ] {
        assert!(stdout.contains(heading), "missing help heading {heading}");
    }
    assert!(stdout.find("Input/output:") < stdout.find("Language and dictionaries:"));
    assert!(stdout.find("Language and dictionaries:") < stdout.find("Conversion:"));
    assert!(stdout.find("Conversion:") < stdout.find("Rendering policy:"));
    assert!(stdout.find("Rendering policy:") < stdout.find("User directives:"));
    assert!(stdout.contains("--numerals <NUMERALS>"));
    assert!(stdout.contains("--recovery <RECOVERY>"));
    assert!(stdout.contains("--directives <PATH>"));
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
fn ruby_rendering_in_plain_text_falls_back_to_parens() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--rendering", "ruby-on-hangul"])
        .write_stdin("漢字\n")
        .assert()
        .success()
        .stdout("한자(漢字)\n");
}

#[test]
fn ruby_rendering_in_html_emits_ruby_element() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--rendering", "ruby-on-hangul", "--format", "text/html"])
        .write_stdin("<p>漢字</p>")
        .assert()
        .success()
        .stdout("<p><ruby>한자<rt>漢字</rt></ruby></p>");
}

#[test]
fn original_with_ruby_gloss_uses_ruby_element_in_html_when_required() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--rendering",
            "original",
            "--original-gloss",
            "ruby",
            "--format",
            "text/html",
            "--require-hangul",
            "漢字",
        ])
        .write_stdin("<p>漢字</p>")
        .assert()
        .success()
        .stdout("<p><ruby>漢字<rt>한자</rt></ruby></p>");
}

#[test]
fn html_preserve_class_flag_preserves_matching_elements() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/html",
            "--html-preserve-class",
            "no-translate",
        ])
        .write_stdin("<div class=\"no-translate\">漢字</div><div>漢字</div>")
        .assert()
        .success()
        .stdout("<div class=\"no-translate\">漢字</div><div>한자(漢字)</div>");
}

#[test]
fn html_preserve_class_flag_inherits_to_descendants() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/html",
            "--html-preserve-class",
            "no-translate",
        ])
        .write_stdin("<section class=\"no-translate\"><p>漢字</p></section><p>漢字</p>")
        .assert()
        .success()
        .stdout("<section class=\"no-translate\"><p>漢字</p></section><p>한자(漢字)</p>");
}

#[test]
fn html_preserve_attr_flag_with_value_matches_exact_value() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/html",
            "--html-preserve-attr",
            "translate=no",
        ])
        .write_stdin("<p translate=\"no\">漢字</p><p translate=\"yes\">漢字</p>")
        .assert()
        .success()
        .stdout("<p translate=\"no\">漢字</p><p translate=\"yes\">한자(漢字)</p>");
}

#[test]
fn html_preserve_attr_flag_without_value_matches_presence() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/html",
            "--html-preserve-attr",
            "data-no-mt",
        ])
        .write_stdin("<p data-no-mt>漢字</p><p>漢字</p>")
        .assert()
        .success()
        .stdout("<p data-no-mt>漢字</p><p>한자(漢字)</p>");
}

#[test]
fn html_preserve_class_flag_decodes_html_entities_in_attribute_value() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/html",
            "--html-preserve-class",
            "no-translate",
        ])
        .write_stdin("<div class=\"no&#45;translate\">漢字</div><div>漢字</div>")
        .assert()
        .success()
        .stdout("<div class=\"no&#45;translate\">漢字</div><div>한자(漢字)</div>");
}

#[test]
fn html_preserve_attr_flag_decodes_html_entities_in_attribute_value() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/html",
            "--html-preserve-attr",
            "data-tag=A&B",
        ])
        .write_stdin("<p data-tag=\"A&amp;B\">漢字</p><p data-tag=\"AB\">漢字</p>")
        .assert()
        .success()
        .stdout("<p data-tag=\"A&amp;B\">漢字</p><p data-tag=\"AB\">한자(漢字)</p>");
}

#[test]
fn html_preserve_flags_compose_or_semantics() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/html",
            "--html-preserve-class",
            "no-translate",
            "--html-preserve-class",
            "skip",
            "--html-preserve-attr",
            "data-no-mt",
        ])
        .write_stdin(
            "<p class=\"skip\">漢字</p><p data-no-mt>漢字</p><p class=\"no-translate\">漢字</p><p>漢字</p>",
        )
        .assert()
        .success()
        .stdout(
            "<p class=\"skip\">漢字</p><p data-no-mt>漢字</p><p class=\"no-translate\">漢字</p><p>한자(漢字)</p>",
        );
}

#[test]
fn html_preserve_class_rejected_for_plain_text_format() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/plain",
            "--html-preserve-class",
            "no-translate",
        ])
        .write_stdin("漢字\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--html-preserve-class is only valid with --format text/html",
        ));
}

#[test]
fn html_preserve_attr_rejected_for_plain_text_format() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/plain",
            "--html-preserve-attr",
            "translate=no",
        ])
        .write_stdin("漢字\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--html-preserve-attr is only valid with --format text/html",
        ));
}

#[test]
fn original_gloss_rejected_without_original_rendering() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--original-gloss", "ruby"])
        .write_stdin("漢字\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--original-gloss is only valid with --rendering original",
        ));
}

#[test]
fn segmentation_option_selects_eager_longest_match() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture(
        temp.path().join("user.tsv"),
        temp.path().join("user.gukfst"),
        "hanja\thangul\n行事\t행사\n行事場\t행사장\n場所\t장소\n",
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--no-stdict",
            "--dictionary",
            dictionary.to_str().unwrap(),
            "--rendering",
            "hangul-hanja-parens",
            "--segmentation",
            "eager",
        ])
        .write_stdin("行事場所\n")
        .assert()
        .success()
        .stdout("행사장(行事場)소(所)\n");
}

#[test]
fn short_segmentation_option_selects_eager_longest_match() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture(
        temp.path().join("user.tsv"),
        temp.path().join("user.gukfst"),
        "hanja\thangul\n行事\t행사\n行事場\t행사장\n場所\t장소\n",
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--no-stdict",
            "--dictionary",
            dictionary.to_str().unwrap(),
            "--rendering",
            "hangul-hanja-parens",
            "-s",
            "eager",
        ])
        .write_stdin("行事場所\n")
        .assert()
        .success()
        .stdout("행사장(行事場)소(所)\n");
}

#[test]
fn numeral_option_selects_positional_arabic_conversion() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--no-stdict", "--numerals", "positional-arabic"])
        .write_stdin("二〇一六年\n")
        .assert()
        .success()
        .stdout("2016년\n");
}

#[test]
fn numeral_option_selects_smart_conversion() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--no-stdict", "--numerals", "smart"])
        .write_stdin("十一月 一千二百三十四\n")
        .assert()
        .success()
        .stdout("11월 1234\n");
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
fn ko_kp_plain_stdin_writes_ready_output_before_eof() {
    assert_plain_stdin_writes_before_eof(["--preset", "ko-kp"], "北\n", "북\n");
}

#[test]
fn html_stdin_writes_complete_block_before_eof() {
    assert_plain_stdin_writes_before_eof(
        [
            "--format",
            "text/html",
            "--no-stdict",
            "--disambiguation",
            "off",
        ],
        "<p>北</p>",
        "<p>북</p>",
    );
}

fn assert_plain_stdin_writes_before_eof<const N: usize>(
    args: [&str; N],
    input: &str,
    expected: &str,
) {
    let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("gukhanmun"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let expected_len = expected.len();

    let reader = std::thread::spawn(move || {
        let mut buffer = vec![0; expected_len];
        let result = stdout.read_exact(&mut buffer).map(|()| buffer);
        sender.send(result).unwrap();
    });

    stdin.write_all(input.as_bytes()).unwrap();
    stdin.flush().unwrap();

    let buffer = match receiver.recv_timeout(Duration::from_secs(2)) {
        Ok(result) => result.unwrap(),
        Err(error) => {
            child.kill().unwrap();
            drop(stdin);
            child.wait().unwrap();
            reader.join().unwrap();
            panic!("expected ready output before stdin EOF: {error}");
        }
    };

    drop(stdin);
    let status = child.wait().unwrap();
    reader.join().unwrap();

    assert!(status.success());
    assert_eq!(buffer, expected.as_bytes());
}

#[test]
fn reads_from_file_and_writes_to_output_file() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("input.txt");
    let output = temp.path().join("output.txt");
    fs::write(&input, "北京\n").unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--disambiguation",
            "off",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout("");

    assert_eq!(fs::read_to_string(output).unwrap(), "베이징\n");
}

#[test]
fn same_input_and_output_path_is_replaced_after_successful_conversion() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("text.txt");
    fs::write(&path, "北京\n上海\n").unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--disambiguation",
            "off",
            path.to_str().unwrap(),
            "-o",
            path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout("");

    assert_eq!(fs::read_to_string(path).unwrap(), "베이징\n상하이\n");
}

#[cfg(unix)]
#[test]
fn same_input_and_output_path_preserves_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let path = temp.path().join("private.txt");
    fs::write(&path, "北京\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--disambiguation",
            "off",
            path.to_str().unwrap(),
            "-o",
            path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout("");

    assert_eq!(fs::read_to_string(&path).unwrap(), "베이징\n");
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
        .args([
            "--dictionary",
            dictionary.to_str().unwrap(),
            "--disambiguation",
            "off",
        ])
        .write_stdin("漢字 北京\n")
        .assert()
        .success()
        .stdout("사용자 베이징\n");
}

#[test]
fn default_plain_stdin_preserves_homophone_context_across_lines() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture(
        temp.path().join("user.tsv"),
        temp.path().join("user.gukfst"),
        "hanja\thangul\n漢字\t한자\n翰字\t한자\n",
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--dictionary", dictionary.to_str().unwrap()])
        .write_stdin("漢字\n翰字\n")
        .assert()
        .success()
        .stdout("한자(漢字)\n한자(翰字)\n");
}

#[test]
fn disambiguation_off_suppresses_dictionary_homophones() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture(
        temp.path().join("user.tsv"),
        temp.path().join("user.gukfst"),
        "hanja\thangul\n漢字\t한자\n翰字\t한자\n",
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--dictionary",
            dictionary.to_str().unwrap(),
            "--disambiguation",
            "off",
        ])
        .write_stdin("漢字\n")
        .assert()
        .success()
        .stdout("한자\n");
}

#[test]
fn disambiguation_per_document_marks_dictionary_homophones() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture(
        temp.path().join("user.tsv"),
        temp.path().join("user.gukfst"),
        "hanja\thangul\n漢字\t한자\n翰字\t한자\n",
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--dictionary",
            dictionary.to_str().unwrap(),
            "--disambiguation",
            "per-document",
            "--homophone-detection",
            "dictionary-wide",
        ])
        .write_stdin("漢字\n")
        .assert()
        .success()
        .stdout("한자(漢字)\n");
}

#[test]
fn first_occurrence_per_document_clears_repeated_required_hanja() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture(
        temp.path().join("user.tsv"),
        temp.path().join("user.gukfst"),
        "hanja\thangul\trequire_hanja\n漢字\t한자\ttrue\n",
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--no-stdict",
            "--dictionary",
            dictionary.to_str().unwrap(),
            "--disambiguation",
            "off",
            "--first-occurrence",
            "per-document",
        ])
        .write_stdin("漢字 漢字\n")
        .assert()
        .success()
        .stdout("한자(漢字) 한자\n");
}

#[test]
fn first_occurrence_per_section_resets_at_html_headings() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture(
        temp.path().join("user.tsv"),
        temp.path().join("user.gukfst"),
        "hanja\thangul\trequire_hanja\n漢字\t한자\ttrue\n",
    );

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/html",
            "--no-stdict",
            "--dictionary",
            dictionary.to_str().unwrap(),
            "--disambiguation",
            "off",
            "--first-occurrence",
            "per-section",
        ])
        .write_stdin("<h1>漢字</h1><p>漢字</p><h2>漢字</h2>")
        .assert()
        .success()
        .stdout("<h1>한자(漢字)</h1><p>한자</p><h2>한자(漢字)</h2>");
}

#[test]
fn directive_literal_requires_hanja() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--no-stdict", "--require-hanja", "漢字"])
        .write_stdin("漢字 天地\n")
        .assert()
        .success()
        .stdout("한자(漢字) 천지\n");
}

#[test]
fn directive_glob_requires_hangul_in_original_mode() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--no-stdict",
            "--rendering",
            "original",
            "--require-hangul-glob",
            "*字",
        ])
        .write_stdin("漢字 天地\n")
        .assert()
        .success()
        .stdout("漢字(한자) 天地\n");
}

#[test]
fn skip_directive_collapses_hangul_primary_renderer() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--no-stdict",
            "--rendering",
            "hangul-hanja-parens",
            "--skip-annotation",
            "漢字",
        ])
        .write_stdin("漢字\n")
        .assert()
        .success()
        .stdout("한자\n");
}

#[test]
fn skip_directive_glob_collapses_hanja_primary_renderer() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--no-stdict",
            "--rendering",
            "hanja-hangul-parens",
            "--skip-annotation-glob",
            "*字",
        ])
        .write_stdin("漢字 天地\n")
        .assert()
        .success()
        .stdout("漢字 天地(천지)\n");
}

#[test]
fn directives_file_applies_literal_and_glob_rules() {
    let temp = tempdir().unwrap();
    let directives = temp.path().join("directives.tsv");
    fs::write(
        &directives,
        "action\tpattern\tkind\nrequire-hanja\t漢字\tliteral\nrequire-hanja\t*地\tglob\n",
    )
    .unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--no-stdict",
            "--disambiguation",
            "off",
            "--directives",
            directives.to_str().unwrap(),
        ])
        .write_stdin("漢字 天地\n")
        .assert()
        .success()
        .stdout("한자(漢字) 천지(天地)\n");
}

#[test]
fn multiple_directives_files_compose_with_inline_directives() {
    let temp = tempdir().unwrap();
    let first = temp.path().join("first.tsv");
    let second = temp.path().join("second.tsv");
    fs::write(
        &first,
        "action\tpattern\tkind\nrequire-hanja\t天地\tliteral\n",
    )
    .unwrap();
    fs::write(
        &second,
        "action\tpattern\tkind\nskip-annotation\t天地\tliteral\n",
    )
    .unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--no-stdict",
            "--disambiguation",
            "off",
            "--directives",
            first.to_str().unwrap(),
            "--directives",
            second.to_str().unwrap(),
            "--require-hanja",
            "漢字",
        ])
        .write_stdin("漢字 天地\n")
        .assert()
        .success()
        .stdout("한자(漢字) 천지\n");
}

#[test]
fn malformed_directives_file_reports_path_and_line() {
    let temp = tempdir().unwrap();
    let directives = temp.path().join("directives.tsv");
    fs::write(
        &directives,
        "action\tpattern\tkind\nrequire-hanja\t\tliteral\n",
    )
    .unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--directives", directives.to_str().unwrap()])
        .write_stdin("漢字\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("directives.tsv:2"))
        .stderr(predicate::str::contains("`pattern` must not be empty"));
}

#[test]
fn stdin_streaming_preserves_dictionary_matches_across_chunk_boundaries() {
    let temp = tempdir().unwrap();
    let dictionary = build_dictionary_fixture(
        temp.path().join("user.tsv"),
        temp.path().join("user.gukfst"),
        "hanja\thangul\n汽車길\t기찻길\n",
    );
    let prefix = "a".repeat(8189);
    let input = format!("{prefix}汽車길\n");
    let expected = format!("{prefix}기찻길\n");

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--no-stdict", "--dictionary", dictionary.to_str().unwrap()])
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected);
}

#[test]
fn stdin_reports_invalid_utf8() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .write_stdin(vec![0xff])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to read UTF-8 input"));
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
        .args([
            "--dictionary",
            dictionary.to_str().unwrap(),
            "--disambiguation",
            "off",
        ])
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
        .args([
            "--dictionary",
            dictionary.to_str().unwrap(),
            "--disambiguation",
            "off",
        ])
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
        .args(["--format", "text/html", "--disambiguation", "off"])
        .write_stdin("<p>北京</p>")
        .assert()
        .success()
        .stdout("<p>베이징</p>");
}

#[test]
fn default_strict_recovery_rejects_malformed_html() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/html",
            "--no-stdict",
            "--disambiguation",
            "off",
        ])
        .write_stdin("<p>學校 <1invalid> 北京")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to convert HTML fragment"));
}

#[test]
fn recovery_lenient_preserves_malformed_html_and_continues() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/html",
            "--no-stdict",
            "--disambiguation",
            "off",
            "--recovery",
            "lenient",
        ])
        .write_stdin("<p>學校 <1invalid> 北京")
        .assert()
        .success()
        .stdout("<p>학교 <1invalid> 북경");
}

#[test]
fn invalid_recovery_value_is_rejected() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--recovery", "invalid"])
        .write_stdin("漢字\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn format_text_markdown_converts_markdown_input() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--format", "text/markdown", "--disambiguation", "off"])
        .write_stdin("# 北京\n")
        .assert()
        .success()
        .stdout("베이징\n======\n");
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
        .args([
            "--format",
            "text/markdown; variant=GFM",
            "--disambiguation",
            "off",
        ])
        .write_stdin("~~北京~~\n")
        .assert()
        .success()
        .stdout("~~베이징~~\n");
}

#[test]
fn format_text_markdown_gfm_variant_no_space_enables_gfm_extensions() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/markdown;variant=GFM",
            "--disambiguation",
            "off",
        ])
        .write_stdin("~~北京~~\n")
        .assert()
        .success()
        .stdout("~~베이징~~\n");
}

#[test]
fn format_text_markdown_gfm_variant_extra_whitespace_and_unknown_param() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--format",
            "text/markdown;   foo=bar;   variant=GFM",
            "--disambiguation",
            "off",
        ])
        .write_stdin("~~北京~~\n")
        .assert()
        .success()
        .stdout("~~베이징~~\n");
}

#[test]
fn html_extension_infers_html_format() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("doc.html");
    let output = temp.path().join("out.html");
    fs::write(&input, "<p>北京</p>").unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--disambiguation",
            "off",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(output).unwrap(), "<p>베이징</p>");
}

#[test]
fn md_extension_infers_markdown_format() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("doc.md");
    let output = temp.path().join("out.md");
    fs::write(&input, "# 北京\n").unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--disambiguation",
            "off",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(output).unwrap(), "베이징\n======\n");
}

#[test]
fn unknown_extension_falls_back_to_plain_text() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("doc.txt");
    let output = temp.path().join("out.txt");
    fs::write(&input, "北京\n").unwrap();

    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args([
            "--disambiguation",
            "off",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(output).unwrap(), "베이징\n");
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

#[test]
fn verbose_flag_long_is_accepted() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--verbose"])
        .write_stdin("漢字\n")
        .assert()
        .success();
}

#[test]
fn verbose_flag_short_is_accepted() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["-v"])
        .write_stdin("漢字\n")
        .assert()
        .success();
}

#[test]
fn verbose_flag_emits_debug_output_to_stderr() {
    let assert = Command::cargo_bin("gukhanmun")
        .unwrap()
        .env_remove("RUST_LOG")
        .args(["--verbose"])
        .write_stdin("漢字\n")
        .assert()
        .success();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("segmentation"),
        "expected segmentation debug event in stderr: {stderr}"
    );
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
            rules: Vec::new(),
            allow_unmatched_rules: false,
        },
    )
    .unwrap();
    output
}
