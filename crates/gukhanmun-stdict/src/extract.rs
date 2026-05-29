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

//! Extractor for Standard Korean Language Dictionary JSON dumps.

use std::collections::btree_map::Entry as BTreeEntry;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;

use serde::Deserialize;
use zip::ZipArchive;

/// Error returned while extracting Standard Korean Language Dictionary data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Filesystem or stream I/O failed.
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),

    /// JSON input could not be decoded.
    #[error("failed to decode dictionary JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// ZIP archive input could not be read.
    #[error("failed to read dictionary ZIP archive: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Result type returned by Standard Korean Language Dictionary extraction APIs.
pub type Result<T> = std::result::Result<T, Error>;

/// Extracts a canonical dictionary TSV from a Standard Korean Language
/// Dictionary dump path.
///
/// The input may be a single JSON file, a directory containing JSON shards, or
/// the official ZIP archive.  Output rows are sorted by dictionary key and use
/// the TSV schema consumed by `gukhanmun-mkdict`.
pub fn extract_path_to_tsv(path: &Path, writer: impl Write) -> Result<ExtractStats> {
    let mut extractor = Extractor::default();
    extractor.read_path(path)?;
    extractor.write_tsv(writer)
}

/// Extracts both the canonical dictionary TSV and the multi-syllable suffix
/// override TSV from a Standard Korean Language Dictionary dump path.
///
/// `tsv_writer` receives the canonical `hanja\treading\t…` rows consumed by
/// `gukhanmun-mkdict`; `suffix_writer` receives the `hanja\tinitial\tsuffix`
/// rows that record multi-syllable entries whose leading morpheme keeps its
/// original sound outside word-initial position (see [`crate::ko_kr`]).
pub fn extract_path_to_files(
    path: &Path,
    tsv_writer: impl Write,
    suffix_writer: impl Write,
) -> Result<ExtractStats> {
    let mut extractor = Extractor::default();
    extractor.read_path(path)?;
    extractor.write_suffix_tsv(suffix_writer)?;
    extractor.write_tsv(tsv_writer)
}

/// Extracts a canonical dictionary TSV from one Standard Korean Language
/// Dictionary JSON reader.
///
/// This helper is intended for tests and tools that already opened an
/// individual JSON shard.  Use [`extract_path_to_tsv`] for the official ZIP
/// archive or a directory of shards.
pub fn extract_json_reader_to_tsv(reader: impl Read, writer: impl Write) -> Result<ExtractStats> {
    let mut extractor = Extractor::default();
    extractor.read_json(reader)?;
    extractor.write_tsv(writer)
}

/// Statistics describing one extraction run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExtractStats {
    /// Number of JSON `item` records read from the dump.
    pub items_seen: usize,
    /// Number of canonical TSV dictionary rows written after de-duplication.
    pub entries_written: usize,
    /// Number of otherwise valid keys skipped because an earlier item already
    /// emitted the same key.
    pub duplicate_keys: usize,
    /// Number of JSON items skipped because they could not produce a supported
    /// dictionary entry.
    pub skipped_items: usize,
}

#[derive(Default)]
struct Extractor {
    stats: ExtractStats,
    entries: BTreeMap<String, Entry>,
    /// Word-initial readings seen for multi-syllable hanja keys.
    initial_forms: BTreeMap<String, BTreeSet<String>>,
    /// Suffix and bound-noun readings seen for multi-syllable hanja keys.
    suffix_forms: BTreeMap<String, BTreeSet<String>>,
}

impl Extractor {
    fn read_path(&mut self, path: &Path) -> Result<()> {
        if path.is_dir() {
            tracing::info!(path = %path.display(), input_type = "dir", "extracting Standard Korean Language Dictionary");
            let mut paths = fs::read_dir(path)?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            paths.sort();
            for path in paths {
                if path
                    .extension()
                    .is_some_and(|extension| extension == "json")
                {
                    self.read_json(fs::File::open(path)?)?;
                }
            }
        } else if path.extension().is_some_and(|extension| extension == "zip") {
            tracing::info!(path = %path.display(), input_type = "zip", "extracting Standard Korean Language Dictionary");
            self.read_zip(fs::File::open(path)?)?;
        } else {
            tracing::info!(path = %path.display(), input_type = "json", "extracting Standard Korean Language Dictionary");
            self.read_json(fs::File::open(path)?)?;
        }
        Ok(())
    }

    fn read_zip<R>(&mut self, reader: R) -> Result<()>
    where
        R: Read + Seek,
    {
        let mut archive = ZipArchive::new(reader)?;
        let mut names = archive
            .file_names()
            .filter(|name| name.ends_with(".json"))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        names.sort();
        tracing::debug!(json_file_count = names.len(), "reading ZIP archive");

        for name in names {
            let mut file = archive.by_name(&name)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            self.read_json(Cursor::new(bytes))?;
        }

        Ok(())
    }

    fn read_json(&mut self, reader: impl Read) -> Result<()> {
        let dump = serde_json::from_reader::<_, Dump>(reader)?;
        tracing::debug!(
            items_ingested = dump.channel.item.len(),
            "processed JSON dump"
        );
        for item in dump.channel.item {
            self.stats.items_seen += 1;
            self.ingest_item(item);
        }
        Ok(())
    }

    fn ingest_item(&mut self, item: Item) {
        let Some(word_info) = item.word_info else {
            tracing::debug!(reason = "missing word_info", "skipping dictionary item");
            self.stats.skipped_items += 1;
            return;
        };
        if word_info.word_unit.as_deref() != Some("단어") {
            tracing::debug!(
                word_unit = ?word_info.word_unit,
                reason = "not a single word",
                "skipping dictionary item"
            );
            self.stats.skipped_items += 1;
            return;
        }
        let Some(reading) = word_info
            .word
            .as_deref()
            .map(normalize_word)
            .filter(|reading| !reading.is_empty())
        else {
            tracing::debug!(
                reason = "missing or empty reading",
                "skipping dictionary item"
            );
            self.stats.skipped_items += 1;
            return;
        };
        let originals = word_info.original_language_info.as_deref().unwrap_or(&[]);
        let Some(keys) = keys_from_originals(originals) else {
            tracing::debug!(
                reason = "no hanja keys extracted",
                "skipping dictionary item"
            );
            self.stats.skipped_items += 1;
            return;
        };
        let priority = entry_priority(&word_info, originals);
        let suffix_headword = is_suffix_headword(&word_info);

        for key in keys {
            self.track_multisyllable_form(&key, &reading, suffix_headword);
            match self.entries.entry(key) {
                BTreeEntry::Vacant(entry) => {
                    entry.insert(Entry {
                        reading: reading.clone(),
                        priority,
                    });
                }
                BTreeEntry::Occupied(mut entry) => {
                    self.stats.duplicate_keys += 1;
                    if priority > entry.get().priority {
                        tracing::trace!(key = %entry.key(), "resolved duplicate dictionary key by priority");
                        entry.insert(Entry {
                            reading: reading.clone(),
                            priority,
                        });
                    }
                }
            }
        }
    }

    /// Records a multi-syllable hanja key's reading under its word-initial or
    /// suffix bucket, so [`Extractor::write_suffix_tsv`] can later emit the
    /// keys whose leading morpheme keeps its original sound outside word-initial
    /// position. Single hanja are intentionally skipped: their initial sound law
    /// is recovered by the engine from the bundled unihan readings.
    fn track_multisyllable_form(&mut self, key: &str, reading: &str, suffix_headword: bool) {
        if key.chars().take(2).count() < 2
            || !key.chars().all(is_hanja)
            || reading.chars().count() != key.chars().count()
        {
            return;
        }
        let bucket = if suffix_headword {
            &mut self.suffix_forms
        } else {
            &mut self.initial_forms
        };
        bucket
            .entry(key.to_owned())
            .or_default()
            .insert(reading.to_owned());
    }

    /// Writes the multi-syllable suffix override table.
    ///
    /// A row is emitted for every hanja key that has both a word-initial reading
    /// `I` and a suffix or bound-noun reading `S` that differ only in their first
    /// syllable (the initial sound law alternation, for example `年代` →
    /// `연대`/`년대`). Wholesale alternations from semantically distinct readings
    /// are excluded by the first-syllable-only test.
    fn write_suffix_tsv(&self, mut writer: impl Write) -> Result<()> {
        writeln!(writer, "hanja\tinitial\tsuffix")?;
        for (key, suffixes) in &self.suffix_forms {
            let Some(initials) = self.initial_forms.get(key) else {
                continue;
            };
            if let Some((initial, suffix)) = initials
                .iter()
                .flat_map(|initial| suffixes.iter().map(move |suffix| (initial, suffix)))
                .find(|(initial, suffix)| differs_only_in_first_syllable(initial, suffix))
            {
                writeln!(writer, "{key}\t{initial}\t{suffix}")?;
            }
        }
        Ok(())
    }

    fn write_tsv(mut self, mut writer: impl Write) -> Result<ExtractStats> {
        writeln!(writer, "hanja\thangul\trequire_hanja\trequire_hangul")?;
        for (key, entry) in &self.entries {
            writeln!(writer, "{key}\t{}\tfalse\tfalse", entry.reading)?;
        }
        self.stats.entries_written = self.entries.len();
        tracing::info!(
            entries_written = self.stats.entries_written,
            "wrote dictionary TSV"
        );
        Ok(self.stats)
    }
}

#[derive(Clone, Debug)]
struct Entry {
    reading: String,
    priority: EntryPriority,
}

#[derive(Debug, Deserialize)]
struct Dump {
    channel: Channel,
}

#[derive(Debug, Deserialize)]
struct Channel {
    #[serde(default)]
    item: Vec<Item>,
}

#[derive(Debug, Deserialize)]
struct Item {
    word_info: Option<WordInfo>,
}

#[derive(Debug, Deserialize)]
struct WordInfo {
    word: Option<String>,
    word_unit: Option<String>,
    #[serde(default)]
    original_language_info: Option<Vec<OriginalLanguageInfo>>,
    #[serde(default)]
    pos_info: Vec<PosInfo>,
}

#[derive(Debug, Deserialize)]
struct PosInfo {
    pos: Option<String>,
    #[serde(default)]
    comm_pattern_info: Vec<CommPatternInfo>,
}

#[derive(Debug, Deserialize)]
struct CommPatternInfo {
    #[serde(default)]
    sense_info: Vec<SenseInfo>,
}

#[derive(Debug, Deserialize)]
struct SenseInfo {
    definition: Option<String>,
    definition_original: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OriginalLanguageInfo {
    original_language: Option<String>,
    language_type: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum EntryPriority {
    Redirect,
    Default,
    ForeignHanjaSpelling,
}

fn entry_priority(word_info: &WordInfo, originals: &[OriginalLanguageInfo]) -> EntryPriority {
    if has_only_redirect_senses(word_info) {
        EntryPriority::Redirect
    } else if originals.iter().any(|original| {
        let language_type = original.language_type.as_deref().unwrap_or("");
        !matches!(language_type, "한자" | "고유어")
            && !language_type.contains("병기")
            && original
                .original_language
                .as_deref()
                .is_some_and(|original_language| foreign_hanja_piece(original_language).is_some())
    }) {
        EntryPriority::ForeignHanjaSpelling
    } else {
        EntryPriority::Default
    }
}

fn has_only_redirect_senses(word_info: &WordInfo) -> bool {
    let mut saw_definition = false;

    for sense in word_info
        .pos_info
        .iter()
        .flat_map(|pos| &pos.comm_pattern_info)
        .flat_map(|pattern| &pattern.sense_info)
    {
        let definition = sense
            .definition
            .as_deref()
            .or(sense.definition_original.as_deref())
            .map(str::trim)
            .unwrap_or("");
        if definition.is_empty() {
            continue;
        }
        saw_definition = true;
        if !is_redirect_definition(definition) {
            return false;
        }
    }

    saw_definition
}

fn is_redirect_definition(definition: &str) -> bool {
    definition.trim_start().starts_with('→')
}

fn keys_from_originals(originals: &[OriginalLanguageInfo]) -> Option<Vec<String>> {
    let mut keys = Vec::new();
    let mut current = vec![PartialKey::default()];

    for original in originals {
        let language_type = original.language_type.as_deref().unwrap_or("");
        if language_type.contains("병기") {
            push_keys(&mut keys, &mut current);
            continue;
        }

        let pieces = original_language_pieces(original)?;
        let mut expanded = Vec::with_capacity(current.len() * pieces.alternatives.len());
        for prefix in &current {
            for piece in &pieces.alternatives {
                let mut key = prefix.key.clone();
                key.push_str(piece);
                expanded.push(PartialKey {
                    has_hanja: prefix.has_hanja || piece.chars().any(is_hanja),
                    key,
                });
            }
        }
        current = expanded;
        if pieces.boundary_after {
            push_keys(&mut keys, &mut current);
        }
    }

    push_keys(&mut keys, &mut current);
    if keys.is_empty() { None } else { Some(keys) }
}

#[derive(Clone, Debug, Default)]
struct PartialKey {
    key: String,
    has_hanja: bool,
}

#[derive(Clone, Debug)]
struct OriginalPieces {
    alternatives: Vec<String>,
    boundary_after: bool,
}

fn original_language_pieces(original: &OriginalLanguageInfo) -> Option<OriginalPieces> {
    let language_type = original.language_type.as_deref().unwrap_or("");
    let original_language = original.original_language.as_deref()?;

    match language_type {
        "한자" | "고유어" => native_original_pieces(original_language),
        _ => {
            if language_type.contains("병기") {
                Some(OriginalPieces {
                    alternatives: vec![String::new()],
                    boundary_after: true,
                })
            } else {
                foreign_hanja_piece(original_language).map(|piece| OriginalPieces {
                    alternatives: vec![piece],
                    boundary_after: false,
                })
            }
        }
    }
}

fn native_original_pieces(input: &str) -> Option<OriginalPieces> {
    let normalized = normalize_original_language(input)?;
    let boundary_after = normalized.ends_with('/');
    let normalized = normalized.trim_end_matches('/');
    Some(OriginalPieces {
        alternatives: split_inline_alternatives(normalized)?,
        boundary_after,
    })
}

fn push_keys(keys: &mut Vec<String>, current: &mut Vec<PartialKey>) {
    for key in current.drain(..) {
        if key.has_hanja && !key.key.is_empty() {
            keys.push(key.key);
        }
    }
    current.push(PartialKey::default());
}

fn normalize_original_language(input: &str) -> Option<String> {
    let mut output = String::new();
    let mut rest = input;

    while let Some(start) = rest.find("<equ>") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + "<equ>".len()..];
        let end = after_start.find("</equ>")?;
        output.push_str(decode_entity(after_start[..end].trim())?.encode_utf8(&mut [0; 4]));
        rest = &after_start[end + "</equ>".len()..];
    }

    output.push_str(rest);
    output.retain(|ch| ch != '▽');
    if output.contains('<') || output.contains('&') {
        return None;
    }
    Some(output)
}

fn split_inline_alternatives(input: &str) -> Option<Vec<String>> {
    let alternatives = input
        .split('/')
        .map(str::trim)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if alternatives.iter().any(String::is_empty) {
        None
    } else {
        Some(alternatives)
    }
}

fn foreign_hanja_piece(input: &str) -> Option<String> {
    let normalized = normalize_original_language(input)?;
    if !normalized.is_empty() && normalized.chars().all(is_hanja) {
        return Some(normalized);
    }

    let mut output = String::new();
    let mut rest = normalized.as_str();
    while let Some(open) = rest.find('[') {
        let after_open = &rest[open + '['.len_utf8()..];
        let close = after_open.find(']')?;
        let candidate = &after_open[..close];
        if candidate.is_empty() || !candidate.chars().all(is_hanja) {
            return None;
        }
        output.push_str(candidate);
        rest = &after_open[close + ']'.len_utf8()..];
    }

    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn decode_entity(entity: &str) -> Option<char> {
    let value = entity
        .strip_prefix("&#x")
        .and_then(|value| value.strip_suffix(';'))
        .and_then(|value| u32::from_str_radix(value, 16).ok())
        .or_else(|| {
            entity
                .strip_prefix("&#")
                .and_then(|value| value.strip_suffix(';'))
                .and_then(|value| value.parse().ok())
        })?;
    char::from_u32(value)
}

/// Returns whether a head word denotes a suffix or bound noun, whose hanja keep
/// their original (non-word-initial) sound. Suffixes are written with a leading
/// hyphen (`-년`); bound nouns carry the `의존 명사` part of speech. Prefixes
/// (`부-`, trailing hyphen) are word-initial and excluded.
fn is_suffix_headword(word_info: &WordInfo) -> bool {
    let leading_hyphen = word_info
        .word
        .as_deref()
        .is_some_and(|word| word.trim_start().starts_with('-'));
    let bound_noun = word_info
        .pos_info
        .iter()
        .any(|pos| pos.pos.as_deref() == Some("의존 명사"));
    leading_hyphen || bound_noun
}

/// Returns whether two equal-length readings differ in exactly their first
/// syllable, the shape of an initial sound law alternation (`연대`/`년대`).
fn differs_only_in_first_syllable(a: &str, b: &str) -> bool {
    let mut a = a.chars();
    let mut b = b.chars();
    match (a.next(), b.next()) {
        (Some(first_a), Some(first_b)) if first_a != first_b => a.eq(b),
        _ => false,
    }
}

fn normalize_word(word: &str) -> String {
    let without_number = word.trim_end_matches(|ch: char| ch.is_ascii_digit());
    without_number
        .chars()
        .filter(|ch| !matches!(ch, '-' | '^'))
        .collect()
}

fn is_hanja(ch: char) -> bool {
    matches!(
        ch,
        '\u{2F00}'..='\u{2FFF}'
            | '\u{3007}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
            | '\u{2CEB0}'..='\u{2EBEF}'
            | '\u{2EBF0}'..='\u{2EE5F}'
            | '\u{2F800}'..='\u{2FA1F}'
            | '\u{30000}'..='\u{3134F}'
            | '\u{31350}'..='\u{323AF}'
            | '\u{323B0}'..='\u{3347F}'
    )
}
