use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};
use zip::ZipArchive;

const UNICODE_VERSION: &str = "17.0.0";
const UNIHAN_URL: &str = "https://www.unicode.org/Public/17.0.0/ucd/Unihan.zip";
const UNIHAN_SHA256: &str = "f7a48b2b545acfaa77b2d607ae28747404ce02baefee16396c5d2d7a8ef34b5e";
const EXPECTED_KHANGUL_ENTRY_COUNT: usize = 8_525;
const GENERATED_PATH: &str = "crates/gukhanmun-core/src/generated/unihan_readings.rs";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let check = env::args().skip(1).any(|arg| arg == "--check");
    let workspace_root = workspace_root()?;
    let zip_path = unihan_zip_path(&workspace_root);
    let output_path = workspace_root.join(GENERATED_PATH);

    let zip_bytes = read_or_download_unihan_zip(&zip_path)?;
    verify_sha256(&zip_bytes)?;
    let readings = parse_unihan_readings(&read_unihan_readings(&zip_bytes)?)?;
    if readings.len() != EXPECTED_KHANGUL_ENTRY_COUNT {
        return Err(format!(
            "expected {EXPECTED_KHANGUL_ENTRY_COUNT} kHangul entries, got {}",
            readings.len()
        )
        .into());
    }
    let generated = render_generated(&readings);

    if check {
        let current = fs::read_to_string(&output_path)?;
        if current != generated {
            return Err(format!(
                "{} is not up to date; run `mise run generate-unihan`",
                GENERATED_PATH
            )
            .into());
        }
        return Ok(());
    }

    fs::create_dir_all(output_path.parent().expect("generated path has a parent"))?;
    fs::write(output_path, generated)?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to find workspace root".into())
}

fn unihan_zip_path(workspace_root: &Path) -> PathBuf {
    env::var_os("GUKHANMUN_UNIHAN_ZIP")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            workspace_root
                .join("target")
                .join("unihan")
                .join(format!("Unihan-{UNICODE_VERSION}.zip"))
        })
}

fn read_or_download_unihan_zip(path: &Path) -> Result<Vec<u8>> {
    if path.exists() {
        return Ok(fs::read(path)?);
    }

    fs::create_dir_all(path.parent().expect("zip path has a parent"))?;
    let status = Command::new("curl")
        .args([
            "--location",
            "--fail",
            "--silent",
            "--show-error",
            "--output",
        ])
        .arg(path)
        .arg(UNIHAN_URL)
        .status()?;
    if !status.success() {
        return Err(format!("failed to download {UNIHAN_URL}").into());
    }
    Ok(fs::read(path)?)
}

fn verify_sha256(bytes: &[u8]) -> Result<()> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != UNIHAN_SHA256 {
        return Err(format!(
            "Unihan.zip checksum mismatch: expected {UNIHAN_SHA256}, got {actual}"
        )
        .into());
    }
    Ok(())
}

fn read_unihan_readings(zip_bytes: &[u8]) -> Result<String> {
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes))?;
    let mut file = archive.by_name("Unihan_Readings.txt")?;
    let mut readings = String::new();
    file.read_to_string(&mut readings)?;
    Ok(readings)
}

fn parse_unihan_readings(input: &str) -> Result<Vec<(char, String)>> {
    let mut readings = Vec::new();
    for (line_index, line) in input.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((code, property, value)) = parse_unihan_line(line) else {
            continue;
        };
        if property != "kHangul" {
            continue;
        }

        let code_point = code
            .strip_prefix("U+")
            .ok_or_else(|| format!("line {} has invalid code point {code}", line_index + 1))?;
        let scalar = u32::from_str_radix(code_point, 16)?;
        let ch = char::from_u32(scalar)
            .ok_or_else(|| format!("line {} has invalid scalar value {code}", line_index + 1))?;
        let reading = canonical_khangul_reading(value)
            .ok_or_else(|| format!("line {} has no kHangul reading", line_index + 1))?;
        readings.push((ch, reading.to_owned()));
    }

    readings.sort_by_key(|(ch, _)| *ch);
    readings.dedup_by_key(|(ch, _)| *ch);
    Ok(readings)
}

fn parse_unihan_line(line: &str) -> Option<(&str, &str, &str)> {
    let mut fields = line.splitn(3, '\t');
    Some((fields.next()?, fields.next()?, fields.next()?))
}

fn canonical_khangul_reading(value: &str) -> Option<&str> {
    let mut fallback = None;
    for field in value.split_whitespace() {
        let (reading, tags) = field.split_once(':').unwrap_or((field, ""));
        fallback.get_or_insert(reading);
        if tags.contains('E') {
            return Some(reading);
        }
    }
    fallback
}

fn render_generated(readings: &[(char, String)]) -> String {
    let mut output = String::new();
    writeln!(
        output,
        "// @generated by gukhanmun-unihan. Do not edit by hand."
    )
    .unwrap();
    writeln!(output, "//").unwrap();
    writeln!(output, "// Unicode version: {UNICODE_VERSION}").unwrap();
    writeln!(output, "// Source: {UNIHAN_URL}").unwrap();
    writeln!(output, "// Source SHA-256: {UNIHAN_SHA256}").unwrap();
    writeln!(
        output,
        "// Policy: prefer the kHangul reading tagged E as the canonical pre-initial-sound-law fallback reading."
    )
    .unwrap();
    writeln!(output).unwrap();
    writeln!(output, "#[allow(dead_code)]").unwrap();
    writeln!(
        output,
        "pub(crate) const UNIHAN_VERSION: &str = \"{UNICODE_VERSION}\";"
    )
    .unwrap();
    writeln!(output, "#[allow(dead_code)]").unwrap();
    writeln!(
        output,
        "pub(crate) const KHANGUL_ENTRY_COUNT: usize = {};",
        readings.len()
    )
    .unwrap();
    writeln!(
        output,
        "pub(crate) static KHANGUL_READINGS: &[(char, &str)] = &["
    )
    .unwrap();
    for (ch, reading) in readings {
        writeln!(
            output,
            "    ('\\u{{{:X}}}', \"{}\"),",
            *ch as u32,
            escape_rust_string(reading)
        )
        .unwrap();
    }
    writeln!(output, "];").unwrap();
    output
}

fn escape_rust_string(s: &str) -> String {
    let mut escaped = String::new();
    for ch in s.chars() {
        for escaped_ch in ch.escape_default() {
            escaped.push(escaped_ch);
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{canonical_khangul_reading, parse_unihan_readings, render_generated};
    use proptest::prelude::*;

    #[test]
    fn parser_keeps_only_khangul_rows() {
        let input = "\
# comment
U+9F8D\tkHangul\t룡:0E 용:0
U+8001\tkHangul\t노:0 로:0E
U+9F8D\tkMandarin\tlong2
U+99AC\tkHangul\t마:0E
";

        let readings = parse_unihan_readings(input).unwrap();

        assert_eq!(
            readings,
            vec![
                ('老', "로".into()),
                ('馬', "마".into()),
                ('龍', "룡".into())
            ]
        );
    }

    #[test]
    fn renderer_emits_sorted_ascii_rust_source() {
        let generated = render_generated(&[('馬', "마".into()), ('龍', "룡".into())]);

        assert!(generated.contains("('\\u{99AC}', \"\\u{b9c8}\"),"));
        assert!(generated.contains("('\\u{9F8D}', \"\\u{b8e1}\"),"));
        assert!(generated.is_ascii());
    }

    #[test]
    fn canonical_reading_prefers_the_e_tagged_reading() {
        assert_eq!(canonical_khangul_reading("노:0 로:0E"), Some("로"));
        assert_eq!(canonical_khangul_reading("룡:0E 용:0"), Some("룡"));
        assert_eq!(canonical_khangul_reading("마:0"), Some("마"));
    }

    proptest! {
        #[test]
        fn canonical_reading_falls_back_to_first_reading_without_e_tag(
            first in "[가-힣]{1,3}",
            second in "[가-힣]{1,3}",
            tag in "[0-9A-DF-Z]{0,3}",
        ) {
            let value = format!("{first}:{tag} {second}:0");

            prop_assert_eq!(canonical_khangul_reading(&value), Some(first.as_str()));
        }
    }
}
