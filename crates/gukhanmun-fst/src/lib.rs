//! FST dictionary backend for gukhanmun.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use ciborium::de::from_reader;
use fst::automaton::Automaton;
use fst::{IntoStreamer, Map, Streamer};
use gukhanmun_core::{HanjaDictionary, Match, MatchMark};

const MAGIC: &[u8; 8] = b"GUKHMFST";
const FORMAT_VERSION: u32 = 1;
const FIXED_HEADER_LEN: usize = 64;
const MARK_REQUIRE_HANJA: u8 = 0b0000_0001;
const MARK_REQUIRE_HANGUL: u8 = 0b0000_0010;
const VALUE_READING_LEN_MASK: u64 = 0xffff;
const VALUE_MARK_SHIFT: u64 = 16;
const VALUE_OFFSET_SHIFT: u64 = 24;

/// Dictionary backed by a gukhanmun FST file.
#[derive(Clone, Debug)]
pub struct FstDictionary {
    metadata: BTreeMap<String, String>,
    map: Map<Vec<u8>>,
    readings: Vec<u8>,
    entry_count: u64,
    max_word_chars: Option<usize>,
}

impl FstDictionary {
    /// Opens a dictionary file from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let bytes = fs::read(path.as_ref()).map_err(|source| {
            Error::new(format!(
                "failed to read {}: {source}",
                path.as_ref().display()
            ))
        })?;
        Self::from_bytes(&bytes)
    }

    /// Decodes a dictionary from bytes in the gukhanmun FST file format.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let header = FixedHeader::parse(bytes)?;
        let metadata_bytes = checked_slice(bytes, header.metadata_offset, header.metadata_len)
            .ok_or_else(|| Error::new("metadata range is outside the file"))?;
        let metadata =
            from_reader::<BTreeMap<String, String>, _>(metadata_bytes).map_err(|source| {
                Error::new(format!("failed to decode dictionary metadata: {source}"))
            })?;
        let fst_bytes = checked_slice(bytes, header.fst_offset, header.fst_len)
            .ok_or_else(|| Error::new("FST range is outside the file"))?;
        let readings = checked_slice(bytes, header.readings_offset, header.readings_len)
            .ok_or_else(|| Error::new("reading table range is outside the file"))?
            .to_vec();
        let map = Map::new(fst_bytes.to_vec())
            .map_err(|source| Error::new(format!("failed to decode FST map: {source}")))?;
        let entry_count = parse_u64_metadata(&metadata, "entry_count")
            .unwrap_or_else(|| u64::try_from(map.len()).unwrap_or(u64::MAX));
        let max_word_chars = parse_usize_metadata(&metadata, "max_word_chars")
            .or_else(|| max_key_chars_from_map(&map));

        Ok(Self {
            metadata,
            map,
            readings,
            entry_count,
            max_word_chars,
        })
    }

    /// Returns build metadata embedded in the dictionary file.
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
        let reading_start = usize::try_from(reading_offset)
            .map_err(|_| Error::new("reading offset is too large"))?;
        let reading_end = reading_start
            .checked_add(usize::from(reading_len))
            .ok_or_else(|| Error::new("reading range overflow"))?;
        let reading_bytes = self
            .readings
            .get(reading_start..reading_end)
            .ok_or_else(|| Error::new("reading range is outside the reading table"))?;
        let reading = std::str::from_utf8(reading_bytes)
            .map_err(|source| {
                Error::new(format!("reading table contains invalid UTF-8: {source}"))
            })?
            .to_owned();

        Ok(LookupEntry { reading, mark })
    }
}

impl HanjaDictionary for FstDictionary {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        let mut stream = self
            .map
            .search(KeyIsPrefixOf::new(s.as_bytes()))
            .into_stream();
        let mut matches = Vec::new();
        while let Some((key, encoded)) = stream.next() {
            if let Ok(entry) = self.decode_entry(encoded) {
                matches.push(Match {
                    byte_len: key.len(),
                    reading: entry.reading,
                    mark: entry.mark,
                });
            }
        }
        matches.sort_by_key(|matched| matched.byte_len);
        Box::new(matches.into_iter())
    }

    fn max_word_chars(&self) -> Option<usize> {
        self.max_word_chars
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        let mut stream = self.map.stream();
        while let Some((key, encoded)) = stream.next() {
            if key == hanja.as_bytes() {
                continue;
            }
            if self
                .decode_entry(encoded)
                .is_ok_and(|entry| entry.reading == reading)
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

/// Error returned while opening or decoding an FST dictionary.
#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

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
            return Err(Error::new(
                "dictionary file is shorter than the fixed header",
            ));
        }
        if &bytes[..8] != MAGIC {
            return Err(Error::new("invalid dictionary magic"));
        }
        let version = read_u32(&bytes[8..12]);
        if version != FORMAT_VERSION {
            return Err(Error::new(format!(
                "unsupported dictionary version {version}"
            )));
        }
        let header_len = read_u32(&bytes[12..16]);
        if header_len != FIXED_HEADER_LEN as u32 {
            return Err(Error::new(format!(
                "unsupported dictionary header length {header_len}"
            )));
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

fn parse_u64_metadata(metadata: &BTreeMap<String, String>, key: &str) -> Option<u64> {
    metadata.get(key).and_then(|value| value.parse().ok())
}

fn parse_usize_metadata(metadata: &BTreeMap<String, String>, key: &str) -> Option<usize> {
    metadata.get(key).and_then(|value| value.parse().ok())
}

fn max_key_chars_from_map(map: &Map<Vec<u8>>) -> Option<usize> {
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
        .map_err(|source| Error::new(format!("failed to read dictionary header: {source}")))?;
    Ok(u64::from_le_bytes(bytes))
}

fn checked_slice(bytes: &[u8], offset: u64, len: u64) -> Option<&[u8]> {
    let offset = usize::try_from(offset).ok()?;
    let len = usize::try_from(len).ok()?;
    bytes.get(offset..offset.checked_add(len)?)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use ciborium::ser::into_writer;
    use fst::MapBuilder;
    use gukhanmun_core::{MapDictionary, RenderMode, convert_plain_text};
    use proptest::prelude::*;
    use tempfile::tempdir;

    use super::{FstDictionary, HanjaDictionary, MatchMark};

    const MAGIC: &[u8; 8] = b"GUKHMFST";
    const FORMAT_VERSION: u32 = 1;
    const FIXED_HEADER_LEN: usize = 64;
    const MARK_REQUIRE_HANJA: u8 = 0b0000_0001;
    const MARK_REQUIRE_HANGUL: u8 = 0b0000_0010;
    const VALUE_MARK_SHIFT: u64 = 16;
    const VALUE_OFFSET_SHIFT: u64 = 24;

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
    fn rejects_malformed_headers() {
        let valid = fixture_bytes(&[entry("天地", "천지", false, false)]);
        let mut bad_magic = valid.clone();
        bad_magic[0] = b'X';
        assert!(
            FstDictionary::from_bytes(&bad_magic)
                .unwrap_err()
                .to_string()
                .contains("magic")
        );

        let mut bad_version = valid.clone();
        bad_version[8..12].copy_from_slice(&999u32.to_le_bytes());
        assert!(
            FstDictionary::from_bytes(&bad_version)
                .unwrap_err()
                .to_string()
                .contains("version")
        );

        let truncated = &valid[..valid.len() - 1];
        assert!(
            FstDictionary::from_bytes(truncated)
                .unwrap_err()
                .to_string()
                .contains("reading")
        );
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
        let mut metadata = BTreeMap::new();
        metadata.insert("source".to_owned(), "fixture".to_owned());
        metadata.insert("license".to_owned(), "CC0-1.0".to_owned());
        metadata.insert("build_date".to_owned(), "1970-01-01T00:00:00Z".to_owned());
        metadata.insert("entry_count".to_owned(), entries.len().to_string());
        metadata.insert("version".to_owned(), FORMAT_VERSION.to_string());
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
        let mut metadata_bytes = Vec::new();
        into_writer(&metadata, &mut metadata_bytes).unwrap();

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
        output.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
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
