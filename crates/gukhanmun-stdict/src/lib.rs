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

/// Extracts canonical TSV rows from Standard Korean Language Dictionary dumps.
pub mod extract;

static KO_KR_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/stdict.gukfst"));
static KO_KR: OnceLock<gukhanmun_fst::FstDictionary> = OnceLock::new();

/// Returns the bundled South Korean Standard Korean Language Dictionary.
///
/// The dictionary is embedded as FST bytes generated from the canonical TSV
/// snapshot in this crate's `data` directory and is decoded lazily on first
/// use.
pub fn ko_kr() -> &'static gukhanmun_fst::FstDictionary {
    KO_KR.get_or_init(|| {
        gukhanmun_fst::FstDictionary::from_static_bytes(KO_KR_BYTES)
            .expect("embedded Standard Korean Language Dictionary FST is valid")
    })
}
