// Gukhanmun: Bundled Open Korean Dictionary data for Gukhanmun.
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

//! Extractor for Open Korean Dictionary (우리말샘) JSON dumps.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufReader, Read, Seek, Write};
use std::path::Path;

use gukhanmun_core::{MatchMark, ReadingCandidate, ReadingCandidates, select_default_reading};
use gukhanmun_dict_extract::{
    OriginalLanguageInfo, foreign_hanja_piece, is_hanja, keys_from_originals, normalize_word,
};
use serde::Deserialize;
use zip::ZipArchive;

/// Error returned while extracting Open Korean Dictionary data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Filesystem or stream I/O failed.
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),

    /// A curated default disagrees with the extracted candidates.
    #[error("invalid curated dictionary default: {0}")]
    DefaultDecision(String),

    /// JSON input could not be decoded.
    #[error("failed to decode dictionary JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// ZIP archive input could not be read.
    #[error("failed to read dictionary ZIP archive: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Result type returned by Open Korean Dictionary extraction APIs.
pub type Result<T> = std::result::Result<T, Error>;

/// Lexical category used to partition Open Korean Dictionary output.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Category {
    /// `일반어`, general vocabulary.
    General,

    /// `북한어`, North Korean vocabulary and orthography.
    NorthKorean,

    /// `방언`, dialect vocabulary.
    Dialect,

    /// `옛말`, archaic vocabulary.
    Archaic,
}

impl Category {
    /// Returns the trimmed source label used by `senseinfo.type`.
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "일반어",
            Self::NorthKorean => "북한어",
            Self::Dialect => "방언",
            Self::Archaic => "옛말",
        }
    }

    /// Returns the stable category identifier used by the default decision table.
    pub fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::NorthKorean => "north-korean",
            Self::Dialect => "dialect",
            Self::Archaic => "archaic",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label.trim() {
            "일반어" => Some(Self::General),
            "북한어" => Some(Self::NorthKorean),
            "방언" => Some(Self::Dialect),
            "옛말" => Some(Self::Archaic),
            _ => None,
        }
    }
}

/// Statistics describing one supported Open Korean Dictionary category in one
/// extraction run.
///
/// Items whose `senseinfo.type` does not map to a supported [`Category`] are
/// ignored before per-category statistics are updated.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExtractStats {
    /// Number of JSON `item` records read for this category.
    pub items_seen: usize,
    /// Number of canonical TSV dictionary rows written after de-duplication.
    pub entries_written: usize,
    /// Number of repeated key occurrences within the same category.
    pub duplicate_keys: usize,
    /// Number of JSON items skipped because they could not produce a supported
    /// dictionary entry.
    pub skipped_items: usize,
    /// Number of curated defaults absent or incomplete in this input.
    pub skipped_default_pins: usize,
}

/// Statistics for every supported Open Korean Dictionary category.
///
/// The extractor intentionally does not expose a global item counter; items
/// from unsupported `senseinfo.type` labels are outside these category totals.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CategoryStats {
    /// General vocabulary statistics.
    pub general: ExtractStats,
    /// North Korean vocabulary statistics.
    pub north_korean: ExtractStats,
    /// Dialect vocabulary statistics.
    pub dialect: ExtractStats,
    /// Archaic vocabulary statistics.
    pub archaic: ExtractStats,
}

impl CategoryStats {
    fn get_mut(&mut self, category: Category) -> &mut ExtractStats {
        match category {
            Category::General => &mut self.general,
            Category::NorthKorean => &mut self.north_korean,
            Category::Dialect => &mut self.dialect,
            Category::Archaic => &mut self.archaic,
        }
    }
}

/// Writable outputs for every Open Korean Dictionary category.
pub struct CategoryWriters<G, N, D, A> {
    /// Output writer for `일반어`.
    pub general: G,
    /// Output writer for `북한어`.
    pub north_korean: N,
    /// Output writer for `방언`.
    pub dialect: D,
    /// Output writer for `옛말`.
    pub archaic: A,
}

/// Extracts canonical dictionary TSVs from an Open Korean Dictionary dump path.
///
/// The input may be a single JSON file, a directory containing JSON shards, or
/// the official ZIP archive.  Directory entries and ZIP members are read in
/// lexicographic order to match the current stdict extractor policy.
/// Directories and ZIP archives validate complete curated candidate signatures.
/// A single JSON file may omit known candidates from a pin, as with
/// [`extract_json_reader_to_files`].
pub fn extract_path_to_files<G, N, D, A>(
    path: &Path,
    writers: CategoryWriters<G, N, D, A>,
) -> Result<CategoryStats>
where
    G: Write,
    N: Write,
    D: Write,
    A: Write,
{
    let mut extractor = Extractor {
        allow_partial_pins: path.is_file() && !has_extension(path, "zip"),
        ..Extractor::default()
    };
    extractor.read_path(path)?;
    extractor.write_files(writers)
}

/// Extracts canonical category TSVs from one Open Korean Dictionary JSON reader.
///
/// This helper is intended for tests and tools that already opened an
/// individual JSON shard. Curated defaults apply when the complete candidate
/// signature is present. Fewer candidates with known readings, possibly missing
/// a positional suffix, use the lexical default and increment
/// [`ExtractStats::skipped_default_pins`]. Unknown candidates still fail
/// validation. Use a directory or ZIP archive through
/// [`extract_path_to_files`] to require complete signatures for present keys.
pub fn extract_json_reader_to_files<G, N, D, A>(
    reader: impl Read,
    writers: CategoryWriters<G, N, D, A>,
) -> Result<CategoryStats>
where
    G: Write,
    N: Write,
    D: Write,
    A: Write,
{
    let mut extractor = Extractor {
        allow_partial_pins: true,
        ..Extractor::default()
    };
    extractor.read_json(reader)?;
    extractor.write_files(writers)
}

#[derive(Default)]
struct Extractor {
    allow_partial_pins: bool,
    stats: CategoryStats,
    entries: BTreeMap<Category, BTreeMap<String, BTreeMap<String, Entry>>>,
}

impl Extractor {
    fn read_path(&mut self, path: &Path) -> Result<()> {
        if path.is_dir() {
            tracing::info!(path = %path.display(), input_type = "dir", "extracting Open Korean Dictionary");
            let mut paths = fs::read_dir(path)?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            paths.sort();
            for file_path in paths {
                if file_path.is_file() && has_extension(&file_path, "json") {
                    self.read_json(fs::File::open(file_path)?)?;
                }
            }
        } else if has_extension(path, "zip") {
            tracing::info!(path = %path.display(), input_type = "zip", "extracting Open Korean Dictionary");
            self.read_zip(fs::File::open(path)?)?;
        } else {
            tracing::info!(path = %path.display(), input_type = "json", "extracting Open Korean Dictionary");
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
            .filter(|name| zip_member_has_extension(name, "json"))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        names.sort();

        for name in names {
            let file = archive.by_name(&name)?;
            if file.is_file() {
                self.read_json(file)?;
            }
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
            self.ingest_item(item);
        }
        Ok(())
    }

    fn ingest_item(&mut self, item: Item) {
        let category = item
            .senseinfo
            .as_ref()
            .and_then(|senseinfo| senseinfo.lexical_type.as_deref())
            .and_then(Category::from_label);
        let Some(category) = category else {
            return;
        };
        self.stats.get_mut(category).items_seen += 1;

        let Some(wordinfo) = item.wordinfo else {
            self.increment_skipped(category);
            return;
        };
        if wordinfo.word_unit.as_deref().map(str::trim) != Some("어휘") {
            self.increment_skipped(category);
            return;
        }
        let Some(reading) = wordinfo
            .word
            .as_deref()
            .map(str::trim)
            .map(normalize_word)
            .filter(|reading| !reading.is_empty())
        else {
            self.increment_skipped(category);
            return;
        };
        let originals = wordinfo.original_language_info.as_deref().unwrap_or(&[]);
        let Some(mut keys) = keys_from_originals(originals) else {
            self.increment_skipped(category);
            return;
        };
        if has_foreign_hanja_spelling(originals) {
            // Single hanja borrowed for foreign readings, such as Japanese
            // 引き -> 引, are weaker than the engine's bundled Sino-Korean
            // readings for the same character. Keep multi-character foreign
            // place names and similar entries, such as 北京 -> 베이징.
            keys.retain(|key| key.chars().nth(1).is_some());
            if keys.is_empty() {
                self.increment_skipped(category);
                return;
            }
        }

        let suffix_headword = wordinfo
            .word
            .as_deref()
            .is_some_and(|word| word.trim_start().starts_with('-'))
            || item
                .senseinfo
                .as_ref()
                .and_then(|sense| sense.pos.as_deref())
                .is_some_and(|pos| pos.trim() == "의존 명사");
        for key in keys {
            let entries = self
                .entries
                .entry(category)
                .or_default()
                .entry(key)
                .or_default();
            if !entries.is_empty() {
                self.stats.get_mut(category).duplicate_keys += 1;
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
                ids: BTreeSet::new(),
            });
            if let Some(id) = item.target_code {
                records.ids.insert(id);
            }
        }
    }

    fn increment_skipped(&mut self, category: Category) {
        self.stats.get_mut(category).skipped_items += 1;
    }

    fn write_files<G, N, D, A>(
        mut self,
        writers: CategoryWriters<G, N, D, A>,
    ) -> Result<CategoryStats>
    where
        G: Write,
        N: Write,
        D: Write,
        A: Write,
    {
        let pins = default_decisions(include_str!("../data/defaults.tsv"))?;
        let mut outputs = BTreeMap::new();
        for category in [
            Category::General,
            Category::NorthKorean,
            Category::Dialect,
            Category::Archaic,
        ] {
            let mut bytes = Vec::new();
            self.write_category(category, &mut bytes, &pins)?;
            outputs.insert(category, bytes);
        }
        let CategoryWriters {
            mut general,
            mut north_korean,
            mut dialect,
            mut archaic,
        } = writers;
        for (category, writer) in [
            (Category::General, &mut general as &mut dyn Write),
            (Category::NorthKorean, &mut north_korean as &mut dyn Write),
            (Category::Dialect, &mut dialect as &mut dyn Write),
            (Category::Archaic, &mut archaic as &mut dyn Write),
        ] {
            writer.write_all(&outputs[&category])?;
            writer.flush()?;
        }
        Ok(self.stats)
    }

    fn write_category(
        &mut self,
        category: Category,
        mut writer: impl Write,
        pins: &BTreeMap<(Category, String), DefaultDecision>,
    ) -> Result<()> {
        writeln!(
            writer,
            "hanja\thangul\trequire_hanja\trequire_hangul\tpreference\tsuffix_reading\tsource_ids"
        )?;
        let entries = self.entries.remove(&category).unwrap_or_default();
        self.stats.get_mut(category).skipped_default_pins = pins
            .keys()
            .filter(|(c, key)| *c == category && !entries.contains_key(key))
            .count();
        for (key, records) in entries {
            let mut candidates = extraction_candidates(&key, &records);
            if candidates.len() > 1 {
                let allowed = ReadingCandidates::new(
                    candidates
                        .iter()
                        .map(|c| ReadingCandidate {
                            reading: c.reading.clone(),
                            suffix_reading: c.suffix_reading.clone(),
                            mark: MatchMark::default(),
                            preference: 0,
                        })
                        .collect(),
                )
                .map_err(|e| {
                    Error::DefaultDecision(format!("invalid candidates for `{key}`: {e}"))
                })?;
                let selected = select_default_reading(&allowed);
                for candidate in &mut candidates {
                    candidate.preference = u32::from(
                        candidate.reading == selected.reading
                            && candidate.suffix_reading == selected.suffix_reading,
                    );
                }
            }
            if let Some(pin) = pins.get(&(category, key.clone())) {
                let incomplete = self.allow_partial_pins
                    && candidates.len() < pin.candidates.len()
                    && candidates.iter().all(|candidate| {
                        pin.candidates.iter().any(|(reading, suffix)| {
                            candidate.reading == *reading
                                && (candidate.suffix_reading == *suffix
                                    || candidate.suffix_reading.is_none())
                        })
                    });
                if incomplete {
                    self.stats.get_mut(category).skipped_default_pins += 1;
                } else {
                    apply_default(&key, &mut candidates, pin)?;
                }
            } else if candidates.len() > 1 {
                tracing::debug!(
                    category = category.id(),
                    key,
                    "unpinned competing readings use lexical default"
                );
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
                self.stats.get_mut(category).entries_written += 1;
            }
        }
        tracing::info!(category = category.id(), stats = ?self.stats.get_mut(category), "wrote dictionary TSV");
        writer.flush()?;
        Ok(())
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
            preference: 0,
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
    // A source-observed suffix-only spelling remains a lexical alternative
    // even when it also contributes to a positional pair. This preserves
    // existing scalar defaults and every source reading without word-specific
    // compatibility rules.
    for entry in entries.values().filter(|entry| entry.initial.is_none()) {
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

fn apply_default(
    key: &str,
    candidates: &mut [ExtractCandidate],
    pin: &DefaultDecision,
) -> Result<()> {
    let signature = candidates
        .iter()
        .map(|c| (c.reading.clone(), c.suffix_reading.clone()))
        .collect::<Vec<_>>();
    if signature != pin.candidates {
        return Err(Error::DefaultDecision(format!(
            "candidate set changed for `{key}`"
        )));
    }
    for candidate in candidates.iter_mut() {
        candidate.preference = 0;
    }
    let chosen = candidates
        .iter_mut()
        .find(|c| c.reading == pin.reading && c.suffix_reading == pin.suffix_reading)
        .ok_or_else(|| Error::DefaultDecision(format!("chosen candidate missing for `{key}`")))?;
    chosen.preference = 1;
    Ok(())
}

fn default_decisions(text: &str) -> Result<BTreeMap<(Category, String), DefaultDecision>> {
    let mut pins = BTreeMap::new();
    for line in text.lines().skip(1) {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 6
            || fields[1].is_empty()
            || fields[2].is_empty()
            || fields[5].trim().is_empty()
        {
            return Err(Error::DefaultDecision(
                "expected category, key, reading, suffix, candidate signature and reason".into(),
            ));
        }
        let category = [
            Category::General,
            Category::NorthKorean,
            Category::Dialect,
            Category::Archaic,
        ]
        .into_iter()
        .find(|c| c.id() == fields[0])
        .ok_or_else(|| Error::DefaultDecision(format!("unknown category `{}`", fields[0])))?;
        let pin = DefaultDecision {
            reading: fields[2].into(),
            suffix_reading: (!fields[3].is_empty()).then(|| fields[3].into()),
            candidates: serde_json::from_str(fields[4])?,
        };
        if pin.candidates.is_empty()
            || !pin.candidates.windows(2).all(|w| w[0] < w[1])
            || !pin
                .candidates
                .contains(&(pin.reading.clone(), pin.suffix_reading.clone()))
        {
            return Err(Error::DefaultDecision(format!(
                "invalid candidate signature or selection for `{}`",
                fields[1]
            )));
        }
        if pins.insert((category, fields[1].into()), pin).is_some() {
            return Err(Error::DefaultDecision(format!(
                "duplicate category/key `{}/{}`",
                fields[0], fields[1]
            )));
        }
    }
    Ok(pins)
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn zip_member_has_extension(name: &str, expected: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn has_foreign_hanja_spelling(originals: &[OriginalLanguageInfo]) -> bool {
    originals.iter().any(|original| {
        let language_type = original.language_type.as_deref().unwrap_or("");
        !matches!(language_type, "한자" | "고유어")
            && !language_type.contains("병기")
            && original
                .original_language
                .as_deref()
                .is_some_and(|original_language| foreign_hanja_piece(original_language).is_some())
    })
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
    wordinfo: Option<WordInfo>,
    senseinfo: Option<SenseInfo>,
}

#[derive(Debug, Deserialize)]
struct WordInfo {
    word: Option<String>,
    word_unit: Option<String>,
    #[serde(default)]
    original_language_info: Option<Vec<OriginalLanguageInfo>>,
}

#[derive(Debug, Deserialize)]
struct SenseInfo {
    pos: Option<String>,
    #[serde(rename = "type")]
    lexical_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(id: u64, key: &str, word: &str, category: &str, pos: &str) -> Item {
        serde_json::from_value(json!({
            "target_code": id,
            "wordinfo": {"word": word, "word_unit": "어휘", "original_language_info": [
                {"original_language": key, "language_type": "한자"}
            ]},
            "senseinfo": {"type": category, "pos": pos}
        }))
        .unwrap()
    }

    fn rows(items: Vec<Item>) -> (Vec<u8>, Vec<u8>) {
        let mut extractor = Extractor::default();
        for item in items {
            extractor.ingest_item(item);
        }
        let pins = default_decisions(include_str!("../data/defaults.tsv")).unwrap();
        for (category, keys) in &extractor.entries {
            assert!(
                keys.keys()
                    .all(|key| !pins.contains_key(&(*category, key.clone()))),
                "synthetic test must not depend on production defaults"
            );
        }
        let mut general = Vec::new();
        let mut north = Vec::new();
        extractor
            .write_files(CategoryWriters {
                general: &mut general,
                north_korean: &mut north,
                dialect: Vec::new(),
                archaic: Vec::new(),
            })
            .unwrap();
        (general, north)
    }

    #[test]
    fn unions_numeric_sense_ids_and_preserves_category_identity_independent_of_order() {
        let items = || {
            vec![
                item(30, "甲乙", "가을", "일반어", "명사"),
                item(2, "甲乙", "가을", "일반어", "명사"),
                item(2, "甲乙", "가을", "일반어", "명사"),
                item(4, "甲乙", "나을", "일반어", "명사"),
                item(5, "甲乙", "라을", "북한어", "명사"),
            ]
        };
        let forward = rows(items());
        let mut reverse = items();
        reverse.reverse();
        assert_eq!(forward, rows(reverse));
        let general = String::from_utf8(forward.0).unwrap();
        assert!(general.contains("甲乙\t가을\tfalse\tfalse\t1\t\t2,30\n"));
        assert!(general.contains("甲乙\t나을\tfalse\tfalse\t0\t\t4\n"));
        assert!(
            String::from_utf8(forward.1)
                .unwrap()
                .contains("甲乙\t라을\tfalse\tfalse\t\t\t5\n")
        );
    }

    #[test]
    fn pairs_only_source_attested_aligned_sound_law_forms_and_keeps_standalone() {
        let (general, north) = rows(vec![
            item(1, "年代", "연대", "일반어", "명사"),
            item(2, "年代", "-년대", "일반어", "접사"),
            item(3, "年代", "연대", "일반어", "의존 명사"),
            item(4, "年代", "련대", "일반어", "의존 명사"),
            item(5, "年代", "곤대", "일반어", "의존 명사"),
            item(6, "年", "연", "일반어", "명사"),
            item(7, "年", "-년", "일반어", "접사"),
            item(8, "年代", "년대", "북한어", "명사"),
            item(9, "年代", "련대", "북한어", "명사"),
        ]);
        let general = String::from_utf8(general).unwrap();
        assert!(general.contains("年代\t연대\tfalse\tfalse\t0\t\t1,3\n"));
        assert!(general.contains("年代\t연대\tfalse\tfalse\t0\t년대\t1,2\n"));
        assert!(general.contains("年代\t연대\tfalse\tfalse\t0\t련대\t1,4\n"));
        assert!(general.contains("年代\t년대\tfalse\tfalse\t0\t\t2\n"));
        assert!(general.contains("年代\t련대\tfalse\tfalse\t0\t\t4\n"));
        assert!(general.contains("年代\t곤대\tfalse\tfalse\t1\t\t5\n"));
        assert!(general.contains("年\t년\tfalse\tfalse\t1\t\t7\n"));
        assert!(general.contains("年\t연\tfalse\tfalse\t0\t\t6\n"));
        let north = String::from_utf8(north).unwrap();
        assert!(north.contains("年代\t년대\tfalse\tfalse\t1\t\t8\n"));
        assert!(!north.contains("\t연대\t"));
    }

    #[test]
    fn prefixes_mixed_keys_and_misaligned_readings_never_supply_pairs() {
        let (general, _) = rows(vec![
            item(1, "年度", "연도", "일반어", "명사"),
            item(2, "年度", "년도-", "일반어", "접사"),
            item(3, "年대", "연대", "일반어", "명사"),
            item(4, "年대", "-년대", "일반어", "접사"),
            item(5, "年代記", "연대", "일반어", "명사"),
            item(6, "年代記", "-년대", "일반어", "접사"),
        ]);
        let general = String::from_utf8(general).unwrap();
        assert_eq!(general.lines().count(), 7);
        assert!(
            general
                .lines()
                .skip(1)
                .all(|line| line.split('\t').nth(5) == Some(""))
        );
    }

    #[test]
    fn curated_defaults_validate_signature_selection_reason_and_category() {
        let text = "category\tkey\treading\tsuffix\tcandidates\treason\n\
            general\t甲乙\t나을\t\t[[\"가을\",null],[\"나을\",null]]\tKeep the previous default\n\
            north-korean\t甲乙\t라을\t\t[[\"라을\",null]]\tKeep North Korean spelling\n";
        let pins = default_decisions(text).unwrap();
        assert_eq!(pins.len(), 2);
        let mut candidates = vec![
            ExtractCandidate {
                reading: "가을".into(),
                suffix_reading: None,
                preference: 0,
                source_ids: BTreeSet::new(),
            },
            ExtractCandidate {
                reading: "나을".into(),
                suffix_reading: None,
                preference: 0,
                source_ids: BTreeSet::new(),
            },
        ];
        apply_default(
            "甲乙",
            &mut candidates,
            &pins[&(Category::General, "甲乙".into())],
        )
        .unwrap();
        assert_eq!(candidates[1].preference, 1);
        candidates.pop();
        assert!(
            apply_default(
                "甲乙",
                &mut candidates,
                &pins[&(Category::General, "甲乙".into())]
            )
            .is_err()
        );
        for bad in [
            text.replace("나을\t\t", "다을\t\t"),
            text.replace("general\t", "unknown\t"),
            text.replace("Keep the previous default", ""),
            format!("{text}{}\n", text.lines().nth(1).unwrap()),
        ] {
            assert!(default_decisions(&bad).is_err());
        }
    }
}
