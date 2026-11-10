// Gukhanmun: FST dictionary backend for Gukhanmun.
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

//! FST dictionary backend for Gukhanmun.

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use ciborium::de::from_reader;
use fst::automaton::Automaton;
use fst::{IntoStreamer, Map, Streamer};
use gukhanmun_core::{
    CandidateError, CandidateMatch, CandidateRecord, DictionaryEvidence, DictionaryRecord,
    HanjaDictionary, LexemeIndex, Match, MatchMark, PhraseEvidence, PhraseIndex, ReadingCandidate,
    ReadingCandidates, candidate_matches_at_spellings, select_default_reading,
};

const MAGIC: &[u8; 8] = b"GUKHMFST";
const FORMAT_VERSION: u32 = 2;
const FIXED_HEADER_LEN: usize = 64;
const MARK_REQUIRE_HANJA: u8 = 0b0000_0001;
const MARK_REQUIRE_HANGUL: u8 = 0b0000_0010;
const VALUE_READING_LEN_MASK: u64 = 0xffff;
const VALUE_MARK_SHIFT: u64 = 16;
const VALUE_OFFSET_SHIFT: u64 = 24;

/// Dictionary backed by a Gukhanmun FST file.
///
/// The fixed header and CBOR metadata are decoded eagerly.  The FST map bytes
/// and reading table share one backing byte source: owned heap bytes for
/// [`FstDictionary::open`] and [`FstDictionary::from_bytes`], or static bytes
/// for [`FstDictionary::from_static_bytes`].
///
/// Loading validates the reachable graph before enumerating keys, including
/// when maximum-length metadata is present. The limits are 1,048,576 accepted
/// keys, 128 MiB of total key bytes, 134,217,728 expanded node visits counting
/// nonaccepting branches, and a 4,096-byte path. A separate 64 MiB graph admission
/// charge (128 bytes per unique node plus 32 bytes per transition) bounds graph
/// work. This is an accounting limit, not a measurement of backing bytes,
/// allocator overhead or RSS. Exceeding these limits returns [`Error::SourceEvidence`].
#[derive(Clone, Debug)]
pub struct FstDictionary {
    metadata: BTreeMap<String, String>,
    format_version: u32,
    map: Map<ByteSection>,
    readings: ByteSection,
    entry_count: u64,
    max_word_chars: Option<usize>,
    lexemes: LexemeIndex,
    evidence: Option<Arc<DictionaryEvidence>>,
    phrase_evidence: Option<Arc<PhraseEvidence>>,
    phrase_index: Option<Arc<PhraseIndex>>,
}

impl FstDictionary {
    /// Opens a dictionary file from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        tracing::info!(path = %path.display(), "opening FST dictionary");
        let bytes = fs::read(path).map_err(|source| Error::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_bytes(&bytes)
    }

    /// Decodes a dictionary from bytes in the Gukhanmun FST file format.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_source(ByteSource::Owned(Arc::<[u8]>::from(bytes)))
    }

    /// Decodes a dictionary from static bytes in the Gukhanmun FST file format.
    ///
    /// This is intended for embedded dictionaries built with `include_bytes!`.
    /// The FST map and reading table borrow from the static byte slice without
    /// copying either section.
    pub fn from_static_bytes(bytes: &'static [u8]) -> Result<Self, Error> {
        Self::from_source(ByteSource::Static(bytes))
    }

    fn from_source(source: ByteSource) -> Result<Self, Error> {
        let bytes = source.as_ref();
        let header = FixedHeader::parse(bytes)?;
        let format_version = read_u32(&bytes[8..12]);
        let metadata_bytes = checked_slice(bytes, header.metadata_offset, header.metadata_len)
            .ok_or(Error::SectionOutOfBounds {
                section: "metadata",
            })?;
        let StoredMetadata {
            strings: metadata,
            evidence,
            phrase_evidence,
        } = decode_metadata(metadata_bytes)?;
        let fst_bytes = source
            .section(header.fst_offset, header.fst_len)
            .ok_or(Error::SectionOutOfBounds { section: "FST" })?;
        let readings = source
            .section(header.readings_offset, header.readings_len)
            .ok_or(Error::SectionOutOfBounds {
                section: "readings",
            })?;
        let map = Map::new(fst_bytes).map_err(|source| Error::FstDecode { source })?;
        validate_map_resources(&map)?;
        let entry_count = parse_u64_metadata(&metadata, "entry_count")
            .unwrap_or_else(|| u64::try_from(map.len()).unwrap_or(u64::MAX));
        let max_word_chars = parse_usize_metadata(&metadata, "max_word_chars")
            .or_else(|| max_key_chars_from_map(&map));
        let lexemes = metadata
            .get("aliases")
            .map_or_else(LexemeIndex::default, |encoded| LexemeIndex::decode(encoded));

        tracing::debug!(
            byte_length = bytes.len(),
            format_version,
            entry_count,
            ?max_word_chars,
            "decoded FST dictionary"
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
            map,
            readings,
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
            let mut lookup_error = None;
            let mut records = evidence.candidate_keys().into_iter().filter_map(|key| {
                let error = match dictionary.lookup(key) {
                    Ok(Some(entry)) => {
                        return Some(CandidateRecord {
                            hanja: key.into(),
                            candidates: entry.candidates,
                        });
                    }
                    Ok(None) => Error::SourceEvidence {
                        reason: format!("phrase evidence references absent key {key:?}"),
                    },
                    Err(error) => error,
                };
                lookup_error.get_or_insert(error);
                None
            });
            let validation = evidence.validate_records(records.by_ref());
            records.for_each(drop);
            if let Some(error) = lookup_error {
                return Err(error);
            }
            validation.map_err(|error| Error::SourceEvidence {
                reason: error.to_string(),
            })?;
            dictionary.phrase_index = Some(Arc::new(
                PhraseIndex::from_shared_evidence(evidence.clone()).map_err(|error| {
                    Error::SourceEvidence {
                        reason: error.to_string(),
                    }
                })?,
            ));
        }
        Ok(dictionary)
    }

    /// Returns string-valued build metadata embedded in the dictionary file.
    /// Critical evidence is available through [`HanjaDictionary::source_evidence`].
    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }

    /// Returns the number of entries recorded at build time.
    pub fn entry_count(&self) -> u64 {
        self.entry_count
    }

    /// Returns the exact dictionary entry for `hanja`, if present.
    pub fn lookup(&self, hanja: &str) -> Result<Option<LookupEntry>, Error> {
        let Some(encoded) = self.map.get(hanja.as_bytes()) else {
            return Ok(None);
        };
        self.decode_entry(encoded).map(Some)
    }

    fn decode_entry(&self, encoded: u64) -> Result<LookupEntry, Error> {
        let (reading_len, mark, reading_offset) = decode_value(encoded);
        let reading_start =
            usize::try_from(reading_offset).map_err(|_| Error::ValueOutOfRange {
                field: "reading offset",
            })?;
        let reading_end =
            reading_start
                .checked_add(usize::from(reading_len))
                .ok_or(Error::ValueOverflow {
                    field: "reading range",
                })?;
        let reading_bytes = self
            .readings
            .as_ref()
            .get(reading_start..reading_end)
            .ok_or(Error::SectionOutOfBounds {
                section: "reading table entry",
            })?;
        if (encoded >> VALUE_MARK_SHIFT) as u8 & 0x80 != 0 {
            if self.format_version == 1 || (encoded >> VALUE_MARK_SHIFT) as u8 != 0x80 {
                return Err(CandidateError::Malformed.into());
            }
            return Ok(LookupEntry {
                candidates: ReadingCandidates::decode_payload(reading_bytes)?,
            });
        }
        if self.format_version >= 2 && (encoded >> VALUE_MARK_SHIFT) as u8 & !3 != 0 {
            return Err(CandidateError::Malformed.into());
        }
        let reading = std::str::from_utf8(reading_bytes)
            .map_err(|source| Error::InvalidUtf8 {
                field: "reading",
                source,
            })?
            .to_owned();

        Ok(LookupEntry {
            candidates: ReadingCandidates::new(vec![ReadingCandidate {
                reading,
                suffix_reading: None,
                mark,
                preference: 0,
                alias: None,
            }])?,
        })
    }
}

#[derive(Clone, Debug)]
enum ByteSource {
    Owned(Arc<[u8]>),
    Static(&'static [u8]),
}

impl ByteSource {
    fn section(&self, offset: u64, len: u64) -> Option<ByteSection> {
        let offset = usize::try_from(offset).ok()?;
        let len = usize::try_from(len).ok()?;
        let end = offset.checked_add(len)?;
        (end <= self.as_ref().len()).then(|| ByteSection {
            source: self.clone(),
            range: offset..end,
        })
    }
}

impl AsRef<[u8]> for ByteSource {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Static(bytes) => bytes,
        }
    }
}

#[derive(Clone, Debug)]
struct ByteSection {
    source: ByteSource,
    range: Range<usize>,
}

impl AsRef<[u8]> for ByteSection {
    fn as_ref(&self) -> &[u8] {
        &self.source.as_ref()[self.range.clone()]
    }
}

impl HanjaDictionary for FstDictionary {
    fn has_owned_phrase_evidence(&self) -> bool {
        self.phrase_evidence()
            .is_some_and(|evidence| !evidence.rules.is_empty())
    }

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
        let mut stream = self
            .map
            .search(KeyIsPrefixOf::new(s.as_bytes()))
            .into_stream();
        let mut matches = Vec::new();
        while let Some((key, encoded)) = stream.next() {
            match self.decode_entry(encoded) {
                Ok(entry) => matches.push(CandidateMatch {
                    byte_len: key.len(),
                    candidates: entry.candidates,
                }),
                Err(error) => {
                    tracing::warn!(key_len = key.len(), error = ?error, "skipping FST entry with undecodable value")
                }
            }
        }
        matches.sort_by_key(|matched| matched.byte_len);
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
        let mut stream = self.map.stream();
        let mut records = Vec::new();
        while let Some((key, encoded)) = stream.next() {
            if let (Ok(hanja), Ok(entry)) = (std::str::from_utf8(key), self.decode_entry(encoded)) {
                records.push(CandidateRecord {
                    hanja: hanja.into(),
                    candidates: entry.candidates,
                });
            }
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

/// Error returned while opening or decoding an FST dictionary.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Candidate data is invalid, truncated, or noncanonical.
    #[error(transparent)]
    Candidates(#[from] CandidateError),

    /// Reading a dictionary file from disk failed.
    #[error("failed to read {path}: {source}")]
    Io {
        /// Path that failed to open or read.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The input is shorter than the fixed FST header.
    #[error("dictionary file is shorter than the fixed header: {actual} bytes")]
    ShortHeader {
        /// Number of bytes supplied by the caller.
        actual: usize,
    },

    /// The fixed header magic bytes do not identify a Gukhanmun FST file.
    #[error("invalid dictionary magic")]
    InvalidMagic,

    /// The file format version is not supported by this crate.
    #[error("unsupported dictionary version {version}")]
    UnsupportedVersion {
        /// Version read from the fixed header.
        version: u32,
    },

    /// The fixed header length field is not supported.
    #[error("unsupported dictionary header length {header_len}")]
    UnsupportedHeaderLength {
        /// Header length read from the fixed header.
        header_len: u32,
    },

    /// Reading the fixed header failed.
    #[error("failed to read dictionary header: {source}")]
    HeaderRead {
        /// Underlying read error.
        #[source]
        source: std::io::Error,
    },

    /// A section range from the header points outside the file.
    #[error("{section} range is outside the file")]
    SectionOutOfBounds {
        /// Name of the section that was out of bounds.
        section: &'static str,
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

    /// The embedded FST map could not be decoded.
    #[error("failed to decode FST map: {source}")]
    FstDecode {
        /// Underlying FST decode error.
        #[source]
        source: fst::Error,
    },

    /// A packed FST value did not fit the host representation.
    #[error("{field} is too large")]
    ValueOutOfRange {
        /// Field that exceeded its valid range.
        field: &'static str,
    },

    /// A packed FST value overflowed while computing a range.
    #[error("{field} overflow")]
    ValueOverflow {
        /// Field that overflowed.
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
    fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < FIXED_HEADER_LEN {
            return Err(Error::ShortHeader {
                actual: bytes.len(),
            });
        }
        if &bytes[..8] != MAGIC {
            return Err(Error::InvalidMagic);
        }
        let version = read_u32(&bytes[8..12]);
        if !matches!(version, 1..=4) {
            tracing::error!(
                version,
                expected = FORMAT_VERSION,
                "unsupported FST format version"
            );
            return Err(Error::UnsupportedVersion { version });
        }
        let header_len = read_u32(&bytes[12..16]);
        if header_len != FIXED_HEADER_LEN as u32 {
            return Err(Error::UnsupportedHeaderLength { header_len });
        }
        let mut cursor = Cursor::new(&bytes[16..FIXED_HEADER_LEN]);
        Ok(Self {
            metadata_offset: read_next_u64(&mut cursor)?,
            metadata_len: read_next_u64(&mut cursor)?,
            fst_offset: read_next_u64(&mut cursor)?,
            fst_len: read_next_u64(&mut cursor)?,
            readings_offset: read_next_u64(&mut cursor)?,
            readings_len: read_next_u64(&mut cursor)?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct KeyIsPrefixOf<'a> {
    bytes: &'a [u8],
}

impl<'a> KeyIsPrefixOf<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

impl Automaton for KeyIsPrefixOf<'_> {
    type State = Option<usize>;

    fn start(&self) -> Self::State {
        Some(0)
    }

    fn is_match(&self, state: &Self::State) -> bool {
        state.is_some()
    }

    fn can_match(&self, state: &Self::State) -> bool {
        state.is_some()
    }

    fn accept(&self, state: &Self::State, byte: u8) -> Self::State {
        let position = (*state)?;
        if self.bytes.get(position).copied() == Some(byte) {
            Some(position + 1)
        } else {
            None
        }
    }
}

fn decode_value(value: u64) -> (u16, MatchMark, u64) {
    let reading_len = (value & VALUE_READING_LEN_MASK) as u16;
    let mark = decode_mark(((value >> VALUE_MARK_SHIFT) & 0xff) as u8);
    let reading_offset = value >> VALUE_OFFSET_SHIFT;
    (reading_len, mark, reading_offset)
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
    gukhanmun_core::check_metadata_resources(bytes).map_err(|error| match error {
        gukhanmun_core::MetadataResourceError::InvalidCbor { offset } => Error::MetadataDecode {
            source: ciborium::de::Error::Syntax(offset),
        },
        error => Error::SourceEvidence {
            reason: error.to_string(),
        },
    })?;
    let mut reader = std::io::Cursor::new(bytes);
    let decoded: StoredMetadata =
        from_reader(&mut reader).map_err(|source| Error::MetadataDecode { source })?;
    if let Some(evidence) = decoded.evidence.as_ref() {
        evidence.validate().map_err(|error| Error::SourceEvidence {
            reason: error.to_string(),
        })?;
    }
    if let Some(evidence) = decoded.phrase_evidence.as_ref() {
        gukhanmun_core::check_phrase_index_resources(evidence).map_err(|error| {
            Error::SourceEvidence {
                reason: error.to_string(),
            }
        })?;
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

// These limits cover the accepted language, not the untrusted FST length footer.
// A minimized DAG can encode exponentially many keys in a few hundred bytes.
const MAX_FST_KEYS: usize = 1_048_576;
const MAX_FST_KEY_BYTES: usize = 128 * 1024 * 1024;
const MAX_FST_EXPANDED_STEPS: usize = 128 * 1024 * 1024;
const MAX_FST_DEPTH: usize = 4096;
// Charge 128 per unique node and 32 per edge to bound memo growth and
// edge processing; this is not
// an exact allocator/RSS measurement. The DFS stack is separately depth-limited.
const MAX_FST_GRAPH_CHARGE: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Default, Debug)]
struct FstResources {
    keys: usize,
    key_bytes: usize,
    steps: usize,
    depth: usize,
    max_output: u64,
}

fn fst_resource_error(reason: &str) -> Error {
    Error::SourceEvidence {
        reason: format!("FST resource validation: {reason}"),
    }
}

fn validate_map_resources<D: AsRef<[u8]>>(map: &Map<D>) -> Result<(), Error> {
    // Fallible layout checks protect even panic=abort builds. Unwind handling
    // is only an additional defense against future raw decoder changes.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        map_resources(map, MAX_FST_GRAPH_CHARGE).map(|_| ())
    }))
    .map_err(|_| fst_resource_error("malformed node encoding"))?
}

// Validate the layouts used by fst 0.4's raw decoder before calling it: that
// decoder uses unchecked subtraction/indexing and asserts packed widths.
fn checked_fst_node<'a, D: AsRef<[u8]>>(
    fst: &'a fst::raw::Fst<D>,
    version: u64,
    addr: usize,
    charge: &mut usize,
    graph_limit: usize,
) -> Result<fst::raw::Node<'a>, Error> {
    let malformed = || fst_resource_error("malformed node encoding");
    let data = fst.as_bytes();
    let mut count = 0usize;
    let mut deltas = None;
    let mut input_index = None;
    if addr != 0 {
        let state = *data.get(addr).ok_or_else(malformed)?;
        let kind = state >> 6;
        let uncommon = usize::from(state & 63 == 0);
        if kind == 3 {
            // Single edge to the immediately preceding serialized node.
            let start = addr.checked_sub(uncommon).ok_or_else(malformed)?;
            if start < 17 {
                return Err(malformed());
            }
            count = 1;
        } else {
            let extra = uncommon;
            let pack_at = addr.checked_sub(extra + 1).ok_or_else(malformed)?;
            let pack = data[pack_at];
            let transition_width = usize::from(pack >> 4);
            let output_width = usize::from(pack & 15);
            count = if kind == 2 {
                1
            } else if extra == 0 {
                usize::from(state & 63)
            } else {
                match data[addr - 1] {
                    1 => 256,
                    n => usize::from(n),
                }
            };
            if transition_width > 8 || output_width > 8 || (count != 0 && transition_width == 0) {
                return Err(malformed());
            }
            let final_width = if kind != 2 && state & 64 != 0 {
                output_width
            } else {
                0
            };
            let index_size = if kind != 2 && version >= 2 && count > 32 {
                256
            } else {
                0
            };
            let input_size = if kind == 2 { 0 } else { count };
            let start = pack_at
                .checked_sub(
                    index_size
                        + input_size
                        + count * (transition_width + output_width)
                        + final_width,
                )
                .filter(|start| *start >= 16)
                .ok_or_else(malformed)?;
            if index_size != 0 {
                input_index = Some(pack_at - index_size);
            }
            deltas = Some((
                start,
                start + final_width + count * output_width,
                transition_width,
            ));
        }
    }
    // Charge all outgoing edges before inspecting their bytes, not only when
    // DFS later follows them. Count is always at most 256.
    *charge += 128 + count * 32;
    if *charge > graph_limit {
        return Err(fst_resource_error("graph work limit exceeded"));
    }
    if let Some((start, first_delta, width)) = deltas {
        for index in 0..count {
            let at = first_delta + index * width;
            let mut packed = [0u8; 8];
            packed[..width].copy_from_slice(&data[at..at + width]);
            let delta = u64::from_le_bytes(packed);
            if delta != 0 {
                let delta = usize::try_from(delta).map_err(|_| malformed())?;
                let target = start.checked_sub(delta).ok_or_else(malformed)?;
                if target < 16 {
                    return Err(malformed());
                }
            }
        }
    }
    if let Some(index_start) = input_index {
        for input in 0..256 {
            let index = usize::from(data[index_start + input]);
            if index < count && usize::from(data[index_start - 1 - index]) != input {
                return Err(malformed());
            }
        }
        // Values >= count are legal absence markers, but not for an input
        // actually present in the reverse-ordered input table.
        for index in 0..count {
            let input = usize::from(data[index_start - 1 - index]);
            if usize::from(data[index_start + input]) != index {
                return Err(malformed());
            }
        }
    }
    Ok(fst.node(addr))
}

fn map_resources<D: AsRef<[u8]>>(
    map: &Map<D>,
    graph_limit: usize,
) -> Result<(FstResources, usize), Error> {
    struct Frame<'a> {
        node: fst::raw::Node<'a>,
        next: usize,
        resources: FstResources,
    }
    impl<'a> Frame<'a> {
        fn new(node: fst::raw::Node<'a>) -> Self {
            Self {
                node,
                next: 0,
                resources: FstResources {
                    keys: usize::from(node.is_final()),
                    steps: 1,
                    max_output: node.final_output().value(),
                    ..FstResources::default()
                },
            }
        }
    }
    let fst = map.as_fst();
    // Map::new has already checked the raw header/footer lengths and version.
    let data = fst.as_bytes();
    let version = u64::from_le_bytes(data[..8].try_into().expect("checked raw header"));
    let footer_end = data.len() - if version > 2 { 4 } else { 0 };
    let root = u64::from_le_bytes(
        data[footer_end - 8..footer_end]
            .try_into()
            .expect("checked raw footer"),
    );
    let root =
        usize::try_from(root).map_err(|_| fst_resource_error("root address out of range"))?;
    let mut charge = 0usize;
    let mut stack = vec![Frame::new(checked_fst_node(
        fst,
        version,
        root,
        &mut charge,
        graph_limit,
    )?)];
    let mut completed = BTreeMap::<usize, FstResources>::new();
    loop {
        let frame = stack.last_mut().expect("root remains until completion");
        // Address zero is fst's implicit empty final node. All serialized
        // nodes must be entirely beyond the sixteen-byte raw FST header.
        let node_start = if frame.node.addr() == 0 {
            0
        } else {
            frame.node.addr() + 1 - frame.node.as_slice().len()
        };
        if frame.node.addr() != 0 && node_start < 16 {
            return Err(fst_resource_error("node overlaps the raw header"));
        }
        if frame.next == frame.node.len() {
            let finished = stack.pop().expect("current frame exists");
            if stack.is_empty() {
                return Ok((finished.resources, charge));
            }
            completed.insert(finished.node.addr(), finished.resources);
            continue;
        }
        let transition = frame.node.transition(frame.next);
        if (transition.addr != 0 && transition.addr < 16)
            || transition.addr >= node_start
            || (frame.next > 0 && frame.node.transition(frame.next - 1).inp >= transition.inp)
        {
            return Err(fst_resource_error(
                "invalid backward transition or input order",
            ));
        }
        if let Some(child) = completed.get(&transition.addr) {
            let resources = &mut frame.resources;
            // Every operand has already passed its limit, so these additions
            // fit usize even on 32-bit hosts.
            resources.keys += child.keys;
            resources.key_bytes += child.key_bytes + child.keys;
            resources.steps += child.steps;
            resources.depth = resources.depth.max(child.depth + 1);
            resources.max_output = resources.max_output.max(
                child
                    .max_output
                    .checked_add(transition.out.value())
                    .ok_or_else(|| fst_resource_error("output sum overflow"))?,
            );
            if resources.keys > MAX_FST_KEYS {
                return Err(fst_resource_error("accepted key count limit exceeded"));
            }
            if resources.key_bytes > MAX_FST_KEY_BYTES {
                return Err(fst_resource_error("accepted key byte limit exceeded"));
            }
            if resources.steps > MAX_FST_EXPANDED_STEPS {
                return Err(fst_resource_error("expanded traversal limit exceeded"));
            }
            if resources.depth > MAX_FST_DEPTH {
                return Err(fst_resource_error("key depth limit exceeded"));
            }
            frame.next += 1;
        } else {
            if stack.len() > MAX_FST_DEPTH {
                return Err(fst_resource_error("key depth limit exceeded"));
            }
            stack.push(Frame::new(checked_fst_node(
                fst,
                version,
                transition.addr,
                &mut charge,
                graph_limit,
            )?));
        }
    }
}

fn max_key_chars_from_map<D>(map: &Map<D>) -> Option<usize>
where
    D: AsRef<[u8]>,
{
    let mut stream = map.keys();
    let mut max = None;
    while let Some(key) = stream.next() {
        let Ok(key) = std::str::from_utf8(key) else {
            continue;
        };
        let chars = key.chars().count();
        max = Some(max.map_or(chars, |current: usize| current.max(chars)));
    }
    max
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("slice has exactly four bytes"))
}

fn read_next_u64(cursor: &mut Cursor<&[u8]>) -> Result<u64, Error> {
    let mut bytes = [0; 8];
    cursor
        .read_exact(&mut bytes)
        .map_err(|source| Error::HeaderRead { source })?;
    Ok(u64::from_le_bytes(bytes))
}

fn checked_slice(bytes: &[u8], offset: u64, len: u64) -> Option<&[u8]> {
    let offset = usize::try_from(offset).ok()?;
    let len = usize::try_from(len).ok()?;
    bytes.get(offset..offset.checked_add(len)?)
}

#[cfg(test)]
mod tests {
    // Encode a layered minimal DAG directly: both a/b transitions lead to the
    // same preceding node, and address zero is the implicit final state. This
    // is raw FST version 2 (no checksum), with zero outputs and one-byte deltas.
    // Its 2^depth accepted keys never need to be materialized by the fixture.
    fn compact_binary_map(depth: u32, recorded_len: u64) -> Vec<u8> {
        let mut bytes = 2u64.to_le_bytes().to_vec();
        bytes.extend_from_slice(&0u64.to_le_bytes());
        let mut root = 0u64;
        for _ in 0..depth {
            let delta = if root == 0 { 0 } else { 1 };
            bytes.extend_from_slice(&[delta, delta, b'b', b'a', 0x10, 0x02]);
            root = bytes.len() as u64 - 1;
        }
        bytes.extend_from_slice(&recorded_len.to_le_bytes());
        bytes.extend_from_slice(&root.to_le_bytes());
        bytes
    }

    fn container_with_map(map: &[u8], max_chars: Option<&str>) -> Vec<u8> {
        let metadata: BTreeMap<&str, &str> = max_chars
            .map(|value| ("max_word_chars", value))
            .into_iter()
            .collect();
        let mut encoded = Vec::new();
        into_writer(&metadata, &mut encoded).unwrap();
        let fst_offset = FIXED_HEADER_LEN + encoded.len();
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(FIXED_HEADER_LEN as u32).to_le_bytes());
        for value in [
            FIXED_HEADER_LEN,
            encoded.len(),
            fst_offset,
            map.len(),
            fst_offset + map.len(),
            0,
        ] {
            bytes.extend_from_slice(&(value as u64).to_le_bytes());
        }
        bytes.extend(encoded);
        bytes.extend_from_slice(map);
        bytes
    }

    #[test]
    fn compact_fst_expansion_is_rejected_independently_of_metadata_and_footer() {
        let small = fst::Map::new(compact_binary_map(3, 8)).unwrap();
        assert_eq!(
            small.stream().into_str_keys().unwrap(),
            ["aaa", "aab", "aba", "abb", "baa", "bab", "bba", "bbb"]
        );
        for recorded in [1, 1 << 24] {
            let raw = compact_binary_map(24, recorded);
            assert!(raw.len() < 200);
            for max_chars in [None, Some("24"), Some("invalid")] {
                let bytes = container_with_map(&raw, max_chars);
                assert!(matches!(FstDictionary::from_bytes(&bytes),
                    Err(Error::SourceEvidence { reason }) if reason.contains("accepted key count limit")));
            }
        }
        let map = fst::Map::new(compact_binary_map(20, 1)).unwrap();
        let (resources, charge) = super::map_resources(&map, super::MAX_FST_GRAPH_CHARGE).unwrap();
        assert_eq!(resources.keys, 1 << 20);
        assert_eq!(resources.key_bytes, 20 << 20);
        assert_eq!(charge, 21 * 128 + 40 * 32);
        // Bound graph work by counted operations, independently of wall time.
        assert!(super::map_resources(&map, charge - 1).is_err());
    }

    #[test]
    fn fst_preflight_preserves_unicode_empty_and_static_loading() {
        let mut builder = MapBuilder::memory();
        for key in [b"".as_slice(), "天地人".as_bytes(), &[0xff; 32]] {
            builder.insert(key, 0).unwrap();
        }
        let map = builder.into_inner().unwrap();
        for max_chars in [None, Some("invalid")] {
            let bytes = container_with_map(&map, max_chars);
            let owned = FstDictionary::from_bytes(&bytes).unwrap();
            assert_eq!(owned.max_word_chars, Some(3));
            let static_bytes = Box::leak(bytes.into_boxed_slice());
            let borrowed = FstDictionary::from_static_bytes(static_bytes).unwrap();
            assert_eq!(borrowed.max_word_chars, Some(3));
            assert!(matches!(
                borrowed.map.as_fst().as_inner().source,
                super::ByteSource::Static(_)
            ));
        }
        let empty = MapBuilder::memory().into_inner().unwrap();
        assert_eq!(
            FstDictionary::from_bytes(&container_with_map(&empty, None))
                .unwrap()
                .max_word_chars,
            None
        );
    }

    #[test]
    fn fst_preflight_rejects_deep_and_malformed_graphs() {
        let mut builder = MapBuilder::memory();
        builder
            .insert(vec![b'a'; super::MAX_FST_DEPTH + 1], 0)
            .unwrap();
        let map = builder.into_inner().unwrap();
        assert!(
            matches!(FstDictionary::from_bytes(&container_with_map(&map, Some("1"))),
            Err(Error::SourceEvidence { reason }) if reason.contains("key depth limit"))
        );
        let mut bad = compact_binary_map(2, 4);
        // The second node starts at 22. A delta of 23 underflows its address
        // in fst's raw decoder (or wraps on builds without overflow checks).
        bad[22] = 23;
        assert!(matches!(
            FstDictionary::from_bytes(&container_with_map(&bad, None)),
            Err(Error::SourceEvidence { .. })
        ));
    }

    #[test]
    fn fst_preflight_bounds_key_bytes_and_nonaccepting_paths() {
        let mut bytes = compact_binary_map(20, 1 << 20);
        bytes.truncate(bytes.len() - 16);
        for _ in 0..109 {
            // A unary parent, encoded using the general transition form.
            bytes.extend_from_slice(&[1, b'a', 0x10, 1]);
        }
        let root = bytes.len() as u64 - 1;
        bytes.extend_from_slice(&(1u64 << 20).to_le_bytes());
        bytes.extend_from_slice(&root.to_le_bytes());
        assert!(
            matches!(FstDictionary::from_bytes(&container_with_map(&bytes, Some("1"))),
            Err(Error::SourceEvidence { reason }) if reason.contains("accepted key byte limit"))
        );

        let mut bytes = 2u64.to_le_bytes().to_vec();
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&[0, 0, 0]); // Nonfinal leaf with no transitions.
        for _ in 0..27 {
            bytes.extend_from_slice(&[1, 1, b'b', b'a', 0x10, 2]);
        }
        let root = bytes.len() as u64 - 1;
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&root.to_le_bytes());
        assert!(
            matches!(FstDictionary::from_bytes(&container_with_map(&bytes, Some("1"))),
            Err(Error::SourceEvidence { reason }) if reason.contains("expanded traversal limit"))
        );
    }

    #[test]
    fn malformed_fst_layouts_fail_without_unwind_recovery() {
        fn map_with_body(body: &[u8]) -> fst::Map<Vec<u8>> {
            let mut bytes = 2u64.to_le_bytes().to_vec();
            bytes.extend_from_slice(&0u64.to_le_bytes());
            bytes.extend_from_slice(body);
            let root = bytes.len() as u64 - 1;
            bytes.extend_from_slice(&1u64.to_le_bytes());
            bytes.extend_from_slice(&root.to_le_bytes());
            fst::Map::new(bytes).unwrap()
        }
        for body in [
            &[0, 0x00, b'a', 0x80][..],   // Zero transition width.
            &[0, 0x90, b'a', 0x80][..],   // Transition width above eight.
            &[0, 0x19, b'a', 0x80][..],   // Output width above eight.
            &[255, 0x10, b'a', 0x80][..], // Backreference subtraction overflow.
            &[0, 0x10, 1, 0][..],         // 256 transitions without their tables.
            &[0, 0, 0x80, 0x81][..],      // Node extends into the header.
            &[255, 255, 255, 255, 255, 255, 255, 255, 0x80, 0x81][..],
        ] {
            let map = map_with_body(body);
            // Unlike validate_map_resources, this entry point has no catch_unwind.
            assert!(super::map_resources(&map, super::MAX_FST_GRAPH_CHARGE).is_err());
        }
        let map = map_with_body(&[0xc0, 0, 0, 0]);
        assert!(
            super::checked_fst_node(map.as_fst(), 2, 16, &mut 0, super::MAX_FST_GRAPH_CHARGE)
                .is_err()
        );

        for count in [40u16, 256] {
            let mut builder = MapBuilder::memory();
            for byte in 0..count {
                builder.insert([byte as u8], 0).unwrap();
            }
            let bytes = builder.into_inner().unwrap();
            let map = fst::Map::new(bytes.clone()).unwrap();
            assert_eq!(
                super::map_resources(&map, super::MAX_FST_GRAPH_CHARGE)
                    .unwrap()
                    .0
                    .keys,
                usize::from(count)
            );
            let root = bytes.len() - 21; // Raw version three, including checksum.
            let extra = usize::from(bytes[root] & 63 == 0);
            let index_start = root - extra - 1 - 256;
            for (input, index) in [(1usize, 0u8), (0, 255)] {
                let mut bad = bytes.clone();
                bad[index_start + input] = index;
                let map = fst::Map::new(bad).unwrap();
                assert!(super::map_resources(&map, super::MAX_FST_GRAPH_CHARGE).is_err());
            }
        }
    }

    #[test]
    fn metadata_resource_preflight_rejects_alias_expansion_and_huge_nested_headers() {
        let aliases = "a\tb\tlexical\tc\n".repeat(32_768);
        let mut bytes = Vec::new();
        ciborium::into_writer(&BTreeMap::from([("aliases", aliases)]), &mut bytes).unwrap();
        assert!(bytes.len() < 512 * 1024);
        assert!(matches!(super::decode_metadata(&bytes),
            Err(super::Error::SourceEvidence { reason }) if reason.contains("ordinary metadata allocation charge")));
        for field in ["candidate_evidence", "phrase_evidence"] {
            let mut bytes = vec![0xa1, 0x60 + field.len() as u8];
            bytes.extend_from_slice(field.as_bytes());
            bytes.push(0x9b);
            bytes.extend_from_slice(&u64::MAX.to_be_bytes());
            assert!(matches!(super::decode_metadata(&bytes),
                Err(super::Error::SourceEvidence { reason }) if reason.contains("limit")));
        }
    }

    #[test]
    fn metadata_preflight_retains_tagged_and_chunked_alias_compatibility() {
        for input in [
            b"\xa1\xd8\x2a\x67aliases\x6da\tb\tlexical\tc".as_slice(),
            b"\xa1\x67aliases\xd8\x2a\x6da\tb\tlexical\tc",
            b"\xbf\x7f\x63ali\x64ases\xff\x7f\x62a\t\x62b\t\x68lexical\t\x61c\xff\xff",
            b"\xd8\x2a\xa1\x67aliases\x6da\tb\tlexical\tc",
        ] {
            let metadata = super::decode_metadata(input).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(metadata.strings.get("aliases").unwrap(), "a\tb\tlexical\tc");
        }
    }

    use std::collections::BTreeMap;
    use std::fs;

    use ciborium::ser::into_writer;
    use fst::MapBuilder;
    use gukhanmun_core::{MapDictionary, RenderMode, convert_plain_text};
    use proptest::prelude::*;
    use tempfile::tempdir;
    use tracing_test::traced_test;

    use super::{Error, FstDictionary, HanjaDictionary, MatchMark};

    const MAGIC: &[u8; 8] = b"GUKHMFST";
    const FORMAT_VERSION: u32 = 1;
    const FIXED_HEADER_LEN: usize = 64;
    const MARK_REQUIRE_HANJA: u8 = 0b0000_0001;
    const MARK_REQUIRE_HANGUL: u8 = 0b0000_0010;
    const VALUE_MARK_SHIFT: u64 = 16;
    const VALUE_OFFSET_SHIFT: u64 = 24;

    #[derive(serde::Serialize)]
    struct EvidencePayload<'a>(
        #[serde(serialize_with = "gukhanmun_core::serialize_dictionary_evidence")]
        &'a gukhanmun_core::DictionaryEvidence,
    );

    #[test]
    fn loaded_phrase_index_is_consistent_and_shared_by_dictionary_clones() {
        let evidence = phrase_evidence();
        let bytes = fixture_bytes_with_all_evidence(
            &[entry("降伏", "항복", false, false)],
            None,
            Some(&evidence),
        );
        let dictionary = FstDictionary::from_bytes(&bytes).unwrap();
        assert!(dictionary.has_owned_phrase_evidence());
        // The dictionary and index retain the same immutable graph, allowing
        // engines to reuse its validation without rescanning all phrase rules.
        assert_eq!(
            std::sync::Arc::strong_count(dictionary.phrase_evidence.as_ref().unwrap()),
            2
        );
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
        assert!(
            FstDictionary::from_bytes(&fixture_bytes(&[entry("降伏", "항복", false, false)]))
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
    #[test]
    fn loads_valid_bytes_metadata_and_lookup() {
        let bytes = fixture_bytes(&[
            entry("天地", "천지", false, false),
            entry("漢字", "한자", true, false),
            entry("色깔論", "색깔론", false, true),
        ]);

        let dictionary = FstDictionary::from_bytes(&bytes).unwrap();

        assert_eq!(dictionary.entry_count(), 3);
        assert_eq!(dictionary.metadata().get("source").unwrap(), "fixture");
        assert_eq!(dictionary.max_word_chars(), Some(3));
        let hanja = dictionary.lookup("漢字").unwrap().unwrap();
        assert_eq!(hanja.reading(), "한자");
        assert!(hanja.mark().require_hanja);
        assert!(!hanja.mark().require_hangul);
        let mixed = dictionary.lookup("色깔論").unwrap().unwrap();
        assert_eq!(mixed.reading(), "색깔론");
        assert!(!mixed.mark().require_hanja);
        assert!(mixed.mark().require_hangul);
    }

    #[test]
    fn open_reads_a_dictionary_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("dict.gukfst");
        fs::write(&path, fixture_bytes(&[entry("天地", "천지", false, false)])).unwrap();

        let dictionary = FstDictionary::open(&path).unwrap();

        assert_eq!(
            dictionary.lookup("天地").unwrap().unwrap().reading(),
            "천지"
        );
    }

    #[test]
    fn from_static_bytes_matches_owned_loading() {
        let bytes = fixture_bytes(&[
            entry("天地", "천지", false, false),
            entry("漢字", "한자", true, false),
            entry("色깔論", "색깔론", false, true),
        ]);
        let static_bytes = Box::leak(bytes.clone().into_boxed_slice());
        let owned = FstDictionary::from_bytes(&bytes).unwrap();
        let static_dict = FstDictionary::from_static_bytes(static_bytes).unwrap();

        assert_equivalent_dictionaries(&owned, &static_dict);
    }

    #[test]
    fn dictionary_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<FstDictionary>();
    }

    #[traced_test]
    #[test]
    fn unsupported_version_emits_error_event() {
        let valid = fixture_bytes(&[entry("天地", "천지", false, false)]);
        let mut bad_version = valid.clone();
        bad_version[8..12].copy_from_slice(&999u32.to_le_bytes());

        let result = FstDictionary::from_bytes(&bad_version);

        assert!(matches!(
            result.unwrap_err(),
            super::Error::UnsupportedVersion { version: 999 }
        ));
        assert!(logs_contain("unsupported FST format version"));
    }

    #[test]
    fn rejects_malformed_headers() {
        let valid = fixture_bytes(&[entry("天地", "천지", false, false)]);
        let mut bad_magic = valid.clone();
        bad_magic[0] = b'X';
        assert!(matches!(
            FstDictionary::from_bytes(&bad_magic).unwrap_err(),
            super::Error::InvalidMagic
        ));

        let mut bad_version = valid.clone();
        bad_version[8..12].copy_from_slice(&999u32.to_le_bytes());
        assert!(matches!(
            FstDictionary::from_bytes(&bad_version).unwrap_err(),
            super::Error::UnsupportedVersion { version: 999 }
        ));

        let truncated = &valid[..valid.len() - 1];
        assert!(matches!(
            FstDictionary::from_bytes(truncated).unwrap_err(),
            super::Error::SectionOutOfBounds {
                section: "readings"
            }
        ));
    }

    #[test]
    fn decode_errors_preserve_structured_variants_and_sources() {
        let mut invalid_metadata = fixture_bytes(&[entry("天地", "천지", false, false)]);
        let metadata_offset = FIXED_HEADER_LEN;
        invalid_metadata[metadata_offset] = 0xff;
        let metadata_error = FstDictionary::from_bytes(&invalid_metadata).unwrap_err();
        assert!(matches!(
            metadata_error,
            super::Error::MetadataDecode { .. }
        ));
        assert!(std::error::Error::source(&metadata_error).is_some());

        let mut invalid_reading = fixture_bytes(&[entry("天地", "천지", false, false)]);
        *invalid_reading.last_mut().unwrap() = 0xff;
        let dictionary = FstDictionary::from_bytes(&invalid_reading).unwrap();
        let utf8_error = dictionary.lookup("天地").unwrap_err();
        assert!(matches!(
            utf8_error,
            super::Error::InvalidUtf8 {
                field: "reading",
                ..
            }
        ));
        assert!(std::error::Error::source(&utf8_error).is_some());
    }

    #[test]
    fn evidence_validation_preserves_later_lookup_errors_after_early_link_failure() {
        for (key, reading) in [("天地", "천지"), ("天地", "다름"), ("不存在", "천지")]
        {
            let evidence = source_evidence(&[(key, reading), ("漢字", "한자")]);
            let mut bytes = fixture_bytes_with_evidence(
                &[
                    entry("天地", "천지", false, false),
                    entry("漢字", "한자", false, false),
                ],
                Some(&evidence),
            );
            *bytes.last_mut().unwrap() = 0xff;
            let error = FstDictionary::from_bytes(&bytes).unwrap_err();
            if key == "不存在" {
                assert!(
                    matches!(error, super::Error::SourceEvidence {reason} if reason == r#"evidence references absent key "不存在""#)
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

    #[test]
    fn matches_at_returns_every_prefix_match() {
        let dictionary = FstDictionary::from_bytes(&fixture_bytes(&[
            entry("行事", "행사", false, false),
            entry("行事場", "행사장", false, false),
            entry("場所", "장소", false, false),
        ]))
        .unwrap();

        let matches = dictionary.matches_at("行事場入口").collect::<Vec<_>>();

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].byte_len, "行事".len());
        assert_eq!(matches[0].reading, "행사");
        assert_eq!(matches[1].byte_len, "行事場".len());
        assert_eq!(matches[1].reading, "행사장");
    }

    #[test]
    fn has_homophone_detects_other_forms_with_same_reading() {
        let dictionary = FstDictionary::from_bytes(&fixture_bytes(&[
            entry("漢字", "한자", false, false),
            entry("翰字", "한자", false, false),
            entry("天地", "천지", false, false),
        ]))
        .unwrap();

        assert!(dictionary.has_homophone("漢字", "한자"));
        assert!(!dictionary.has_homophone("天地", "천지"));
    }

    #[test]
    fn lattice_regressions_pass_with_fst_backend() {
        let dictionary = FstDictionary::from_bytes(&fixture_bytes(&[
            entry("行事", "행사", false, false),
            entry("行事場", "행사장", false, false),
            entry("場所", "장소", false, false),
            entry("入口", "입구", false, false),
            entry("汽車길", "기찻길", false, false),
        ]))
        .unwrap();

        assert_eq!(
            convert_plain_text("行事場入口", &dictionary, RenderMode::HangulHanjaParens),
            "행사장(行事場)입구(入口)"
        );
        assert_eq!(
            convert_plain_text("行事場所", &dictionary, RenderMode::HangulHanjaParens),
            "행사(行事)장소(場所)"
        );
        assert_eq!(
            convert_plain_text("汽車길", &dictionary, RenderMode::HangulHanjaParens),
            "기찻길(汽車길)"
        );
    }

    proptest! {
        #[test]
        fn arbitrary_raw_nodes_do_not_need_unwind_recovery(
            body in prop::collection::vec(any::<u8>(), 4..512),
            version in 1u64..=3,
        ) {
            let mut bytes = version.to_le_bytes().to_vec();
            bytes.extend_from_slice(&0u64.to_le_bytes());
            bytes.extend_from_slice(&body);
            let root = bytes.len() as u64 - 1;
            bytes.extend_from_slice(&1u64.to_le_bytes());
            bytes.extend_from_slice(&root.to_le_bytes());
            if version == 3 {
                bytes.extend_from_slice(&0u32.to_le_bytes());
            }
            let map = fst::Map::new(bytes).unwrap();
            let _ = super::map_resources(&map, super::MAX_FST_GRAPH_CHARGE);
        }

        #[test]
        fn generated_fst_matches_map_dictionary(entries in unique_entries()) {
            let bytes = fixture_bytes(
                &entries
                    .iter()
                    .map(|(hanja, reading, require_hanja, require_hangul)| {
                        entry(hanja, reading, *require_hanja, *require_hangul)
                    })
                    .collect::<Vec<_>>()
            );
            let fst = FstDictionary::from_bytes(&bytes).unwrap();
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
                let fst_matches = fst.matches_at(&format!("{hanja}뒤")).collect::<Vec<_>>();
                let map_matches = map.matches_at(&format!("{hanja}뒤")).collect::<Vec<_>>();
                prop_assert_eq!(fst_matches, map_matches);
                let lookup = fst.lookup(&hanja).unwrap().unwrap();
                prop_assert_eq!(lookup.reading(), reading.as_str());
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
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

    fn fixture_bytes(entries: &[TestEntry<'_>]) -> Vec<u8> {
        fixture_bytes_with_evidence(entries, None)
    }

    fn fixture_bytes_with_evidence(
        entries: &[TestEntry<'_>],
        evidence: Option<&gukhanmun_core::DictionaryEvidence>,
    ) -> Vec<u8> {
        fixture_bytes_with_all_evidence(entries, evidence, None)
    }

    fn fixture_bytes_with_all_evidence(
        entries: &[TestEntry<'_>],
        evidence: Option<&gukhanmun_core::DictionaryEvidence>,
        phrases: Option<&gukhanmun_core::PhraseEvidence>,
    ) -> Vec<u8> {
        let format_version = if evidence.is_some() || phrases.is_some() {
            4u32
        } else {
            FORMAT_VERSION
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("source".to_owned(), "fixture".to_owned());
        metadata.insert("license".to_owned(), "CC0-1.0".to_owned());
        metadata.insert("build_date".to_owned(), "1970-01-01T00:00:00Z".to_owned());
        metadata.insert("entry_count".to_owned(), entries.len().to_string());
        metadata.insert("version".to_owned(), format_version.to_string());
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
        let mut encoded_metadata = ciborium::value::Value::serialized(&metadata).unwrap();
        if let (Some(evidence), ciborium::value::Value::Map(fields)) =
            (evidence, &mut encoded_metadata)
        {
            fields.push((
                ciborium::value::Value::Text("candidate_evidence".into()),
                ciborium::value::Value::serialized(&EvidencePayload(evidence)).unwrap(),
            ));
        }
        if let (Some(phrases), ciborium::value::Value::Map(fields)) =
            (phrases, &mut encoded_metadata)
        {
            fields.push((
                ciborium::value::Value::Text("phrase_evidence".into()),
                ciborium::value::Value::serialized(phrases).unwrap(),
            ));
        }
        let mut metadata_bytes = Vec::new();
        into_writer(&encoded_metadata, &mut metadata_bytes).unwrap();

        let mut readings = Vec::new();
        let mut builder = MapBuilder::memory();
        let mut sorted = entries.to_vec();
        sorted.sort_by(|left, right| left.hanja.cmp(right.hanja));
        for entry in sorted {
            let reading_offset = readings.len() as u64;
            let value = (entry.reading.len() as u64)
                | (u64::from(encode_mark(entry.mark)) << VALUE_MARK_SHIFT)
                | (reading_offset << VALUE_OFFSET_SHIFT);
            builder.insert(entry.hanja.as_bytes(), value).unwrap();
            readings.extend_from_slice(entry.reading.as_bytes());
        }
        let fst_bytes = builder.into_inner().unwrap();

        let metadata_offset = FIXED_HEADER_LEN as u64;
        let fst_offset = metadata_offset + metadata_bytes.len() as u64;
        let readings_offset = fst_offset + fst_bytes.len() as u64;
        let mut output = Vec::new();
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&format_version.to_le_bytes());
        output.extend_from_slice(&(FIXED_HEADER_LEN as u32).to_le_bytes());
        output.extend_from_slice(&metadata_offset.to_le_bytes());
        output.extend_from_slice(&(metadata_bytes.len() as u64).to_le_bytes());
        output.extend_from_slice(&fst_offset.to_le_bytes());
        output.extend_from_slice(&(fst_bytes.len() as u64).to_le_bytes());
        output.extend_from_slice(&readings_offset.to_le_bytes());
        output.extend_from_slice(&(readings.len() as u64).to_le_bytes());
        output.extend(metadata_bytes);
        output.extend(fst_bytes);
        output.extend(readings);
        output
    }

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

    fn assert_equivalent_dictionaries(left: &FstDictionary, right: &FstDictionary) {
        assert_eq!(left.metadata(), right.metadata());
        assert_eq!(left.entry_count(), right.entry_count());
        assert_eq!(left.max_word_chars(), right.max_word_chars());
        for key in ["天地", "漢字", "色깔論"] {
            assert_eq!(left.lookup(key).unwrap(), right.lookup(key).unwrap());
        }
        assert_eq!(
            left.matches_at("色깔論이다").collect::<Vec<_>>(),
            right.matches_at("色깔論이다").collect::<Vec<_>>()
        );
        assert_eq!(
            left.entries().unwrap().collect::<Vec<_>>(),
            right.entries().unwrap().collect::<Vec<_>>()
        );
        assert_eq!(
            left.has_homophone("漢字", "한자"),
            right.has_homophone("漢字", "한자")
        );
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
