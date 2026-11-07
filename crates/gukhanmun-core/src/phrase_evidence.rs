// Gukhanmun: Validated source phrase constraints carried by dictionary containers.
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

use crate::{CandidateRecord, WordId};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

/// Maximum encoded critical metadata bytes accepted before deserialization.
pub const MAX_EVIDENCE_METADATA_BYTES: usize = 128 * 1024 * 1024;

/// Maximum source rules in one validated phrase evidence collection.
pub const MAX_PHRASE_RULES: usize = 65_536;
/// Maximum chunks in one source phrase.
pub const MAX_PHRASE_CHUNKS: usize = 16;
/// Maximum Unicode scalars consumed by a phrase including optional separators.
pub const MAX_PHRASE_SPAN_CHARS: usize = 128;
/// Maximum concurrent phrase matches before runtime evidence must be withheld.
pub const MAX_PHRASE_MATCHES: usize = 256;
/// Maximum alternatives, candidate links, or terminal extensions per component.
pub const MAX_PHRASE_ALTERNATIVES: usize = 32;
/// Maximum source witnesses on a rule or relation.
pub const MAX_PHRASE_WITNESSES: usize = 256;

/// An exact source-local candidate anchor; it does not name a promoted output form.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct PhraseCandidateLink {
    /// Original word-initial candidate reading.
    pub reading: String,
    /// Original positional reading, preserving explicit absence.
    pub suffix_reading: Option<String>,
}
impl PhraseCandidateLink {
    /// Identifies an original candidate using its complete positional forms.
    pub fn new(reading: String, suffix_reading: Option<String>) -> Self {
        Self {
            reading,
            suffix_reading,
        }
    }
}

/// A complete source spelling and its permitted source-local candidate anchors.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct PhraseSpelling {
    /// Exact dictionary spelling; no substring or fallback matching is authorized.
    pub key: String,
    /// Permitted anchors. Empty means this spelling cannot match dictionary edges.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "deserialize_alternatives")
    )]
    pub candidates: Vec<PhraseCandidateLink>,
    /// Attested sibling donor records; required for non-original chunk spellings.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_witnesses"))]
    pub sibling_witnesses: Vec<WordId>,
}
impl PhraseSpelling {
    /// Creates an original spelling with exact candidate links.
    pub fn new(key: String, candidates: Vec<PhraseCandidateLink>) -> Self {
        Self {
            key,
            candidates,
            sibling_witnesses: Vec::new(),
        }
    }
    /// Attaches source records proving a one-step sibling spelling substitution.
    pub fn with_sibling_witnesses(mut self, witnesses: Vec<WordId>) -> Self {
        self.sibling_witnesses = witnesses;
        self
    }
}

/// One caret-delimited source chunk; its complete hangul reading may match literal text.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct PhraseChunk {
    /// Complete attested hangul reading, never character-wise mixed with hanja.
    pub reading: String,
    /// Original spelling first, followed by independently witnessed alternatives.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "deserialize_alternatives")
    )]
    pub spellings: Vec<PhraseSpelling>,
}
impl PhraseChunk {
    /// Creates a source chunk with its original spelling first.
    pub fn new(reading: String, spellings: Vec<PhraseSpelling>) -> Self {
        Self { reading, spellings }
    }
}

/// A typed original word-to-subheadword relationship, never a sense-ID join.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct SubheadwordWitness {
    /// Source parent word record.
    pub parent: WordId,
    /// Explicitly linked child word record.
    pub child: WordId,
}
impl SubheadwordWitness {
    /// Retains one original typed relationship used by the compiler.
    pub fn new(parent: WordId, child: WordId) -> Self {
        Self { parent, child }
    }
}

/// An atomic replacement for the final chunk licensed by all possible parents.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct TerminalExtension {
    /// Attested literal hangul suffix appended to the final chunk reading.
    pub literal_suffix: String,
    /// Complete child keys and anchors, including the literal suffix.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "deserialize_alternatives")
    )]
    pub spellings: Vec<PhraseSpelling>,
    /// Relationships covering every possible source parent binding.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_witnesses"))]
    pub witnesses: Vec<SubheadwordWitness>,
}
impl TerminalExtension {
    /// Creates a terminal projection after the source compiler establishes agreement.
    pub fn new(
        literal_suffix: String,
        spellings: Vec<PhraseSpelling>,
        witnesses: Vec<SubheadwordWitness>,
    ) -> Self {
        Self {
            literal_suffix,
            spellings,
            witnesses,
        }
    }
}

/// One atomic phrase constraint over the already selected segmentation path.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct PhraseRule {
    /// Original phrase word records establishing the unique aligned reading tuple.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_witnesses"))]
    pub source_ids: Vec<WordId>,
    /// Ordered chunks with zero or one U+0020 permitted at each caret boundary.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_chunks"))]
    pub chunks: Vec<PhraseChunk>,
    /// Optional terminal replacements, each applied atomically with the base rule.
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "deserialize_alternatives")
    )]
    pub terminal_extensions: Vec<TerminalExtension>,
}
impl PhraseRule {
    /// Creates a base phrase; terminal extensions may subsequently be attached.
    pub fn new(source_ids: Vec<WordId>, chunks: Vec<PhraseChunk>) -> Self {
        Self {
            source_ids,
            chunks,
            terminal_extensions: Vec::new(),
        }
    }
    /// Attaches independently verified terminal projections.
    pub fn with_terminal_extensions(mut self, extensions: Vec<TerminalExtension>) -> Self {
        self.terminal_extensions = extensions;
        self
    }
    /// Maximum scalar span including optional spaces and the longest terminal form.
    ///
    /// Exterior boundary witnesses and particle lookahead are runtime additions.
    pub fn max_span_chars(&self) -> usize {
        let mut lengths = self
            .chunks
            .iter()
            .map(|chunk| {
                chunk
                    .spellings
                    .iter()
                    .map(|spelling| spelling.key.chars().count())
                    .chain(core::iter::once(chunk.reading.chars().count()))
                    .max()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        if let (Some(last), Some(chunk)) = (lengths.last_mut(), self.chunks.last()) {
            for extension in &self.terminal_extensions {
                *last = (*last)
                    .max(chunk.reading.chars().count() + extension.literal_suffix.chars().count());
                for spelling in &extension.spellings {
                    *last = (*last).max(spelling.key.chars().count());
                }
            }
        }
        lengths.into_iter().sum::<usize>() + self.chunks.len().saturating_sub(1)
    }
    fn spellings(&self) -> impl Iterator<Item = &PhraseSpelling> {
        self.chunks.iter().flat_map(|chunk| &chunk.spellings).chain(
            self.terminal_extensions
                .iter()
                .flat_map(|extension| &extension.spellings),
        )
    }
}

/// Versioned phrase evidence embedded as critical FST/CDB metadata.
///
/// This collection belongs to its supplying dictionary leaf. Identical keys or
/// bytes in another leaf do not grant authority over that leaf's candidates.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct PhraseEvidence {
    /// Transport schema version, currently 1.
    pub schema_version: u32,
    /// Strict phrase policy version, currently 1.
    pub policy_version: u32,
    /// Complete rules, including conflicts that runtime must intersect locally.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_rules"))]
    pub rules: Vec<PhraseRule>,
}

/// Malformed or stale source phrase evidence, rejected before runtime use.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PhraseEvidenceError {
    /// Unknown schemas and policies must never be silently ignored.
    #[error("unsupported phrase evidence schema or policy")]
    UnsupportedVersion,
    /// A collection exceeds a host resource limit.
    #[error("phrase evidence exceeds a host resource limit")]
    LimitExceeded,
    /// Source alignment, witness, or candidate structure is invalid.
    #[error("invalid phrase evidence structure: {0}")]
    InvalidStructure(&'static str),
    /// A linked exact dictionary key or positional candidate is absent.
    #[error("phrase evidence references an absent candidate under key {0:?}")]
    UnknownCandidate(String),
}
impl PhraseEvidence {
    /// Creates current-version evidence; validate it against the supplying dictionary.
    pub fn new(rules: Vec<PhraseRule>) -> Self {
        Self {
            schema_version: 1,
            policy_version: 1,
            rules,
        }
    }
    /// Maximum rule span, excluding runtime exterior and particle witnesses.
    pub fn max_span_chars(&self) -> usize {
        self.rules
            .iter()
            .map(PhraseRule::max_span_chars)
            .max()
            .unwrap_or(0)
    }
    /// Exact keys requiring dictionary linkage; literal-only spellings are omitted.
    pub fn candidate_keys(&self) -> BTreeSet<&str> {
        self.rules
            .iter()
            .flat_map(PhraseRule::spellings)
            .filter(|s| !s.candidates.is_empty())
            .map(|s| s.key.as_str())
            .collect()
    }
    /// Checks typed structure and host limits, without authenticating raw source semantics.
    ///
    /// The offline compiler must establish source category exclusions, all-parent
    /// agreement, spelling collision checks, and one-step sibling provenance.
    pub fn validate(&self) -> Result<(), PhraseEvidenceError> {
        use PhraseEvidenceError::*;
        if self.schema_version != 1 || self.policy_version != 1 {
            return Err(UnsupportedVersion);
        }
        bounded(self.rules.len(), MAX_PHRASE_RULES)?;
        let mut seen = BTreeSet::new();
        for rule in &self.rules {
            if !seen.insert(rule) {
                return Err(InvalidStructure("duplicate rule"));
            }
            nonempty_unique(&rule.source_ids, MAX_PHRASE_WITNESSES)?;
            bounded(rule.chunks.len(), MAX_PHRASE_CHUNKS)?;
            if rule.chunks.len() < 2 {
                return Err(InvalidStructure("phrase needs at least two chunks"));
            }
            for chunk in &rule.chunks {
                if !hangul(&chunk.reading) {
                    return Err(InvalidStructure("chunk reading is not syllabic hangul"));
                }
                validate_spellings(&chunk.spellings, &chunk.reading, None)?;
            }
            bounded(rule.terminal_extensions.len(), MAX_PHRASE_ALTERNATIVES)?;
            let terminal = rule.chunks.last().expect("checked chunk count");
            let mut extensions = BTreeSet::new();
            for extension in &rule.terminal_extensions {
                if !extensions.insert(&extension.literal_suffix) {
                    return Err(InvalidStructure("duplicate terminal suffix"));
                }
                if !hangul(&extension.literal_suffix) {
                    return Err(InvalidStructure("terminal suffix is not hangul"));
                }
                nonempty_unique(&extension.witnesses, MAX_PHRASE_WITNESSES)?;
                let reading = alloc::format!("{}{}", terminal.reading, extension.literal_suffix);
                validate_spellings(
                    &extension.spellings,
                    &reading,
                    Some((&terminal.spellings, &extension.literal_suffix)),
                )?;
            }
            bounded(rule.max_span_chars(), MAX_PHRASE_SPAN_CHARS)?;
        }
        Ok(())
    }
    /// Checks every exact positional candidate link after final dictionary normalization.
    pub fn validate_records(
        &self,
        records: impl IntoIterator<Item = CandidateRecord>,
    ) -> Result<(), PhraseEvidenceError> {
        self.validate()?;
        let mut links: BTreeMap<&str, BTreeSet<&PhraseCandidateLink>> = BTreeMap::new();
        for spelling in self.rules.iter().flat_map(PhraseRule::spellings) {
            for candidate in &spelling.candidates {
                links.entry(&spelling.key).or_default().insert(candidate);
            }
        }
        for record in records {
            if let Some(candidates) = links.remove(record.hanja.as_str()) {
                for candidate in candidates {
                    if !record.candidates.as_slice().iter().any(|stored| {
                        stored.reading == candidate.reading
                            && stored.suffix_reading == candidate.suffix_reading
                    }) {
                        return Err(PhraseEvidenceError::UnknownCandidate(record.hanja));
                    }
                }
            }
        }
        if let Some(key) = links.into_keys().next() {
            return Err(PhraseEvidenceError::UnknownCandidate(key.into()));
        }
        Ok(())
    }
}
fn bounded(len: usize, limit: usize) -> Result<(), PhraseEvidenceError> {
    if len > limit {
        Err(PhraseEvidenceError::LimitExceeded)
    } else {
        Ok(())
    }
}
fn nonempty_unique<T: Ord>(values: &[T], limit: usize) -> Result<(), PhraseEvidenceError> {
    bounded(values.len(), limit)?;
    if values.is_empty() || values.iter().collect::<BTreeSet<_>>().len() != values.len() {
        return Err(PhraseEvidenceError::InvalidStructure(
            "empty or duplicate witnesses",
        ));
    }
    Ok(())
}
fn hangul(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|ch| ('가'..='힣').contains(&ch))
}
fn validate_spellings(
    spellings: &[PhraseSpelling],
    reading: &str,
    terminal: Option<(&[PhraseSpelling], &str)>,
) -> Result<(), PhraseEvidenceError> {
    use PhraseEvidenceError::*;
    bounded(spellings.len(), MAX_PHRASE_ALTERNATIVES)?;
    if spellings.is_empty() {
        return Err(InvalidStructure("missing original spelling"));
    }
    let mut keys = BTreeSet::new();
    for (index, spelling) in spellings.iter().enumerate() {
        if !keys.insert(&spelling.key) {
            return Err(InvalidStructure("duplicate spelling"));
        }
        if spelling.key.chars().count() != reading.chars().count() {
            return Err(InvalidStructure("unaligned spelling"));
        }
        let hanja = if let Some((parents, suffix)) = terminal {
            let Some(base) = spelling.key.strip_suffix(suffix) else {
                return Err(InvalidStructure("unaligned terminal suffix"));
            };
            if !parents.iter().any(|p| p.key == base) {
                return Err(InvalidStructure("terminal has no base spelling"));
            }
            base
        } else {
            spelling.key.as_str()
        };
        if hanja.is_empty() || !hanja.chars().all(crate::is_hanja) {
            return Err(InvalidStructure("spelling is not hanja"));
        }
        if index > 0 || !spelling.sibling_witnesses.is_empty() {
            nonempty_unique(&spelling.sibling_witnesses, MAX_PHRASE_WITNESSES)?;
        }
        bounded(spelling.candidates.len(), MAX_PHRASE_ALTERNATIVES)?;
        let mut candidates = BTreeSet::new();
        for candidate in &spelling.candidates {
            bounded(candidate.reading.chars().count(), MAX_PHRASE_SPAN_CHARS)?;
            if let Some(suffix) = &candidate.suffix_reading {
                bounded(suffix.chars().count(), MAX_PHRASE_SPAN_CHARS)?;
            }
            if !hangul(&candidate.reading)
                || !candidates.insert(candidate)
                || (candidate.reading != reading
                    && candidate.suffix_reading.as_deref() != Some(reading))
                || candidate
                    .suffix_reading
                    .as_ref()
                    .is_some_and(|s| !hangul(s))
            {
                return Err(InvalidStructure("invalid candidate link"));
            }
        }
    }
    Ok(())
}

#[cfg(feature = "serde")]
fn bounded_vec<'de, D, T, const LIMIT: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    struct Visitor<T, const LIMIT: usize>(core::marker::PhantomData<T>);
    impl<'de, T: serde::Deserialize<'de>, const LIMIT: usize> serde::de::Visitor<'de>
        for Visitor<T, LIMIT>
    {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("a bounded phrase evidence sequence")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
            use serde::de::Error as _;
            if seq.size_hint().is_some_and(|size| size > LIMIT) {
                return Err(A::Error::custom(
                    "phrase evidence collection exceeds host limit",
                ));
            }
            let mut values = Vec::new();
            while let Some(value) = seq.next_element()? {
                if values.len() == LIMIT {
                    return Err(A::Error::custom(
                        "phrase evidence collection exceeds host limit",
                    ));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Visitor::<T, LIMIT>(core::marker::PhantomData))
}
#[cfg(feature = "serde")]
fn deserialize_rules<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    bounded_vec::<D, T, MAX_PHRASE_RULES>(d)
}
#[cfg(feature = "serde")]
fn deserialize_chunks<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    bounded_vec::<D, T, MAX_PHRASE_CHUNKS>(d)
}
#[cfg(feature = "serde")]
fn deserialize_alternatives<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    bounded_vec::<D, T, MAX_PHRASE_ALTERNATIVES>(d)
}
#[cfg(feature = "serde")]
fn deserialize_witnesses<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    bounded_vec::<D, T, MAX_PHRASE_WITNESSES>(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HanjaDictionary, MapDictionary};
    use alloc::vec;

    fn evidence() -> PhraseEvidence {
        PhraseEvidence::new(vec![PhraseRule::new(
            vec![WordId(1)],
            vec![
                PhraseChunk::new(
                    "무조건".into(),
                    vec![PhraseSpelling::new("無條件".into(), vec![])],
                ),
                PhraseChunk::new(
                    "항복".into(),
                    vec![PhraseSpelling::new(
                        "降伏".into(),
                        vec![PhraseCandidateLink::new("항복".into(), None)],
                    )],
                ),
            ],
        )])
    }
    #[test]
    fn literal_only_chunks_need_no_dictionary_links() {
        let mut dictionary = MapDictionary::new();
        dictionary.insert("降伏", "항복");
        let evidence = evidence();
        assert_eq!(evidence.max_span_chars(), 6);
        evidence
            .validate_records(dictionary.candidate_entries().unwrap())
            .unwrap();
        dictionary.set_phrase_evidence(evidence.clone()).unwrap();
        assert_eq!(dictionary.phrase_evidence(), Some(&evidence));
        let forwarded: alloc::boxed::Box<dyn HanjaDictionary> =
            alloc::boxed::Box::new(dictionary.clone());
        assert_eq!(forwarded.phrase_evidence(), Some(&evidence));
        let forwarded = alloc::sync::Arc::new(dictionary.clone());
        assert_eq!(
            HanjaDictionary::phrase_evidence(&&forwarded),
            Some(&evidence)
        );
        let mut invalid = evidence;
        invalid.rules[0].chunks[1].spellings[0].key = "降服".into();
        assert!(dictionary.set_phrase_evidence(invalid).is_err());
        assert!(dictionary.phrase_evidence().is_some());
        dictionary.insert("無關", "무관");
        assert!(dictionary.phrase_evidence().is_none());
    }
    #[test]
    fn stale_positional_links_are_rejected() {
        let mut dictionary = MapDictionary::new();
        dictionary.insert("降伏", "항복");
        let mut evidence = evidence();
        evidence.rules[0].chunks[1].spellings[0].candidates[0].suffix_reading = Some("강복".into());
        assert!(matches!(
            evidence.validate_records(dictionary.candidate_entries().unwrap()),
            Err(PhraseEvidenceError::UnknownCandidate(_))
        ));
    }
    #[test]
    fn terminal_projections_require_source_witness_and_exact_alignment() {
        let mut evidence = evidence();
        let mut projection = TerminalExtension::new(
            "하다".into(),
            vec![PhraseSpelling::new(
                "降伏하다".into(),
                vec![PhraseCandidateLink::new("항복하다".into(), None)],
            )],
            vec![SubheadwordWitness::new(WordId(2), WordId(3))],
        );
        evidence.rules[0]
            .terminal_extensions
            .push(projection.clone());
        assert_eq!(evidence.max_span_chars(), 8);
        evidence.validate().unwrap();
        projection.spellings[0].key = "降服하다".into();
        evidence.rules[0].terminal_extensions[0] = projection;
        assert!(evidence.validate().is_err());
        evidence.rules[0].terminal_extensions[0].spellings[0].key = "降伏하다".into();
        evidence.rules[0].terminal_extensions[0].witnesses.clear();
        assert!(evidence.validate().is_err());
    }
    #[test]
    fn unproved_alternatives_and_future_policy_fail_closed() {
        let mut evidence = evidence();
        evidence.rules[0].chunks[1]
            .spellings
            .push(PhraseSpelling::new(
                "降服".into(),
                vec![PhraseCandidateLink::new("항복".into(), None)],
            ));
        assert!(evidence.validate().is_err());
        evidence.rules[0].chunks[1].spellings[1]
            .sibling_witnesses
            .push(WordId(2));
        evidence.validate().unwrap();
        evidence.policy_version = 2;
        assert_eq!(
            evidence.validate(),
            Err(PhraseEvidenceError::UnsupportedVersion)
        );
    }
    #[cfg(feature = "serde")]
    #[test]
    fn transport_rejects_oversized_sequences_and_unknown_fields() {
        let mut value = serde_json::to_value(evidence()).unwrap();
        let chunk = value["rules"][0]["chunks"][0].clone();
        value["rules"][0]["chunks"] = serde_json::Value::Array(vec![chunk; MAX_PHRASE_CHUNKS + 1]);
        assert!(serde_json::from_value::<PhraseEvidence>(value).is_err());
        let mut value = serde_json::to_value(evidence()).unwrap();
        value["future_rule_semantics"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<PhraseEvidence>(value).is_err());
    }
}
