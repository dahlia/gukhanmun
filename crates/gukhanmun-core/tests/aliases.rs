// Gukhanmun: Alias identity, payload, and middleware regression tests.
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

use gukhanmun_core::*;

fn candidate(reading: &str, alias: Option<AliasTarget>) -> ReadingCandidate {
    ReadingCandidate {
        reading: reading.into(),
        suffix_reading: None,
        mark: MatchMark::default(),
        preference: 0,
        alias,
    }
}

fn alias(canonical: &str, kind: AliasKind) -> AliasTarget {
    AliasTarget {
        canonical: canonical.into(),
        kind,
    }
}

fn annotation(hanja: &str, reading: &str) -> Annotation {
    let mut annotation = Annotation::default();
    annotation.hanja = hanja.into();
    annotation.dictionary_hanja = Some(hanja.into());
    annotation.reading = reading.into();
    annotation.from_dictionary = true;
    annotation
}

#[test]
fn alias_payload_round_trips_all_kinds() {
    let candidates = ReadingCandidates::new(vec![
        candidate("연락", Some(alias("連絡", AliasKind::Sibling))),
        candidate("련락", Some(alias("連絡", AliasKind::Recognition))),
    ])
    .unwrap();
    let encoded = candidates.encode_payload().unwrap();
    assert_eq!(
        ReadingCandidates::decode_payload(&encoded).unwrap(),
        candidates
    );
}

#[test]
fn sibling_alias_resolves_to_canonical_lexeme() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("連絡", "연락");
    dictionary.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate(
            "연락",
            Some(alias("連絡", AliasKind::Sibling)),
        )])
        .unwrap(),
    );
    let index = LexemeIndex::from_dictionary(&dictionary).unwrap();
    assert_eq!(index.canonical("聯絡", "연락"), "連絡");
    assert_eq!(index.canonical("連絡", "연락"), "連絡");
}

#[test]
fn sibling_spellings_are_not_homophones_but_real_homophones_are() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("連絡", "연락");
    dictionary.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate(
            "연락",
            Some(alias("連絡", AliasKind::Sibling)),
        )])
        .unwrap(),
    );
    dictionary.insert("刻本", "각본");
    dictionary.insert("脚本", "각본");

    let marked = mark_homophones(
        vec![
            OutputToken::<PlainScopeData>::Annotated(annotation("連絡", "연락")),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(annotation("聯絡", "연락")),
        ],
        &dictionary,
        ContextWindow::PerDocument,
    );
    assert!(!annotated(&marked, 0).homophone);
    assert!(!annotated(&marked, 2).homophone);

    let marked = mark_homophones(
        vec![
            OutputToken::<PlainScopeData>::Annotated(annotation("刻本", "각본")),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(annotation("脚本", "각본")),
        ],
        &dictionary,
        ContextWindow::PerDocument,
    );
    assert!(annotated(&marked, 0).homophone);
    assert!(annotated(&marked, 2).homophone);
}

#[test]
fn first_occurrence_groups_sibling_spellings() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("連絡", "연락");
    dictionary.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate(
            "연락",
            Some(alias("連絡", AliasKind::Sibling)),
        )])
        .unwrap(),
    );
    let lexemes = LexemeIndex::from_dictionary(&dictionary).unwrap();
    let filtered = filter_first_occurrences_with_lexemes(
        vec![
            OutputToken::<PlainScopeData>::Annotated(annotation("連絡", "연락")),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(annotation("聯絡", "연락")),
        ],
        ContextWindow::PerDocument,
        lexemes,
    );
    assert!(annotated(&filtered, 0).first_in_context);
    assert!(!annotated(&filtered, 2).first_in_context);
}

#[test]
fn canonical_directive_reaches_sibling_spelling() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("連絡", "연락");
    dictionary.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate(
            "연락",
            Some(alias("連絡", AliasKind::Sibling)),
        )])
        .unwrap(),
    );
    let lexemes = LexemeIndex::from_dictionary(&dictionary).unwrap();
    let mut directives = UserDirectives::new();
    directives.require_hanja("連絡");
    let applied = apply_user_directives_with_lexemes(
        vec![OutputToken::<PlainScopeData>::Annotated(annotation(
            "聯絡", "연락",
        ))],
        &directives,
        lexemes,
    );
    assert!(annotated(&applied, 0).require_hanja);
}

fn annotated<S: ScopeData>(tokens: &[OutputToken<S>], index: usize) -> &Annotation {
    match &tokens[index] {
        OutputToken::Annotated(annotation) => annotation,
        _ => panic!("expected an annotation at {index}"),
    }
}

#[test]
fn recognition_alias_does_not_suppress_homophones() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("連絡", "연락");
    dictionary.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate(
            "연락",
            Some(alias("連絡", AliasKind::Recognition)),
        )])
        .unwrap(),
    );
    let marked = mark_homophones(
        vec![
            OutputToken::<PlainScopeData>::Annotated(annotation("連絡", "연락")),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(annotation("聯絡", "연락")),
        ],
        &dictionary,
        ContextWindow::PerDocument,
    );
    assert!(annotated(&marked, 0).homophone);
    assert!(annotated(&marked, 2).homophone);
}

#[test]
fn chain_does_not_borrow_lower_priority_identity() {
    let mut high = MapDictionary::new();
    high.insert("聯絡", "연락");
    let mut low = MapDictionary::new();
    low.insert("連絡", "연락");
    low.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate(
            "연락",
            Some(alias("連絡", AliasKind::Sibling)),
        )])
        .unwrap(),
    );
    let dictionary = ChainDictionary::from_iter([high, low]);
    let marked = mark_homophones(
        vec![
            OutputToken::<PlainScopeData>::Annotated(annotation("連絡", "연락")),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(annotation("聯絡", "연락")),
        ],
        &dictionary,
        ContextWindow::PerDocument,
    );
    assert!(annotated(&marked, 0).homophone);
    assert!(annotated(&marked, 2).homophone);
}

#[test]
fn alias_only_dictionary_is_not_its_own_homophone() {
    let mut dictionary = MapDictionary::new();
    dictionary.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate(
            "연락",
            Some(alias("連絡", AliasKind::Sibling)),
        )])
        .unwrap(),
    );
    assert!(!dictionary.has_homophone("聯絡", "연락"));
}

#[test]
fn empty_canonical_is_rejected() {
    let result =
        ReadingCandidates::new(vec![candidate("연락", Some(alias("", AliasKind::Lexical)))]);
    assert_eq!(result.unwrap_err(), CandidateError::EmptyReading);
}

#[test]
fn default_homophone_path_canonicalizes_alias_queries() {
    struct DefaultIdentityDictionary(MapDictionary);

    impl HanjaDictionary for DefaultIdentityDictionary {
        fn candidate_matches_at<'a>(
            &'a self,
            s: &'a str,
        ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
            self.0.candidate_matches_at(s)
        }
        fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            self.0.matches_at(s)
        }
        fn candidate_entries<'a>(
            &'a self,
        ) -> Option<Box<dyn Iterator<Item = CandidateRecord> + 'a>> {
            self.0.candidate_entries()
        }
    }

    let mut inner = MapDictionary::new();
    inner.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate(
            "연락",
            Some(alias("連絡", AliasKind::Sibling)),
        )])
        .unwrap(),
    );
    let dictionary = DefaultIdentityDictionary(inner);
    assert!(!dictionary.has_homophone("聯絡", "연락"));
}

#[test]
fn conflicting_aliases_on_one_reading_are_rejected() {
    let first = ReadingCandidate {
        reading: "갑".into(),
        suffix_reading: Some("을".into()),
        mark: MatchMark::default(),
        preference: 10,
        alias: Some(alias("甲", AliasKind::Lexical)),
    };
    let second = ReadingCandidate {
        reading: "갑".into(),
        suffix_reading: Some("병".into()),
        mark: MatchMark::default(),
        preference: 0,
        alias: Some(alias("乙", AliasKind::Lexical)),
    };
    assert_eq!(
        ReadingCandidates::new(vec![first, second]).unwrap_err(),
        CandidateError::ConflictingAlias
    );
}

#[test]
fn mixed_cached_and_uncached_chain_keeps_alias_identity() {
    struct DefaultIdentityDictionary(MapDictionary);

    impl HanjaDictionary for DefaultIdentityDictionary {
        fn candidate_matches_at<'a>(
            &'a self,
            s: &'a str,
        ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
            self.0.candidate_matches_at(s)
        }
        fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            self.0.matches_at(s)
        }
        fn candidate_entries<'a>(
            &'a self,
        ) -> Option<Box<dyn Iterator<Item = CandidateRecord> + 'a>> {
            self.0.candidate_entries()
        }
    }

    let mut alias = MapDictionary::new();
    alias.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate("연락", Some(alias_target_for_test("連絡")))])
            .unwrap(),
    );
    let dictionaries: Vec<Box<dyn HanjaDictionary>> = vec![
        Box::new(DefaultIdentityDictionary(alias)),
        Box::new(MapDictionary::new()),
    ];
    let chain = ChainDictionary::from_iter(dictionaries);
    assert!(!chain.has_homophone("聯絡", "연락"));
}

fn alias_target_for_test(canonical: &str) -> AliasTarget {
    AliasTarget {
        canonical: canonical.into(),
        kind: AliasKind::Sibling,
    }
}

#[test]
fn chain_fallback_keeps_canonicalized_query() {
    let mut alias = MapDictionary::new();
    alias.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate("연락", Some(alias_target_for_test("連絡")))])
            .unwrap(),
    );
    let mut canonical = MapDictionary::new();
    canonical.insert("連絡", "연락");
    let dictionaries: Vec<Box<dyn HanjaDictionary>> = vec![Box::new(alias), Box::new(canonical)];
    let chain = ChainDictionary::from_iter(dictionaries);
    assert!(!chain.has_homophone("連絡", "연락"));
}

#[test]
fn lookup_only_constituent_exercises_chain_fallback() {
    struct LookupOnlyDictionary;

    impl HanjaDictionary for LookupOnlyDictionary {
        fn candidate_matches_at<'a>(
            &'a self,
            _s: &'a str,
        ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
            Box::new(std::iter::empty())
        }
        fn matches_at<'a>(&'a self, _s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            Box::new(std::iter::empty())
        }
    }

    let mut alias = MapDictionary::new();
    alias.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate("연락", Some(alias_target_for_test("連絡")))])
            .unwrap(),
    );
    let mut canonical = MapDictionary::new();
    canonical.insert("連絡", "연락");
    let dictionaries: Vec<Box<dyn HanjaDictionary>> = vec![
        Box::new(alias),
        Box::new(canonical),
        Box::new(LookupOnlyDictionary),
    ];
    let chain = ChainDictionary::from_iter(dictionaries);
    assert!(!chain.has_homophone("聯絡", "연락"));
}

#[test]
fn uncached_enumerable_dictionary_gets_identity_in_middleware() {
    struct DefaultIdentityDictionary(MapDictionary);

    impl HanjaDictionary for DefaultIdentityDictionary {
        fn candidate_matches_at<'a>(
            &'a self,
            s: &'a str,
        ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
            self.0.candidate_matches_at(s)
        }
        fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            self.0.matches_at(s)
        }
        fn candidate_entries<'a>(
            &'a self,
        ) -> Option<Box<dyn Iterator<Item = CandidateRecord> + 'a>> {
            self.0.candidate_entries()
        }
    }

    let mut inner = MapDictionary::new();
    inner.insert("連絡", "연락");
    inner.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate("연락", Some(alias_target_for_test("連絡")))])
            .unwrap(),
    );
    let dictionary = DefaultIdentityDictionary(inner);
    let marked = mark_homophones(
        vec![
            OutputToken::<PlainScopeData>::Annotated(annotation("連絡", "연락")),
            OutputToken::Text(" ".into()),
            OutputToken::Annotated(annotation("聯絡", "연락")),
        ],
        &dictionary,
        ContextWindow::PerDocument,
    );
    assert!(!annotated(&marked, 0).homophone);
    assert!(!annotated(&marked, 2).homophone);
    let mut filter =
        FirstOccurrenceFilter::from_dictionary(ContextWindow::PerDocument, &dictionary);
    let mut filtered = Vec::new();
    for token in marked.clone() {
        filtered.extend(filter.push_token(token));
    }
    filtered.extend(filter.finish());
    assert!(annotated(&filtered, 0).first_in_context);
    assert!(!annotated(&filtered, 2).first_in_context);

    let mut marker = HomophoneMarker::new(&dictionary, ContextWindow::PerDocument);
    let mut streamed = Vec::new();
    for token in marked {
        streamed.extend(marker.push_token(token));
    }
    streamed.extend(marker.finish());
    assert!(!annotated(&streamed, 0).homophone);
    assert!(!annotated(&streamed, 2).homophone);
}

#[test]
fn chain_override_keeps_homophone_detection_with_lookup_only_child() {
    struct LookupOnlyDictionary;

    impl HanjaDictionary for LookupOnlyDictionary {
        fn candidate_matches_at<'a>(
            &'a self,
            _s: &'a str,
        ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
            Box::new(std::iter::empty())
        }
        fn matches_at<'a>(&'a self, _s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
            Box::new(std::iter::empty())
        }
    }

    let mut high = MapDictionary::new();
    high.insert("聯絡", "연락");
    let mut low = MapDictionary::new();
    low.insert("連絡", "연락");
    low.insert_candidates(
        "聯絡",
        ReadingCandidates::new(vec![candidate("연락", Some(alias_target_for_test("連絡")))])
            .unwrap(),
    );
    let dictionaries: Vec<Box<dyn HanjaDictionary>> = vec![
        Box::new(high),
        Box::new(low),
        Box::new(LookupOnlyDictionary),
    ];
    let chain = ChainDictionary::from_iter(dictionaries);
    assert!(chain.has_homophone("聯絡", "연락"));
}
