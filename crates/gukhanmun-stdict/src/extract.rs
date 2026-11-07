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

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufReader, Cursor, Read, Seek, Write};
use std::path::Path;

use gukhanmun_core::{
    DictionaryEvidence, ReadingForm, SenseId, SourceReference, SourceRole, WordId, WordOrigin,
};
use gukhanmun_dict_extract::aliases::{AliasPolicy, AliasSource, derive_aliases};
use gukhanmun_dict_extract::evidence::{
    EvidenceBudget, SourceGraph, SourceRedirect, SourceSense, SourceWord, parse_sense_redirect,
};
use gukhanmun_dict_extract::{
    OriginalLanguageInfo, foreign_hanja_piece, is_hanja, keys_from_originals, normalize_word,
};
use serde::Deserialize;
use zip::ZipArchive;

mod phrases;
pub use phrases::{extract_json_reader_to_phrase_files, extract_path_to_phrase_files};

/// Error returned while extracting Standard Korean Language Dictionary data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Filesystem or stream I/O failed.
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),

    /// A curated default no longer agrees with the source candidate set.
    #[error("invalid curated dictionary default: {0}")]
    DefaultDecision(String),

    /// JSON input could not be decoded.
    #[error("failed to decode dictionary JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// ZIP archive input could not be read.
    #[error("failed to read dictionary ZIP archive: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Alias seed or generated data was not valid UTF-8.
    #[error("alias data is not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
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

/// Extracts the canonical TSV, suffix overrides, aliases, relations, and
/// alias manifest from one input.
///
/// The alias files are derived after every shard is read so source-id
/// ambiguity checks see the complete dictionary.
pub fn extract_path_to_all_files(
    path: &Path,
    tsv_writer: impl Write,
    suffix_writer: impl Write,
    aliases_writer: impl Write,
    relations_writer: impl Write,
    manifest_writer: impl Write,
    seed_aliases: Option<&Path>,
) -> Result<ExtractStats> {
    let mut extractor = Extractor::default();
    extractor.read_path(path)?;
    extractor.write_alias_files(
        aliases_writer,
        relations_writer,
        manifest_writer,
        seed_aliases,
    )?;
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

/// Extracts additive source evidence JSON without changing canonical TSV admission.
///
/// Raw word and sense IDs are indexed before candidate permissions are compiled,
/// including phrases and records with no hanja keys. Sense IDs with unidentified
/// owners remain unresolved rather than disappearing from collision checks.
/// The original TSV candidates
/// and their source IDs determine which evidence records are emitted. Missing
/// contributor IDs leave a candidate uncovered rather than inventing provenance.
pub fn extract_path_to_evidence_json(path: &Path, writer: impl Write) -> Result<()> {
    let mut extractor = Extractor {
        source_words: Some(Vec::new()),
        ..Extractor::default()
    };
    extractor.read_path(path)?;
    extractor.write_evidence_json(writer)
}

/// Extracts evidence from one JSON shard for tests and source-processing tools.
///
/// Use [`extract_path_to_evidence_json`] when links may point to another shard;
/// a missing target conservatively invalidates the entire candidate promotion.
pub fn extract_json_reader_to_evidence_json(reader: impl Read, writer: impl Write) -> Result<()> {
    let mut extractor = Extractor {
        source_words: Some(Vec::new()),
        ..Extractor::default()
    };
    extractor.read_json(reader)?;
    extractor.write_evidence_json(writer)
}

/// Statistics describing one extraction run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExtractStats {
    /// Number of JSON `item` records read from the dump.
    pub items_seen: usize,
    /// Number of canonical TSV dictionary rows written after de-duplication.
    pub entries_written: usize,
    /// Number of repeated key occurrences encountered while collecting readings.
    /// Competing readings are retained, rather than counted as skipped items.
    pub duplicate_keys: usize,
    /// Number of JSON items skipped because they could not produce a supported
    /// dictionary entry.
    pub skipped_items: usize,
    /// Curated default keys absent from this input subset.
    pub skipped_default_pins: usize,
}

#[derive(Default)]
struct Extractor {
    stats: ExtractStats,
    phrase_sources: Option<Vec<phrases::RawWord>>,
    entries: BTreeMap<String, BTreeMap<String, Entry>>,
    /// Word-initial readings seen for multi-syllable hanja keys.
    initial_forms: BTreeMap<String, BTreeSet<String>>,
    /// Suffix and bound-noun readings seen for multi-syllable hanja keys.
    suffix_forms: BTreeMap<String, BTreeSet<String>>,
    /// Raw headword rows used for compound alias derivation.
    alias_sources: Vec<AliasSource>,
    /// Complete raw graph, collected only for the additive evidence export.
    source_words: Option<Vec<SourceWord>>,
    /// Valid sense IDs on raw rows without a usable owning word identifier.
    unresolved_senses: BTreeSet<SenseId>,
    /// Candidates with an unidentifiable contributor cannot claim full coverage.
    missing_source_forms: BTreeSet<(String, String)>,
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
        let dump = serde_json::from_reader::<_, Dump>(BufReader::new(reader))?;
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
        let source_id = item.target_code;
        if let Some(words) = &mut self.phrase_sources {
            words.push(phrases::RawWord::from_item(&item));
        }
        if let Some(words) = &mut self.source_words {
            if let Some(id) = source_id {
                words.push(source_word(WordId(id), item.word_info.as_ref()));
            } else {
                self.unresolved_senses.extend(
                    item.word_info
                        .as_ref()
                        .into_iter()
                        .flat_map(|info| &info.pos_info)
                        .flat_map(|pos| &pos.comm_pattern_info)
                        .flat_map(|pattern| &pattern.sense_info)
                        .filter_map(|sense| sense.sense_code.map(SenseId)),
                );
            }
        }
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
        let categories = word_info
            .pos_info
            .iter()
            .flat_map(|pos| &pos.comm_pattern_info)
            .flat_map(|pattern| &pattern.sense_info)
            .flat_map(|sense| &sense.cat_info)
            .filter_map(|cat| cat.cat.clone())
            .collect::<BTreeSet<_>>();
        self.alias_sources.push(AliasSource {
            word: word_info.word.clone().unwrap_or_default(),
            reading: reading.clone(),
            spellings: keys.clone(),
            source_ids: source_id.into_iter().collect(),
            categories,
        });

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
            if self.source_words.is_some() && source_id.is_none() {
                self.missing_source_forms
                    .insert((key.clone(), reading.clone()));
            }
            self.track_multisyllable_form(&key, &reading, suffix_headword);
            let entries = self.entries.entry(key).or_default();
            if !entries.is_empty() {
                self.stats.duplicate_keys += 1;
            }
            let entry = entries.entry(reading.clone()).or_insert_with(|| Entry {
                reading: reading.clone(),
                initial: None,
                suffix: None,
            });
            let position = if suffix_headword {
                &mut entry.suffix
            } else {
                &mut entry.initial
            };
            let records = position.get_or_insert_with(|| SourceRecords {
                priority,
                ids: BTreeSet::new(),
            });
            records.priority = records.priority.max(priority);
            if let Some(source_id) = source_id {
                records.ids.insert(source_id);
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

    fn merge_seed_aliases(
        aliases: &mut Vec<u8>,
        manifest: &mut Vec<u8>,
        seed_path: &Path,
    ) -> Result<()> {
        use std::collections::BTreeMap;

        let seed = fs::read_to_string(seed_path)?;
        let mut seed_rows = BTreeMap::<(String, String), Vec<String>>::new();
        for (index, line) in seed.lines().enumerate().skip(1) {
            if line.is_empty() {
                continue;
            }
            let fields = line.split('\t').map(str::to_owned).collect::<Vec<_>>();
            if fields.len() != 9 || fields[0].is_empty() || fields[1].is_empty() {
                return Err(Error::DefaultDecision(format!(
                    "{}:{}: expected 9 fields with nonempty hanja and hangul",
                    seed_path.display(),
                    index + 1,
                )));
            }
            seed_rows.insert((fields[0].clone(), fields[1].clone()), fields);
        }
        let text = String::from_utf8(std::mem::take(aliases))?;
        let mut lines = text.lines();
        let header = lines.next().unwrap_or_default().to_owned();
        let mut rows = BTreeMap::<(String, String), String>::new();
        for line in lines {
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() >= 2 {
                rows.insert(
                    (fields[0].to_owned(), fields[1].to_owned()),
                    line.to_owned(),
                );
            }
        }
        for (key, fields) in &seed_rows {
            rows.insert(key.clone(), fields.join("\t"));
        }
        let mut output = String::new();
        output.push_str(&header);
        output.push('\n');
        for line in rows.values() {
            output.push_str(line);
            output.push('\n');
        }
        *aliases = output.into_bytes();

        let manifest_text = String::from_utf8(std::mem::take(manifest))?;
        let mut records = BTreeMap::<(String, String), serde_json::Value>::new();
        for line in manifest_text.lines() {
            if let Ok(record) = serde_json::from_str::<serde_json::Value>(line)
                && let (Some(alias), Some(reading)) = (
                    record.get("alias").and_then(|value| value.as_str()),
                    record.get("reading").and_then(|value| value.as_str()),
                )
            {
                records.insert((alias.to_owned(), reading.to_owned()), record);
            }
        }
        for ((alias, reading), fields) in seed_rows {
            let mut record = records
                .remove(&(alias.clone(), reading.clone()))
                .unwrap_or_else(|| {
                    serde_json::json!({
                        "schema": 1,
                        "alias": alias,
                        "reading": reading,
                        "compound_ids": [],
                        "donor_ids": [],
                    })
                });
            record["kind"] = serde_json::Value::String(fields[7].clone());
            record["target"] = serde_json::Value::String(fields[6].clone());
            let via = fields[8].split_once('>').unwrap_or((&fields[8], ""));
            record["via"] = serde_json::json!({"from": via.0, "to": via.1});
            records.insert((alias, reading), record);
        }
        let mut output = String::new();
        for record in records.values() {
            output.push_str(&serde_json::to_string(record)?);
            output.push('\n');
        }
        *manifest = output.into_bytes();
        Ok(())
    }

    /// Writes derived compound aliases, sibling relations, and their manifest.
    fn write_alias_files(
        &self,
        mut aliases_writer: impl Write,
        mut relations_writer: impl Write,
        mut manifest_writer: impl Write,
        seed_aliases: Option<&Path>,
    ) -> Result<()> {
        let (aliases, relations) = derive_aliases(&self.alias_sources, &AliasPolicy::default());
        let mut aliases_buffer = Vec::new();
        let mut manifest_buffer = Vec::new();
        writeln!(
            aliases_buffer,
            "hanja\thangul\trequire_hanja\trequire_hangul\tpreference\tsuffix_reading\tcanonical\talias_kind\tvia"
        )?;
        for alias in &aliases {
            if !self.entries.contains_key(&alias.canonical)
                || !self
                    .entries
                    .get(&alias.canonical)
                    .is_some_and(|entries| entries.contains_key(&alias.reading))
            {
                continue;
            }
            writeln!(
                aliases_buffer,
                "{}\t{}\tfalse\tfalse\t10\t\t{}\trecognition\t{}>{}",
                alias.hanja, alias.reading, alias.canonical, alias.via.0, alias.via.1,
            )?;
        }
        writeln!(relations_writer, "hanja\thangul\tcanonical\talias_kind")?;
        for relation in &relations {
            let source_exists = self
                .entries
                .get(&relation.hanja)
                .is_some_and(|entries| entries.contains_key(&relation.reading));
            let canonical_exists = self
                .entries
                .get(&relation.canonical)
                .is_some_and(|entries| entries.contains_key(&relation.reading));
            if !source_exists || !canonical_exists {
                continue;
            }
            writeln!(
                relations_writer,
                "{}\t{}\t{}\tsibling",
                relation.hanja, relation.reading, relation.canonical,
            )?;
        }
        for alias in &aliases {
            if !self.entries.contains_key(&alias.canonical)
                || !self
                    .entries
                    .get(&alias.canonical)
                    .is_some_and(|entries| entries.contains_key(&alias.reading))
            {
                continue;
            }
            let record = serde_json::json!({
                "schema": 1,
                "alias": alias.hanja,
                "target": alias.canonical,
                "reading": alias.reading,
                "kind": "recognition",
                "via": {"from": alias.via.0, "to": alias.via.1},
                "compound_ids": alias.compound_source_ids,
                "donor_ids": alias.donor_source_ids,
            });
            serde_json::to_writer(&mut manifest_buffer, &record)?;
            writeln!(manifest_buffer)?;
        }
        if let Some(seed) = seed_aliases {
            Self::merge_seed_aliases(&mut aliases_buffer, &mut manifest_buffer, seed)?;
        }
        aliases_writer.write_all(&aliases_buffer)?;
        manifest_writer.write_all(&manifest_buffer)?;
        tracing::info!(
            aliases = aliases.len(),
            relations = relations.len(),
            "wrote alias data"
        );
        Ok(())
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

    fn write_evidence_json(mut self, mut writer: impl Write) -> Result<()> {
        let graph = SourceGraph::new(self.source_words.take().unwrap_or_default())
            .with_unresolved_senses(std::mem::take(&mut self.unresolved_senses));
        let mut entries = BTreeMap::new();
        for (key, source_entries) in &self.entries {
            let mut evidence = Vec::new();
            for candidate in extraction_candidates(key, source_entries) {
                if candidate.source_ids.is_empty()
                    || self
                        .missing_source_forms
                        .contains(&(key.clone(), candidate.reading.clone()))
                    || candidate.suffix_reading.as_ref().is_some_and(|suffix| {
                        self.missing_source_forms
                            .contains(&(key.clone(), suffix.clone()))
                    })
                {
                    continue;
                }
                evidence.push(graph.compile_candidate(
                    ReadingForm {
                        reading: candidate.reading,
                        suffix_reading: candidate.suffix_reading,
                    },
                    candidate.source_ids.into_iter().map(WordId),
                    EvidenceBudget::default(),
                ));
            }
            if !evidence.is_empty() {
                entries.insert(key.clone(), evidence);
            }
        }
        let evidence = DictionaryEvidence {
            schema_version: 1,
            role: SourceRole::Reference,
            entries,
        };
        evidence
            .validate()
            .map_err(|error| Error::DefaultDecision(error.to_string()))?;
        serde_json::to_writer(&mut writer, &evidence)?;
        writeln!(writer)?;
        Ok(())
    }

    fn write_tsv(mut self, mut writer: impl Write) -> Result<ExtractStats> {
        let pins = default_decisions()?;
        self.stats.skipped_default_pins = pins
            .keys()
            .filter(|key| !self.entries.contains_key(*key))
            .count();
        writeln!(
            writer,
            "hanja\thangul\trequire_hanja\trequire_hangul\tpreference\tsuffix_reading\tsource_ids"
        )?;
        self.stats.entries_written = 0;
        for (key, entries) in &self.entries {
            let mut candidates = extraction_candidates(key, entries);
            let signature = candidates
                .iter()
                .map(|c| (c.reading.clone(), c.suffix_reading.clone()))
                .collect::<Vec<_>>();
            if let Some(pin) = pins.get(key) {
                if pin.candidates != signature {
                    return Err(Error::DefaultDecision(format!(
                        "candidate set changed for `{key}`"
                    )));
                }
                let highest = candidates.iter().map(|c| c.preference).max().unwrap();
                let Some(chosen) = candidates
                    .iter_mut()
                    .find(|c| c.reading == pin.reading && c.suffix_reading == pin.suffix_reading)
                else {
                    return Err(Error::DefaultDecision(format!(
                        "chosen candidate missing for `{key}`"
                    )));
                };
                if chosen.preference != highest {
                    return Err(Error::DefaultDecision(format!(
                        "chosen candidate for `{key}` is outside the highest priority band"
                    )));
                }
                chosen.preference += 1;
            }
            let explicit = candidates.len() > 1;
            for candidate in candidates {
                let preference = if explicit {
                    candidate.preference.to_string()
                } else {
                    String::new()
                };
                let ids = candidate
                    .source_ids
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                writeln!(
                    writer,
                    "{key}\t{}\tfalse\tfalse\t{preference}\t{}\t{ids}",
                    candidate.reading,
                    candidate.suffix_reading.as_deref().unwrap_or("")
                )?;
                self.stats.entries_written += 1;
            }
        }
        tracing::info!(
            entries_written = self.stats.entries_written,
            skipped_default_pins = self.stats.skipped_default_pins,
            "wrote dictionary TSV"
        );
        Ok(self.stats)
    }
}

#[derive(Clone, Debug)]
struct Entry {
    reading: String,
    initial: Option<SourceRecords>,
    suffix: Option<SourceRecords>,
}

#[derive(Clone, Debug)]
struct SourceRecords {
    priority: EntryPriority,
    ids: BTreeSet<u64>,
}

struct ExtractCandidate {
    reading: String,
    suffix_reading: Option<String>,
    preference: u32,
    source_ids: BTreeSet<u64>,
}

impl ExtractCandidate {
    fn from_sources(
        reading: &str,
        suffix_reading: Option<&str>,
        sources: &[&SourceRecords],
    ) -> Self {
        Self {
            reading: reading.to_owned(),
            suffix_reading: suffix_reading.map(str::to_owned),
            preference: sources
                .iter()
                .map(|source| source.priority)
                .max()
                .expect("candidate has a source") as u32
                * 10,
            source_ids: sources
                .iter()
                .flat_map(|source| source.ids.iter().copied())
                .collect(),
        }
    }
}

fn extraction_candidates(key: &str, entries: &BTreeMap<String, Entry>) -> Vec<ExtractCandidate> {
    let key_chars = key.chars().count();
    let eligible = key_chars >= 2 && key.chars().all(is_hanja);
    let mut consumed = BTreeSet::new();
    let mut candidates = Vec::new();
    for initial in entries.values().filter(|entry| entry.initial.is_some()) {
        let initial_sources = initial.initial.as_ref().expect("word-initial record");
        let pairs = entries
            .values()
            .filter(|suffix| {
                eligible
                    && suffix.suffix.is_some()
                    && initial.reading.chars().count() == key_chars
                    && suffix.reading.chars().count() == key_chars
                    && gukhanmun_core::is_initial_sound_law_pair(&initial.reading, &suffix.reading)
            })
            .collect::<Vec<_>>();
        // A separate suffix record with the same reading establishes a
        // position-independent choice even when another suffix also pairs.
        if pairs.is_empty() || initial.suffix.is_some() {
            let sources = core::iter::once(initial_sources)
                .chain(initial.suffix.as_ref())
                .collect::<Vec<_>>();
            candidates.push(ExtractCandidate::from_sources(
                &initial.reading,
                None,
                &sources,
            ));
        }
        for suffix in pairs {
            consumed.insert(suffix.reading.clone());
            candidates.push(ExtractCandidate::from_sources(
                &initial.reading,
                Some(&suffix.reading),
                &[
                    initial_sources,
                    suffix.suffix.as_ref().expect("suffix record"),
                ],
            ));
        }
    }
    for entry in entries
        .values()
        .filter(|entry| entry.initial.is_none() && !consumed.contains(&entry.reading))
    {
        candidates.push(ExtractCandidate::from_sources(
            &entry.reading,
            None,
            &[entry.suffix.as_ref().expect("suffix-only record")],
        ));
    }
    candidates
        .sort_by(|a, b| (&a.reading, &a.suffix_reading).cmp(&(&b.reading, &b.suffix_reading)));
    candidates
}

struct DefaultDecision {
    reading: String,
    suffix_reading: Option<String>,
    candidates: Vec<(String, Option<String>)>,
}

fn default_decisions() -> Result<BTreeMap<String, DefaultDecision>> {
    let mut pins = BTreeMap::new();
    for line in include_str!("../data/defaults.tsv").lines().skip(1) {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 5
            || fields[0].is_empty()
            || fields[1].is_empty()
            || fields[4].trim().is_empty()
        {
            return Err(Error::DefaultDecision(
                "expected key, reading, suffix, candidate signature and reason".into(),
            ));
        }
        let pin = DefaultDecision {
            reading: fields[1].into(),
            suffix_reading: (!fields[2].is_empty()).then(|| fields[2].into()),
            candidates: serde_json::from_str(fields[3])?,
        };
        if pins.insert(fields[0].into(), pin).is_some() {
            return Err(Error::DefaultDecision(format!(
                "duplicate key `{}`",
                fields[0]
            )));
        }
    }
    Ok(pins)
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
    #[serde(default)]
    target_code: Option<u64>,
    word_info: Option<WordInfo>,
}

#[derive(Debug, Deserialize)]
struct WordInfo {
    #[serde(default)]
    relation_info: Vec<LexicalInfo>,
    word: Option<String>,
    word_type: Option<String>,
    #[serde(default)]
    lexical_info: Vec<LexicalInfo>,
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
    sense_code: Option<u64>,
    #[serde(default)]
    lexical_info: Vec<LexicalInfo>,
    definition: Option<String>,
    definition_original: Option<String>,
    #[serde(default)]
    cat_info: Vec<CatInfo>,
}

#[derive(Debug, Deserialize)]
struct CatInfo {
    cat: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LexicalInfo {
    unit: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    link_target_code: Option<serde_json::Value>,
}

fn source_synonyms(relations: &[LexicalInfo]) -> Vec<Option<SourceReference>> {
    relations
        .iter()
        .filter(|relation| relation.kind.as_deref() == Some("동의어"))
        .map(|relation| {
            let value = relation.link_target_code.as_ref()?;
            let id = value.as_u64().or_else(|| {
                let text = value.as_str()?;
                if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
                    return None;
                }
                text.parse().ok()
            })?;
            match relation.unit.as_deref() {
                Some("어휘") => Some(SourceReference::Word(WordId(id))),
                Some("의미") => Some(SourceReference::Sense(SenseId(id))),
                _ => None,
            }
        })
        .collect()
}

fn source_word(id: WordId, info: Option<&WordInfo>) -> SourceWord {
    let Some(info) = info else {
        return SourceWord {
            id,
            reading: String::new(),
            origin: WordOrigin::Unknown,
            senses: Vec::new(),
            synonyms: Vec::new(),
        };
    };
    let origin = match info.word_type.as_deref() {
        Some("한자어" | "고유어") => WordOrigin::Native,
        Some("외래어") => WordOrigin::Foreign,
        Some("혼종어") => {
            let mut native = false;
            let mut foreign = false;
            let mut unknown = false;
            for original in info.original_language_info.as_deref().unwrap_or(&[]) {
                match original.language_type.as_deref() {
                    Some("한자" | "고유어") => native = true,
                    Some(language) if language.contains("병기") => {}
                    Some("") | None => unknown = true,
                    Some(_) => foreign = true,
                }
            }
            match (native, foreign, unknown) {
                (true, false, false) => WordOrigin::Native,
                (true, true, false) => WordOrigin::Mixed,
                _ => WordOrigin::Unknown,
            }
        }
        _ => WordOrigin::Unknown,
    };
    SourceWord {
        id,
        // normalize_word removes only editorial hyphens/carets and trailing
        // homograph digits. Literal phrase boundaries remain intact.
        reading: info.word.as_deref().map(normalize_word).unwrap_or_default(),
        origin,
        synonyms: source_synonyms(&info.lexical_info),
        senses: info
            .pos_info
            .iter()
            .flat_map(|pos| &pos.comm_pattern_info)
            .flat_map(|pattern| &pattern.sense_info)
            .map(|sense| {
                let mut redirect = parse_sense_redirect(sense.definition_original.as_deref());
                if redirect == SourceRedirect::None
                    && sense
                        .definition
                        .as_deref()
                        .is_some_and(is_redirect_definition)
                {
                    // A visible arrow without a machine target must not become a
                    // substantive terminal merely because the markup is absent.
                    redirect = SourceRedirect::Invalid;
                }
                SourceSense {
                    id: sense.sense_code.map(SenseId),
                    foreign_name: sense
                        .cat_info
                        .iter()
                        .any(|category| matches!(category.cat.as_deref(), Some("인명" | "지명"))),
                    redirect,
                    synonyms: source_synonyms(&sense.lexical_info),
                }
            })
            .collect(),
    }
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
    gukhanmun_core::is_initial_sound_law_pair(a, b)
}
