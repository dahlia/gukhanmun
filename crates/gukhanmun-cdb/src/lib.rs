//! CDB dictionary backend for gukhanmun.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::BTreeMap;
use std::path::Path;

use ciborium::de::from_reader;
use gukhanmun_core::{HanjaDictionary, Match, MatchMark};

const META_KEY: &[u8] = b"__gukhanmun_meta__";
const MARK_REQUIRE_HANJA: u8 = 0b0000_0001;
const MARK_REQUIRE_HANGUL: u8 = 0b0000_0010;

/// Dictionary backed by a gukhanmun CDB-trie file.
pub struct CdbDictionary {
    metadata: BTreeMap<String, String>,
    cdb: cdb::CDB,
    entry_count: u64,
    max_word_chars: Option<usize>,
}

impl CdbDictionary {
    /// Opens a dictionary file from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let cdb = cdb::CDB::open(path.as_ref()).map_err(|source| {
            Error::new(format!(
                "failed to open {}: {source}",
                path.as_ref().display()
            ))
        })?;
        let metadata_bytes = get_required(&cdb, META_KEY, "dictionary metadata")?;
        let metadata = from_reader::<BTreeMap<String, String>, _>(metadata_bytes.as_slice())
            .map_err(|source| {
                Error::new(format!("failed to decode dictionary metadata: {source}"))
            })?;
        let entry_count = parse_u64_metadata(&metadata, "entry_count").unwrap_or(0);
        let max_word_chars = parse_usize_metadata(&metadata, "max_word_chars");

        Ok(Self {
            metadata,
            cdb,
            entry_count,
            max_word_chars,
        })
    }

    /// Returns build metadata embedded in the dictionary file.
    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }

    /// Returns the number of complete dictionary entries recorded at build time.
    pub fn entry_count(&self) -> u64 {
        self.entry_count
    }

    /// Returns the exact dictionary entry for `hanja`, if present.
    pub fn lookup(&self, hanja: &str) -> Result<Option<LookupEntry>, Error> {
        let Some(value) = get_optional(&self.cdb, hanja.as_bytes())? else {
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
            let Ok(Some(value)) = get_optional(&self.cdb, prefix.as_bytes()) else {
                break;
            };
            if let Ok(Some(entry)) = decode_record(&value) {
                matches.push(Match {
                    byte_len: prefix.len(),
                    reading: entry.reading,
                    mark: entry.mark,
                });
            }
        }

        Box::new(matches.into_iter())
    }

    fn max_word_chars(&self) -> Option<usize> {
        self.max_word_chars
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        self.cdb.iter().any(|record| {
            let Ok((key, value)) = record else {
                return false;
            };
            if key == META_KEY || key == hanja.as_bytes() {
                return false;
            }
            decode_record(&value)
                .is_ok_and(|entry| entry.is_some_and(|entry| entry.reading == reading))
        })
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

fn get_required(cdb: &cdb::CDB, key: &[u8], name: &str) -> Result<Vec<u8>, Error> {
    get_optional(cdb, key)?.ok_or_else(|| Error::new(format!("missing {name}")))
}

fn get_optional(cdb: &cdb::CDB, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    cdb.get(key)
        .transpose()
        .map_err(|source| Error::new(format!("failed to read CDB record: {source}")))
}

fn decode_record(value: &[u8]) -> Result<Option<LookupEntry>, Error> {
    if value.len() < 4 {
        return Err(Error::new("CDB record is shorter than the fixed prefix"));
    }
    if value[0] == 0 {
        return Ok(None);
    }

    let mark = decode_mark(value[1]);
    let reading_len = u16::from_le_bytes([value[2], value[3]]) as usize;
    let reading_end = 4usize
        .checked_add(reading_len)
        .ok_or_else(|| Error::new("reading range overflow"))?;
    let reading_bytes = value
        .get(4..reading_end)
        .ok_or_else(|| Error::new("reading range is outside the CDB record"))?;
    let reading = std::str::from_utf8(reading_bytes)
        .map_err(|source| Error::new(format!("reading contains invalid UTF-8: {source}")))?
        .to_owned();

    Ok(Some(LookupEntry { reading, mark }))
}

fn decode_mark(encoded: u8) -> MatchMark {
    MatchMark {
        require_hanja: encoded & MARK_REQUIRE_HANJA != 0,
        require_hangul: encoded & MARK_REQUIRE_HANGUL != 0,
    }
}

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

fn parse_u64_metadata(metadata: &BTreeMap<String, String>, key: &str) -> Option<u64> {
    metadata.get(key).and_then(|value| value.parse().ok())
}

fn parse_usize_metadata(metadata: &BTreeMap<String, String>, key: &str) -> Option<usize> {
    metadata.get(key).and_then(|value| value.parse().ok())
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

    use super::{CdbDictionary, META_KEY, encode_record};

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
