// Gukhanmun: Builds Gukhanmun dictionary backend files from canonical TSV input.
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

//! Dictionary builder support for `gukhanmun-mkdict`.
//!
//! The crate owns parsers for normalized dictionary inputs and writers for the
//! first on-disk FST and CDB dictionary formats. Runtime lookup is handled by
//! backend crates.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error as StdError;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use ciborium::ser::into_writer;
use fst::MapBuilder;
use gukhanmun_cdb::CdbDictionary;
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
const RESERVED_METADATA_KEYS: &[&str] = &[
    "entry_count",
    "version",
    "max_word_chars",
    "max_key_bytes",
    "prefix_count",
];
const CDB_META_KEY: &[u8] = b"__gukhanmun_meta__";
const CDB_MARK_REQUIRE_HANJA: u8 = 0b0000_0001;
const CDB_MARK_REQUIRE_HANGUL: u8 = 0b0000_0010;

/// Error returned while parsing inputs or building dictionary files.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The input violated the builder contract.
    #[error("{0}")]
    Message(String),

    /// An underlying operation failed with extra builder context.
    #[error("{context}: {source}")]
    Source {
        /// Builder context for the failing operation.
        context: String,
        /// Underlying source error.
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
    },

    /// FST backend validation or decoding failed.
    #[error(transparent)]
    Fst(#[from] gukhanmun_fst::Error),

    /// CDB backend validation or decoding failed.
    #[error(transparent)]
    Cdb(#[from] gukhanmun_cdb::Error),
}

impl Error {
    fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    fn source(context: impl Into<String>, source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Source {
            context: context.into(),
            source: Box::new(source),
        }
    }
}

/// Result type returned by dictionary builder APIs.
pub type Result<T> = std::result::Result<T, Error>;

trait ResultContext<T> {
    fn context(self, context: impl Into<String>) -> Result<T>;

    fn with_context(self, context: impl FnOnce() -> String) -> Result<T>;
}

impl<T, E> ResultContext<T> for std::result::Result<T, E>
where
    E: StdError + Send + Sync + 'static,
{
    fn context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|source| Error::source(context, source))
    }

    fn with_context(self, context: impl FnOnce() -> String) -> Result<T> {
        self.map_err(|source| Error::source(context(), source))
    }
}

trait OptionContext<T> {
    fn context(self, context: impl Into<String>) -> Result<T>;
}

impl<T> OptionContext<T> for Option<T> {
    fn context(self, context: impl Into<String>) -> Result<T> {
        self.ok_or_else(|| Error::message(context.into()))
    }
}

macro_rules! bail {
    ($($arg:tt)*) => {
        return Err(Error::message(format!($($arg)*)))
    };
}

macro_rules! ensure {
    ($condition:expr, $($arg:tt)*) => {
        if !$condition {
            bail!($($arg)*);
        }
    };
}

/// The maximum accepted UTF-8 key length when the CLI option is omitted.
pub const DEFAULT_MAX_KEY_BYTES: usize = 1024;

/// The supported output backend format for this implementation step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DictionaryFormat {
    /// Build the FST dictionary file format.
    Fst,

    /// Build the CDB-trie dictionary file format.
    Cdb,
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

    /// Replaces the dictionary-provided rendering constraints in place.
    pub fn set_mark(&mut self, mark: EntryMark) {
        self.mark = mark;
    }
}

/// Selector kind used by a rules-file row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuleKind {
    /// Match a single dictionary entry whose hanja key equals `pattern`.
    Entry,

    /// Match every dictionary entry whose hanja key contains the hanja
    /// substring in `pattern`.
    Contains,

    /// Match every dictionary entry whose hangul reading equals `pattern`.
    Reading,
}

impl RuleKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "entry" => Some(Self::Entry),
            "contains" => Some(Self::Contains),
            "reading" => Some(Self::Reading),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Contains => "contains",
            Self::Reading => "reading",
        }
    }
}

/// One row from a rules file: a selector that picks dictionary entries and the
/// mark bits to OR into their [`EntryMark`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rule {
    kind: RuleKind,
    pattern: String,
    mark: EntryMark,
    reason: String,
    location: String,
}

impl Rule {
    /// Creates a rule for programmatic callers.
    ///
    /// This constructor is unchecked: the pattern, mark bits, and reason are
    /// stored verbatim.  All semantic validation — non-empty pattern, at least
    /// one mark bit set, and `contains` patterns that are hanja-only — runs in
    /// [`apply_rules`], so even programmatically constructed rules surface the
    /// same errors as rules parsed from a TSV file.
    pub fn new(
        kind: RuleKind,
        pattern: impl Into<String>,
        mark: EntryMark,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            pattern: pattern.into(),
            mark,
            reason: reason.into(),
            location: "<programmatic>".to_owned(),
        }
    }

    /// Returns the selector kind.
    pub fn kind(&self) -> RuleKind {
        self.kind
    }

    /// Returns the selector pattern.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Returns the mark bits the rule contributes.
    pub fn mark(&self) -> EntryMark {
        self.mark
    }

    /// Returns the human-readable reason this rule exists.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Returns the source location used for error reporting.
    pub fn location(&self) -> &str {
        &self.location
    }
}

/// Parses a rules TSV file.
///
/// The expected header is `kind`, `pattern`, `require_hanja`,
/// `require_hangul`, `reason` in any column order.  Unknown columns are
/// ignored with a warning printed to stderr, mirroring how dictionary inputs
/// are parsed.  Duplicate `(kind, pattern)` pairs are rejected.
pub fn parse_rules_file(path: &Path) -> Result<Vec<Rule>> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    parse_rules_reader(BufReader::new(file), path)
}

fn parse_rules_reader(reader: impl BufRead, path: &Path) -> Result<Vec<Rule>> {
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
    let columns = parse_rules_header(&header)?;
    let mut rules = Vec::new();
    let mut seen = BTreeSet::<(RuleKind, String)>::new();

    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        let line = line.with_context(|| format!("failed to read {}", path.display()))?;
        if line.is_empty() {
            continue;
        }
        let location = format!("{}:{line_number}", path.display());
        let rule = parse_rule_row(&line, &columns, &location)?;
        if !seen.insert((rule.kind, rule.pattern.clone())) {
            bail!(
                "{}: duplicate rule for kind `{}` and pattern `{}`",
                location,
                rule.kind.as_str(),
                rule.pattern,
            );
        }
        rules.push(rule);
    }

    Ok(rules)
}

#[derive(Clone, Debug)]
struct RulesHeaderColumns {
    kind: usize,
    pattern: usize,
    require_hanja: usize,
    require_hangul: usize,
    reason: usize,
    column_count: usize,
}

fn parse_rules_header(header: &str) -> Result<RulesHeaderColumns> {
    let columns = header.split('\t').collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut kind = None;
    let mut pattern = None;
    let mut require_hanja = None;
    let mut require_hangul = None;
    let mut reason = None;

    for (index, column) in columns.iter().enumerate() {
        ensure!(
            !column.is_empty(),
            "rules TSV header contains an empty column name"
        );
        ensure!(
            seen.insert(*column),
            "rules TSV header contains duplicate `{column}` column"
        );
        match *column {
            "kind" => kind = Some(index),
            "pattern" => pattern = Some(index),
            "require_hanja" => require_hanja = Some(index),
            "require_hangul" => require_hangul = Some(index),
            "reason" => reason = Some(index),
            extra => eprintln!("ignoring unsupported rules TSV column `{extra}`"),
        }
    }

    Ok(RulesHeaderColumns {
        kind: kind.ok_or_else(|| Error::message("rules TSV missing required `kind` column"))?,
        pattern: pattern
            .ok_or_else(|| Error::message("rules TSV missing required `pattern` column"))?,
        require_hanja: require_hanja
            .ok_or_else(|| Error::message("rules TSV missing required `require_hanja` column"))?,
        require_hangul: require_hangul
            .ok_or_else(|| Error::message("rules TSV missing required `require_hangul` column"))?,
        reason: reason
            .ok_or_else(|| Error::message("rules TSV missing required `reason` column"))?,
        column_count: columns.len(),
    })
}

fn parse_rule_row(line: &str, columns: &RulesHeaderColumns, location: &str) -> Result<Rule> {
    let fields = line.split('\t').collect::<Vec<_>>();
    ensure!(
        fields.len() >= columns.column_count,
        "{location}: expected {} TSV fields, got {}",
        columns.column_count,
        fields.len()
    );

    let kind_field = fields[columns.kind];
    let kind = RuleKind::parse(kind_field).ok_or_else(|| {
        Error::message(format!(
            "{location}: unknown rule kind `{kind_field}`; expected `entry`, `contains`, or `reading`"
        ))
    })?;
    let pattern = fields[columns.pattern];
    ensure!(
        !pattern.is_empty(),
        "{location}: `pattern` must not be empty"
    );
    let require_hanja = parse_required_bool(fields[columns.require_hanja], location)?;
    let require_hangul = parse_required_bool(fields[columns.require_hangul], location)?;
    ensure!(
        require_hanja || require_hangul,
        "{location}: rule must set at least one of `require_hanja` or `require_hangul`"
    );
    let reason = fields[columns.reason].trim();
    ensure!(
        !reason.is_empty(),
        "{location}: `reason` must not be empty so future maintainers can audit the rule"
    );

    Ok(Rule {
        kind,
        pattern: pattern.to_owned(),
        mark: EntryMark {
            require_hanja,
            require_hangul,
        },
        reason: reason.to_owned(),
        location: location.to_owned(),
    })
}

fn parse_required_bool(value: &str, location: &str) -> Result<bool> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" | "" => Ok(false),
        other => bail!("{location}: invalid boolean value `{other}`"),
    }
}

/// Applies parsed rules to dictionary entries by OR-merging their mark bits.
///
/// When `allow_unmatched` is false, all rules that matched no entry are
/// collected and reported as a single error so editors can fix them in one
/// pass.  When true, unmatched rules are silently ignored (useful for partial
/// dictionaries shared across builds).
pub fn apply_rules(
    entries: &mut [DictionaryEntry],
    rules: &[Rule],
    allow_unmatched: bool,
) -> Result<()> {
    if rules.is_empty() {
        return Ok(());
    }

    for rule in rules {
        ensure!(
            !rule.pattern.is_empty(),
            "{}: rule pattern must not be empty",
            rule.location,
        );
        ensure!(
            rule.mark.require_hanja || rule.mark.require_hangul,
            "{}: rule must set at least one of `require_hanja` or `require_hangul`",
            rule.location,
        );
        if matches!(rule.kind, RuleKind::Contains) {
            ensure!(
                rule.pattern.chars().all(gukhanmun_core::is_hanja),
                "{}: `contains` rule pattern `{}` must consist only of hanja characters; \
                 dictionary keys can be mixed-script so a pattern with hangul or other \
                 scripts would silently match unrelated entries",
                rule.location,
                rule.pattern,
            );
        }
    }
    let mut matched = vec![false; rules.len()];

    for entry in entries.iter_mut() {
        let hanja = entry.hanja().to_owned();
        let reading = entry.reading().to_owned();
        for (i, rule) in rules.iter().enumerate() {
            let hit = match rule.kind {
                RuleKind::Entry => hanja == rule.pattern,
                RuleKind::Contains => hanja.contains(rule.pattern.as_str()),
                RuleKind::Reading => reading == rule.pattern,
            };
            if hit {
                matched[i] = true;
                let mut mark = entry.mark();
                mark.require_hanja |= rule.mark.require_hanja;
                mark.require_hangul |= rule.mark.require_hangul;
                entry.set_mark(mark);
            }
        }
    }

    if !allow_unmatched {
        let mut unmatched = rules
            .iter()
            .zip(matched.iter())
            .filter(|(_, hit)| !**hit)
            .map(|(rule, _)| {
                format!(
                    "{}: rule `{}={}` matched no entries",
                    rule.location,
                    rule.kind.as_str(),
                    rule.pattern,
                )
            })
            .collect::<Vec<_>>();
        if !unmatched.is_empty() {
            unmatched.sort();
            bail!(
                "{} unmatched rule(s):\n  {}",
                unmatched.len(),
                unmatched.join("\n  ")
            );
        }
    }

    Ok(())
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

    /// Paths to rules TSV files whose entries OR-merge marks into the
    /// dictionary entries before serialization.
    pub rules: Vec<PathBuf>,

    /// Allow rules that match no entries to pass instead of erroring.
    pub allow_unmatched_rules: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            format: DictionaryFormat::Fst,
            merge: MergePolicy::Error,
            validate: false,
            max_key_bytes: DEFAULT_MAX_KEY_BYTES,
            metadata: BTreeMap::new(),
            rules: Vec::new(),
            allow_unmatched_rules: false,
        }
    }
}

/// Builds a dictionary file from normalized TSV, CSV, or JSONL inputs.
pub fn build_dictionary(
    input_paths: &[PathBuf],
    output_path: impl AsRef<Path>,
    options: &BuildOptions,
) -> Result<()> {
    ensure!(
        !input_paths.is_empty(),
        "at least one input file is required"
    );
    let mut entries = read_and_merge_inputs(input_paths, options)?;
    if !options.rules.is_empty() {
        let mut rules = Vec::new();
        let mut seen = BTreeSet::<(RuleKind, String)>::new();
        for path in &options.rules {
            for rule in parse_rules_file(path)? {
                if !seen.insert((rule.kind, rule.pattern.clone())) {
                    bail!(
                        "{}: duplicate rule for kind `{}` and pattern `{}`",
                        rule.location,
                        rule.kind.as_str(),
                        rule.pattern,
                    );
                }
                rules.push(rule);
            }
        }
        apply_rules(&mut entries, &rules, options.allow_unmatched_rules)?;
    }
    let metadata = build_metadata(&options.metadata, &entries)?;
    match options.format {
        DictionaryFormat::Fst => {
            let bytes = build_fst_bytes(&entries, &metadata)?;
            fs::write(output_path.as_ref(), &bytes)
                .with_context(|| format!("failed to write {}", output_path.as_ref().display()))?;

            if options.validate {
                let dictionary = FstDictionary::open(output_path.as_ref()).with_context(|| {
                    format!("failed to validate {}", output_path.as_ref().display())
                })?;
                validate_fst_round_trip(&entries, &dictionary)?;
            }
        }
        DictionaryFormat::Cdb => {
            reject_reserved_cdb_keys(&entries)?;
            build_cdb_file(&entries, &metadata, output_path.as_ref())?;

            if options.validate {
                let dictionary = CdbDictionary::open(output_path.as_ref()).with_context(|| {
                    format!("failed to validate {}", output_path.as_ref().display())
                })?;
                validate_cdb_round_trip(&entries, &dictionary)?;
            }
        }
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
        let entries = parse_input(BufReader::new(file), path, options.max_key_bytes)?;
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

fn reject_reserved_cdb_keys(entries: &[DictionaryEntry]) -> Result<()> {
    for entry in entries {
        ensure!(
            entry.hanja().as_bytes() != CDB_META_KEY,
            "`{}` is reserved for CDB metadata",
            entry.hanja()
        );
    }
    Ok(())
}

fn parse_input(
    reader: impl BufRead,
    path: &Path,
    max_key_bytes: usize,
) -> Result<Vec<DictionaryEntry>> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("csv") => parse_csv(reader, path, max_key_bytes),
        Some("jsonl") => parse_jsonl(reader, path, max_key_bytes),
        _ => parse_tsv(reader, path, max_key_bytes),
    }
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

fn parse_csv(
    reader: impl BufRead,
    path: &Path,
    max_key_bytes: usize,
) -> Result<Vec<DictionaryEntry>> {
    let mut reader = csv::Reader::from_reader(reader);
    let header = reader
        .headers()
        .with_context(|| format!("failed to read CSV header from {}", path.display()))?
        .iter()
        .collect::<Vec<_>>()
        .join("\t");
    let columns = parse_header_with_format(&header, "CSV")?;
    let mut entries = Vec::new();

    for (index, record) in reader.records().enumerate() {
        let location = format!("{}:{}", path.display(), index + 2);
        let record = record.with_context(|| format!("failed to read CSV record at {location}"))?;
        let fields = record.iter().collect::<Vec<_>>();
        entries.push(parse_fields(&fields, &columns, max_key_bytes, &location)?);
    }

    Ok(entries)
}

fn parse_jsonl(
    reader: impl BufRead,
    path: &Path,
    max_key_bytes: usize,
) -> Result<Vec<DictionaryEntry>> {
    let mut entries = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 1;
        let line = line.with_context(|| format!("failed to read {}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: JsonLineEntry = serde_json::from_str(&line).with_context(|| {
            format!(
                "failed to parse JSONL record at {}:{line_number}",
                path.display()
            )
        })?;
        entries.push(normalize_entry(
            &record.hanja,
            &record.hangul,
            EntryMark {
                require_hanja: record.require_hanja,
                require_hangul: record.require_hangul,
            },
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
    parse_header_with_format(header, "TSV")
}

fn parse_header_with_format(header: &str, format_name: &str) -> Result<HeaderColumns> {
    let columns = header.split('\t').collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut hanja = None;
    let mut hangul = None;
    let mut require_hanja = None;
    let mut require_hangul = None;

    for (index, column) in columns.iter().enumerate() {
        ensure!(
            !column.is_empty(),
            "{format_name} header contains an empty column name"
        );
        ensure!(
            seen.insert(*column),
            "{format_name} header contains duplicate `{column}` column"
        );
        match *column {
            "hanja" => hanja = Some(index),
            "hangul" => hangul = Some(index),
            "require_hanja" => require_hanja = Some(index),
            "require_hangul" => require_hangul = Some(index),
            extra => eprintln!("ignoring unsupported {format_name} column `{extra}`"),
        }
    }

    Ok(HeaderColumns {
        hanja: hanja.ok_or_else(|| Error::message("missing required `hanja` column"))?,
        hangul: hangul.ok_or_else(|| Error::message("missing required `hangul` column"))?,
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
    parse_fields(&fields, columns, max_key_bytes, location)
}

fn parse_fields(
    fields: &[&str],
    columns: &HeaderColumns,
    max_key_bytes: usize,
    location: &str,
) -> Result<DictionaryEntry> {
    ensure!(
        fields.len() >= columns.column_count,
        "{location}: expected {} TSV fields, got {}",
        columns.column_count,
        fields.len()
    );

    let hanja = fields[columns.hanja];
    let hangul = fields[columns.hangul];
    let require_hanja = parse_optional_bool(fields, columns.require_hanja, location)?;
    let require_hangul = parse_optional_bool(fields, columns.require_hangul, location)?;

    normalize_entry(
        hanja,
        hangul,
        EntryMark {
            require_hanja,
            require_hangul,
        },
        max_key_bytes,
        location,
    )
}

fn normalize_entry(
    hanja: &str,
    hangul: &str,
    mark: EntryMark,
    max_key_bytes: usize,
    location: &str,
) -> Result<DictionaryEntry> {
    ensure!(!hanja.is_empty(), "{location}: `hanja` must not be empty");
    ensure!(!hangul.is_empty(), "{location}: `hangul` must not be empty");
    ensure!(
        hanja.len() <= max_key_bytes,
        "{location}: key `{hanja}` exceeds --max-key-bytes={max_key_bytes}"
    );

    Ok(DictionaryEntry::new(hanja, hangul, mark))
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

fn build_cdb_file(
    entries: &[DictionaryEntry],
    metadata: &BTreeMap<String, String>,
    output_path: &Path,
) -> Result<()> {
    let records = build_cdb_records(entries);
    let mut metadata = metadata.clone();
    metadata.insert("prefix_count".to_owned(), records.len().to_string());
    let mut metadata_bytes = Vec::new();
    into_writer(&metadata, &mut metadata_bytes).context("failed to encode dictionary metadata")?;

    let output_name = output_path.to_str().ok_or_else(|| {
        Error::message(format!(
            "CDB output path must be valid UTF-8: {}",
            output_path.display()
        ))
    })?;
    let mut writer = cdb::CDBWriter::create(output_name)
        .with_context(|| format!("failed to create {}", output_path.display()))?;
    writer
        .add(CDB_META_KEY, &metadata_bytes)
        .context("failed to add CDB metadata record")?;
    for (key, record) in records {
        let value = encode_cdb_record(record.as_ref())?;
        writer
            .add(key.as_bytes(), &value)
            .with_context(|| format!("failed to add CDB record `{key}`"))?;
    }
    writer
        .finish()
        .with_context(|| format!("failed to finish {}", output_path.display()))
}

fn build_cdb_records(entries: &[DictionaryEntry]) -> BTreeMap<String, Option<DictionaryEntry>> {
    let mut records = BTreeMap::new();
    for entry in entries {
        let mut prefix = String::new();
        for ch in entry.hanja().chars() {
            prefix.push(ch);
            records.entry(prefix.clone()).or_insert(None);
        }
        records.insert(entry.hanja().to_owned(), Some(entry.clone()));
    }
    records
}

fn encode_cdb_record(entry: Option<&DictionaryEntry>) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    match entry {
        Some(entry) => {
            let reading_len = u16::try_from(entry.reading().len())
                .with_context(|| format!("reading for `{}` is too long", entry.hanja()))?;
            output.push(1);
            output.push(encode_cdb_mark(entry.mark()));
            output.extend_from_slice(&reading_len.to_le_bytes());
            output.extend_from_slice(entry.reading().as_bytes());
        }
        None => {
            output.push(0);
            output.push(0);
            output.extend_from_slice(&0u16.to_le_bytes());
        }
    }
    Ok(output)
}

fn encode_cdb_mark(mark: EntryMark) -> u8 {
    let mut encoded = 0;
    if mark.require_hanja {
        encoded |= CDB_MARK_REQUIRE_HANJA;
    }
    if mark.require_hangul {
        encoded |= CDB_MARK_REQUIRE_HANGUL;
    }
    encoded
}

fn validate_fst_round_trip(entries: &[DictionaryEntry], dictionary: &FstDictionary) -> Result<()> {
    ensure!(
        dictionary.entry_count() == entries.len() as u64,
        "round-trip validation failed: entry count mismatch"
    );
    for entry in entries {
        let actual = dictionary.lookup(entry.hanja())?.ok_or_else(|| {
            Error::message(format!(
                "round-trip validation failed: `{}` is missing",
                entry.hanja()
            ))
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

fn validate_cdb_round_trip(entries: &[DictionaryEntry], dictionary: &CdbDictionary) -> Result<()> {
    ensure!(
        dictionary.entry_count() == entries.len() as u64,
        "round-trip validation failed: entry count mismatch"
    );
    for entry in entries {
        let actual = dictionary.lookup(entry.hanja())?.ok_or_else(|| {
            Error::message(format!(
                "round-trip validation failed: `{}` is missing",
                entry.hanja()
            ))
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

#[derive(Debug, Deserialize)]
struct JsonLineEntry {
    hanja: String,
    hangul: String,
    #[serde(default, alias = "requireHanja")]
    require_hanja: bool,
    #[serde(default, alias = "requireHangul")]
    require_hangul: bool,
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
        .ok_or_else(|| Error::message("metadata must use KEY=VAL syntax"))?;
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

    fn parse_rules_str(input: &str) -> Result<Vec<Rule>> {
        parse_rules_reader(input.as_bytes(), Path::new("rules.tsv"))
    }

    #[test]
    fn parses_minimal_rules_tsv() {
        let input = "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
                     entry\t漢字\ttrue\tfalse\thomophone\n\
                     contains\t驟\ttrue\tfalse\trare hanja\n\
                     reading\t사기\ttrue\tfalse\tcommon homophone\n";

        let rules = parse_rules_str(input).unwrap();

        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].kind(), RuleKind::Entry);
        assert_eq!(rules[0].pattern(), "漢字");
        assert!(rules[0].mark().require_hanja);
        assert!(!rules[0].mark().require_hangul);
        assert_eq!(rules[0].reason(), "homophone");
        assert_eq!(rules[1].kind(), RuleKind::Contains);
        assert_eq!(rules[2].kind(), RuleKind::Reading);
    }

    #[test]
    fn rejects_unknown_rule_kind() {
        let input = "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
                     glob\t漢*\ttrue\tfalse\tnope\n";

        let error = parse_rules_str(input).unwrap_err();

        let text = error.to_string();
        assert!(text.contains("unknown rule kind `glob`"), "{text}");
        // Recovery guidance must enumerate the currently accepted kinds.
        assert!(text.contains("`entry`"), "{text}");
        assert!(text.contains("`contains`"), "{text}");
        assert!(text.contains("`reading`"), "{text}");
    }

    #[test]
    fn rejects_contains_rule_with_non_hanja_pattern_from_tsv() {
        // Mixed-script dictionary keys (e.g. `布告하다`) mean a non-hanja
        // `contains` pattern would silently mark unrelated entries; reject at
        // the apply step.
        let input = "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
                     contains\t하다\ttrue\tfalse\ttypo\n";
        let rules = parse_rules_str(input).unwrap();
        let mut entries = vec![entry("布告하다", "포고하다")];

        let error = apply_rules(&mut entries, &rules, false).unwrap_err();

        assert!(
            error.to_string().contains("must consist only of hanja"),
            "{error}"
        );
    }

    #[test]
    fn rejects_rule_with_empty_reason() {
        let input = "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
                     entry\t漢字\ttrue\tfalse\t\n";

        let error = parse_rules_str(input).unwrap_err();

        assert!(error.to_string().contains("reason"), "{error}");
    }

    #[test]
    fn rejects_rule_with_no_mark_bits_set() {
        let input = "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
                     entry\t漢字\tfalse\tfalse\tno-op\n";

        let error = parse_rules_str(input).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("at least one of `require_hanja` or `require_hangul`"),
            "{error}"
        );
    }

    #[test]
    fn rejects_duplicate_rule_keys() {
        let input = "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
                     entry\t漢字\ttrue\tfalse\tfirst\n\
                     entry\t漢字\tfalse\ttrue\tsecond\n";

        let error = parse_rules_str(input).unwrap_err();

        assert!(error.to_string().contains("duplicate rule"), "{error}");
    }

    #[test]
    fn allows_overlapping_rules_across_kinds() {
        let input = "kind\tpattern\trequire_hanja\trequire_hangul\treason\n\
                     entry\t漢字\ttrue\tfalse\thomophone entry\n\
                     contains\t漢\ttrue\tfalse\trare character\n";

        let rules = parse_rules_str(input).unwrap();

        assert_eq!(rules.len(), 2);
    }

    fn entry(hanja: &str, reading: &str) -> DictionaryEntry {
        DictionaryEntry::new(hanja, reading, EntryMark::default())
    }

    #[test]
    fn apply_rules_or_merges_marks_across_kinds() {
        let mut entries = vec![
            entry("漢字", "한자"),
            entry("天地", "천지"),
            entry("史記", "사기"),
            entry("詐欺", "사기"),
        ];
        let rules = vec![
            Rule::new(
                RuleKind::Entry,
                "漢字",
                EntryMark {
                    require_hanja: true,
                    require_hangul: false,
                },
                "homophone-heavy entry",
            ),
            Rule::new(
                RuleKind::Contains,
                "天",
                EntryMark {
                    require_hanja: true,
                    require_hangul: false,
                },
                "rare hanja",
            ),
            Rule::new(
                RuleKind::Reading,
                "사기",
                EntryMark {
                    require_hanja: true,
                    require_hangul: false,
                },
                "ambiguous reading",
            ),
        ];

        apply_rules(&mut entries, &rules, false).unwrap();

        assert!(
            entries[0].mark().require_hanja,
            "entry rule applied to 漢字"
        );
        assert!(
            entries[1].mark().require_hanja,
            "contains rule applied to 天地"
        );
        assert!(
            entries[2].mark().require_hanja,
            "reading rule applied to 史記"
        );
        assert!(
            entries[3].mark().require_hanja,
            "reading rule applied to 詐欺"
        );
    }

    #[test]
    fn apply_rules_or_merges_multiple_rules_on_one_entry() {
        let mut entries = vec![entry("漢字", "한자")];
        let rules = vec![
            Rule::new(
                RuleKind::Entry,
                "漢字",
                EntryMark {
                    require_hanja: true,
                    require_hangul: false,
                },
                "entry-level",
            ),
            Rule::new(
                RuleKind::Reading,
                "한자",
                EntryMark {
                    require_hanja: false,
                    require_hangul: true,
                },
                "reading-level",
            ),
        ];

        apply_rules(&mut entries, &rules, false).unwrap();

        let mark = entries[0].mark();
        assert!(mark.require_hanja);
        assert!(mark.require_hangul);
    }

    #[test]
    fn apply_rules_reports_all_unmatched_rules_in_one_error() {
        let mut entries = vec![entry("漢字", "한자")];
        let rules = vec![
            Rule::new(
                RuleKind::Entry,
                "天地",
                EntryMark {
                    require_hanja: true,
                    require_hangul: false,
                },
                "missing entry",
            ),
            Rule::new(
                RuleKind::Contains,
                "驟",
                EntryMark {
                    require_hanja: true,
                    require_hangul: false,
                },
                "missing contains",
            ),
        ];

        let error = apply_rules(&mut entries, &rules, false).unwrap_err();

        let text = error.to_string();
        assert!(text.contains("entry=天地"), "{text}");
        assert!(text.contains("contains=驟"), "{text}");
        assert!(text.contains("2 unmatched"), "{text}");
    }

    #[test]
    fn apply_rules_accepts_multi_hanja_contains_pattern() {
        // `contains` is a substring matcher, so multi-character hanja patterns
        // mark every entry containing the substring.
        let mut entries = vec![
            entry("國民學校", "국민학교"),
            entry("國民年金", "국민연금"),
            entry("民國", "민국"),
        ];
        let rules = vec![Rule::new(
            RuleKind::Contains,
            "國民",
            EntryMark {
                require_hanja: true,
                require_hangul: false,
            },
            "compound containing 國民",
        )];

        apply_rules(&mut entries, &rules, false).unwrap();

        assert!(entries[0].mark().require_hanja);
        assert!(entries[1].mark().require_hanja);
        assert!(
            !entries[2].mark().require_hanja,
            "民國 does not contain the substring 國民"
        );
    }

    #[test]
    fn apply_rules_rejects_contains_rule_with_non_hanja_character() {
        // Dictionary keys can be mixed-script (e.g. `布告하다`), so a `contains`
        // pattern with hangul would silently mark every `~하다` entry.
        let mut entries = vec![entry("布告하다", "포고하다"), entry("漢字", "한자")];
        let rules = vec![Rule::new(
            RuleKind::Contains,
            "하",
            EntryMark {
                require_hanja: true,
                require_hangul: false,
            },
            "typo: meant a rare hanja",
        )];

        let error = apply_rules(&mut entries, &rules, false).unwrap_err();

        let text = error.to_string();
        assert!(text.contains("must consist only of hanja"), "{text}");
        assert!(
            !entries[0].mark().require_hanja,
            "the typo'd rule must not silently mark 布告하다"
        );
    }

    #[test]
    fn apply_rules_rejects_programmatic_empty_pattern() {
        let mut entries = vec![entry("漢字", "한자")];
        let rules = vec![Rule::new(
            RuleKind::Entry,
            "",
            EntryMark {
                require_hanja: true,
                require_hangul: false,
            },
            "programmatic mistake",
        )];

        let error = apply_rules(&mut entries, &rules, false).unwrap_err();

        assert!(error.to_string().contains("must not be empty"), "{error}");
    }

    #[test]
    fn apply_rules_rejects_programmatic_no_mark_bits() {
        let mut entries = vec![entry("漢字", "한자")];
        let rules = vec![Rule::new(
            RuleKind::Entry,
            "漢字",
            EntryMark::default(),
            "programmatic mistake",
        )];

        let error = apply_rules(&mut entries, &rules, false).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("at least one of `require_hanja` or `require_hangul`"),
            "{error}"
        );
    }

    #[test]
    fn apply_rules_allows_unmatched_when_configured() {
        let mut entries = vec![entry("漢字", "한자")];
        let rules = vec![Rule::new(
            RuleKind::Entry,
            "天地",
            EntryMark {
                require_hanja: true,
                require_hangul: false,
            },
            "missing entry",
        )];

        apply_rules(&mut entries, &rules, true).unwrap();

        assert!(!entries[0].mark().require_hanja);
    }
}
