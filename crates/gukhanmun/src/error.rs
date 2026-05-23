// Gukhanmun: umbrella library that wires the engine and adapters together.
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

//! Umbrella error type that aggregates failures from the sub-crates that
//! make up the gukhanmun pipeline.

use thiserror::Error;

/// Aggregated error type returned by the umbrella `gukhanmun` crate.
///
/// Each variant wraps a sub-crate's error and is gated on the same Cargo
/// feature that activates the sub-crate. The variant for the always-present
/// `gukhanmun-core` crate is unconditional.
///
/// `Error` is `#[non_exhaustive]` so that additional variants can be added in
/// a minor release as new sub-crates or boundary failures are introduced.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// An error emitted by `gukhanmun-core` (engine, dictionaries, IR).
    #[error(transparent)]
    Core(#[from] gukhanmun_core::Error),

    /// An error emitted by `gukhanmun-html` (HTML reader / writer).
    #[cfg(feature = "html")]
    #[error(transparent)]
    Html(#[from] gukhanmun_html::HtmlError),

    /// An error emitted by `gukhanmun-markdown` (Markdown reader / writer).
    #[cfg(feature = "markdown")]
    #[error(transparent)]
    Markdown(#[from] gukhanmun_markdown::MarkdownError),

    /// An error emitted by `gukhanmun-fst` (FST dictionary backend).
    #[cfg(feature = "fst")]
    #[error(transparent)]
    Fst(#[from] gukhanmun_fst::Error),

    /// An error emitted by `gukhanmun-cdb` (CDB dictionary backend).
    #[cfg(feature = "cdb")]
    #[error(transparent)]
    Cdb(#[from] gukhanmun_cdb::Error),

    /// An I/O error encountered while loading a dictionary or reading
    /// boundary bytes.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A configuration error reported by the umbrella facade itself, such
    /// as requesting the bundled `stdict` without the `stdict` feature
    /// enabled, or supplying an unsupported dictionary format.
    #[error("configuration error: {0}")]
    Config(String),
}

/// Convenience [`std::result::Result`] alias that defaults the error
/// parameter to the umbrella [`enum@Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
