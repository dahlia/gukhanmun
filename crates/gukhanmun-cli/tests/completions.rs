// Gukhanmun: Generates static shell completions from the CLI schema.
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
use std::process::{Command as StdCommand, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

const SHELLS: [&str; 5] = ["bash", "zsh", "fish", "powershell", "nushell"];

#[test]
fn generation_is_deterministic_and_completes_its_own_interface() {
    for shell in SHELLS {
        let output = Command::cargo_bin("gukhanmun")
            .unwrap()
            .args(["--generate-completion", shell])
            .assert()
            .success()
            .stderr("")
            .get_output()
            .stdout
            .clone();
        let second = Command::cargo_bin("gukhanmun")
            .unwrap()
            .args(["--generate-completion", shell])
            .assert()
            .success()
            .stderr("")
            .get_output()
            .stdout
            .clone();
        assert_eq!(output, second, "{shell}");
        let text = String::from_utf8(output).unwrap();
        for candidate in [
            "gukhanmun",
            "generate-completion",
            "ko-kr",
            "ko-kp",
            "nushell",
        ] {
            assert!(text.contains(candidate), "{shell} omitted {candidate}");
        }
    }
}

#[test]
fn generation_does_not_wait_for_stdin_to_close() {
    for shell in SHELLS {
        let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("gukhanmun"))
            .args(["--generate-completion", shell])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "{shell}: {status}");
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("{shell} waited for conversion input");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(stdin);
    }
}

#[test]
fn generation_rejects_missing_invalid_or_mixed_arguments() {
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .arg("--generate-completion")
        .assert()
        .code(2)
        .stdout("");
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .args(["--generate-completion", "invalid"])
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("invalid value"));
    let temp = tempdir().unwrap();
    let output = temp.path().join("output.txt");
    fs::write(&output, "unchanged").unwrap();
    for args in [
        vec!["-v"],
        vec!["--dictionary", "missing.gukfst"],
        vec!["missing-input"],
        vec!["-o", output.to_str().unwrap()],
    ] {
        Command::cargo_bin("gukhanmun")
            .unwrap()
            .args(["--generate-completion", "bash"])
            .args(args)
            .assert()
            .code(2)
            .stdout("");
    }
    assert_eq!(fs::read_to_string(output).unwrap(), "unchanged");
}

#[test]
fn completion_names_remain_valid_input_filenames_and_stdin_conversion_works() {
    let temp = tempdir().unwrap();
    for filename in ["completions", "nushell"] {
        fs::write(temp.path().join(filename), "漢字\n").unwrap();
        Command::cargo_bin("gukhanmun")
            .unwrap()
            .current_dir(temp.path())
            .args(["-S", filename])
            .assert()
            .success()
            .stdout("한자\n");
    }
    Command::cargo_bin("gukhanmun")
        .unwrap()
        .arg("-S")
        .write_stdin("漢字\n")
        .assert()
        .success()
        .stdout("한자\n");
}

#[test]
fn relocated_executable_uses_the_public_command_name() {
    let temp = tempdir().unwrap();
    let binary = temp
        .path()
        .join(format!("private-runner{}", std::env::consts::EXE_SUFFIX));
    fs::copy(assert_cmd::cargo::cargo_bin("gukhanmun"), &binary).unwrap();
    for shell in SHELLS {
        Command::new(&binary)
            .args(["--generate-completion", shell])
            .assert()
            .success()
            .stdout(predicate::str::contains("gukhanmun"))
            .stdout(predicate::str::contains("private-runner").not());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn output_failure_is_an_error_without_a_generator_panic() {
    for shell in SHELLS {
        let output = StdCommand::new(assert_cmd::cargo::cargo_bin("gukhanmun"))
            .args(["--generate-completion", shell])
            .stdout(
                fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/full")
                    .unwrap(),
            )
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_ne!(output.status.code(), Some(101));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
}
