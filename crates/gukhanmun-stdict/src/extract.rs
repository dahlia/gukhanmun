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
use std::io::{BufReader, Cursor, Read, Seek, Write};
use std::path::Path;

use gukhanmun_dict_extract::{
    OriginalLanguageInfo, foreign_hanja_piece, is_hanja, keys_from_originals, normalize_word,
};
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
                    self.read_json(BufReader::new(fs::File::open(path)?))?;
                }
            }
        } else if path.extension().is_some_and(|extension| extension == "zip") {
            tracing::info!(path = %path.display(), input_type = "zip", "extracting Standard Korean Language Dictionary");
            self.read_zip(fs::File::open(path)?)?;
        } else {
            tracing::info!(path = %path.display(), input_type = "json", "extracting Standard Korean Language Dictionary");
            self.read_json(BufReader::new(fs::File::open(path)?))?;
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
        let base_priority = entry_priority(&word_info, originals);
        let suffix_headword = is_suffix_headword(&word_info);

        for key in keys {
            // A single hanja borrowed for a foreign reading (the loanword 삐끼
            // from Japanese 引き, keyed on 引) would otherwise shadow that
            // character's Sino-Korean reading in every compound containing it
            // (引數 → 삐끼수 instead of 인수). Single hanja are recovered more
            // accurately from the engine's bundled unihan readings (引 → 인), so
            // drop single-character foreign-spelling keys. Multi-character
            // foreign units such as 北京 → 베이징 are kept.
            if base_priority == EntryPriority::ForeignHanjaSpelling && key.chars().count() == 1 {
                continue;
            }
            // Standard Korean Orthography §30 (한글 맞춤법 第30項) spells six
            // Sino-Korean compounds with a saisiot (數字 → 숫자, not 수자). The
            // dictionary also lists the saisiot-free homograph (수자) under the
            // same hanja key with equal priority, so the winner would otherwise
            // depend on dump order. Promote the prescribed (hanja, reading) pair
            // above Default so it always wins for these six keys. Only a Default
            // entry is promoted: a redirect-only entry must not outrank a
            // substantive one, and a foreign-spelling entry must not be demoted.
            let priority = if base_priority == EntryPriority::Default
                && is_saisiot_prescribed(&key, &reading)
            {
                EntryPriority::SaisiotPrescribed
            } else {
                base_priority
            };
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum EntryPriority {
    Redirect,
    Default,
    SaisiotPrescribed,
    ForeignHanjaSpelling,
}

/// Returns whether `key` and `reading` are one of the six Sino-Korean compounds
/// that Standard Korean Orthography §30 (한글 맞춤법 第30項) spells with a
/// saisiot (사이시옷). Their reading is promoted to
/// [`EntryPriority::SaisiotPrescribed`] by [`Extractor::ingest_item`] so it wins
/// over a saisiot-free homograph sharing the same hanja key. The list is closed
/// and named directly by the orthographic standard, so no general saisiot
/// heuristic is needed.
fn is_saisiot_prescribed(key: &str, reading: &str) -> bool {
    match key {
        "庫間" => reading == "곳간",
        "貰房" => reading == "셋방",
        "數字" => reading == "숫자",
        "車間" => reading == "찻간",
        "退間" => reading == "툇간",
        "回數" => reading == "횟수",
        _ => false,
    }
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
