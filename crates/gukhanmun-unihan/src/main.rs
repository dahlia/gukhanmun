// Gukhanmun: Generates gukhanmun-core fallback readings from Unicode Unihan data.
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

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Read, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};
use zip::ZipArchive;

const UNICODE_VERSION: &str = "17.0.0";
const UNIHAN_URL: &str = "https://www.unicode.org/Public/17.0.0/ucd/Unihan.zip";
const UNIHAN_SHA256: &str = "f7a48b2b545acfaa77b2d607ae28747404ce02baefee16396c5d2d7a8ef34b5e";
const EXPECTED_KHANGUL_ENTRY_COUNT: usize = 8_525;
const EXPECTED_VARIANT_CLASS_COUNT: usize = 7_124;
const EXPECTED_VARIANT_MEMBER_COUNT: usize = 14_944;
const EXPECTED_JOYO_PAIR_COUNT: usize = 342;
const EXPECTED_ASAHI_PAIR_COUNT: usize = 16;
const GENERATED_PATH: &str = "crates/gukhanmun-core/src/generated/unihan_readings.rs";
const GENERATED_VARIANTS_PATH: &str = "crates/gukhanmun-core/src/generated/hanja_variants.rs";

// GPLv3 notice block prepended to the generated source.  The generator owns
// the whole file (it overwrites it with `fs::write`), so it must emit the
// license header itself; otherwise a regeneration would silently strip it.
const LICENSE_HEADER: &str = "\
// Gukhanmun: Generated Unihan kHangul fallback reading tables.
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
";

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
    let generated = format_rust_source(&render_generated(&readings))?;
    let variants = parse_variants(
        &read_zip_text(&zip_bytes, "Unihan_Variants.txt")?,
        &read_zip_text(&zip_bytes, "Unihan_IRGSources.txt")?,
        &fs::read_to_string(workspace_root.join("data/joyo-variants.tsv"))?,
        &fs::read_to_string(workspace_root.join("data/asahi-variants.tsv"))?,
    )?;
    let variant_members = variants.classes.iter().map(Vec::len).sum::<usize>();
    if variants.classes.len() != EXPECTED_VARIANT_CLASS_COUNT
        || variant_members != EXPECTED_VARIANT_MEMBER_COUNT
        || variants.joyo.len() != EXPECTED_JOYO_PAIR_COUNT
        || variants.asahi.len() != EXPECTED_ASAHI_PAIR_COUNT
    {
        return Err(format!(
            "variant data drifted: expected {EXPECTED_VARIANT_CLASS_COUNT} classes/{EXPECTED_VARIANT_MEMBER_COUNT} members/{EXPECTED_JOYO_PAIR_COUNT} Joyo/{EXPECTED_ASAHI_PAIR_COUNT} Asahi, got {}/{}/{}/{}",
            variants.classes.len(), variant_members, variants.joyo.len(), variants.asahi.len()
        ).into());
    }
    let generated_variants = format_rust_source(&render_generated_variants(&variants))?;
    let variants_output_path = workspace_root.join(GENERATED_VARIANTS_PATH);

    if check {
        let current = fs::read_to_string(&output_path)?;
        if current != generated {
            return Err(format!(
                "{} is not up to date; run `mise run generate-unihan`",
                GENERATED_PATH
            )
            .into());
        }
        let current_variants = fs::read_to_string(&variants_output_path)?;
        if current_variants != generated_variants {
            return Err(format!(
                "{} is not up to date; run `mise run generate-unihan`",
                GENERATED_VARIANTS_PATH
            )
            .into());
        }
        return Ok(());
    }

    fs::create_dir_all(output_path.parent().expect("generated path has a parent"))?;
    fs::write(output_path, generated)?;
    fs::write(variants_output_path, generated_variants)?;
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

fn format_rust_source(source: &str) -> Result<String> {
    let mut child = Command::new("rustfmt")
        .args(["--emit", "stdout", "--edition", "2024"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("rustfmt stdin is piped");
    let output = std::thread::scope(|scope| -> Result<std::process::Output> {
        let writer = scope.spawn(move || stdin.write_all(source.as_bytes()));
        let output = child.wait_with_output()?;
        writer
            .join()
            .map_err(|_| "rustfmt stdin writer panicked")??;
        Ok(output)
    })?;
    if !output.status.success() {
        return Err("rustfmt failed while formatting generated Rust source".into());
    }
    Ok(String::from_utf8(output.stdout)?)
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
    read_zip_text(zip_bytes, "Unihan_Readings.txt")
}

fn read_zip_text(zip_bytes: &[u8], name: &str) -> Result<String> {
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes))?;
    let mut file = archive.by_name(name)?;
    let mut readings = String::new();
    file.read_to_string(&mut readings)?;
    Ok(readings)
}

#[derive(Debug)]
struct Variants {
    classes: Vec<Vec<char>>,
    compatibility: Vec<(char, char)>,
    simplified: Vec<(char, char)>,
    traditional: Vec<(char, char)>,
    z: Vec<(char, char)>,
    joyo: Vec<(char, char)>,
    asahi: Vec<(char, char)>,
}

fn parse_scalar(code: &str) -> Result<char> {
    let code = code.strip_prefix("U+").ok_or("invalid Unihan scalar")?;
    char::from_u32(u32::from_str_radix(code, 16)?)
        .ok_or_else(|| "invalid Unihan scalar value".into())
}

fn variant_values(value: &str) -> Result<Vec<char>> {
    value
        .split_whitespace()
        .map(|field| parse_scalar(field.split('<').next().unwrap_or(field)))
        .collect()
}

fn parse_pair_file(input: &str) -> Result<Vec<(char, char)>> {
    let mut pairs = Vec::new();
    for line in input
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let (new, old) = line.split_once('\t').ok_or("invalid variant pair row")?;
        let mut new_chars = new.chars();
        let Some(new_char) = new_chars.next() else {
            continue;
        };
        if new_chars.next().is_some() {
            continue;
        }
        for old in old.split_whitespace().flat_map(str::chars) {
            pairs.push((old, new_char));
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    Ok(pairs)
}

fn parse_variants(
    variants_input: &str,
    irg_input: &str,
    joyo_input: &str,
    asahi_input: &str,
) -> Result<Variants> {
    let joyo = parse_pair_file(joyo_input)?;
    let asahi = parse_pair_file(asahi_input)?;
    let mut edges = Vec::new();
    let mut simplified = Vec::new();
    let mut traditional = Vec::new();
    let mut z = Vec::new();
    for line in variants_input
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let Some((code, property, value)) = parse_unihan_line(line) else {
            continue;
        };
        if !matches!(
            property,
            "kZVariant" | "kSimplifiedVariant" | "kTraditionalVariant"
        ) {
            continue;
        }
        let source = parse_scalar(code)?;
        let values = variant_values(value)?;
        for target in &values {
            edges.push((source, *target));
        }
        if let Some(target) = values.first().copied() {
            match property {
                "kSimplifiedVariant" => simplified.push((source, target)),
                "kTraditionalVariant" if values.len() == 1 => traditional.push((source, target)),
                "kZVariant" if values.len() == 1 => z.push((source, target)),
                _ => {}
            }
        }
    }
    let mut compatibility = Vec::new();
    for line in irg_input
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let Some((code, property, value)) = parse_unihan_line(line) else {
            continue;
        };
        if property == "kCompatibilityVariant" {
            compatibility.push((parse_scalar(code)?, parse_scalar(value)?));
        }
    }
    edges.extend(joyo.iter().copied());
    edges.extend(asahi.iter().copied());
    edges.extend(compatibility.iter().copied());
    simplified.retain(|(source, target)| source != target);

    let mut graph = BTreeMap::<char, BTreeSet<char>>::new();
    for (left, right) in edges {
        graph.entry(left).or_default().insert(right);
        graph.entry(right).or_default().insert(left);
    }
    let mut seen = BTreeSet::new();
    let mut classes = Vec::new();
    for start in graph.keys().copied() {
        if seen.contains(&start) {
            continue;
        }
        let mut pending = vec![start];
        let mut class = Vec::new();
        while let Some(ch) = pending.pop() {
            if !seen.insert(ch) {
                continue;
            }
            class.push(ch);
            if let Some(neighbors) = graph.get(&ch) {
                pending.extend(neighbors.iter().copied());
            }
        }
        class.sort_unstable();
        classes.push(class);
    }
    classes.sort_by_key(|class| class[0]);
    for pairs in [
        &mut compatibility,
        &mut simplified,
        &mut traditional,
        &mut z,
    ] {
        pairs.sort_unstable();
        pairs.dedup();
    }
    Ok(Variants {
        classes,
        compatibility,
        simplified,
        traditional,
        z,
        joyo,
        asahi,
    })
}

fn parse_unihan_readings(input: &str) -> Result<Vec<(char, Vec<String>)>> {
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
        let char_readings = ordered_khangul_readings(value);
        if char_readings.is_empty() {
            return Err(format!("line {} has no kHangul reading", line_index + 1).into());
        }
        readings.push((ch, char_readings));
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

/// Returns every distinct kHangul reading of a character, in source order but
/// with the canonical reading (the one [`canonical_khangul_reading`] selects)
/// moved to the front.  `KHANGUL_READINGS` keeps only the first (canonical)
/// reading for fallback phonetization, while `KHANGUL_ALL_READINGS` keeps the
/// whole list so the parenthetical-annotation collapser can validate
/// author-supplied alternative readings such as `數字(수자)` or `議論(의론)`.
fn ordered_khangul_readings(value: &str) -> Vec<String> {
    let mut readings: Vec<&str> = Vec::new();
    for field in value.split_whitespace() {
        let (reading, _tags) = field.split_once(':').unwrap_or((field, ""));
        if !reading.is_empty() && !readings.contains(&reading) {
            readings.push(reading);
        }
    }
    if let Some(canonical) = canonical_khangul_reading(value)
        && let Some(position) = readings.iter().position(|reading| *reading == canonical)
    {
        let canonical = readings.remove(position);
        readings.insert(0, canonical);
    }
    readings.into_iter().map(str::to_owned).collect()
}

fn render_generated(readings: &[(char, Vec<String>)]) -> String {
    let mut output = String::new();
    output.push_str(LICENSE_HEADER);
    writeln!(output).unwrap();
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
        "// Policy: KHANGUL_READINGS keeps the canonical kHangul reading (the one"
    )
    .unwrap();
    writeln!(
        output,
        "// tagged E, else the first) per character for fallback phonetization;"
    )
    .unwrap();
    writeln!(
        output,
        "// KHANGUL_ALL_READINGS keeps every kHangul reading (canonical first) for"
    )
    .unwrap();
    writeln!(output, "// reading validation.").unwrap();
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
    for (ch, char_readings) in readings {
        writeln!(
            output,
            "    ('\\u{{{:X}}}', \"{}\"),",
            *ch as u32,
            escape_rust_string(&char_readings[0])
        )
        .unwrap();
    }
    writeln!(output, "];").unwrap();
    writeln!(
        output,
        "pub(crate) static KHANGUL_ALL_READINGS: &[(char, &[&str])] = &["
    )
    .unwrap();
    for (ch, char_readings) in readings {
        let joined = char_readings
            .iter()
            .map(|reading| format!("\"{}\"", escape_rust_string(reading)))
            .collect::<Vec<_>>()
            .join(", ");
        let line = format!("    ('\\u{{{:X}}}', &[{joined}]),", *ch as u32);
        if char_readings.len() < 4 {
            writeln!(output, "{line}").unwrap();
        } else {
            writeln!(output, "    (").unwrap();
            writeln!(output, "        '\\u{{{:X}}}',", *ch as u32).unwrap();
            writeln!(output, "        &[{joined}],").unwrap();
            writeln!(output, "    ),").unwrap();
        }
    }
    writeln!(output, "];").unwrap();
    output
}

fn render_generated_variants(variants: &Variants) -> String {
    let mut output = String::new();
    output.push_str(&LICENSE_HEADER.replace(
        "Generated Unihan kHangul fallback reading tables.",
        "Generated Han character variant normalization tables.",
    ));
    writeln!(output).unwrap();
    writeln!(
        output,
        "// @generated by gukhanmun-unihan. Do not edit by hand."
    )
    .unwrap();
    writeln!(output, "// Unicode version: {UNICODE_VERSION}").unwrap();
    writeln!(
        output,
        "// Sources: {UNIHAN_URL}, data/joyo-variants.tsv, data/asahi-variants.tsv"
    )
    .unwrap();
    writeln!(output).unwrap();
    render_pair_table(&mut output, "COMPATIBILITY_FOLDS", &variants.compatibility);
    render_pair_table(&mut output, "SIMPLIFIED_FORMS", &variants.simplified);
    render_pair_table(&mut output, "TRADITIONAL_FORMS", &variants.traditional);
    render_pair_table(&mut output, "Z_FORMS", &variants.z);
    render_pair_table(
        &mut output,
        "Z_CANONICAL_FORMS",
        &canonical_component_forms(&variants.z),
    );
    render_pair_table(&mut output, "JOYO_FORMS", &variants.joyo);
    render_pair_table(&mut output, "ASAHI_FORMS", &variants.asahi);
    render_pair_table(
        &mut output,
        "COMPATIBILITY_REVERSE_FORMS",
        &unique_reverse_pairs(&variants.compatibility),
    );
    render_pair_table(
        &mut output,
        "SIMPLIFIED_REVERSE_FORMS",
        &unique_reverse_pairs(&variants.simplified),
    );
    render_pair_table(
        &mut output,
        "Z_REVERSE_FORMS",
        &unique_reverse_pairs(&variants.z),
    );
    render_pair_table(
        &mut output,
        "JOYO_REVERSE_FORMS",
        &unique_reverse_pairs(&variants.joyo),
    );
    render_pair_table(
        &mut output,
        "ASAHI_REVERSE_FORMS",
        &unique_reverse_pairs(&variants.asahi),
    );
    render_char_table(&mut output, "JOYO_TARGETS", &targets(&variants.joyo));
    render_char_table(&mut output, "ASAHI_TARGETS", &targets(&variants.asahi));
    output
}

fn unique_reverse_pairs(pairs: &[(char, char)]) -> Vec<(char, char)> {
    let mut sources = BTreeMap::<char, BTreeSet<char>>::new();
    for (source, target) in pairs {
        sources.entry(*target).or_default().insert(*source);
    }
    sources
        .into_iter()
        .filter_map(|(target, sources)| {
            (sources.len() == 1).then(|| (target, *sources.first().expect("one source")))
        })
        .collect()
}

fn canonical_component_forms(pairs: &[(char, char)]) -> Vec<(char, char)> {
    let mut graph = BTreeMap::<char, BTreeSet<char>>::new();
    for (left, right) in pairs {
        graph.entry(*left).or_default().insert(*right);
        graph.entry(*right).or_default().insert(*left);
    }
    let mut seen = BTreeSet::new();
    let mut canonical = Vec::new();
    for start in graph.keys().copied() {
        if seen.contains(&start) {
            continue;
        }
        let mut pending = vec![start];
        let mut members = Vec::new();
        while let Some(ch) = pending.pop() {
            if !seen.insert(ch) {
                continue;
            }
            members.push(ch);
            if let Some(neighbors) = graph.get(&ch) {
                pending.extend(neighbors.iter().copied());
            }
        }
        members.sort_unstable();
        let representative = members[0];
        canonical.extend(
            members
                .into_iter()
                .skip(1)
                .map(|member| (member, representative)),
        );
    }
    canonical.sort_unstable();
    canonical
}

fn targets(pairs: &[(char, char)]) -> Vec<char> {
    pairs
        .iter()
        .map(|(_, target)| *target)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn render_pair_table(output: &mut String, name: &str, pairs: &[(char, char)]) {
    writeln!(output, "pub(crate) static {name}: &[(char, char)] = &[").unwrap();
    for (from, to) in pairs {
        writeln!(
            output,
            "    ('\\u{{{:X}}}', '\\u{{{:X}}}'),",
            *from as u32, *to as u32
        )
        .unwrap();
    }
    writeln!(output, "];").unwrap();
}

fn render_char_table(output: &mut String, name: &str, chars: &[char]) {
    writeln!(output, "pub(crate) static {name}: &[char] = &[").unwrap();
    for ch in chars {
        writeln!(output, "    '\\u{{{:X}}}',", *ch as u32).unwrap();
    }
    writeln!(output, "];").unwrap();
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
    use super::{
        canonical_component_forms, canonical_khangul_reading, ordered_khangul_readings,
        parse_unihan_readings, parse_variants, render_generated, unique_reverse_pairs,
    };
    use proptest::prelude::*;

    fn owned(readings: &[&str]) -> Vec<String> {
        readings
            .iter()
            .map(|reading| (*reading).to_owned())
            .collect()
    }

    #[test]
    fn variant_parser_builds_transitive_classes_and_directional_maps() {
        let variants = parse_variants(
            "U+4E00\tkSimplifiedVariant\tU+4E01\nU+4E01\tkSimplifiedVariant\tU+4E02\nU+4E02\tkSimplifiedVariant\tU+4E02\nU+82B8\tkTraditionalVariant\tU+85DD\nU+85DD\tkZVariant\tU+85DD<kMatthews\n",
            "U+FA19\tkCompatibilityVariant\tU+795E\n",
            "芸\t藝\n",
            "侠\t俠\n",
        )
        .unwrap();
        assert!(
            variants
                .classes
                .iter()
                .any(|class| class.contains(&'芸') && class.contains(&'藝'))
        );
        assert_eq!(variants.joyo, vec![('藝', '芸')]);
        assert_eq!(variants.asahi, vec![('俠', '侠')]);
        assert_eq!(variants.compatibility, vec![('神', '神')]);
        assert_eq!(variants.simplified, vec![('一', '丁'), ('丁', '丂')]);
    }

    #[test]
    fn reverse_variant_index_keeps_only_unique_sources() {
        assert_eq!(
            unique_reverse_pairs(&[('發', '发'), ('髮', '发'), ('藝', '艺')]),
            vec![('艺', '藝')]
        );
    }

    #[test]
    fn z_variant_components_choose_the_lowest_scalar_once() {
        assert_eq!(
            canonical_component_forms(&[('値', '值'), ('值', '値'), ('叱', '𠮟'), ('𠮟', '叱'),]),
            vec![('值', '値'), ('𠮟', '叱')]
        );
    }

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
                ('老', owned(&["로", "노"])),
                ('馬', owned(&["마"])),
                ('龍', owned(&["룡", "용"])),
            ]
        );
    }

    #[test]
    fn ordered_readings_put_the_canonical_reading_first() {
        // The E-tagged reading leads, then the remaining readings in source
        // order; duplicates collapse.
        assert_eq!(ordered_khangul_readings("논:0 론:0E"), owned(&["론", "논"]));
        assert_eq!(
            ordered_khangul_readings("구:0EN 귀:0N 균:0N"),
            owned(&["구", "귀", "균"])
        );
        assert_eq!(ordered_khangul_readings("마:0"), owned(&["마"]));
        assert_eq!(ordered_khangul_readings("수:0 수:0E"), owned(&["수"]));
    }

    #[test]
    fn renderer_emits_sorted_ascii_rust_source() {
        let generated = render_generated(&[('馬', owned(&["마"])), ('龍', owned(&["룡", "용"]))]);

        // Canonical single-reading table.
        assert!(generated.contains("('\\u{99AC}', \"\\u{b9c8}\"),"));
        assert!(generated.contains("('\\u{9F8D}', \"\\u{b8e1}\"),"));
        // Full reading-set table keeps every reading in canonical-first order.
        assert!(generated.contains("('\\u{99AC}', &[\"\\u{b9c8}\"]),"));
        assert!(generated.contains("('\\u{9F8D}', &[\"\\u{b8e1}\", \"\\u{c6a9}\"]),"));
        assert!(generated.contains("pub(crate) static KHANGUL_ALL_READINGS"));
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
