// Gukhanmun: Bundled NIKL loanword examples for Gukhanmun.
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

//! Offline, independently selectable NIKL loanword examples dictionary.
//! Data permissions and attribution are distributed in `ATTRIBUTION.md`.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
use gukhanmun_fst::FstDictionary;
use std::sync::OnceLock;
static BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/loanwords.gukfst"));
static DICTIONARY: OnceLock<FstDictionary> = OnceLock::new();

/// Returns the embedded loanword examples dictionary, decoded once on first use.
///
/// Built offline from the pinned TSV, including all source candidates and explicit
/// preferred readings. Compose user dictionaries before this dictionary and preset
/// dictionaries after it. Neither Korean preset enables it implicitly.
pub fn dictionary() -> &'static FstDictionary {
    DICTIONARY.get_or_init(|| {
        FstDictionary::from_static_bytes(BYTES).expect("validated embedded loanword dictionary")
    })
}
