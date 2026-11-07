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
use alloc::vec::Vec;

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhraseIndex {
    prefixes: BTreeMap<String, Vec<PhraseRuleRef>>,
    max_span_chars: usize,
}
impl PhraseIndex {
    /// Validates evidence structure and compiles reusable first-chunk lookup tables.
    ///
    /// Candidate linkage remains the supplying dictionary's responsibility; use
    /// [`PhraseEvidence::validate_records`] before attaching the collection. This
    /// constructor never changes candidate order, rule order, or phrase semantics.
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
        })
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
