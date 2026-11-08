// Gukhanmun: Reusable immutable lookup index for validated source phrase evidence.
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

use crate::{PhraseEvidence, PhraseEvidenceError};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(test)]
extern crate std;
#[cfg(test)]
std::thread_local! {
    static VALIDATION_SCANS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct PhraseRuleRef {
    pub(crate) rule: usize,
    pub(crate) extension: Option<usize>,
}

/// Immutable source-local lookup index shared by engines using one dictionary.
///
/// The index contains rule positions, not source ownership. Composite dictionaries
/// qualify those positions with the registered leaf owner at lookup time. Build
/// the index only from validated evidence and keep that evidence unchanged for
/// its lifetime. Dictionary implementations must return an index built from the
/// exact collection exposed by [`crate::HanjaDictionary::phrase_evidence`].
#[derive(Clone, Debug)]
pub struct PhraseIndex {
    prefixes: BTreeMap<String, Vec<PhraseRuleRef>>,
    max_span_chars: usize,
    // Retaining the immutable owner makes pointer identity a validation proof:
    // the allocation cannot be mutated or reused while this index exists.
    shared_evidence: Option<Arc<PhraseEvidence>>,
}

impl PartialEq for PhraseIndex {
    fn eq(&self, other: &Self) -> bool {
        self.prefixes == other.prefixes && self.max_span_chars == other.max_span_chars
    }
}

impl Eq for PhraseIndex {}

impl PhraseIndex {
    /// Validates evidence structure and compiles reusable first-chunk lookup tables.
    ///
    /// Candidate linkage remains the supplying dictionary's responsibility; use
    /// [`PhraseEvidence::validate_records`] before attaching the collection. This
    /// constructor never changes candidate order, rule order, or phrase semantics.
    /// Engines revalidate evidence paired with this borrowed-input index. Use
    /// [`Self::from_shared_evidence`] to reuse validation across engines as well.
    pub fn new(evidence: &PhraseEvidence) -> Result<Self, PhraseEvidenceError> {
        evidence.validate()?;
        let mut prefixes: BTreeMap<String, Vec<PhraseRuleRef>> = BTreeMap::new();
        for (rule_index, rule) in evidence.rules.iter().enumerate() {
            let first = &rule.chunks[0];
            for extension in
                core::iter::once(None).chain((0..rule.terminal_extensions.len()).map(Some))
            {
                let handle = PhraseRuleRef {
                    rule: rule_index,
                    extension,
                };
                for key in first
                    .spellings
                    .iter()
                    .map(|spelling| spelling.key.as_str())
                    .chain(core::iter::once(first.reading.as_str()))
                {
                    prefixes.entry(key.into()).or_default().push(handle);
                }
            }
        }
        for handles in prefixes.values_mut() {
            handles.sort();
            handles.dedup();
        }
        Ok(Self {
            prefixes,
            max_span_chars: evidence.max_span_chars(),
            shared_evidence: None,
        })
    }

    /// Validates and indexes an immutable shared phrase collection without copying it.
    ///
    /// The index retains an [`Arc`] to the collection. Engines borrowing that
    /// exact collection can verify the pairing in constant time without scanning
    /// its rules again. Other collections still require structural validation and
    /// index compatibility checks, even when their contents are equal.
    ///
    /// This validates structure only. The dictionary provider must authenticate
    /// source evidence and call [`PhraseEvidence::validate_records`] to establish
    /// candidate linkage before exposing the collection to the engine.
    pub fn from_shared_evidence(
        evidence: Arc<PhraseEvidence>,
    ) -> Result<Self, PhraseEvidenceError> {
        let mut index = Self::new(&evidence)?;
        index.shared_evidence = Some(evidence);
        Ok(index)
    }

    /// Maximum source scalar span, including optional separators and terminal forms.
    ///
    /// Engines add exterior-boundary and particle lookahead separately.
    pub fn max_span_chars(&self) -> usize {
        self.max_span_chars
    }

    /// Whether the collection contains no matchable phrase rules.
    pub fn is_empty(&self) -> bool {
        self.prefixes.is_empty()
    }

    pub(crate) fn prefixes(&self) -> &BTreeMap<String, Vec<PhraseRuleRef>> {
        &self.prefixes
    }

    // Check the complete derived index without allocating another prefix map or
    // copying the evidence graph. Equal derived indexes are interchangeable:
    // candidate links and source witnesses are read from the current evidence.
    pub(crate) fn validate_evidence(
        &self,
        evidence: &PhraseEvidence,
    ) -> Result<(), PhraseEvidenceError> {
        if self
            .shared_evidence
            .as_ref()
            .is_some_and(|shared| core::ptr::eq(shared.as_ref(), evidence))
        {
            return Ok(());
        }
        #[cfg(test)]
        VALIDATION_SCANS.with(|scans| scans.set(scans.get() + 1));
        evidence.validate()?;
        let mismatch =
            || PhraseEvidenceError::InvalidStructure("phrase index does not match evidence");
        if self.max_span_chars != evidence.max_span_chars() {
            return Err(mismatch());
        }
        let mut expected = 0usize;
        for (rule_index, rule) in evidence.rules.iter().enumerate() {
            let first = &rule.chunks[0];
            for extension in
                core::iter::once(None).chain((0..rule.terminal_extensions.len()).map(Some))
            {
                let handle = PhraseRuleRef {
                    rule: rule_index,
                    extension,
                };
                for key in first
                    .spellings
                    .iter()
                    .map(|spelling| spelling.key.as_str())
                    .chain(core::iter::once(first.reading.as_str()))
                {
                    if self
                        .prefixes
                        .get(key)
                        .is_none_or(|handles| handles.binary_search(&handle).is_err())
                    {
                        return Err(mismatch());
                    }
                    expected = expected.checked_add(1).ok_or_else(mismatch)?;
                }
            }
        }
        // Validation guarantees distinct first spellings, disjoint from the
        // hangul reading. Each expected key/handle pair is therefore unique.
        // Matching the total count rejects extra keys, rules and extensions.
        let actual = self.prefixes.values().map(Vec::len).sum::<usize>();
        if actual != expected {
            return Err(mismatch());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HanjaDictionary, MapDictionary, PhraseCandidateLink, PhraseChunk, PhraseRule,
        PhraseSpelling, SubheadwordWitness, TerminalExtension, WordId,
    };
    use alloc::{boxed::Box, sync::Arc, vec};

    fn evidence() -> PhraseEvidence {
        PhraseEvidence::new(vec![
            PhraseRule::new(
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
            )
            .with_terminal_extensions(vec![TerminalExtension::new(
                "하다".into(),
                vec![PhraseSpelling::new(
                    "降伏하다".into(),
                    vec![PhraseCandidateLink::new("항복하다".into(), None)],
                )],
                vec![SubheadwordWitness::new(WordId(2), WordId(3))],
            )]),
        ])
    }
    #[test]
    fn index_preserves_source_local_rule_and_extension_positions() {
        let evidence = evidence();
        let index = PhraseIndex::new(&evidence).unwrap();
        assert_eq!(index.max_span_chars(), evidence.max_span_chars());
        assert_eq!(
            index.prefixes["無條件"],
            vec![
                PhraseRuleRef {
                    rule: 0,
                    extension: None
                },
                PhraseRuleRef {
                    rule: 0,
                    extension: Some(0)
                }
            ]
        );
        assert_eq!(index.prefixes["無條件"], index.prefixes["무조건"]);
        assert!(!index.is_empty());
        assert!(
            PhraseIndex::new(&PhraseEvidence::new(vec![]))
                .unwrap()
                .is_empty()
        );
        let mut invalid = evidence;
        invalid.rules[0].chunks.clear();
        assert!(PhraseIndex::new(&invalid).is_err());
    }

    #[test]
    fn compatibility_rejects_stale_keys_handles_extensions_and_horizon() {
        let evidence = evidence();
        let index = PhraseIndex::new(&evidence).unwrap();
        index.validate_evidence(&evidence).unwrap();

        let mut same_size = evidence.clone();
        same_size.rules[0].chunks[0].spellings[0].key = "無條項".into();
        assert!(index.validate_evidence(&same_size).is_err());
        same_size.rules[0].chunks[0].spellings[0].key = "無條件".into();
        same_size.rules[0].chunks[0].reading = "무조항".into();
        assert!(index.validate_evidence(&same_size).is_err());

        let mut invalid = evidence.clone();
        invalid.rules[0].chunks.clear();
        assert!(index.validate_evidence(&invalid).is_err());
        let mut no_extension = evidence.clone();
        no_extension.rules[0].terminal_extensions.clear();
        assert!(index.validate_evidence(&no_extension).is_err());

        let mut wrong_horizon = index.clone();
        wrong_horizon.max_span_chars += 1;
        assert!(wrong_horizon.validate_evidence(&evidence).is_err());

        for handle in [
            PhraseRuleRef {
                rule: 1,
                extension: None,
            },
            PhraseRuleRef {
                rule: 0,
                extension: Some(1),
            },
        ] {
            let mut stale = index.clone();
            *stale
                .prefixes
                .get_mut("無條件")
                .unwrap()
                .last_mut()
                .unwrap() = handle;
            assert!(stale.validate_evidence(&evidence).is_err());
        }
        let mut extra = index.clone();
        extra
            .prefixes
            .insert("무관".into(), index.prefixes["무조건"].clone());
        assert!(extra.validate_evidence(&evidence).is_err());

        // Source witnesses are not encoded in the index. Equal index structure
        // remains usable with independently validated evidence at another address.
        let mut equivalent = evidence.clone();
        equivalent.rules[0].source_ids = vec![WordId(100)];
        index.validate_evidence(&equivalent).unwrap();
    }

    #[test]
    fn shared_owner_proves_validation_without_scanning_again() {
        let shared = Arc::new(evidence());
        let index = PhraseIndex::from_shared_evidence(shared.clone()).unwrap();
        let borrowed = PhraseIndex::new(&shared).unwrap();
        assert_eq!(index, borrowed);
        assert_eq!(Arc::strong_count(&shared), 2);
        VALIDATION_SCANS.with(|scans| scans.set(0));
        for _ in 0..8 {
            index.validate_evidence(&shared).unwrap();
        }
        VALIDATION_SCANS.with(|scans| assert_eq!(scans.get(), 0));

        // Equal contents at a different address still go through validation.
        let equivalent = shared.as_ref().clone();
        index.validate_evidence(&equivalent).unwrap();
        borrowed.validate_evidence(&shared).unwrap();
        VALIDATION_SCANS.with(|scans| assert_eq!(scans.get(), 2));

        let weak = Arc::downgrade(&shared);
        drop(shared);
        let retained = weak.upgrade().unwrap();
        let cloned_index = index.clone();
        drop(index);
        cloned_index.validate_evidence(&retained).unwrap();
        VALIDATION_SCANS.with(|scans| assert_eq!(scans.get(), 2));
    }

    #[test]
    fn changed_shared_evidence_cannot_reuse_validation_proof() {
        let mut shared = Arc::new(evidence());
        let index = PhraseIndex::from_shared_evidence(shared.clone()).unwrap();
        assert!(Arc::get_mut(&mut shared).is_none());
        Arc::make_mut(&mut shared).policy_version += 1;
        VALIDATION_SCANS.with(|scans| scans.set(0));
        assert_eq!(
            index.validate_evidence(&shared),
            Err(PhraseEvidenceError::UnsupportedVersion)
        );
        VALIDATION_SCANS.with(|scans| assert_eq!(scans.get(), 1));
        assert!(PhraseIndex::from_shared_evidence(shared).is_err());
        // The old allocation stays alive and immutable until the index is gone.
        index
            .validate_evidence(index.shared_evidence.as_deref().unwrap())
            .unwrap();
        VALIDATION_SCANS.with(|scans| assert_eq!(scans.get(), 1));
    }

    #[test]
    fn map_keeps_index_and_evidence_atomic_and_wrappers_borrow_it() {
        let mut dictionary = MapDictionary::new();
        dictionary.insert("降伏", "항복");
        dictionary.insert("降伏하다", "항복하다");
        dictionary.set_phrase_evidence(evidence()).unwrap();
        let index = dictionary.phrase_index().unwrap();
        assert_eq!(
            index,
            &PhraseIndex::new(dictionary.phrase_evidence().unwrap()).unwrap()
        );
        assert!(core::ptr::eq(
            index,
            dictionary.owned_phrase_evidence()[0].index.unwrap()
        ));
        let boxed: Box<dyn HanjaDictionary> = Box::new(dictionary.clone());
        assert!(core::ptr::eq(
            boxed.phrase_index().unwrap(),
            boxed.owned_phrase_evidence()[0].index.unwrap()
        ));
        let shared = Arc::new(dictionary.clone());
        assert!(core::ptr::eq(
            shared.phrase_index().unwrap(),
            HanjaDictionary::phrase_index(&&shared).unwrap()
        ));
        assert!(core::ptr::eq(
            dictionary.phrase_evidence().unwrap(),
            shared.phrase_evidence().unwrap()
        ));
        VALIDATION_SCANS.with(|scans| scans.set(0));
        for _ in 0..8 {
            drop(crate::Engine::<crate::PlainScopeData, _>::new(&dictionary));
            drop(crate::Engine::<crate::PlainScopeData, _>::new(&shared));
        }
        VALIDATION_SCANS.with(|scans| assert_eq!(scans.get(), 0));
        let old = dictionary.phrase_index().unwrap().clone();
        let mut invalid = evidence();
        invalid.rules[0].chunks[1].spellings[0].key = "降服".into();
        assert!(dictionary.set_phrase_evidence(invalid).is_err());
        assert_eq!(dictionary.phrase_index(), Some(&old));
        dictionary.insert("無關", "무관");
        assert!(dictionary.phrase_index().is_none());
        assert!(dictionary.phrase_evidence().is_none());
    }
}
