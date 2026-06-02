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

use std::collections::BTreeMap;
use std::collections::btree_map::Entry as BTreeEntry;
use std::fs;
use std::io::{BufReader, Read, Seek, Write};
use std::path::Path;

use gukhanmun_dict_extract::{OriginalLanguageInfo, keys_from_originals, normalize_word};
use serde::Deserialize;
use zip::ZipArchive;

/// Error returned while extracting Open Korean Dictionary data.
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

/// Statistics describing one extraction run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExtractStats {
    /// Number of JSON `item` records read from the dump.
    pub items_seen: usize,
    /// Number of canonical TSV dictionary rows written after de-duplication.
    pub entries_written: usize,
    /// Number of otherwise valid keys skipped because an earlier item already
    /// emitted the same key in the same category.
    pub duplicate_keys: usize,
    /// Number of JSON items skipped because they could not produce a supported
    /// dictionary entry.
    pub skipped_items: usize,
}

/// Statistics for every Open Korean Dictionary category.
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
    let mut extractor = Extractor::default();
    extractor.read_path(path)?;
    extractor.write_files(writers)
}

/// Extracts canonical category TSVs from one Open Korean Dictionary JSON reader.
///
/// This helper is intended for tests and tools that already opened an
/// individual JSON shard.
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
    let mut extractor = Extractor::default();
    extractor.read_json(reader)?;
    extractor.write_files(writers)
}

#[derive(Default)]
struct Extractor {
    stats: CategoryStats,
    entries: BTreeMap<Category, BTreeMap<String, String>>,
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
                if file_path
                    .extension()
                    .is_some_and(|extension| extension == "json")
                {
                    self.read_json(BufReader::new(fs::File::open(file_path)?))?;
                }
            }
        } else if path.extension().is_some_and(|extension| extension == "zip") {
            tracing::info!(path = %path.display(), input_type = "zip", "extracting Open Korean Dictionary");
            self.read_zip(fs::File::open(path)?)?;
        } else {
            tracing::info!(path = %path.display(), input_type = "json", "extracting Open Korean Dictionary");
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

        for name in names {
            let file = archive.by_name(&name)?;
            self.read_json(BufReader::new(file))?;
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
            self.increment_skipped(None);
            return;
        };
        self.stats.get_mut(category).items_seen += 1;

        let Some(wordinfo) = item.wordinfo else {
            self.increment_skipped(Some(category));
            return;
        };
        if wordinfo.word_unit.as_deref().map(str::trim) != Some("어휘") {
            self.increment_skipped(Some(category));
            return;
        }
        let Some(mut reading) = wordinfo
            .word
            .as_deref()
            .map(str::trim)
            .map(normalize_word)
            .filter(|reading| !reading.is_empty())
        else {
            self.increment_skipped(Some(category));
            return;
        };
        let originals = wordinfo.original_language_info.as_deref().unwrap_or(&[]);
        let Some(keys) = keys_from_originals(originals) else {
            self.increment_skipped(Some(category));
            return;
        };

        let entries = self.entries.entry(category).or_default();
        let key_count = keys.len();
        for (index, key) in keys.into_iter().enumerate() {
            match entries.entry(key) {
                BTreeEntry::Vacant(entry) => {
                    let value = if index + 1 == key_count {
                        std::mem::take(&mut reading)
                    } else {
                        reading.clone()
                    };
                    entry.insert(value);
                }
                BTreeEntry::Occupied(_) => {
                    self.stats.get_mut(category).duplicate_keys += 1;
                }
            }
        }
    }

    fn increment_skipped(&mut self, category: Option<Category>) {
        if let Some(category) = category {
            self.stats.get_mut(category).skipped_items += 1;
        }
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
        self.write_category(Category::General, writers.general)?;
        self.write_category(Category::NorthKorean, writers.north_korean)?;
        self.write_category(Category::Dialect, writers.dialect)?;
        self.write_category(Category::Archaic, writers.archaic)?;
        Ok(self.stats)
    }

    fn write_category(&mut self, category: Category, mut writer: impl Write) -> Result<()> {
        writeln!(writer, "hanja\thangul\trequire_hanja\trequire_hangul")?;
        let entries = self.entries.remove(&category).unwrap_or_default();
        for (key, reading) in &entries {
            writeln!(writer, "{key}\t{reading}\tfalse\tfalse")?;
        }
        self.stats.get_mut(category).entries_written = entries.len();
        tracing::info!(
            category = category.label(),
            entries_written = entries.len(),
            "wrote dictionary TSV"
        );
        Ok(())
    }
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
    #[serde(rename = "type")]
    lexical_type: Option<String>,
}
