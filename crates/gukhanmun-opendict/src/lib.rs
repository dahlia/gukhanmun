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

//! Bundled Open Korean Dictionary (우리말샘) data for Gukhanmun.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::sync::OnceLock;

use gukhanmun_fst::FstDictionary;

/// Extracts canonical TSV rows from Open Korean Dictionary dumps.
pub mod extract;

static GENERAL_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/general.gukfst"));
static NORTH_KOREAN_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/north-korean.gukfst"));
static DIALECT_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/dialect.gukfst"));
static ARCHAIC_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/archaic.gukfst"));

static GENERAL: OnceLock<FstDictionary> = OnceLock::new();
static NORTH_KOREAN: OnceLock<FstDictionary> = OnceLock::new();
static DIALECT: OnceLock<FstDictionary> = OnceLock::new();
static ARCHAIC: OnceLock<FstDictionary> = OnceLock::new();

/// Returns the bundled Open Korean Dictionary `일반어` dictionary.
///
/// The dictionary is embedded as FST bytes generated from the canonical TSV
/// snapshot in this crate's `data` directory and is decoded lazily on first
/// use. It is intentionally separate from [`north_korean`], [`dialect`], and
/// [`archaic`] so callers can compose exactly the categories they want with
/// `ChainDictionary`.
pub fn general() -> &'static FstDictionary {
    GENERAL.get_or_init(|| decode_static_dictionary(GENERAL_BYTES, "일반어"))
}

/// Returns the bundled Open Korean Dictionary `북한어` dictionary.
///
/// This category carries North Korean readings and orthography such as
/// `歷史` → `력사`, `來日` → `래일`, and `勞動` → `로동`. It is kept as its own
/// dictionary so presets can prioritize it above the South Korean standard
/// dictionary without forcing callers to load the other Open Korean Dictionary
/// categories.
pub fn north_korean() -> &'static FstDictionary {
    NORTH_KOREAN.get_or_init(|| decode_static_dictionary(NORTH_KOREAN_BYTES, "북한어"))
}

/// Returns the bundled Open Korean Dictionary `방언` dictionary.
///
/// The category is exposed independently because dialect entries are useful for
/// custom pipelines but should not be enabled by the stock South or North
/// Korean presets.
pub fn dialect() -> &'static FstDictionary {
    DIALECT.get_or_init(|| decode_static_dictionary(DIALECT_BYTES, "방언"))
}

/// Returns the bundled Open Korean Dictionary `옛말` dictionary.
///
/// The category is exposed independently because archaic entries are useful for
/// custom pipelines but should not be enabled by the stock South or North
/// Korean presets.
pub fn archaic() -> &'static FstDictionary {
    ARCHAIC.get_or_init(|| decode_static_dictionary(ARCHAIC_BYTES, "옛말"))
}

fn decode_static_dictionary(bytes: &'static [u8], category: &'static str) -> FstDictionary {
    FstDictionary::from_static_bytes(bytes).unwrap_or_else(|error| {
        panic!("embedded Open Korean Dictionary {category} FST is invalid: {error}")
    })
}
