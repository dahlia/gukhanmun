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

//! Gukhanmun umbrella library.
//!
//! This crate is the high-level entry point for the gukhanmun hanja-to-hangul
//! conversion pipeline. It re-exports public items from the workspace's
//! sibling crates under feature flags and provides a [`Builder`] /
//! [`Converter`] facade that wires the reader, engine, middlewares, renderer,
//! and writer stages together.
//!
//! See the workspace's `DESIGN.md` for the architectural overview.
//!
//! # Quick start (default preset, bundled dictionary)
//!
//! ```
//! # #[cfg(feature = "stdict")] {
//! use gukhanmun::Builder;
//!
//! let converter = Builder::new().build()?;
//! assert_eq!(converter.convert_text_to_string("學校")?, "학교");
//! # } Ok::<(), gukhanmun::Error>(())
//! ```
//!
//! # Custom user dictionary, no bundled data
//!
//! ```
//! use gukhanmun::{Builder, MapDictionary};
//!
//! let mut user = MapDictionary::new();
//! user.insert("外字", "외자");
//! let converter = Builder::new()
//!     .no_bundled_stdict()
//!     .push_dictionary(user)
//!     .build()?;
//! assert_eq!(converter.convert_text_to_string("外字")?, "외자");
//! # Ok::<(), gukhanmun::Error>(())
//! ```
//!
//! # North Korean (`ko-kp`) preset
//!
//! ```
//! use gukhanmun::{Builder, Preset};
//!
//! let converter = Builder::with_preset(Preset::KoKp).build()?;
//! // Without the initial sound law, `來日` falls back to `래일`.
//! assert_eq!(converter.convert_text_to_string("來日")?, "래일");
//! # Ok::<(), gukhanmun::Error>(())
//! ```
//!
//! # HTML fragment conversion (`feature = "html"`)
//!
//! ```
//! # #[cfg(all(feature = "html", feature = "stdict"))] {
//! use gukhanmun::Builder;
//!
//! let converter = Builder::new().build()?;
//! let output = converter.convert_html_fragment_to_string("<p>學校</p>")?;
//! assert!(output.contains("학교"));
//! # } Ok::<(), gukhanmun::Error>(())
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod builder;
mod error;
mod options;

pub use builder::{Builder, Converter};
pub use error::{Error, Result};
pub use options::{ConversionOptions, Preset};

// Always-on re-exports from `gukhanmun-core` so that downstream users do not
// have to depend on the core crate directly.
pub use gukhanmun_core::{
    Annotation, ChainDictionary, ContextWindow, DictionaryRecord, DirectiveAction, Engine,
    EngineOptions, FirstOccurrenceFilter, HanjaDictionary, HomophoneDetection, HomophoneMarker,
    InputToken, MapDictionary, Match, MatchMark, NumeralStrategy, OriginalGloss, OutputToken,
    PlainScopeData, RecoverableInputError, Recovery, RenderMode, RenderOptions, RenderedToken,
    Renderer, RubyBase, Scope, ScopeData, SegmentationStrategy, UnihanCharDict, UserDirectives,
    apply_user_directives, apply_user_directives_iter, convert_plain_text,
    convert_plain_text_with_options, filter_first_occurrences, is_hanja, mark_homophones,
    mark_homophones_with_detection, process_fallible_tokens, process_fallible_tokens_with_options,
    process_tokens, process_tokens_iter, process_tokens_iter_with_options,
    process_tokens_with_options, read_plain_text, recover_input_token, recover_input_tokens,
    render_tokens, render_tokens_iter, write_plain_text,
};

/// HTML adapter (re-export of [`gukhanmun_html`]).
#[cfg(feature = "html")]
pub mod html {
    pub use gukhanmun_html::*;
}

/// Markdown adapter (re-export of [`gukhanmun_markdown`]).
#[cfg(feature = "markdown")]
pub mod markdown {
    pub use gukhanmun_markdown::*;
}

/// FST dictionary backend (re-export of [`gukhanmun_fst`]).
#[cfg(feature = "fst")]
pub mod fst {
    pub use gukhanmun_fst::*;
}

/// CDB dictionary backend (re-export of [`gukhanmun_cdb`]).
#[cfg(feature = "cdb")]
pub mod cdb {
    pub use gukhanmun_cdb::*;
}

/// Bundled *Standard Korean Language Dictionary* (re-export of
/// [`gukhanmun_stdict`]).
#[cfg(feature = "stdict")]
pub mod stdict {
    pub use gukhanmun_stdict::*;
}
