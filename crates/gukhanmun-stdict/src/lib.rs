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

//! Bundled Standard Korean Language Dictionary for Gukhanmun.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::sync::OnceLock;

use gukhanmun_core::{CandidateMatch, CandidateRecord, DictionaryRecord, HanjaDictionary, Match};
use gukhanmun_fst::FstDictionary;

/// Extracts canonical TSV rows from Standard Korean Language Dictionary dumps.
pub mod extract;

static KO_KR_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/stdict.gukfst"));
static KO_KR_FST: OnceLock<FstDictionary> = OnceLock::new();

fn ko_kr_fst() -> &'static FstDictionary {
    KO_KR_FST.get_or_init(|| {
        FstDictionary::from_static_bytes(KO_KR_BYTES)
            .expect("embedded Standard Korean Language Dictionary FST is valid")
    })
}

/// The bundled South Korean Standard Korean Language Dictionary.
///
/// Wraps the embedded candidate-aware FST. Obtain the shared instance through
/// [`ko_kr`].
pub struct KoKrDictionary {
    fst: &'static FstDictionary,
}

impl KoKrDictionary {
    /// Returns the number of entries recorded in the embedded FST.
    pub fn entry_count(&self) -> u64 {
        self.fst.entry_count()
    }

    /// Returns the exact embedded entry for `hanja`, if present.
    ///
    /// This exposes all candidates and their positional forms, with `reading()`
    /// and `mark()` projecting the selected default.
    pub fn lookup(
        &self,
        hanja: &str,
    ) -> Result<Option<gukhanmun_fst::LookupEntry>, gukhanmun_fst::Error> {
        self.fst.lookup(hanja)
    }
}

impl HanjaDictionary for KoKrDictionary {
    fn candidate_matches_at<'a>(
        &'a self,
        s: &'a str,
    ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
        self.fst.candidate_matches_at(s)
    }
    fn candidate_matches_at_spellings(&self, spellings: &[&str]) -> Vec<(usize, CandidateMatch)> {
        self.fst.candidate_matches_at_spellings(spellings)
    }
    fn candidate_entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = CandidateRecord> + 'a>> {
        self.fst.candidate_entries()
    }
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        self.fst.matches_at(s)
    }

    fn max_word_chars(&self) -> Option<usize> {
        self.fst.max_word_chars()
    }

    fn entries<'a>(&'a self) -> Option<Box<dyn Iterator<Item = DictionaryRecord> + 'a>> {
        self.fst.entries()
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        self.fst.has_homophone(hanja, reading)
    }
}

static KO_KR: OnceLock<KoKrDictionary> = OnceLock::new();

/// Returns the bundled South Korean Standard Korean Language Dictionary.
///
/// The dictionary is embedded as FST bytes generated from the canonical TSV
/// snapshot in this crate's `data` directory and is decoded lazily on first
/// use. Multi-syllable entries that the source records with a distinct suffix
/// or bound-noun form (such as `年代`, read `연대` word-initially but `년대`
/// after a number) carry that form in [`Match::suffix_reading`] so the engine
/// can pick the position-correct reading. Single-hanja initial sound law (for
/// example `年` → `년` after a number) is applied by the engine from the bundled
/// unihan readings and needs no per-entry data here.
pub fn ko_kr() -> &'static KoKrDictionary {
    KO_KR.get_or_init(|| KoKrDictionary { fst: ko_kr_fst() })
}
