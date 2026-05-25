// Gukhanmun: CDB dictionary backend for Gukhanmun.
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

//! CDB dictionary backend for Gukhanmun.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use ciborium::de::from_reader;
use gukhanmun_core::{DictionaryRecord, HanjaDictionary, Match, MatchMark};

const META_KEY: &[u8] = b"__gukhanmun_meta__";
const MARK_REQUIRE_HANJA: u8 = 0b0000_0001;
const MARK_REQUIRE_HANGUL: u8 = 0b0000_0010;

/// The CDB header is 2048 bytes: 256 slots × (u32le pos, u32le count).
const HEADER_SIZE: usize = 2048;

/// Dictionary backed by a Gukhanmun CDB-trie file.
///
/// The CDB bytes are held in an [`Arc<[u8]>`] so that
/// [`CdbDictionary::from_bytes`] can accept a slice and
/// [`CdbDictionary::open`] can read from disk; both share the same
/// look-up implementation.
pub struct CdbDictionary {
    metadata: BTreeMap<String, String>,
    data: Arc<[u8]>,
    entry_count: u64,
    max_word_chars: Option<usize>,
}

impl CdbDictionary {
    /// Opens a dictionary file from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| Error::Open {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_source(Arc::from(bytes.as_slice()))
    }

    /// Decodes a dictionary from bytes in the Gukhanmun CDB-trie format.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_source(Arc::from(bytes))
    }

    /// Decodes a dictionary from static bytes in the Gukhanmun CDB-trie
    /// format.
    ///
    /// This is intended for embedded dictionaries built with
    /// `include_bytes!`.  The CDB data is referenced directly without
    /// copying.
    pub fn from_static_bytes(bytes: &'static [u8]) -> Result<Self, Error> {
        Self::from_source(Arc::from(bytes as &[u8]))
    }

    fn from_source(data: Arc<[u8]>) -> Result<Self, Error> {
        let metadata_bytes = cdb_get(&data, META_KEY)?.ok_or(Error::MissingRecord {
            record: "dictionary metadata",
        })?;
        let metadata = from_reader::<BTreeMap<String, String>, _>(metadata_bytes.as_slice())
            .map_err(|source| Error::MetadataDecode { source })?;
        if let Some(version) = metadata.get("version")
            && version != "1"
        {
            tracing::error!(version = %version, expected = "1", "unsupported CDB format version");
            return Err(Error::UnsupportedVersion {
                version: version.clone(),
            });
        }
        let entry_count = parse_u64_metadata(&metadata, "entry_count").unwrap_or(0);
        let max_word_chars = parse_usize_metadata(&metadata, "max_word_chars");

        tracing::info!(
            format_version = metadata.get("version").map(String::as_str).unwrap_or("1"),
            entry_count,
            "loaded CDB dictionary"
        );
        Ok(Self {
            metadata,
            data,
            entry_count,
            max_word_chars,
        })
    }

    /// Returns build metadata embedded in the dictionary file.
    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }

    /// Returns the number of complete dictionary entries recorded at build
    /// time.
    pub fn entry_count(&self) -> u64 {
        self.entry_count
    }

    /// Returns the exact dictionary entry for `hanja`, if present.
    pub fn lookup(&self, hanja: &str) -> Result<Option<LookupEntry>, Error> {
        let Some(value) = cdb_get(&self.data, hanja.as_bytes())? else {
            return Ok(None);
        };
        let Some(record) = decode_record(&value)? else {
            return Ok(None);
        };
        Ok(Some(record))
    }
}

impl HanjaDictionary for CdbDictionary {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        let max_word_chars = self.max_word_chars.unwrap_or(usize::MAX);
        let mut matches = Vec::new();
        let mut prefix = String::new();

        for (index, ch) in s.chars().enumerate() {
            if index >= max_word_chars {
                break;
            }
            prefix.push(ch);
            let value = match cdb_get(&self.data, prefix.as_bytes()) {
                Ok(Some(value)) => value,
                Ok(None) => break,
                Err(error) => {
                    tracing::warn!(
                        prefix_len = prefix.len(),
                        error = ?error,
                        "aborting CDB prefix traversal due to read error"
                    );
                    break;
                }
            };
            match decode_record(&value) {
                Ok(Some(entry)) => {
                    matches.push(Match {
                        byte_len: prefix.len(),
                        reading: entry.reading,
                        mark: entry.mark,
                    });
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        prefix_len = prefix.len(),
                        error = ?error,
                        "aborting CDB prefix traversal due to decode error"
                    );
                    break;
                }
            }
        }

        Box::new(matches.into_iter())
    }

    fn max_word_chars(&self) -> Option<usize> {
        self.max_word_chars
    }

    fn entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = DictionaryRecord> + 'a>> {
        let mut records = Vec::new();
        for result in cdb_iter(&self.data) {
            let (key, value) = match result {
                Ok(pair) => pair,
                Err(error) => {
                    tracing::warn!(error = ?error, "skipping CDB entry due to iterator error");
                    continue;
                }
            };
            if key == META_KEY {
                continue;
            }
            let entry = match decode_record(&value) {
                Ok(Some(entry)) => entry,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(error = ?error, "skipping malformed CDB entry");
                    continue;
                }
            };
            let Ok(hanja) = String::from_utf8(key) else {
                tracing::warn!("skipping CDB entry with non-UTF-8 key");
                continue;
            };
            records.push(DictionaryRecord {
                hanja,
                reading: entry.reading,
                mark: entry.mark,
            });
        }
        Some(Box::new(records.into_iter()))
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        for result in cdb_iter(&self.data) {
            let (key, value) = match result {
                Ok(pair) => pair,
                Err(_) => continue,
            };
            if key == META_KEY || key == hanja.as_bytes() {
                continue;
            }
            if decode_record(&value).is_ok_and(|entry| entry.is_some_and(|e| e.reading == reading))
            {
                return true;
            }
        }
        false
    }
}

/// A decoded exact-match dictionary entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LookupEntry {
    reading: String,
    mark: MatchMark,
}

impl LookupEntry {
    /// Returns the hangul reading for the entry.
    pub fn reading(&self) -> &str {
        &self.reading
    }

    /// Returns dictionary-provided rendering constraints.
    pub fn mark(&self) -> MatchMark {
        self.mark
    }
}

/// Error returned while opening or decoding a CDB dictionary.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Opening a CDB dictionary file failed.
    #[error("failed to open {path}: {source}")]
    Open {
        /// Path that failed to open.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A required record is missing.
    #[error("missing {record}")]
    MissingRecord {
        /// Human-readable record name.
        record: &'static str,
    },

    /// CBOR metadata could not be decoded.
    #[error("failed to decode dictionary metadata: {source}")]
    MetadataDecode {
        /// Underlying CBOR decode error.
        #[source]
        source: ciborium::de::Error<std::io::Error>,
    },

    /// The metadata version is not supported.
    #[error("unsupported dictionary version {version}")]
    UnsupportedVersion {
        /// Version string read from metadata.
        version: String,
    },

    /// A CDB value did not match the expected record layout.
    #[error("malformed CDB record: {reason}")]
    MalformedRecord {
        /// Description of the malformed record condition.
        reason: &'static str,
    },

    /// A CDB value range overflowed while decoding.
    #[error("{field} overflow")]
    ValueOverflow {
        /// Field that overflowed.
        field: &'static str,
    },

    /// A CDB value range points outside the record.
    #[error("{field} is outside the CDB record")]
    ValueOutOfBounds {
        /// Field that was out of bounds.
        field: &'static str,
    },

    /// A UTF-8 string field was invalid.
    #[error("{field} contains invalid UTF-8: {source}")]
    InvalidUtf8 {
        /// Field that contained invalid UTF-8.
        field: &'static str,
        /// Underlying UTF-8 error.
        #[source]
        source: std::str::Utf8Error,
    },

    /// The CDB data is shorter than the required 2048-byte header.
    #[error("CDB data is too short: {len} bytes")]
    TooShort {
        /// Actual byte length of the data.
        len: usize,
    },

    /// A CDB header slot or record points outside the data buffer.
    #[error("CDB offset {offset} is out of bounds (data len {len})")]
    OutOfBounds {
        /// The out-of-bounds offset.
        offset: usize,
        /// The data length.
        len: usize,
    },
}

// ── Pure CDB operations ────────────────────────────────────────────────────

/// DJB2 hash used by the CDB format (Daniel J. Bernstein's hash).
fn cdb_hash(key: &[u8]) -> u32 {
    key.iter().fold(5381u32, |h, &b| {
        h.wrapping_shl(5).wrapping_add(h) ^ (b as u32)
    })
}

/// Read a little-endian `u32` at `offset` from `data`, returning `None` if
/// out of bounds.
fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    data.get(offset..offset + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
}

/// Look up `key` in the CDB data buffer.  Returns `Ok(None)` when the key is
/// absent, `Ok(Some(value_bytes))` on a match, and `Err` on format errors.
fn cdb_get(data: &[u8], key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    if data.len() < HEADER_SIZE {
        return Err(Error::TooShort { len: data.len() });
    }

    let h = cdb_hash(key);
    let header_slot = (h & 0xff) as usize;
    let header_base = header_slot * 8;

    let table_pos = read_u32(data, header_base).ok_or(Error::OutOfBounds {
        offset: header_base,
        len: data.len(),
    })? as usize;
    let table_count = read_u32(data, header_base + 4).ok_or(Error::OutOfBounds {
        offset: header_base + 4,
        len: data.len(),
    })? as usize;

    if table_count == 0 {
        return Ok(None);
    }

    let start_slot = ((h >> 8) as usize) % table_count;

    for i in 0..table_count {
        let slot = (start_slot + i) % table_count;
        let slot_offset = table_pos + slot * 8;

        let slot_hash = match read_u32(data, slot_offset) {
            Some(v) => v,
            None => return Ok(None),
        };
        let data_pos = match read_u32(data, slot_offset + 4) {
            Some(v) => v as usize,
            None => return Ok(None),
        };

        if data_pos == 0 {
            return Ok(None);
        }

        if slot_hash == h {
            let key_len = match read_u32(data, data_pos) {
                Some(v) => v as usize,
                None => continue,
            };
            let val_len = match read_u32(data, data_pos + 4) {
                Some(v) => v as usize,
                None => continue,
            };
            let key_start = data_pos + 8;
            let val_start = key_start.saturating_add(key_len);
            let val_end = val_start.saturating_add(val_len);

            if val_end > data.len() {
                continue;
            }
            if data[key_start..key_start + key_len] == *key {
                return Ok(Some(data[val_start..val_end].to_vec()));
            }
        }
    }

    Ok(None)
}

/// Iterate all records in the CDB data area (bytes 2048 up to the first
/// hash table).  Yields `(key_bytes, value_bytes)` pairs.
fn cdb_iter(data: &[u8]) -> impl Iterator<Item = Result<(Vec<u8>, Vec<u8>), Error>> + '_ {
    // The data area ends where the first hash table begins.
    let data_end = (0..256usize)
        .filter_map(|i| {
            let pos = read_u32(data, i * 8)? as usize;
            if pos >= HEADER_SIZE { Some(pos) } else { None }
        })
        .min()
        .unwrap_or(data.len());

    CdbIter {
        data,
        pos: HEADER_SIZE,
        data_end,
    }
}

struct CdbIter<'a> {
    data: &'a [u8],
    pos: usize,
    data_end: usize,
}

impl<'a> Iterator for CdbIter<'a> {
    type Item = Result<(Vec<u8>, Vec<u8>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.data_end {
            return None;
        }

        let key_len = read_u32(self.data, self.pos)? as usize;
        let val_len = read_u32(self.data, self.pos + 4)? as usize;
        let key_start = self.pos + 8;
        let val_start = key_start + key_len;
        let next_pos = val_start + val_len;

        if next_pos > self.data_end {
            return Some(Err(Error::OutOfBounds {
                offset: next_pos,
                len: self.data_end,
            }));
        }

        let key = self.data[key_start..key_start + key_len].to_vec();
        let val = self.data[val_start..val_start + val_len].to_vec();
        self.pos = next_pos;
        Some(Ok((key, val)))
    }
}

// ── Record helpers ─────────────────────────────────────────────────────────

fn decode_record(value: &[u8]) -> Result<Option<LookupEntry>, Error> {
    if value.len() < 4 {
        return Err(Error::MalformedRecord {
            reason: "record is shorter than the fixed prefix",
        });
    }
    if value[0] == 0 {
        return Ok(None);
    }

    let mark = decode_mark(value[1]);
    let reading_len = u16::from_le_bytes([value[2], value[3]]) as usize;
    let reading_end = 4usize
        .checked_add(reading_len)
        .ok_or(Error::ValueOverflow {
            field: "reading range",
        })?;
    let reading_bytes = value
        .get(4..reading_end)
        .ok_or(Error::ValueOutOfBounds { field: "reading" })?;
    let reading = std::str::from_utf8(reading_bytes)
        .map_err(|source| Error::InvalidUtf8 {
            field: "reading",
            source,
        })?
        .to_owned();

    Ok(Some(LookupEntry { reading, mark }))
}

fn decode_mark(encoded: u8) -> MatchMark {
    MatchMark {
        require_hanja: encoded & MARK_REQUIRE_HANJA != 0,
        require_hangul: encoded & MARK_REQUIRE_HANGUL != 0,
    }
}

fn parse_u64_metadata(metadata: &BTreeMap<String, String>, key: &str) -> Option<u64> {
    metadata.get(key).and_then(|value| value.parse().ok())
}

fn parse_usize_metadata(metadata: &BTreeMap<String, String>, key: &str) -> Option<usize> {
    metadata.get(key).and_then(|value| value.parse().ok())
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
fn encode_record(entry: Option<(&str, MatchMark)>) -> Vec<u8> {
    let mut output = Vec::new();
    match entry {
        Some((reading, mark)) => {
            output.push(1);
            output.push(encode_mark(mark));
            output.extend_from_slice(&(reading.len() as u16).to_le_bytes());
            output.extend_from_slice(reading.as_bytes());
        }
        None => {
            output.push(0);
            output.push(0);
            output.extend_from_slice(&0u16.to_le_bytes());
        }
    }
    output
}

#[cfg(test)]
fn encode_mark(mark: MatchMark) -> u8 {
    let mut encoded = 0;
    if mark.require_hanja {
        encoded |= MARK_REQUIRE_HANJA;
    }
    if mark.require_hangul {
        encoded |= MARK_REQUIRE_HANGUL;
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    use ciborium::ser::into_writer;
    use gukhanmun_core::{HanjaDictionary, MapDictionary, MatchMark};
    use proptest::prelude::*;
    use tempfile::tempdir;
    use tracing_test::traced_test;

    use super::{CdbDictionary, META_KEY, encode_record};

    #[traced_test]
    #[test]
    fn unsupported_version_emits_error_event() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("dict.gukcdb");
        let metadata = BTreeMap::from([("version".to_owned(), "99".to_owned())]);
        let mut metadata_bytes = Vec::new();
        into_writer(&metadata, &mut metadata_bytes).unwrap();
        let mut writer = cdb::CDBWriter::create(path.to_string_lossy().as_ref()).unwrap();
        writer.add(META_KEY, &metadata_bytes).unwrap();
        writer.finish().unwrap();

        let result = CdbDictionary::open(&path);

        assert!(matches!(
            result,
            Err(super::Error::UnsupportedVersion { .. })
        ));
        assert!(logs_contain("unsupported CDB format version"));
    }

    #[test]
    fn loads_metadata_lookup_and_prefix_matches() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("dict.gukcdb");
        write_fixture(
            &path,
            &[
                entry("行事", "행사", false, false),
                entry("行事場", "행사장", true, false),
                entry("場所", "장소", false, true),
            ],
        );

        let dictionary = CdbDictionary::open(&path).unwrap();

        assert_eq!(dictionary.metadata().get("source").unwrap(), "fixture");
        assert_eq!(dictionary.entry_count(), 3);
        assert_eq!(dictionary.max_word_chars(), Some(3));
        let exact = dictionary.lookup("行事場").unwrap().unwrap();
        assert_eq!(exact.reading(), "행사장");
        assert!(exact.mark().require_hanja);
        assert!(!exact.mark().require_hangul);
        let matches = dictionary.matches_at("行事場入口").collect::<Vec<_>>();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].reading, "행사");
        assert_eq!(matches[1].reading, "행사장");
    }

    #[test]
    fn from_bytes_matches_open() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("dict.gukcdb");
        write_fixture(
            &path,
            &[
                entry("行事", "행사", false, false),
                entry("場所", "장소", false, false),
            ],
        );

        let bytes = fs::read(&path).unwrap();
        let from_bytes = CdbDictionary::from_bytes(&bytes).unwrap();
        let from_open = CdbDictionary::open(&path).unwrap();

        assert_eq!(from_bytes.metadata(), from_open.metadata());
        assert_eq!(from_bytes.entry_count(), from_open.entry_count());
        assert_eq!(from_bytes.max_word_chars(), from_open.max_word_chars());

        let bytes_matches = from_bytes.matches_at("行事入口").collect::<Vec<_>>();
        let open_matches = from_open.matches_at("行事入口").collect::<Vec<_>>();
        assert_eq!(bytes_matches, open_matches);
    }

    #[test]
    fn has_homophone_detects_other_forms_with_same_reading() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("dict.gukcdb");
        write_fixture(
            &path,
            &[
                entry("漢字", "한자", false, false),
                entry("翰字", "한자", false, false),
                entry("天地", "천지", false, false),
            ],
        );
        let dictionary = CdbDictionary::open(&path).unwrap();

        assert!(dictionary.has_homophone("漢字", "한자"));
        assert!(!dictionary.has_homophone("天地", "천지"));
    }

    #[test]
    fn open_errors_preserve_structured_variants_and_sources() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("dict.gukcdb");
        let mut writer = cdb::CDBWriter::create(path.to_string_lossy().as_ref()).unwrap();
        writer.add(META_KEY, &[0xff]).unwrap();
        writer.finish().unwrap();

        let error = match CdbDictionary::open(&path) {
            Ok(_) => panic!("corrupt metadata should fail to open"),
            Err(error) => error,
        };

        assert!(matches!(error, super::Error::MetadataDecode { .. }));
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn lookup_errors_distinguish_malformed_records() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("dict.gukcdb");
        let metadata = BTreeMap::from([
            ("entry_count".to_owned(), "1".to_owned()),
            ("version".to_owned(), "1".to_owned()),
            ("max_word_chars".to_owned(), "2".to_owned()),
        ]);
        let mut metadata_bytes = Vec::new();
        into_writer(&metadata, &mut metadata_bytes).unwrap();
        let mut writer = cdb::CDBWriter::create(path.to_string_lossy().as_ref()).unwrap();
        writer.add(META_KEY, &metadata_bytes).unwrap();
        writer.add("天地".as_bytes(), &[1, 0, 1, 0, 0xff]).unwrap();
        writer.finish().unwrap();
        let dictionary = CdbDictionary::open(&path).unwrap();

        let error = dictionary.lookup("天地").unwrap_err();

        assert!(matches!(
            error,
            super::Error::InvalidUtf8 {
                field: "reading",
                ..
            }
        ));
        assert!(std::error::Error::source(&error).is_some());
    }

    proptest! {
        #[test]
        fn generated_cdb_matches_map_dictionary(entries in unique_entries()) {
            let temp = tempdir().unwrap();
            let path = temp.path().join("dict.gukcdb");
            let fixture_entries = entries
                .iter()
                .map(|(hanja, reading, require_hanja, require_hangul)| {
                    TestEntry {
                        hanja,
                        reading,
                        mark: MatchMark {
                            require_hanja: *require_hanja,
                            require_hangul: *require_hangul,
                        },
                    }
                })
                .collect::<Vec<_>>();
            write_fixture(&path, &fixture_entries);
            let cdb = CdbDictionary::open(&path).unwrap();
            let mut map = MapDictionary::new();

            for (hanja, reading, require_hanja, require_hangul) in entries {
                map.insert_marked(
                    &hanja,
                    &reading,
                    MatchMark {
                        require_hanja,
                        require_hangul,
                    },
                );
                let cdb_matches = cdb.matches_at(&format!("{hanja}뒤")).collect::<Vec<_>>();
                let map_matches = map.matches_at(&format!("{hanja}뒤")).collect::<Vec<_>>();
                prop_assert_eq!(cdb_matches, map_matches);
                let lookup = cdb.lookup(&hanja).unwrap().unwrap();
                prop_assert_eq!(lookup.reading(), reading.as_str());
            }
        }

        #[test]
        fn from_bytes_matches_open_proptest(entries in unique_entries()) {
            let temp = tempdir().unwrap();
            let path = temp.path().join("dict.gukcdb");
            let fixture_entries = entries
                .iter()
                .map(|(hanja, reading, require_hanja, require_hangul)| {
                    TestEntry {
                        hanja,
                        reading,
                        mark: MatchMark {
                            require_hanja: *require_hanja,
                            require_hangul: *require_hangul,
                        },
                    }
                })
                .collect::<Vec<_>>();
            write_fixture(&path, &fixture_entries);
            let bytes = fs::read(&path).unwrap();

            let from_open = CdbDictionary::open(&path).unwrap();
            let from_bytes = CdbDictionary::from_bytes(&bytes).unwrap();

            for (hanja, ..) in &entries {
                let open_matches = from_open.matches_at(&format!("{hanja}뒤")).collect::<Vec<_>>();
                let bytes_matches = from_bytes.matches_at(&format!("{hanja}뒤")).collect::<Vec<_>>();
                prop_assert_eq!(open_matches, bytes_matches);
            }
        }
    }

    #[derive(Clone, Debug)]
    struct TestEntry<'a> {
        hanja: &'a str,
        reading: &'a str,
        mark: MatchMark,
    }

    fn entry<'a>(
        hanja: &'a str,
        reading: &'a str,
        require_hanja: bool,
        require_hangul: bool,
    ) -> TestEntry<'a> {
        TestEntry {
            hanja,
            reading,
            mark: MatchMark {
                require_hanja,
                require_hangul,
            },
        }
    }

    fn write_fixture(path: &Path, entries: &[TestEntry<'_>]) {
        let mut metadata = BTreeMap::new();
        metadata.insert("source".to_owned(), "fixture".to_owned());
        metadata.insert("license".to_owned(), "CC0-1.0".to_owned());
        metadata.insert("build_date".to_owned(), "1970-01-01T00:00:00Z".to_owned());
        metadata.insert("entry_count".to_owned(), entries.len().to_string());
        metadata.insert("version".to_owned(), "1".to_owned());
        metadata.insert(
            "max_word_chars".to_owned(),
            entries
                .iter()
                .map(|entry| entry.hanja.chars().count())
                .max()
                .unwrap_or(0)
                .to_string(),
        );
        metadata.insert(
            "max_key_bytes".to_owned(),
            entries
                .iter()
                .map(|entry| entry.hanja.len())
                .max()
                .unwrap_or(0)
                .to_string(),
        );

        let mut records = BTreeMap::<String, Option<(&str, MatchMark)>>::new();
        for entry in entries {
            let mut prefix = String::new();
            for ch in entry.hanja.chars() {
                prefix.push(ch);
                records.entry(prefix.clone()).or_insert(None);
            }
            records.insert(entry.hanja.to_owned(), Some((entry.reading, entry.mark)));
        }
        metadata.insert("prefix_count".to_owned(), records.len().to_string());

        let mut metadata_bytes = Vec::new();
        into_writer(&metadata, &mut metadata_bytes).unwrap();
        let mut writer = cdb::CDBWriter::create(path.to_string_lossy().as_ref()).unwrap();
        writer.add(META_KEY, &metadata_bytes).unwrap();
        for (key, value) in records {
            writer.add(key.as_bytes(), &encode_record(value)).unwrap();
        }
        writer.finish().unwrap();
        assert!(fs::metadata(path).unwrap().len() > 0);
    }

    fn unique_entries() -> impl Strategy<Value = Vec<(String, String, bool, bool)>> {
        proptest::collection::btree_map(
            "[一-龥]{1,3}",
            ("[가-힣]{1,4}", any::<bool>(), any::<bool>()),
            1..16,
        )
        .prop_map(|entries| {
            entries
                .into_iter()
                .map(|(hanja, (reading, require_hanja, require_hangul))| {
                    (hanja, reading, require_hanja, require_hangul)
                })
                .collect()
        })
    }
}
