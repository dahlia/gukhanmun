// Gukhanmun: Typed source evidence for dictionary candidates and authorized forms.
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

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use crate::CandidateRecord;

/// How one dictionary participates in source-aware composition.
///
/// Roles describe sources independently of reading preferences within a key.
/// Attaching evidence does not itself change legacy dictionary lookup.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SourceRole {
    /// An authored dictionary that takes precedence over cooperative sources.
    Override,
    /// A reference dictionary carrying lexical readings and typed relations.
    Reference,
    /// A spelling source with independent loanword readings.
    Loanword,
}

/// A source dictionary's word identifier, distinct from its sense identifiers.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WordId(pub u64);

/// A source dictionary's sense identifier, distinct from its word identifiers.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SenseId(pub u64);

/// A typed endpoint of a relation in the source dictionary.
///
/// Equal numeric identifiers in the two variants name different records.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SourceReference {
    /// A relation to a complete word record and its possible senses.
    Word(WordId),
    /// A relation restricted to one sense.
    Sense(SenseId),
}

/// The source's attested origin classification for one contribution.
///
/// `Mixed` and `Unknown` must not be interpreted as foreign authorization.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum WordOrigin {
    /// A substantive native or Sino-Korean record.
    Native,
    /// An explicitly foreign record.
    Foreign,
    /// Both native and foreign evidence contributes to this record.
    Mixed,
    /// The source supplies insufficient origin evidence.
    Unknown,
}

/// A complete output form, preserving phrase boundaries and positional spelling.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct ReadingForm {
    /// The original word-initial output spelling, including attested spaces.
    pub reading: String,
    /// An explicitly attested non-word-initial spelling, when present.
    pub suffix_reading: Option<String>,
}

/// A possible raw record and sense contributing to one stored candidate.
///
/// All possible contributions must be retained. A compiler may authorize a
/// replacement only when every contribution permits that complete form.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct SourceContribution {
    /// The contributing source word record.
    pub word_id: WordId,
    /// Its contributing sense, or `None` when the source has no sense record.
    pub sense_id: Option<SenseId>,
    /// Origin established from the source, without inferring it from the key.
    pub origin: WordOrigin,
}

/// One directed, typed source relation used to authorize an output form.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct RelationWitness {
    /// The source endpoint of the original directed relation.
    pub source: SourceReference,
    /// The explicitly typed target endpoint of that relation.
    pub target: SourceReference,
}

/// The original relation that authorizes an alternative output spelling.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum AuthorizationKind {
    /// A direct outgoing synonym of a foreign contribution.
    Synonym,
    /// The terminal output form of an explicit formal redirect.
    Redirect,
}

/// An offline-compiled form authorized by all contributing source senses.
///
/// This grants an output form, not lexical alias identity. Witnesses remain
/// inspectable even when their targets are not admitted as dictionary keys.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct AuthorizedForm {
    /// The complete attested output form.
    pub form: ReadingForm,
    /// Whether a synonym or formal redirect supplies this authorization.
    pub kind: AuthorizationKind,
    /// The typed relations establishing this form for the contributions.
    pub witnesses: Vec<RelationWitness>,
}

/// A conservative relation-compilation failure retained for inspection.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum EvidenceDiagnosticKind {
    /// A typed relation target is absent from the full raw source index.
    MissingTarget,
    /// Possible target records disagree about their terminal output form.
    AmbiguousTarget,
    /// A formal redirect revisits a typed endpoint.
    Cycle,
    /// Compilation exceeded an explicit generation budget.
    BudgetExceeded,
    /// The source relation is not one of the supported typed relations.
    UnsupportedRelation,
    /// Source origin cannot justify a foreign synonym authorization.
    UnknownOrigin,
}

/// A structured explanation of source evidence that could not be compiled.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct EvidenceDiagnostic {
    /// The machine-readable reason authorization was withheld.
    pub kind: EvidenceDiagnosticKind,
    /// The typed source endpoint, when the failure identifies one.
    pub source: Option<SourceReference>,
    /// Additional source-specific explanation for audits and tooling.
    pub message: String,
}

/// Source evidence linked to an exact stored reading and positional form.
///
/// This is separate from [`crate::ReadingCandidates`], whose canonical ordering,
/// marks and alias behavior are unchanged. Omitted candidate evidence is unknown.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct CandidateEvidence {
    /// The exact word-initial reading of the stored candidate.
    pub reading: String,
    /// The exact positional form of the stored candidate, including absence.
    pub suffix_reading: Option<String>,
    /// Every possible source contribution to this candidate.
    pub contributors: Vec<SourceContribution>,
    /// Forms whose authorization was intersected over all contributions.
    pub authorized_forms: Vec<AuthorizedForm>,
    /// Unresolved source relations, without silently granting partial authority.
    pub diagnostics: Vec<EvidenceDiagnostic>,
}

/// Versioned source evidence transported alongside unchanged candidate payloads.
///
/// The entry map and each candidate's `(reading, suffix_reading)` identify a
/// stable stored candidate. Missing entries or candidates explicitly mean unknown
/// evidence. Call [`Self::validate_records`] after decoding and before exposing
/// evidence to conversion. Serde decoding alone does not validate these links.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct DictionaryEvidence {
    /// The evidence schema version. Only version 1 is currently supported.
    pub schema_version: u32,
    /// The declared source role; explicit application registration may override it.
    pub role: SourceRole,
    /// Partial candidate evidence indexed by exact stored dictionary key.
    pub entries: BTreeMap<String, Vec<CandidateEvidence>>,
}

/// Invalid evidence structure or a dangling link to a stored candidate.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum EvidenceError {
    /// The schema version is unsupported and must not be silently ignored.
    #[error("unsupported dictionary evidence schema version {0}")]
    UnsupportedSchemaVersion(u32),
    /// An evidence entry has an empty dictionary key or candidate list.
    #[error("dictionary evidence keys and candidate lists must not be empty")]
    EmptyEntry,
    /// A reading or explicitly supplied positional reading is empty.
    #[error("evidence readings and explicit positional forms must not be empty")]
    EmptyReading,
    /// A candidate contains no possible source contributions.
    #[error("candidate evidence must retain its contributing source records")]
    EmptyContributors,
    /// An authorized form has no original relation witnesses.
    #[error("authorized evidence forms must retain relation witnesses")]
    EmptyWitnesses,
    /// A candidate, contribution, or authorized form is repeated ambiguously.
    #[error("dictionary evidence contains a duplicate candidate, contribution, or authorization")]
    Duplicate,
    /// An evidence key does not exist in the stored candidate records.
    #[error("dictionary evidence references absent key {0:?}")]
    UnknownKey(String),
    /// An evidence reading and positional form do not identify a stored candidate.
    #[error("dictionary evidence references an absent candidate under key {0:?}")]
    UnknownCandidate(String),
}

impl DictionaryEvidence {
    /// Checks the evidence schema and structural invariants without a dictionary.
    ///
    /// This validates transport structure, not the original source relation
    /// semantics. Source compilers must independently check typed targets,
    /// origin restrictions, complete contribution intersections and budgets.
    pub fn validate(&self) -> Result<(), EvidenceError> {
        if self.schema_version != 1 {
            return Err(EvidenceError::UnsupportedSchemaVersion(self.schema_version));
        }
        for (key, candidates) in &self.entries {
            if key.is_empty() || candidates.is_empty() {
                return Err(EvidenceError::EmptyEntry);
            }
            let mut seen_candidates = BTreeSet::new();
            for candidate in candidates {
                validate_form(&candidate.reading, candidate.suffix_reading.as_deref())?;
                if !seen_candidates.insert((&candidate.reading, &candidate.suffix_reading)) {
                    return Err(EvidenceError::Duplicate);
                }
                if candidate.contributors.is_empty() {
                    return Err(EvidenceError::EmptyContributors);
                }
                let mut seen_contributors = BTreeSet::new();
                for contribution in &candidate.contributors {
                    if !seen_contributors.insert((contribution.word_id, contribution.sense_id)) {
                        return Err(EvidenceError::Duplicate);
                    }
                }
                let mut seen_forms = BTreeSet::new();
                for authorized in &candidate.authorized_forms {
                    validate_form(
                        &authorized.form.reading,
                        authorized.form.suffix_reading.as_deref(),
                    )?;
                    if authorized.witnesses.is_empty() {
                        return Err(EvidenceError::EmptyWitnesses);
                    }
                    if !seen_forms.insert((&authorized.form, authorized.kind)) {
                        return Err(EvidenceError::Duplicate);
                    }
                }
            }
        }
        Ok(())
    }

    /// Validates every evidence link against the actual stored candidate records.
    ///
    /// Partial unknown evidence is accepted, while absent keys and mismatched
    /// readings or positional forms fail closed. Candidate preference, marks
    /// and aliases are neither changed nor inferred by this check.
    pub fn validate_records(
        &self,
        records: impl IntoIterator<Item = CandidateRecord>,
    ) -> Result<(), EvidenceError> {
        self.validate()?;
        let mut remaining: BTreeSet<_> = self.entries.keys().collect();
        for record in records {
            let Some(candidates) = self.entries.get(&record.hanja) else {
                continue;
            };
            remaining.remove(&record.hanja);
            for candidate in candidates {
                if !record.candidates.as_slice().iter().any(|stored| {
                    stored.reading == candidate.reading
                        && stored.suffix_reading == candidate.suffix_reading
                }) {
                    return Err(EvidenceError::UnknownCandidate(record.hanja));
                }
            }
        }
        if let Some(key) = remaining.into_iter().next() {
            return Err(EvidenceError::UnknownKey(key.clone()));
        }
        Ok(())
    }
}

fn validate_form(reading: &str, suffix: Option<&str>) -> Result<(), EvidenceError> {
    if reading.is_empty() || suffix.is_some_and(str::is_empty) {
        Err(EvidenceError::EmptyReading)
    } else {
        Ok(())
    }
}

#[cfg(feature = "serde")]
pub use wire::{deserialize_dictionary_evidence, serialize_dictionary_evidence};

#[cfg(feature = "serde")]
mod wire {
    use super::*;
    use serde::ser::{SerializeMap, SerializeTuple};

    type WireContribution = (WordId, Option<SenseId>, WordOrigin);
    type WireCandidate = (
        String,
        Option<String>,
        Vec<WireContribution>,
        Vec<AuthorizedForm>,
        Vec<EvidenceDiagnostic>,
    );

    /// Serializes evidence using the compact critical-container representation.
    ///
    /// This tuple representation omits repeated field names without changing the
    /// separate human-readable JSON schema. Call [`DictionaryEvidence::validate`]
    /// before publishing; serialization does not establish source authority.
    pub fn serialize_dictionary_evidence<S: serde::Serializer>(
        evidence: &DictionaryEvidence,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(3)?;
        tuple.serialize_element(&evidence.schema_version)?;
        tuple.serialize_element(&evidence.role)?;
        tuple.serialize_element(&EntriesRef(&evidence.entries))?;
        tuple.end()
    }

    struct EntriesRef<'a>(&'a BTreeMap<String, Vec<CandidateEvidence>>);
    impl serde::Serialize for EntriesRef<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut map = serializer.serialize_map(Some(self.0.len()))?;
            for (key, candidates) in self.0 {
                map.serialize_entry(key, &CandidatesRef(candidates))?;
            }
            map.end()
        }
    }
    struct CandidatesRef<'a>(&'a [CandidateEvidence]);
    impl serde::Serialize for CandidatesRef<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.collect_seq(self.0.iter().map(CandidateRef))
        }
    }
    struct CandidateRef<'a>(&'a CandidateEvidence);
    impl serde::Serialize for CandidateRef<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut tuple = serializer.serialize_tuple(5)?;
            tuple.serialize_element(&self.0.reading)?;
            tuple.serialize_element(&self.0.suffix_reading)?;
            tuple.serialize_element(&ContributorsRef(&self.0.contributors))?;
            tuple.serialize_element(&self.0.authorized_forms)?;
            tuple.serialize_element(&self.0.diagnostics)?;
            tuple.end()
        }
    }
    struct ContributorsRef<'a>(&'a [SourceContribution]);
    impl serde::Serialize for ContributorsRef<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.collect_seq(
                self.0
                    .iter()
                    .map(|item| (item.word_id, item.sense_id, item.origin)),
            )
        }
    }

    struct WireEntries(BTreeMap<String, Vec<WireCandidate>>);
    impl<'de> serde::Deserialize<'de> for WireEntries {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct EntriesVisitor;
            impl<'de> serde::de::Visitor<'de> for EntriesVisitor {
                type Value = WireEntries;
                fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    formatter.write_str("unique source evidence keys")
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> Result<Self::Value, A::Error> {
                    use serde::de::Error as _;
                    let mut entries = BTreeMap::new();
                    while let Some((key, value)) = map.next_entry::<String, Vec<WireCandidate>>()? {
                        if entries.insert(key, value).is_some() {
                            return Err(A::Error::custom("duplicate source evidence key"));
                        }
                    }
                    Ok(WireEntries(entries))
                }
            }
            deserializer.deserialize_map(EntriesVisitor)
        }
    }

    /// Decodes the compact critical-container representation into typed evidence.
    ///
    /// Tuple lengths, enum variants and duplicate map keys are checked while
    /// decoding. Call [`DictionaryEvidence::validate_records`] before exposing
    /// the result: transport decoding alone does not validate source semantics
    /// or the links to canonical dictionary candidates.
    pub fn deserialize_dictionary_evidence<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<DictionaryEvidence, D::Error> {
        let (schema_version, role, WireEntries(entries)): (u32, SourceRole, WireEntries) =
            serde::Deserialize::deserialize(deserializer)?;
        let entries = entries
            .into_iter()
            .map(|(key, candidates)| {
                let candidates = candidates
                    .into_iter()
                    .map(
                        |(reading, suffix_reading, contributors, authorized_forms, diagnostics)| {
                            CandidateEvidence {
                                reading,
                                suffix_reading,
                                contributors: contributors
                                    .into_iter()
                                    .map(|(word_id, sense_id, origin)| SourceContribution {
                                        word_id,
                                        sense_id,
                                        origin,
                                    })
                                    .collect(),
                                authorized_forms,
                                diagnostics,
                            }
                        },
                    )
                    .collect();
                (key, candidates)
            })
            .collect();
        Ok(DictionaryEvidence {
            schema_version,
            role,
            entries,
        })
    }
}
