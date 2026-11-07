// Gukhanmun: Public source cooperation API regressions.
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

use std::collections::BTreeMap;

use gukhanmun::{
    AuthorizationKind, AuthorizedForm, Builder, CandidateEvidence, ContextWindow,
    DictionaryEvidence, MapDictionary, ReadingForm, RelationWitness, SenseId, SourceContribution,
    SourcePolicy, SourceReference, SourceRole, WordId, WordOrigin,
};

fn dictionary(reading: &str) -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("宙名", reading);
    dictionary
}

fn reference(kind: AuthorizationKind, reading: &str) -> MapDictionary {
    let mut dictionary = dictionary("가온");
    dictionary
        .set_source_evidence(DictionaryEvidence {
            schema_version: 1,
            role: SourceRole::Reference,
            entries: BTreeMap::from([(
                "宙名".into(),
                vec![CandidateEvidence {
                    reading: "가온".into(),
                    suffix_reading: None,
                    contributors: vec![SourceContribution {
                        word_id: WordId(1),
                        sense_id: Some(SenseId(10)),
                        origin: WordOrigin::Foreign,
                    }],
                    authorized_forms: vec![AuthorizedForm {
                        form: ReadingForm {
                            reading: reading.into(),
                            suffix_reading: None,
                        },
                        kind,
                        witnesses: vec![RelationWitness {
                            source: SourceReference::Sense(SenseId(10)),
                            target: SourceReference::Sense(SenseId(20)),
                        }],
                    }],
                    diagnostics: Vec::new(),
                }],
            )]),
        })
        .unwrap();
    dictionary
}

fn builder() -> Builder<'static> {
    Builder::new()
        .no_bundled_dictionaries()
        .homophone_window(ContextWindow::Off)
}

#[test]
fn source_policy_and_reference_filter_are_exposed_in_resolved_options() {
    let converter = builder()
        .reference_only(true)
        .push_dictionary_with_role(dictionary("누리"), SourceRole::Loanword)
        .push_dictionary_with_role(dictionary("가온"), SourceRole::Reference)
        .build()
        .unwrap();
    assert_eq!(
        converter.options().source_policy,
        SourcePolicy::ReferenceAuthorized
    );
    assert!(converter.options().reference_only);
    assert_eq!(
        converter.dictionary().source_policy(),
        SourcePolicy::ReferenceAuthorized
    );
    assert!(converter.dictionary().reference_only());
    assert_eq!(
        converter.dictionary().source_roles(),
        [SourceRole::Loanword, SourceRole::Reference]
    );
    assert_eq!(converter.convert_text_to_string("宙名").unwrap(), "가온");
}

#[test]
fn cooperative_policies_and_ordered_escape_use_the_same_registered_sources() {
    for (policy, expected) in [
        (SourcePolicy::ReferenceAuthorized, "가온"),
        (SourcePolicy::ReferenceFirst, "가온"),
        (SourcePolicy::LoanwordFirst, "누리"),
        (SourcePolicy::Ordered, "누리"),
    ] {
        let converter = builder()
            .source_policy(policy)
            .push_dictionary_with_role(dictionary("누리"), SourceRole::Loanword)
            .push_dictionary_with_role(dictionary("가온"), SourceRole::Reference)
            .build()
            .unwrap();
        assert_eq!(converter.convert_text_to_string("宙名").unwrap(), expected);
    }
    let reversed = builder()
        .source_policy(SourcePolicy::Ordered)
        .push_dictionary_with_role(dictionary("가온"), SourceRole::Reference)
        .push_dictionary_with_role(dictionary("누리"), SourceRole::Loanword)
        .build()
        .unwrap();
    assert_eq!(reversed.convert_text_to_string("宙名").unwrap(), "가온");
}

#[test]
fn authored_registration_forces_override_even_when_binary_role_says_reference() {
    let mut authored = dictionary("다솜");
    authored
        .set_source_evidence(DictionaryEvidence {
            schema_version: 1,
            role: SourceRole::Reference,
            entries: BTreeMap::new(),
        })
        .unwrap();
    let converter = builder()
        .push_dictionary_with_role(dictionary("누리"), SourceRole::Loanword)
        .push_dictionary(authored)
        .push_dictionary_with_role(dictionary("가온"), SourceRole::Reference)
        .build()
        .unwrap();
    assert_eq!(converter.convert_text_to_string("宙名").unwrap(), "다솜");
}

#[test]
fn reference_only_removes_independent_loans_but_keeps_authorized_realizations() {
    let independent = builder()
        .source_policy(SourcePolicy::LoanwordFirst)
        .reference_only(true)
        .push_dictionary_with_role(dictionary("누리"), SourceRole::Loanword)
        .push_dictionary_with_role(dictionary("가온"), SourceRole::Reference)
        .build()
        .unwrap();
    assert_eq!(independent.convert_text_to_string("宙名").unwrap(), "가온");
    let authorized = builder()
        .reference_only(true)
        .push_dictionary_with_role(dictionary("누리"), SourceRole::Loanword)
        .push_dictionary_with_role(
            reference(AuthorizationKind::Synonym, "누리"),
            SourceRole::Reference,
        )
        .build()
        .unwrap();
    assert_eq!(authorized.convert_text_to_string("宙名").unwrap(), "누리");
}

#[test]
fn reference_redirect_can_supply_a_phrase_without_a_loanword_dictionary() {
    let converter = builder()
        .push_dictionary_with_role(
            reference(AuthorizationKind::Redirect, "새 이름"),
            SourceRole::Reference,
        )
        .build()
        .unwrap();
    assert_eq!(converter.convert_text_to_string("宙名").unwrap(), "새 이름");
}

fn alias_dictionary() -> MapDictionary {
    use gukhanmun::{AliasKind, AliasTarget, MatchMark, ReadingCandidate, ReadingCandidates};
    let mut dictionary = MapDictionary::new();
    dictionary.insert("連絡", "연락");
    dictionary.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![ReadingCandidate {
            reading: "연락".into(),
            suffix_reading: None,
            mark: MatchMark {
                require_hanja: true,
                require_hangul: false,
            },
            preference: 0,
            alias: Some(AliasTarget {
                canonical: "連絡".into(),
                kind: AliasKind::Lexical,
            }),
        }])
        .unwrap(),
    );
    dictionary.insert("刻本", "각본");
    dictionary.insert("脚本", "각본");
    dictionary
}

#[test]
fn cooperative_builder_does_not_enumerate_for_private_pipeline_identity() {
    use gukhanmun::{CandidateMatch, CandidateRecord, HanjaDictionary, LexemeIndex, Match};
    struct LookupOnly(MapDictionary);
    impl HanjaDictionary for LookupOnly {
        fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            self.0.matches_at(s)
        }
        fn candidate_matches_at<'a>(
            &'a self,
            s: &'a str,
        ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
            self.0.candidate_matches_at(s)
        }
        fn candidate_entries<'a>(
            &'a self,
        ) -> Option<Box<dyn Iterator<Item = CandidateRecord> + 'a>> {
            panic!("cooperative construction and context-local conversion must not enumerate")
        }
        fn lexemes(&self) -> Option<LexemeIndex> {
            panic!("cooperative construction must not request a merged lexical index")
        }
    }
    for policy in [
        SourcePolicy::ReferenceAuthorized,
        SourcePolicy::ReferenceFirst,
        SourcePolicy::LoanwordFirst,
    ] {
        let converter = builder()
            .source_policy(policy)
            .push_dictionary_with_role(LookupOnly(dictionary("가온")), SourceRole::Reference)
            .build()
            .unwrap();
        assert_eq!(
            converter.convert_text_to_string("宙名 人").unwrap(),
            "가온 인"
        );
    }
}

#[test]
fn selected_alias_identity_reaches_directives_and_first_occurrence() {
    use gukhanmun::{DirectiveAction, HanjaDictionary};
    for policy in [SourcePolicy::ReferenceAuthorized, SourcePolicy::Ordered] {
        let converter = builder()
            .source_policy(policy)
            .push_dictionary_with_role(alias_dictionary(), SourceRole::Reference)
            .first_occurrence_window(ContextWindow::PerDocument)
            .build()
            .unwrap();
        assert_eq!(
            converter.convert_text_to_string("連絡 聯絡").unwrap(),
            "연락 연락"
        );
        // The public dictionary index remains complete even when the private
        // pipeline index is omitted for cooperative resolution.
        assert_eq!(
            converter
                .dictionary()
                .lexemes()
                .unwrap()
                .canonical("聯絡", "연락"),
            "連絡"
        );
        let converter = builder()
            .source_policy(policy)
            .push_dictionary_with_role(alias_dictionary(), SourceRole::Reference)
            .directive("連絡", DirectiveAction::RequireHanja)
            .build()
            .unwrap();
        assert_eq!(
            converter.convert_text_to_string("連絡 聯絡").unwrap(),
            "연락(連絡) 연락(聯絡)"
        );
    }
}

#[test]
fn selected_contextual_and_dictionary_wide_homophones_preserve_fallback() {
    use gukhanmun::HomophoneDetection;
    for policy in [SourcePolicy::ReferenceAuthorized, SourcePolicy::Ordered] {
        let converter = builder()
            .source_policy(policy)
            .push_dictionary_with_role(alias_dictionary(), SourceRole::Reference)
            .homophone_window(ContextWindow::PerDocument)
            .build()
            .unwrap();
        assert_eq!(
            converter.convert_text_to_string("刻本 脚本 人 仁").unwrap(),
            "각본(刻本) 각본(脚本) 인 인"
        );
        let converter = builder()
            .source_policy(policy)
            .push_dictionary_with_role(alias_dictionary(), SourceRole::Reference)
            .homophone_window(ContextWindow::PerDocument)
            .homophone_detection(HomophoneDetection::DictionaryWide)
            .build()
            .unwrap();
        assert_eq!(
            converter.convert_text_to_string("刻本 人").unwrap(),
            "각본(刻本) 인"
        );
    }
}
