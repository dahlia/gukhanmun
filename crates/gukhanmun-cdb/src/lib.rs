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
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use ciborium::de::from_reader;
use gukhanmun_core::{
    CandidateError, CandidateMatch, CandidateRecord, DictionaryEvidence, DictionaryRecord,
    HanjaDictionary, LexemeIndex, Match, MatchMark, PhraseEvidence, PhraseIndex, ReadingCandidate,
    ReadingCandidates, candidate_matches_at_spellings, select_default_reading,
};

const META_KEY: &[u8] = b"__gukhanmun_meta__";
const MARK_REQUIRE_HANJA: u8 = 0b0000_0001;
const MARK_REQUIRE_HANGUL: u8 = 0b0000_0010;

/// The CDB header is 2048 bytes: 256 slots × (u32le pos, u32le count).
const HEADER_SIZE: usize = 2048;

/// Backing byte store for a [`CdbDictionary`]: either a reference-counted
/// owned allocation (from [`CdbDictionary::from_bytes`] /
/// [`CdbDictionary::open`]) or a zero-cost borrow of static memory (from
/// [`CdbDictionary::from_static_bytes`]).
enum DataSource {
    Owned(Arc<[u8]>),
    Static(&'static [u8]),
}

impl Deref for DataSource {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            DataSource::Owned(arc) => arc,
            DataSource::Static(s) => s,
        }
    }
}

/// Dictionary backed by a Gukhanmun CDB-trie file.
///
/// The CDB bytes are held in an internal backing store that is either an
/// [`Arc<[u8]>`]-backed owned allocation or a zero-copy borrow of a
/// `'static` slice, depending on how the dictionary was loaded.
pub struct CdbDictionary {
    metadata: BTreeMap<String, String>,
    format_version: u32,
    data: DataSource,
    entry_count: u64,
    max_word_chars: Option<usize>,
    lexemes: LexemeIndex,
    evidence: Option<Arc<DictionaryEvidence>>,
    phrase_evidence: Option<Arc<PhraseEvidence>>,
    phrase_index: Option<Arc<PhraseIndex>>,
}

impl CdbDictionary {
    /// Opens a dictionary file from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| Error::Open {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_source(DataSource::Owned(Arc::from(bytes.as_slice())))
    }

    /// Decodes a dictionary from bytes in the Gukhanmun CDB-trie format.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_source(DataSource::Owned(Arc::from(bytes)))
    }

    /// Decodes a dictionary from static bytes in the Gukhanmun CDB-trie
    /// format.
    ///
    /// This is intended for embedded dictionaries built with
    /// `include_bytes!`.  The CDB data is referenced directly without
    /// copying.
    pub fn from_static_bytes(bytes: &'static [u8]) -> Result<Self, Error> {
        Self::from_source(DataSource::Static(bytes))
    }

    fn from_source(data: DataSource) -> Result<Self, Error> {
        let metadata_bytes = cdb_get(&data, META_KEY)?.ok_or(Error::MissingRecord {
            record: "dictionary metadata",
        })?;
        let StoredMetadata {
            strings: metadata,
            evidence,
            phrase_evidence,
        } = decode_metadata(metadata_bytes.as_slice())?;
        if let Some(version) = metadata.get("version")
            && !matches!(version.as_str(), "1" | "2" | "3" | "4")
        {
            tracing::error!(version = %version, expected = "1, 2, 3 or 4", "unsupported CDB format version");
            return Err(Error::UnsupportedVersion {
                version: version.clone(),
            });
        }
        let format_version = metadata.get("version").map_or(1, |v| match v.as_str() {
            "4" => 4,
            "3" => 3,
            "2" => 2,
            _ => 1,
        });
        let entry_count = parse_u64_metadata(&metadata, "entry_count").unwrap_or(0);
        let max_word_chars = parse_usize_metadata(&metadata, "max_word_chars");
        let lexemes = metadata
            .get("aliases")
            .map_or_else(LexemeIndex::default, |encoded| LexemeIndex::decode(encoded));

        tracing::info!(
            format_version = metadata.get("version").map(String::as_str).unwrap_or("1"),
            entry_count,
            "loaded CDB dictionary"
        );
        if (format_version == 4) != (evidence.is_some() || phrase_evidence.is_some()) {
            return Err(Error::SourceEvidence {
                reason: "source evidence requires container version 4 and must be present in that version".into(),
            });
        }
        if format_version == 4 && metadata.get("version").map(String::as_str) != Some("4") {
            return Err(Error::SourceEvidence {
                reason: "container and metadata versions disagree".into(),
            });
        }
        let mut dictionary = Self {
            metadata,
            format_version,
            data,
            entry_count,
            max_word_chars,
            lexemes,
            evidence: evidence.map(Arc::new),
            phrase_evidence: phrase_evidence.map(Arc::new),
            phrase_index: None,
        };
        if let Some(evidence) = dictionary.evidence.as_ref() {
            let mut lookup_error = None;
            let mut records = evidence.entries.keys().filter_map(|key| {
                let error = match dictionary.lookup(key) {
                    Ok(Some(entry)) => {
                        return Some(CandidateRecord {
                            hanja: key.clone(),
                            candidates: entry.candidates,
                        });
                    }
                    Ok(None) => Error::SourceEvidence {
                        reason: format!("evidence references absent key {key:?}"),
                    },
                    Err(error) => error,
                };
                lookup_error.get_or_insert(error);
                None
            });
            let validation = evidence.validate_records(records.by_ref());
            // The validator may reject an early link. Still inspect every later
            // evidence key, preserving the original lookup-error precedence.
            records.for_each(drop);
            if let Some(error) = lookup_error {
                return Err(error);
            }
            validation.map_err(|error| Error::SourceEvidence {
                reason: error.to_string(),
            })?;
        }
        if let Some(evidence) = dictionary.phrase_evidence.as_ref() {
            let records = evidence
                .candidate_keys()
                .into_iter()
                .map(|key| {
                    let entry = dictionary
                        .lookup(key)?
                        .ok_or_else(|| Error::SourceEvidence {
                            reason: format!("phrase evidence references absent key {key:?}"),
                        })?;
                    Ok(CandidateRecord {
                        hanja: key.into(),
                        candidates: entry.candidates,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            evidence
                .validate_records(records)
                .map_err(|error| Error::SourceEvidence {
                    reason: error.to_string(),
                })?;
            dictionary.phrase_index =
                Some(Arc::new(PhraseIndex::new(evidence).map_err(|error| {
                    Error::SourceEvidence {
                        reason: error.to_string(),
                    }
                })?));
        }
        Ok(dictionary)
    }

    /// Returns string-valued build metadata embedded in the dictionary file.
    /// Critical evidence is available through [`HanjaDictionary::source_evidence`].
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
        let Some(record) = decode_record_version(&value, self.format_version)? else {
            return Ok(None);
        };
        Ok(Some(record))
    }
}

impl HanjaDictionary for CdbDictionary {
    fn source_evidence(&self) -> Option<&DictionaryEvidence> {
        self.evidence.as_deref()
    }

    fn phrase_evidence(&self) -> Option<&PhraseEvidence> {
        self.phrase_evidence.as_deref()
    }

    fn phrase_index(&self) -> Option<&PhraseIndex> {
        self.phrase_index.as_deref()
    }

    fn candidate_matches_at<'a>(
        &'a self,
        s: &'a str,
    ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
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
            match decode_record_version(&value, self.format_version) {
                Ok(Some(entry)) => {
                    matches.push(CandidateMatch {
                        byte_len: prefix.len(),
                        candidates: entry.candidates,
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

    fn candidate_matches_at_spellings(&self, spellings: &[&str]) -> Vec<(usize, CandidateMatch)> {
        candidate_matches_at_spellings(self, spellings)
    }
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        Box::new(
            self.candidate_matches_at(s)
                .map(|matched| matched.select_default()),
        )
    }
    fn max_word_chars(&self) -> Option<usize> {
        self.max_word_chars
    }

    fn candidate_entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = CandidateRecord> + 'a>> {
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
            let entry = match decode_record_version(&value, self.format_version) {
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
            records.push(CandidateRecord {
                hanja,
                candidates: entry.candidates,
            });
        }
        Some(Box::new(records.into_iter()))
    }

    fn entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = DictionaryRecord> + 'a>> {
        Some(Box::new(
            self.candidate_entries()?
                .map(|record| record.select_default()),
        ))
    }
    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        let hanja = self.lexemes.homophone_canonical(hanja, reading);
        self.candidate_entries()
            .unwrap()
            .any(|record| record.has_homophone(hanja, reading))
    }

    fn lexemes(&self) -> Option<LexemeIndex> {
        Some(self.lexemes.clone())
    }
}

/// A decoded exact-match dictionary entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LookupEntry {
    candidates: ReadingCandidates,
}

impl LookupEntry {
    /// Returns the selected default's word-initial reading.
    pub fn reading(&self) -> &str {
        &select_default_reading(&self.candidates).reading
    }
    /// Returns rendering constraints of the selected default only.
    pub fn mark(&self) -> MatchMark {
        select_default_reading(&self.candidates).mark
    }
    /// Returns all allowed candidates, including positional forms and defaults.
    pub fn candidates(&self) -> &ReadingCandidates {
        &self.candidates
    }
}

/// Error returned while opening or decoding a CDB dictionary.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Candidate data is invalid, truncated, or noncanonical.
    #[error(transparent)]
    Candidates(#[from] CandidateError),

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

    /// Critical source evidence is missing, malformed, or inconsistent with candidates.
    #[error("invalid dictionary source evidence: {reason}")]
    SourceEvidence {
        /// Explanation of the invalid evidence or metadata link.
        reason: String,
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
            self.pos = self.data_end;
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

fn decode_record_version(value: &[u8], version: u32) -> Result<Option<LookupEntry>, Error> {
    if value.len() < 4 {
        return Err(Error::MalformedRecord {
            reason: "record is shorter than the fixed prefix",
        });
    }
    if version == 1 && value[1] & 0x80 != 0 {
        return Err(CandidateError::Malformed.into());
    }
    if value[0] == 0 {
        if version >= 2 && value != [0, 0, 0, 0] {
            return Err(CandidateError::Malformed.into());
        }
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
    if value[1] & 0x80 != 0 {
        if version == 1 || value[1] != 0x80 || value[0] != 1 || value.len() != reading_end {
            return Err(CandidateError::Malformed.into());
        }
        return Ok(Some(LookupEntry {
            candidates: ReadingCandidates::decode_payload(reading_bytes)?,
        }));
    }
    if version >= 2 && (value[0] != 1 || value[1] & !3 != 0 || value.len() != reading_end) {
        return Err(CandidateError::Malformed.into());
    }
    let reading = std::str::from_utf8(reading_bytes)
        .map_err(|source| Error::InvalidUtf8 {
            field: "reading",
            source,
        })?
        .to_owned();

    Ok(Some(LookupEntry {
        candidates: ReadingCandidates::new(vec![ReadingCandidate {
            reading,
            suffix_reading: None,
            mark,
            preference: 0,
            alias: None,
        }])?,
    }))
}

fn decode_mark(encoded: u8) -> MatchMark {
    MatchMark {
        require_hanja: encoded & MARK_REQUIRE_HANJA != 0,
        require_hangul: encoded & MARK_REQUIRE_HANGUL != 0,
    }
}

struct EvidencePayload(DictionaryEvidence);
impl<'de> serde::Deserialize<'de> for EvidencePayload {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        gukhanmun_core::deserialize_dictionary_evidence(deserializer).map(Self)
    }
}

struct StoredMetadata {
    strings: BTreeMap<String, String>,
    evidence: Option<DictionaryEvidence>,
    phrase_evidence: Option<PhraseEvidence>,
}

impl<'de> serde::Deserialize<'de> for StoredMetadata {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MetadataVisitor;
        impl<'de> serde::de::Visitor<'de> for MetadataVisitor {
            type Value = StoredMetadata;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("dictionary metadata with optional critical source evidence")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error as _;
                let mut strings = BTreeMap::new();
                let mut evidence = None;
                let mut phrase_evidence = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "candidate_evidence" {
                        if evidence.is_some() {
                            return Err(A::Error::custom("duplicate candidate evidence"));
                        }
                        evidence = Some(map.next_value::<EvidencePayload>()?.0);
                    } else if key == "phrase_evidence" {
                        if phrase_evidence.is_some() {
                            return Err(A::Error::custom("duplicate phrase evidence"));
                        }
                        phrase_evidence = Some(map.next_value::<PhraseEvidence>()?);
                    } else if strings.insert(key, map.next_value::<String>()?).is_some() {
                        return Err(A::Error::custom("duplicate metadata field"));
                    }
                }
                Ok(StoredMetadata {
                    strings,
                    evidence,
                    phrase_evidence,
                })
            }
        }
        deserializer.deserialize_map(MetadataVisitor)
    }
}

fn decode_metadata(bytes: &[u8]) -> Result<StoredMetadata, Error> {
    if bytes.len() > gukhanmun_core::MAX_EVIDENCE_METADATA_BYTES {
        return Err(Error::SourceEvidence {
            reason: "metadata exceeds host byte limit".into(),
        });
    }
    let mut reader = std::io::Cursor::new(bytes);
    let decoded: StoredMetadata =
        from_reader(&mut reader).map_err(|source| Error::MetadataDecode { source })?;
    if let Some(evidence) = decoded.evidence.as_ref() {
        evidence.validate().map_err(|error| Error::SourceEvidence {
            reason: error.to_string(),
        })?;
    }
    if let Some(evidence) = decoded.phrase_evidence.as_ref() {
        evidence.validate().map_err(|error| Error::SourceEvidence {
            reason: error.to_string(),
        })?;
    }
    if (decoded.evidence.is_some() || decoded.phrase_evidence.is_some())
        && reader.position() != bytes.len() as u64
    {
        return Err(Error::SourceEvidence {
            reason: "trailing bytes in critical metadata".into(),
        });
    }
    Ok(decoded)
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

    #[derive(serde::Serialize)]
    struct EvidencePayload<'a>(
        #[serde(serialize_with = "gukhanmun_core::serialize_dictionary_evidence")]
        &'a gukhanmun_core::DictionaryEvidence,
    );

    #[test]
    fn loaded_phrase_index_is_consistent_and_shared_by_dictionary_borrowers() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("phrases.cdb");
        let evidence = phrase_evidence();
        write_fixture_with_phrases(
            &path,
            &[entry("降伏", "항복", false, false)],
            Some(&evidence),
        );
        let dictionary = std::sync::Arc::new(CdbDictionary::open(&path).unwrap());
        let index = dictionary.phrase_index().unwrap();
        assert_eq!(
            index,
            &gukhanmun_core::PhraseIndex::new(dictionary.phrase_evidence().unwrap()).unwrap()
        );
        let clone = dictionary.clone();
        assert!(std::ptr::eq(index, clone.phrase_index().unwrap()));
        assert!(std::ptr::eq(
            index,
            dictionary.owned_phrase_evidence()[0].index.unwrap()
        ));
        let plain_path = temp.path().join("plain.cdb");
        write_fixture(&plain_path, &[entry("降伏", "항복", false, false)]);
        assert!(
            CdbDictionary::open(plain_path)
                .unwrap()
                .phrase_index()
                .is_none()
        );
    }
    fn phrase_evidence() -> gukhanmun_core::PhraseEvidence {
        use gukhanmun_core::{
            PhraseCandidateLink, PhraseChunk, PhraseEvidence, PhraseRule, PhraseSpelling, WordId,
        };
        PhraseEvidence::new(vec![PhraseRule::new(
            vec![WordId(1)],
            vec![
                PhraseChunk::new(
                    "무조건".into(),
                    vec![PhraseSpelling::new("無條件".into(), vec![])],
                ),
                PhraseChunk::new(
                    "항복".into(),
                    vec![PhraseSpelling::new(
                        "降伏".into(),
                        vec![PhraseCandidateLink::new("항복".into(), None)],
                    )],
                ),
            ],
        )])
    }
    fn source_evidence(readings: &[(&str, &str)]) -> gukhanmun_core::DictionaryEvidence {
        use gukhanmun_core::{
            CandidateEvidence, DictionaryEvidence, SourceContribution, SourceRole, WordId,
            WordOrigin,
        };
        DictionaryEvidence {
            schema_version: 1,
            role: SourceRole::Reference,
            entries: readings
                .iter()
                .enumerate()
                .map(|(index, (key, reading))| {
                    (
                        (*key).into(),
                        vec![CandidateEvidence {
                            reading: (*reading).into(),
                            suffix_reading: None,
                            contributors: vec![SourceContribution {
                                word_id: WordId(index as u64),
                                sense_id: None,
                                origin: WordOrigin::Native,
                            }],
                            authorized_forms: vec![],
                            diagnostics: vec![],
                        }],
                    )
                })
                .collect(),
        }
    }
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

    #[test]
    fn evidence_validation_preserves_later_lookup_errors_after_early_link_failure() {
        for (key, reading) in [("天地", "천지"), ("天地", "다름"), ("不存在", "천지")]
        {
            let temp = tempdir().unwrap();
            let path = temp.path().join("source.gukcdb");
            let evidence = source_evidence(&[(key, reading), ("漢字", "한자")]);
            let mut metadata =
                ciborium::value::Value::serialized(&BTreeMap::from([("version", "4")])).unwrap();
            if let ciborium::value::Value::Map(fields) = &mut metadata {
                fields.push((
                    ciborium::value::Value::Text("candidate_evidence".into()),
                    ciborium::value::Value::serialized(&EvidencePayload(&evidence)).unwrap(),
                ));
            }
            let mut encoded = Vec::new();
            into_writer(&metadata, &mut encoded).unwrap();
            let mut writer = cdb::CDBWriter::create(path.to_str().unwrap()).unwrap();
            writer.add(META_KEY, &encoded).unwrap();
            writer
                .add(
                    "天地".as_bytes(),
                    &encode_record(Some(("천지", MatchMark::default()))),
                )
                .unwrap();
            writer.add("漢字".as_bytes(), &[1, 0, 1, 0, 0xff]).unwrap();
            writer.finish().unwrap();
            let error = match CdbDictionary::open(path) {
                Ok(_) => panic!("malformed source record must fail"),
                Err(error) => error,
            };
            if key == "不存在" {
                assert!(
                    matches!(error,super::Error::SourceEvidence {reason} if reason == r#"evidence references absent key "不存在""#)
                );
            } else {
                assert!(matches!(
                    error,
                    super::Error::InvalidUtf8 {
                        field: "reading",
                        ..
                    }
                ));
            }
        }
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
        write_fixture_with_phrases(path, entries, None);
    }

    fn write_fixture_with_phrases(
        path: &Path,
        entries: &[TestEntry<'_>],
        phrases: Option<&gukhanmun_core::PhraseEvidence>,
    ) {
        let mut metadata = BTreeMap::new();
        metadata.insert("source".to_owned(), "fixture".to_owned());
        metadata.insert("license".to_owned(), "CC0-1.0".to_owned());
        metadata.insert("build_date".to_owned(), "1970-01-01T00:00:00Z".to_owned());
        metadata.insert("entry_count".to_owned(), entries.len().to_string());
        metadata.insert(
            "version".to_owned(),
            if phrases.is_some() { "4" } else { "1" }.to_owned(),
        );
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
        let mut encoded_metadata = ciborium::value::Value::serialized(&metadata).unwrap();
        if let (Some(phrases), ciborium::value::Value::Map(fields)) =
            (phrases, &mut encoded_metadata)
        {
            fields.push((
                ciborium::value::Value::Text("phrase_evidence".into()),
                ciborium::value::Value::serialized(phrases).unwrap(),
            ));
        }
        into_writer(&encoded_metadata, &mut metadata_bytes).unwrap();
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

#[cfg(test)]
mod candidate_record_tests {
    use super::*;
    #[test]
    fn tagged_record_framing_is_versioned_and_bounded() {
        let candidates = ReadingCandidates::new(vec![ReadingCandidate {
            reading: "연대".into(),
            suffix_reading: Some("년대".into()),
            mark: MatchMark::default(),
            preference: 0,
            alias: None,
        }])
        .unwrap();
        let payload = candidates.encode_payload().unwrap();
        let mut record = vec![1, 0x80];
        record.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        record.extend_from_slice(&payload);
        assert_eq!(
            decode_record_version(&record, 2)
                .unwrap()
                .unwrap()
                .candidates(),
            &candidates
        );
        assert!(decode_record_version(&record, 1).is_err());
        for len in 0..record.len() {
            assert!(decode_record_version(&record[..len], 2).is_err());
        }
        let mut trailing = record.clone();
        trailing.push(0);
        assert!(decode_record_version(&trailing, 2).is_err());
        record[1] = 0x81;
        assert!(decode_record_version(&record, 2).is_err());
        assert!(decode_record_version(&[1, 0x80, 2, 0, 0, 0], 2).is_err());
        assert!(decode_record_version(&[0, 0x80, 0, 0], 2).is_err());
        assert!(decode_record_version(&[0, 0x80, 0, 0], 1).is_err());
    }
}
