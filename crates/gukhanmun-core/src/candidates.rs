// Gukhanmun: Dictionary reading candidates, default selection, and payload encoding.
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

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::{DictionaryRecord, HanjaDictionary, Match, MatchMark};

/// One lexical reading, including its positional form and rendering constraints.
///
/// Preference is relative within a key, with larger values preferred. It never
/// competes with entries from a higher-priority dictionary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadingCandidate {
    /// Word-initial hangul spelling.
    pub reading: String,
    /// Non-word-initial spelling, when explicitly supplied by the dictionary.
    pub suffix_reading: Option<String>,
    /// Rendering constraints belonging only to this reading.
    pub mark: MatchMark,
    /// Relative default preference within the key's allowed set.
    pub preference: u32,
}

/// Invalid explicitly authored candidates or malformed serialized candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum CandidateError {
    /// No candidate was provided.
    #[error("a dictionary key must have at least one candidate")]
    EmptySet,
    /// A reading or explicitly present suffix is empty.
    #[error("candidate readings and explicit suffix readings must not be empty")]
    EmptyReading,
    /// A count, string or payload cannot fit the dictionary layout.
    #[error("candidate payload exceeds the dictionary format limits")]
    TooLong,
    /// A payload is truncated, noncanonical, or contains invalid fields.
    #[error("malformed or noncanonical candidate payload")]
    Malformed,
}

/// Nonempty, deduplicated candidates in deterministic default preference order.
///
/// Explicit sets normalize weights by subtracting the minimum, then sort by
/// descending weight, reading and suffix. Marks merge only within identical
/// (reading, suffix) pairs. Legacy API adapters additionally preserve empty
/// strings accepted by those APIs; such values cannot be serialized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadingCandidates(Vec<ReadingCandidate>);

impl ReadingCandidates {
    /// Validates and canonicalizes an explicitly supplied candidate set.
    pub fn new(mut candidates: Vec<ReadingCandidate>) -> Result<Self, CandidateError> {
        if candidates.is_empty() {
            return Err(CandidateError::EmptySet);
        }
        if candidates.iter().any(|c| {
            c.reading.is_empty() || c.suffix_reading.as_ref().is_some_and(String::is_empty)
        }) {
            return Err(CandidateError::EmptyReading);
        }
        candidates
            .sort_by(|a, b| (&a.reading, &a.suffix_reading).cmp(&(&b.reading, &b.suffix_reading)));
        candidates.dedup_by(|later, earlier| {
            if later.reading != earlier.reading || later.suffix_reading != earlier.suffix_reading {
                return false;
            }
            earlier.mark.require_hanja |= later.mark.require_hanja;
            earlier.mark.require_hangul |= later.mark.require_hangul;
            earlier.preference = earlier.preference.max(later.preference);
            true
        });
        let minimum = candidates.iter().map(|c| c.preference).min().unwrap();
        for candidate in &mut candidates {
            candidate.preference -= minimum;
        }
        if candidates.len() == 1 {
            candidates[0].preference = 0;
            return Ok(Self(candidates));
        }
        candidates.sort_by(|a, b| {
            b.preference
                .cmp(&a.preference)
                .then_with(|| (&a.reading, &a.suffix_reading).cmp(&(&b.reading, &b.suffix_reading)))
        });
        Ok(Self(candidates))
    }

    pub(crate) fn from_legacy(
        reading: String,
        suffix_reading: Option<String>,
        mark: MatchMark,
    ) -> Self {
        Self(vec![ReadingCandidate {
            reading,
            suffix_reading,
            mark,
            preference: 0,
        }])
    }

    /// Returns the full canonical set without permitting invariant changes.
    pub fn as_slice(&self) -> &[ReadingCandidate] {
        &self.0
    }

    /// Returns whether this set uses the legacy scalar binary representation.
    pub fn is_scalar(&self) -> bool {
        self.0.len() == 1 && self.0[0].suffix_reading.is_none()
    }

    /// Encodes a v2 candidate payload shared by the FST and CDB backends.
    ///
    /// Scalar sets use the legacy representation instead. Empty legacy strings
    /// are rejected, including `Some("")`, which differs from no suffix.
    pub fn encode_payload(&self) -> Result<Vec<u8>, CandidateError> {
        if self.is_scalar() {
            return Err(CandidateError::Malformed);
        }
        if self.0.iter().any(|c| {
            c.reading.is_empty() || c.suffix_reading.as_ref().is_some_and(String::is_empty)
        }) {
            return Err(CandidateError::EmptyReading);
        }
        let mut output = Vec::new();
        output.extend_from_slice(
            &u16::try_from(self.0.len())
                .map_err(|_| CandidateError::TooLong)?
                .to_le_bytes(),
        );
        for candidate in &self.0 {
            let suffix = candidate.suffix_reading.as_deref().unwrap_or("");
            if output
                .len()
                .saturating_add(9)
                .saturating_add(candidate.reading.len())
                .saturating_add(suffix.len())
                > usize::from(u16::MAX)
            {
                return Err(CandidateError::TooLong);
            }
            output.extend_from_slice(&candidate.preference.to_le_bytes());
            output.push(
                u8::from(candidate.mark.require_hanja)
                    | (u8::from(candidate.mark.require_hangul) << 1),
            );
            output.extend_from_slice(
                &u16::try_from(candidate.reading.len())
                    .map_err(|_| CandidateError::TooLong)?
                    .to_le_bytes(),
            );
            output.extend_from_slice(
                &u16::try_from(suffix.len())
                    .map_err(|_| CandidateError::TooLong)?
                    .to_le_bytes(),
            );
            output.extend_from_slice(candidate.reading.as_bytes());
            output.extend_from_slice(suffix.as_bytes());
        }
        u16::try_from(output.len()).map_err(|_| CandidateError::TooLong)?;
        Ok(output)
    }

    /// Decodes a bounded v2 payload, rejecting invalid or noncanonical sets.
    ///
    /// Callers must validate their file version and outer record framing first.
    pub fn decode_payload(mut bytes: &[u8]) -> Result<Self, CandidateError> {
        fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8], CandidateError> {
            let (head, tail) = bytes
                .split_at_checked(len)
                .ok_or(CandidateError::Malformed)?;
            *bytes = tail;
            Ok(head)
        }
        if bytes.len() > usize::from(u16::MAX) {
            return Err(CandidateError::TooLong);
        }
        let count = u16::from_le_bytes(take(&mut bytes, 2)?.try_into().unwrap()) as usize;
        if count == 0 || count > bytes.len() / 9 {
            return Err(CandidateError::Malformed);
        }
        let mut candidates = Vec::with_capacity(count);
        for _ in 0..count {
            let preference = u32::from_le_bytes(take(&mut bytes, 4)?.try_into().unwrap());
            let mark = take(&mut bytes, 1)?[0];
            if mark & !3 != 0 {
                return Err(CandidateError::Malformed);
            }
            let len = u16::from_le_bytes(take(&mut bytes, 2)?.try_into().unwrap()) as usize;
            let suffix_len = u16::from_le_bytes(take(&mut bytes, 2)?.try_into().unwrap()) as usize;
            let reading = core::str::from_utf8(take(&mut bytes, len)?)
                .map_err(|_| CandidateError::Malformed)?
                .into();
            let suffix = core::str::from_utf8(take(&mut bytes, suffix_len)?)
                .map_err(|_| CandidateError::Malformed)?;
            candidates.push(ReadingCandidate {
                reading,
                suffix_reading: (suffix_len != 0).then(|| suffix.into()),
                mark: MatchMark {
                    require_hanja: mark & 1 != 0,
                    require_hangul: mark & 2 != 0,
                },
                preference,
            });
        }
        if !bytes.is_empty() {
            return Err(CandidateError::Malformed);
        }
        let canonical = Self::new(candidates.clone())?;
        if canonical.0 != candidates || canonical.is_scalar() {
            return Err(CandidateError::Malformed);
        }
        Ok(canonical)
    }
}

/// Selects the recorded default, independently of candidate collection.
///
/// Sets have already been restricted by dictionary precedence and normalized.
/// This first selector uses no text context, so chunk boundaries cannot change
/// its answer. Positional spelling is applied later by the engine.
pub fn select_default_reading(candidates: &ReadingCandidates) -> &ReadingCandidate {
    &candidates.0[0]
}

/// A lexical prefix and all of its allowed readings before selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateMatch {
    /// Matched dictionary key length in UTF-8 bytes.
    pub byte_len: usize,
    /// Allowed readings of this one lexical key.
    pub candidates: ReadingCandidates,
}

impl CandidateMatch {
    pub(crate) fn from_legacy(matched: Match) -> Self {
        Self {
            byte_len: matched.byte_len,
            candidates: ReadingCandidates::from_legacy(
                matched.reading,
                matched.suffix_reading,
                matched.mark,
            ),
        }
    }
    /// Projects the chosen default into the existing segmentation input type.
    pub fn select_default(&self) -> Match {
        let selected = select_default_reading(&self.candidates);
        Match {
            byte_len: self.byte_len,
            reading: selected.reading.clone(),
            suffix_reading: selected.suffix_reading.clone(),
            mark: selected.mark,
        }
    }
}

/// A complete lexical key and its allowed readings for batch policy analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateRecord {
    /// Canonical dictionary key.
    pub hanja: String,
    /// Allowed readings after dictionary composition.
    pub candidates: ReadingCandidates,
}

impl CandidateRecord {
    /// Returns the legacy enumeration projection of the selected default.
    pub fn select_default(&self) -> DictionaryRecord {
        let selected = select_default_reading(&self.candidates);
        DictionaryRecord {
            hanja: self.hanja.clone(),
            reading: selected.reading.clone(),
            mark: selected.mark,
        }
    }
    /// Tests a collision against the selected initial and suffix forms only.
    pub fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        let selected = select_default_reading(&self.candidates);
        self.hanja != hanja
            && (selected.reading == reading || selected.suffix_reading.as_deref() == Some(reading))
    }
}

/// Resolves candidate-aware complete spellings within one dictionary.
///
/// The original spelling wins over alternatives. Composite dictionaries apply
/// dictionary priority first by overriding the trait method instead.
#[doc(hidden)]
pub fn candidate_matches_at_spellings<D: HanjaDictionary + ?Sized>(
    dictionary: &D,
    spellings: &[&str],
) -> Vec<(usize, CandidateMatch)> {
    let Some((source, alternatives)) = spellings.split_first() else {
        return Vec::new();
    };
    let exact: Vec<_> = dictionary
        .candidate_matches_at(source)
        .filter(|m| m.byte_len == source.len())
        .map(|m| (0, m))
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    let mut matches = Vec::new();
    for (offset, spelling) in alternatives.iter().enumerate() {
        matches.extend(
            dictionary
                .candidate_matches_at(spelling)
                .filter(|m| m.byte_len == spelling.len())
                .map(|m| (offset + 1, m)),
        );
    }
    matches
}
