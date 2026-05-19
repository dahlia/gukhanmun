//! Bundled Standard Korean Language Dictionary for gukhanmun.

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
        gukhanmun_fst::FstDictionary::from_bytes(KO_KR_BYTES)
            .expect("embedded Standard Korean Language Dictionary FST is valid")
    })
}
