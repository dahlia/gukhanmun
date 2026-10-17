// Gukhanmun: Experimental source-independent loanword extraction tooling.
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

//! Loanword snapshot validation and extraction.
//!
//! The pinned official corpus and runtime dictionary live in `gukhanmun-loanwords`.
//! Authenticated collection verifies the live response shape. Checksums detect
//! changes; source permissions are documented separately in distributed notices.

#[cfg(feature = "loanwords-collect")]
pub mod collect;
mod extract;
mod files;
mod json;
mod snapshot;

pub use extract::{
    Audit, Candidate, Decision, Decisions, Exclusion, Extraction, Normalization, SourceEvidence,
    audit, extract,
};
pub use files::{
    ExtractionPaths, collect_destination, preflight_paths, process_files, publish_snapshot,
};
pub use json::{canonical_json, parse_json, sha256};
pub use snapshot::{ENDPOINT, Page, Record, Snapshot, SnapshotDiff};

/// A deliberately sanitized failure, safe even when an upstream URL has a key.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// JSON or snapshot contract validation failed.
    #[error("invalid loanword data: {0}")]
    Invalid(&'static str),
    /// A complete sweep disagrees with its declared distinct-record count.
    #[error("loanword count mismatch: declared {declared}, distinct IDs {distinct}")]
    CountMismatch {
        /// API-declared total.
        declared: u64,
        /// Observed distinct opaque identities, without exposing their values.
        distinct: u64,
    },
    /// A conflict requires new or refreshed editorial evidence.
    #[error("unresolved or stale loanword decisions")]
    Decisions,
    /// An input and output, or two outputs, resolve to the same file.
    #[error("loanword input/output paths overlap")]
    PathCollision,
    /// Filesystem operation failed without exposing input content.
    #[error("loanword file operation failed")]
    Io,
    /// Existing artifacts do not match the validated output.
    #[error("loanword artifacts are stale")]
    Stale,
    /// Transport failure, with no credential-bearing error chain.
    #[error("loanword collection transport failed")]
    Transport,
    /// Safe HTTP status only, never a response body or URL.
    #[error("loanword collection HTTP status {0}")]
    Http(u16),
    /// TSV was published but its report failed; check must reject the old pair.
    #[error("TSV published but report publication failed; artifact pair is incomplete")]
    IncompletePair,
}

impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}
