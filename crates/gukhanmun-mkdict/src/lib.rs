//! Dictionary builder support for `gukhanmun-mkdict`.
//!
//! The crate owns the canonical TSV parser and writer for the first on-disk
//! FST dictionary format. Runtime lookup is handled by the `gukhanmun-fst`
//! backend crate.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail, ensure};
use ciborium::ser::into_writer;
use fst::MapBuilder;
use gukhanmun_fst::FstDictionary;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const MAGIC: &[u8; 8] = b"GUKHMFST";
const FORMAT_VERSION: u32 = 1;
const FIXED_HEADER_LEN: usize = 64;
const MARK_REQUIRE_HANJA: u8 = 0b0000_0001;
const MARK_REQUIRE_HANGUL: u8 = 0b0000_0010;
const VALUE_MARK_SHIFT: u64 = 16;
const VALUE_OFFSET_SHIFT: u64 = 24;
const VALUE_MAX_OFFSET: u64 = (1u64 << 40) - 1;
const RESERVED_METADATA_KEYS: &[&str] =
    &["entry_count", "version", "max_word_chars", "max_key_bytes"];

/// The maximum accepted UTF-8 key length when the CLI option is omitted.
pub const DEFAULT_MAX_KEY_BYTES: usize = 1024;

/// The supported output backend format for this implementation step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DictionaryFormat {
    /// Build the FST dictionary file format.
    Fst,
}

/// Conflict policy used when the same hanja key appears more than once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergePolicy {
    /// Treat duplicate keys as an error.
    Error,

    /// Keep the first entry and ignore later duplicates.
    FirstWins,

    /// Replace earlier entries with the last duplicate.
    LastWins,
}

/// Dictionary-provided rendering constraints encoded in built files.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntryMark {
    /// Whether output should keep the original hanja visible.
    pub require_hanja: bool,

    /// Whether output should include a hangul gloss when hanja remains primary.
    pub require_hangul: bool,
}

/// One normalized dictionary entry after parsing and merge handling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DictionaryEntry {
    hanja: String,
    reading: String,
    mark: EntryMark,
}

impl DictionaryEntry {
    /// Creates a dictionary entry from a hanja key, hangul reading, and mark.
    pub fn new(hanja: impl Into<String>, reading: impl Into<String>, mark: EntryMark) -> Self {
        Self {
            hanja: hanja.into(),
            reading: reading.into(),
            mark,
        }
    }

    /// Returns the hanja key.
    pub fn hanja(&self) -> &str {
        &self.hanja
    }

    /// Returns the hangul reading.
    pub fn reading(&self) -> &str {
        &self.reading
    }

    /// Returns dictionary-provided rendering constraints.
    pub fn mark(&self) -> EntryMark {
        self.mark
    }
}

/// Options controlling dictionary file construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildOptions {
    /// Output backend format.
    pub format: DictionaryFormat,

    /// Duplicate-key merge policy.
    pub merge: MergePolicy,

    /// Whether to reopen and validate the generated output.
    pub validate: bool,

    /// Maximum accepted UTF-8 byte length for dictionary keys.
    pub max_key_bytes: usize,

    /// User-supplied metadata values embedded in the output file.
    pub metadata: BTreeMap<String, String>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            format: DictionaryFormat::Fst,
            merge: MergePolicy::Error,
            validate: false,
            max_key_bytes: DEFAULT_MAX_KEY_BYTES,
            metadata: BTreeMap::new(),
        }
    }
}

/// Builds a dictionary file from canonical TSV inputs.
pub fn build_dictionary(
    input_paths: &[PathBuf],
    output_path: impl AsRef<Path>,
    options: &BuildOptions,
) -> Result<()> {
    ensure!(
        !input_paths.is_empty(),
        "at least one input file is required"
    );
    match options.format {
        DictionaryFormat::Fst => {}
    }

    let entries = read_and_merge_inputs(input_paths, options)?;
    let metadata = build_metadata(&options.metadata, &entries)?;
    let bytes = build_fst_bytes(&entries, &metadata)?;
    fs::write(output_path.as_ref(), &bytes)
        .with_context(|| format!("failed to write {}", output_path.as_ref().display()))?;

    if options.validate {
        let dictionary = FstDictionary::open(output_path.as_ref())
            .with_context(|| format!("failed to validate {}", output_path.as_ref().display()))?;
        validate_round_trip(&entries, &dictionary)?;
    }

    Ok(())
}

fn read_and_merge_inputs(
    input_paths: &[PathBuf],
    options: &BuildOptions,
) -> Result<Vec<DictionaryEntry>> {
    let mut merged = BTreeMap::<String, DictionaryEntry>::new();

    for path in input_paths {
        let file =
            fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        let entries = parse_tsv(BufReader::new(file), path, options.max_key_bytes)?;
        for entry in entries {
            match (options.merge, merged.contains_key(entry.hanja())) {
                (MergePolicy::Error, true) => bail!("duplicate entry for `{}`", entry.hanja()),
                (MergePolicy::FirstWins, true) => {}
                (MergePolicy::LastWins, true) | (_, false) => {
                    merged.insert(entry.hanja.clone(), entry);
                }
            }
        }
    }

    Ok(merged.into_values().collect())
}

fn parse_tsv(
    reader: impl BufRead,
    path: &Path,
    max_key_bytes: usize,
) -> Result<Vec<DictionaryEntry>> {
    let mut lines = reader.lines();
    let header = loop {
        let Some(line) = lines.next() else {
            bail!("{} is empty", path.display());
        };
        let line = line.with_context(|| format!("failed to read {}", path.display()))?;
        if !line.is_empty() {
            break line;
        }
    };
    let columns = parse_header(&header)?;
    let mut entries = Vec::new();

    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        let line = line.with_context(|| format!("failed to read {}", path.display()))?;
        if line.is_empty() {
            continue;
        }
        entries.push(parse_row(
            &line,
            &columns,
            max_key_bytes,
            &format!("{}:{line_number}", path.display()),
        )?);
    }

    Ok(entries)
}

#[derive(Clone, Debug)]
struct HeaderColumns {
    hanja: usize,
    hangul: usize,
    require_hanja: Option<usize>,
    require_hangul: Option<usize>,
    column_count: usize,
}

fn parse_header(header: &str) -> Result<HeaderColumns> {
    let columns = header.split('\t').collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut hanja = None;
    let mut hangul = None;
    let mut require_hanja = None;
    let mut require_hangul = None;

    for (index, column) in columns.iter().enumerate() {
        ensure!(
            !column.is_empty(),
            "TSV header contains an empty column name"
        );
        ensure!(
            seen.insert(*column),
            "TSV header contains duplicate `{column}` column"
        );
        match *column {
            "hanja" => hanja = Some(index),
            "hangul" => hangul = Some(index),
            "require_hanja" => require_hanja = Some(index),
            "require_hangul" => require_hangul = Some(index),
            extra => eprintln!("ignoring unsupported TSV column `{extra}`"),
        }
    }

    Ok(HeaderColumns {
        hanja: hanja.ok_or_else(|| anyhow!("missing required `hanja` column"))?,
        hangul: hangul.ok_or_else(|| anyhow!("missing required `hangul` column"))?,
        require_hanja,
        require_hangul,
        column_count: columns.len(),
    })
}

fn parse_row(
    line: &str,
    columns: &HeaderColumns,
    max_key_bytes: usize,
    location: &str,
) -> Result<DictionaryEntry> {
    let fields = line.split('\t').collect::<Vec<_>>();
    ensure!(
        fields.len() >= columns.column_count,
        "{location}: expected {} TSV fields, got {}",
        columns.column_count,
        fields.len()
    );

    let hanja = fields[columns.hanja];
    let hangul = fields[columns.hangul];
    ensure!(!hanja.is_empty(), "{location}: `hanja` must not be empty");
    ensure!(!hangul.is_empty(), "{location}: `hangul` must not be empty");
    ensure!(
        hanja.len() <= max_key_bytes,
        "{location}: key `{hanja}` exceeds --max-key-bytes={max_key_bytes}"
    );

    let require_hanja = parse_optional_bool(fields.as_slice(), columns.require_hanja, location)?;
    let require_hangul = parse_optional_bool(fields.as_slice(), columns.require_hangul, location)?;

    Ok(DictionaryEntry::new(
        hanja,
        hangul,
        EntryMark {
            require_hanja,
            require_hangul,
        },
    ))
}

fn parse_optional_bool(fields: &[&str], index: Option<usize>, location: &str) -> Result<bool> {
    let Some(index) = index else {
        return Ok(false);
    };
    let Some(value) = fields.get(index).copied() else {
        return Ok(false);
    };
    if value.is_empty() {
        return Ok(false);
    }
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => bail!("{location}: invalid boolean value `{value}`"),
    }
}

fn build_metadata(
    user_metadata: &BTreeMap<String, String>,
    entries: &[DictionaryEntry],
) -> Result<BTreeMap<String, String>> {
    for key in RESERVED_METADATA_KEYS {
        ensure!(
            !user_metadata.contains_key(*key),
            "`{key}` metadata is reserved"
        );
    }

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "source".to_owned(),
        user_metadata.get("source").cloned().unwrap_or_default(),
    );
    metadata.insert(
        "license".to_owned(),
        user_metadata.get("license").cloned().unwrap_or_default(),
    );
    metadata.insert(
        "build_date".to_owned(),
        user_metadata
            .get("build_date")
            .cloned()
            .unwrap_or_else(default_build_date),
    );
    metadata.insert("entry_count".to_owned(), entries.len().to_string());
    metadata.insert("version".to_owned(), FORMAT_VERSION.to_string());
    metadata.insert(
        "max_word_chars".to_owned(),
        entries
            .iter()
            .map(|entry| entry.hanja().chars().count())
            .max()
            .unwrap_or(0)
            .to_string(),
    );
    metadata.insert(
        "max_key_bytes".to_owned(),
        entries
            .iter()
            .map(|entry| entry.hanja().len())
            .max()
            .unwrap_or(0)
            .to_string(),
    );

    for (key, value) in user_metadata {
        metadata.entry(key.clone()).or_insert_with(|| value.clone());
    }

    Ok(metadata)
}

fn default_build_date() -> String {
    let Some(epoch) = env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|epoch| epoch.parse::<i64>().ok())
    else {
        return "1970-01-01T00:00:00Z".to_owned();
    };
    OffsetDateTime::from_unix_timestamp(epoch)
        .ok()
        .and_then(|datetime| datetime.format(&Rfc3339).ok())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_owned())
}

fn build_fst_bytes(
    entries: &[DictionaryEntry],
    metadata: &BTreeMap<String, String>,
) -> Result<Vec<u8>> {
    let mut metadata_bytes = Vec::new();
    into_writer(metadata, &mut metadata_bytes).context("failed to encode dictionary metadata")?;

    let mut readings = Vec::new();
    let mut builder = MapBuilder::memory();
    for entry in entries {
        let reading_len = u16::try_from(entry.reading().len())
            .with_context(|| format!("reading for `{}` is too long", entry.hanja()))?;
        let reading_offset =
            u64::try_from(readings.len()).context("reading table offset too large")?;
        ensure!(
            reading_offset <= VALUE_MAX_OFFSET,
            "reading table exceeds the FST value layout"
        );
        let value = encode_value(reading_len, entry.mark(), reading_offset);
        builder
            .insert(entry.hanja().as_bytes(), value)
            .with_context(|| format!("failed to insert `{}` into FST", entry.hanja()))?;
        readings.extend_from_slice(entry.reading().as_bytes());
    }
    let fst_bytes = builder.into_inner().context("failed to finish FST map")?;

    let metadata_offset = u64::try_from(FIXED_HEADER_LEN).expect("header length fits in u64");
    let fst_offset = metadata_offset
        .checked_add(u64::try_from(metadata_bytes.len()).context("metadata too large")?)
        .context("FST offset overflow")?;
    let readings_offset = fst_offset
        .checked_add(u64::try_from(fst_bytes.len()).context("FST bytes too large")?)
        .context("reading table offset overflow")?;
    let header = FixedHeader {
        metadata_offset,
        metadata_len: u64::try_from(metadata_bytes.len()).context("metadata too large")?,
        fst_offset,
        fst_len: u64::try_from(fst_bytes.len()).context("FST bytes too large")?,
        readings_offset,
        readings_len: u64::try_from(readings.len()).context("reading table too large")?,
    };

    let mut output = Vec::with_capacity(
        FIXED_HEADER_LEN + metadata_bytes.len() + fst_bytes.len() + readings.len(),
    );
    header.write(&mut output);
    output.extend(metadata_bytes);
    output.extend(fst_bytes);
    output.extend(readings);
    Ok(output)
}

fn validate_round_trip(entries: &[DictionaryEntry], dictionary: &FstDictionary) -> Result<()> {
    ensure!(
        dictionary.entry_count() == entries.len() as u64,
        "round-trip validation failed: entry count mismatch"
    );
    for entry in entries {
        let actual = dictionary.lookup(entry.hanja())?.ok_or_else(|| {
            anyhow!(
                "round-trip validation failed: `{}` is missing",
                entry.hanja()
            )
        })?;
        let mark = actual.mark();
        ensure!(
            actual.reading() == entry.reading()
                && mark.require_hanja == entry.mark().require_hanja
                && mark.require_hangul == entry.mark().require_hangul,
            "round-trip validation failed for `{}`",
            entry.hanja()
        );
    }
    Ok(())
}

fn encode_value(reading_len: u16, mark: EntryMark, reading_offset: u64) -> u64 {
    u64::from(reading_len)
        | (u64::from(encode_mark(mark)) << VALUE_MARK_SHIFT)
        | (reading_offset << VALUE_OFFSET_SHIFT)
}

fn encode_mark(mark: EntryMark) -> u8 {
    let mut encoded = 0;
    if mark.require_hanja {
        encoded |= MARK_REQUIRE_HANJA;
    }
    if mark.require_hangul {
        encoded |= MARK_REQUIRE_HANGUL;
    }
    encoded
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FixedHeader {
    metadata_offset: u64,
    metadata_len: u64,
    fst_offset: u64,
    fst_len: u64,
    readings_offset: u64,
    readings_len: u64,
}

impl FixedHeader {
    fn write(self, output: &mut Vec<u8>) {
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        output.extend_from_slice(&(FIXED_HEADER_LEN as u32).to_le_bytes());
        output.extend_from_slice(&self.metadata_offset.to_le_bytes());
        output.extend_from_slice(&self.metadata_len.to_le_bytes());
        output.extend_from_slice(&self.fst_offset.to_le_bytes());
        output.extend_from_slice(&self.fst_len.to_le_bytes());
        output.extend_from_slice(&self.readings_offset.to_le_bytes());
        output.extend_from_slice(&self.readings_len.to_le_bytes());
        debug_assert_eq!(output.len(), FIXED_HEADER_LEN);
    }
}

/// Parses one `KEY=VAL` metadata argument.
pub fn parse_metadata_arg(arg: &str) -> Result<(String, String)> {
    let (key, value) = arg
        .split_once('=')
        .ok_or_else(|| anyhow!("metadata must use KEY=VAL syntax"))?;
    ensure!(!key.is_empty(), "metadata key must not be empty");
    Ok((key.to_owned(), value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headered_tsv_and_optional_flags() {
        let input = "hanja\thangul\trequire_hanja\trequire_hangul\tcategory\n漢字\t한자\t1\tfalse\tnoun\n天地\t천지\t\ttrue\tnoun\n";

        let entries = parse_tsv(input.as_bytes(), Path::new("fixture.tsv"), 1024).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].hanja(), "漢字");
        assert_eq!(entries[0].reading(), "한자");
        assert!(entries[0].mark().require_hanja);
        assert!(!entries[0].mark().require_hangul);
        assert!(!entries[1].mark().require_hanja);
        assert!(entries[1].mark().require_hangul);
    }

    #[test]
    fn rejects_invalid_boolean_values() {
        let input = "hanja\thangul\trequire_hanja\n漢字\t한자\tyes\n";

        let error = parse_tsv(input.as_bytes(), Path::new("fixture.tsv"), 1024).unwrap_err();

        assert!(error.to_string().contains("invalid boolean value `yes`"));
    }

    #[test]
    fn rejects_reserved_metadata_keys() {
        let metadata = BTreeMap::from([("entry_count".to_owned(), "1".to_owned())]);

        let error = build_metadata(&metadata, &[]).unwrap_err();

        assert!(error.to_string().contains("reserved"));
    }
}
