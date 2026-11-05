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

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::{DictionaryRecord, HanjaDictionary, Match, MatchMark};

const V3_MAGIC: &[u8; 4] = b"GAL3";

/// How a derived spelling relates to its canonical dictionary word.
///
/// `Sibling` and `Lexical` carry lexical identity: the spelling and its
/// canonical form name the same word.  `Recognition` only records that the
/// spelling reads as the candidate says; it does not assert equivalence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AliasKind {
    /// A non-first spelling of a headword listed with alternative spellings.
    Sibling,
    /// A reviewed compound or constituent variant.
    Lexical,
    /// An inferred surface-to-reading alternative with no identity claim.
    Recognition,
}

impl AliasKind {
    fn priority(self) -> u8 {
        match self {
            Self::Lexical => 2,
            Self::Sibling => 1,
            Self::Recognition => 0,
        }
    }

    fn encode(self) -> u8 {
        match self {
            Self::Sibling => 1,
            Self::Lexical => 2,
            Self::Recognition => 3,
        }
    }

    fn decode(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Sibling),
            2 => Some(Self::Lexical),
            3 => Some(Self::Recognition),
            _ => None,
        }
    }
}

/// A derived spelling's canonical form and identity kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AliasTarget {
    /// The canonical spelling of the same word.
    pub canonical: String,
    /// Whether the relation claims lexical identity.
    pub kind: AliasKind,
}

/// One lexical reading, including its positional form and rendering constraints.
///
/// Preference is relative within a key, with larger values preferred. It never
/// competes with entries from a higher-priority dictionary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReadingCandidate {
    /// Word-initial hangul spelling.
    pub reading: String,
    /// Non-word-initial spelling, when explicitly supplied by the dictionary.
    pub suffix_reading: Option<String>,
    /// Rendering constraints belonging only to this reading.
    pub mark: MatchMark,
    /// Relative default preference within the key's allowed set.
    pub preference: u32,
    /// Derived-spelling metadata, when this candidate is an alias.
    pub alias: Option<AliasTarget>,
}

/// Invalid explicitly authored candidates or malformed serialized candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum CandidateError {
    /// No candidate was provided.
    #[error("a dictionary key must have at least one candidate")]
    EmptySet,
    /// A dictionary key is empty.
    #[error("a dictionary key must not be empty")]
    EmptyKey,
    /// A reading or explicitly present suffix is empty.
    #[error("candidate readings and explicit suffix readings must not be empty")]
    EmptyReading,

    /// Candidates sharing a reading disagree about their lexical identity.
    #[error("candidates sharing a reading must agree on alias identity")]
    ConflictingAlias,
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

fn validate_alias_consistency(candidates: &[ReadingCandidate]) -> Result<(), CandidateError> {
    let mut by_reading = alloc::collections::BTreeMap::<&str, Option<&AliasTarget>>::new();
    for candidate in candidates {
        for reading in
            core::iter::once(candidate.reading.as_str()).chain(candidate.suffix_reading.as_deref())
        {
            match by_reading.entry(reading) {
                alloc::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(candidate.alias.as_ref());
                }
                alloc::collections::btree_map::Entry::Occupied(entry) => {
                    if entry.get() != &candidate.alias.as_ref() {
                        return Err(CandidateError::ConflictingAlias);
                    }
                }
            }
        }
    }
    Ok(())
}

fn merge_alias(left: Option<AliasTarget>, right: Option<AliasTarget>) -> Option<AliasTarget> {
    match (left, right) {
        (None, right) => right,
        (left, None) => left,
        (Some(left), Some(right)) => {
            let left_key = (left.kind.priority(), core::cmp::Reverse(&left.canonical));
            let right_key = (right.kind.priority(), core::cmp::Reverse(&right.canonical));
            Some(if right_key > left_key { right } else { left })
        }
    }
}

impl ReadingCandidates {
    /// Validates and canonicalizes an explicitly supplied candidate set.
    pub fn new(mut candidates: Vec<ReadingCandidate>) -> Result<Self, CandidateError> {
        if candidates.is_empty() {
            return Err(CandidateError::EmptySet);
        }
        validate_alias_consistency(&candidates)?;
        if candidates.iter().any(|c| {
            c.reading.is_empty()
                || c.suffix_reading.as_ref().is_some_and(String::is_empty)
                || c.alias
                    .as_ref()
                    .is_some_and(|alias| alias.canonical.is_empty())
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
            earlier.alias = merge_alias(earlier.alias.take(), later.alias.take());
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
            alias: None,
        }])
    }

    /// Returns the full canonical set without permitting invariant changes.
    pub fn as_slice(&self) -> &[ReadingCandidate] {
        &self.0
    }

    /// Returns whether this set uses the legacy scalar binary representation.
    pub fn is_scalar(&self) -> bool {
        self.0.len() == 1 && self.0[0].suffix_reading.is_none() && self.0[0].alias.is_none()
    }

    /// Encodes a candidate payload shared by the FST and CDB backends.
    ///
    /// Sets without alias metadata use the v2 layout.  Any alias forces the
    /// v3 layout, which adds a kind byte and a canonical spelling per
    /// candidate.  Scalar sets use the legacy representation instead.  Empty
    /// legacy strings are rejected, including `Some("")`, which differs from
    /// no suffix.
    pub fn encode_payload(&self) -> Result<Vec<u8>, CandidateError> {
        if self.is_scalar() {
            return Err(CandidateError::Malformed);
        }
        if self.0.iter().any(|c| {
            c.reading.is_empty()
                || c.suffix_reading.as_ref().is_some_and(String::is_empty)
                || c.alias
                    .as_ref()
                    .is_some_and(|alias| alias.canonical.is_empty())
        }) {
            return Err(CandidateError::EmptyReading);
        }
        if self.0.iter().any(|candidate| candidate.alias.is_some()) {
            self.encode_v3_payload()
        } else {
            self.encode_v2_payload()
        }
    }

    fn encode_v2_payload(&self) -> Result<Vec<u8>, CandidateError> {
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

    fn encode_v3_payload(&self) -> Result<Vec<u8>, CandidateError> {
        let mut output = Vec::new();
        output.extend_from_slice(V3_MAGIC);
        output.extend_from_slice(
            &u16::try_from(self.0.len())
                .map_err(|_| CandidateError::TooLong)?
                .to_le_bytes(),
        );
        for candidate in &self.0 {
            let suffix = candidate.suffix_reading.as_deref().unwrap_or("");
            let canonical = candidate
                .alias
                .as_ref()
                .map_or("", |alias| alias.canonical.as_str());
            if output
                .len()
                .saturating_add(12)
                .saturating_add(candidate.reading.len())
                .saturating_add(suffix.len())
                .saturating_add(canonical.len())
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
            output.push(
                candidate
                    .alias
                    .as_ref()
                    .map_or(0, |alias| alias.kind.encode()),
            );
            output.extend_from_slice(
                &u16::try_from(canonical.len())
                    .map_err(|_| CandidateError::TooLong)?
                    .to_le_bytes(),
            );
            output.extend_from_slice(candidate.reading.as_bytes());
            output.extend_from_slice(suffix.as_bytes());
            output.extend_from_slice(canonical.as_bytes());
        }
        u16::try_from(output.len()).map_err(|_| CandidateError::TooLong)?;
        Ok(output)
    }

    /// Decodes a bounded candidate payload, rejecting invalid or noncanonical sets.
    ///
    /// Callers must validate their file version and outer record framing first.
    /// Both the v2 and v3 payload layouts are accepted.
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
        if bytes.starts_with(V3_MAGIC) {
            bytes = &bytes[V3_MAGIC.len()..];
            let count = u16::from_le_bytes(take(&mut bytes, 2)?.try_into().unwrap()) as usize;
            if count == 0 || count > bytes.len() / 12 {
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
                let suffix_len =
                    u16::from_le_bytes(take(&mut bytes, 2)?.try_into().unwrap()) as usize;
                let alias_kind = take(&mut bytes, 1)?[0];
                let canonical_len =
                    u16::from_le_bytes(take(&mut bytes, 2)?.try_into().unwrap()) as usize;
                let reading = core::str::from_utf8(take(&mut bytes, len)?)
                    .map_err(|_| CandidateError::Malformed)?
                    .into();
                let suffix = core::str::from_utf8(take(&mut bytes, suffix_len)?)
                    .map_err(|_| CandidateError::Malformed)?;
                let canonical = core::str::from_utf8(take(&mut bytes, canonical_len)?)
                    .map_err(|_| CandidateError::Malformed)?;
                let alias = if alias_kind == 0 {
                    if canonical_len != 0 {
                        return Err(CandidateError::Malformed);
                    }
                    None
                } else {
                    if canonical_len == 0 {
                        return Err(CandidateError::Malformed);
                    }
                    Some(AliasTarget {
                        canonical: canonical.into(),
                        kind: AliasKind::decode(alias_kind).ok_or(CandidateError::Malformed)?,
                    })
                };
                candidates.push(ReadingCandidate {
                    reading,
                    suffix_reading: (suffix_len != 0).then(|| suffix.into()),
                    mark: MatchMark {
                        require_hanja: mark & 1 != 0,
                        require_hangul: mark & 2 != 0,
                    },
                    preference,
                    alias,
                });
            }
            if !bytes.is_empty() {
                return Err(CandidateError::Malformed);
            }
            let canonical = Self::new(candidates.clone())?;
            if canonical.0 != candidates || canonical.is_scalar() {
                return Err(CandidateError::Malformed);
            }
            return Ok(canonical);
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
                alias: None,
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
/// This selector uses no text context. The engine may choose a different allowed
/// candidate from following particle evidence, before applying positional spelling.
pub fn select_default_reading(candidates: &ReadingCandidates) -> &ReadingCandidate {
    &candidates.0[0]
}

type LexemeReadings = alloc::collections::BTreeMap<String, (AliasKind, String)>;

/// Resolves derived spellings to the canonical spelling of the same word.
///
/// Only `Sibling` and `Lexical` aliases contribute identity.  A `Recognition`
/// alias is a reading hint, not an equivalence claim, so it is absent from the
/// index and falls back to its own surface spelling.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LexemeIndex {
    by_hanja: alloc::collections::BTreeMap<String, LexemeReadings>,
}

impl LexemeIndex {
    /// Builds an index from a dictionary's enumerable candidate records.
    ///
    /// Returns `None` when the dictionary cannot enumerate its entries.  Such
    /// dictionaries retain spelling-based identity.  Prefer
    /// [`HanjaDictionary::lexemes`] for backends that cache the index.
    pub fn from_dictionary<D>(dictionary: &D) -> Option<Self>
    where
        D: HanjaDictionary + ?Sized,
    {
        let mut by_hanja = alloc::collections::BTreeMap::<String, LexemeReadings>::new();
        for record in dictionary.candidate_entries()? {
            for candidate in record.candidates.as_slice() {
                let Some(alias) = &candidate.alias else {
                    continue;
                };
                let readings = by_hanja.entry(record.hanja.clone()).or_default();
                readings.insert(
                    candidate.reading.clone(),
                    (alias.kind, alias.canonical.clone()),
                );
                if let Some(suffix) = &candidate.suffix_reading {
                    readings.insert(suffix.clone(), (alias.kind, alias.canonical.clone()));
                }
            }
        }
        Some(Self { by_hanja })
    }

    /// Returns the canonical spelling for a matched key and reading.
    ///
    /// Only reviewed and sibling aliases claim lexical identity, so a
    /// recognition alias reports the matched key.  Falls back to the matched
    /// key when the pair has no identity relation.
    pub fn canonical<'a>(&'a self, hanja: &'a str, reading: &str) -> &'a str {
        self.by_hanja
            .get(hanja)
            .and_then(|readings| readings.get(reading))
            .filter(|(kind, _)| matches!(kind, AliasKind::Sibling | AliasKind::Lexical))
            .map_or(hanja, |(_, canonical)| canonical.as_str())
    }

    /// Returns the spelling that homophone marking treats as this word.
    ///
    /// Every alias kind shares its canonical spelling here.  A recognition
    /// alias does not claim lexical identity, but it is still the same word for
    /// homophone purposes, so a canonical spelling is not glossed merely
    /// because a derived alias exists for it.
    pub fn homophone_canonical<'a>(&'a self, hanja: &'a str, reading: &str) -> &'a str {
        self.by_hanja
            .get(hanja)
            .and_then(|readings| readings.get(reading))
            .map_or(hanja, |(_, canonical)| canonical.as_str())
    }

    /// Returns whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.by_hanja.is_empty()
    }

    /// Encodes the index as tab-separated lines for dictionary metadata.
    ///
    /// Each line is `hanja<TAB>reading<TAB>kind<TAB>canonical`.
    pub fn encode(&self) -> String {
        let mut output = String::new();
        for (hanja, readings) in &self.by_hanja {
            for (reading, (kind, canonical)) in readings {
                output.push_str(hanja);
                output.push('\t');
                output.push_str(reading);
                output.push('\t');
                output.push_str(match kind {
                    AliasKind::Sibling => "sibling",
                    AliasKind::Lexical => "lexical",
                    AliasKind::Recognition => "recognition",
                });
                output.push('\t');
                output.push_str(canonical);
                output.push('\n');
            }
        }
        output
    }

    /// Decodes the tab-separated metadata representation.
    pub fn decode(text: &str) -> Self {
        let mut by_hanja = alloc::collections::BTreeMap::<String, LexemeReadings>::new();
        for line in text.lines() {
            let mut fields = line.split('\t');
            let (Some(hanja), Some(reading), Some(kind), Some(canonical)) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let kind = match kind {
                "sibling" => AliasKind::Sibling,
                "lexical" => AliasKind::Lexical,
                "recognition" => AliasKind::Recognition,
                _ => continue,
            };
            if hanja.is_empty() || reading.is_empty() || canonical.is_empty() {
                continue;
            }
            by_hanja
                .entry(hanja.to_string())
                .or_default()
                .insert(reading.to_string(), (kind, canonical.to_string()));
        }
        Self { by_hanja }
    }

    pub(crate) fn set_key(&mut self, hanja: &str, candidates: &ReadingCandidates) {
        self.by_hanja.remove(hanja);
        for candidate in candidates.as_slice() {
            let Some(alias) = &candidate.alias else {
                continue;
            };
            let readings = self.by_hanja.entry(hanja.to_string()).or_default();
            readings.insert(
                candidate.reading.clone(),
                (alias.kind, alias.canonical.clone()),
            );
            if let Some(suffix) = &candidate.suffix_reading {
                readings.insert(suffix.clone(), (alias.kind, alias.canonical.clone()));
            }
        }
        if self
            .by_hanja
            .get(hanja)
            .is_some_and(alloc::collections::BTreeMap::is_empty)
        {
            self.by_hanja.remove(hanja);
        }
    }

    pub(crate) fn insert(&mut self, hanja: &str, reading: &str, kind: AliasKind, canonical: &str) {
        self.by_hanja
            .entry(hanja.to_string())
            .or_default()
            .insert(reading.to_string(), (kind, canonical.to_string()));
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &str, AliasKind, &str)> {
        self.by_hanja.iter().flat_map(|(hanja, readings)| {
            readings.iter().map(move |(reading, (kind, canonical))| {
                (hanja.as_str(), reading.as_str(), *kind, canonical.as_str())
            })
        })
    }

    /// Merges another index into this one, preferring this index's entries.
    pub fn merge(&mut self, other: &Self) {
        for (hanja, readings) in &other.by_hanja {
            let target = self.by_hanja.entry(hanja.clone()).or_default();
            for (reading, value) in readings {
                target
                    .entry(reading.clone())
                    .or_insert_with(|| value.clone());
            }
        }
    }
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
    /// Returns the lexical spelling used for identity and policy.
    ///
    /// Reviewed and sibling aliases return their canonical spelling.  Plain
    /// entries and recognition aliases return the stored key.
    pub fn lexeme(&self) -> &str {
        let selected = select_default_reading(&self.candidates);
        selected.alias.as_ref().map_or(&self.hanja, |alias| {
            if matches!(alias.kind, AliasKind::Sibling | AliasKind::Lexical) {
                &alias.canonical
            } else {
                &self.hanja
            }
        })
    }

    /// Returns the spelling homophone marking attributes to this record.
    ///
    /// Unlike [`CandidateRecord::lexeme`], every alias kind reports its
    /// canonical spelling, because two spellings of one word are never
    /// homophones.
    pub fn homophone_key(&self) -> &str {
        select_default_reading(&self.candidates)
            .alias
            .as_ref()
            .map_or(&self.hanja, |alias| &alias.canonical)
    }

    /// Tests a collision against the selected initial and suffix forms only.
    pub fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        let selected = select_default_reading(&self.candidates);
        self.homophone_key() != hanja
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
