// Gukhanmun: Owned shared facade parity, ownership, and concurrency regressions.
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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

#[cfg(feature = "html")]
use gukhanmun::Recovery;
use gukhanmun::{
    AliasKind, AuthorizationKind, AuthorizedForm, Builder, CandidateEvidence, CandidateMatch,
    CandidateRecord, ContextWindow, Converter, DictionaryEvidence, DirectiveAction, Error,
    HanjaDictionary, HanjaVariantSet, HomophoneDetection, LexemeIndex, MapDictionary, MatchMark,
    NumeralStrategy, OriginalGloss, PhraseCandidateLink, PhraseChunk, PhraseEvidence, PhraseRule,
    PhraseSpelling, Preset, PunctuationProfile, ReadingCandidate, ReadingCandidates, ReadingForm,
    RelationWitness, RenderMode, RenderOptions, RubyBase, SegmentationStrategy, SenseId,
    SharedBuilder, SharedConverter, SourceContribution, SourcePolicy, SourceReference, SourceRole,
    SpaceAfter, StopStyle, WordId, WordOrigin,
};
use gukhanmun_core::Match;

macro_rules! pair {
    ($($settings:tt)*) => {{
        let local = Builder::new()
            .no_bundled_dictionaries()
            .homophone_window(ContextWindow::Off)
            $($settings)*
            .build()
            .expect("local builder");
        let shared = SharedBuilder::new()
            .no_bundled_dictionaries()
            .homophone_window(ContextWindow::Off)
            $($settings)*
            .build()
            .expect("shared builder");
        (local, shared)
    }};
}

fn assert_text(local: &Converter<'_>, shared: &SharedConverter, input: &str, expected: &str) {
    assert_eq!(
        local.convert_text_to_string(input).unwrap(),
        expected,
        "local: {input}"
    );
    assert_eq!(
        shared.convert_text_to_string(input).unwrap(),
        expected,
        "shared: {input}"
    );
}

fn word(reading: &str) -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("宙名", reading);
    dictionary
}

fn boxed_word(reading: &str) -> Box<dyn HanjaDictionary + Send + Sync + 'static> {
    Box::new(word(reading))
}

fn candidate(reading: &str, preference: u32) -> ReadingCandidate {
    ReadingCandidate {
        reading: reading.into(),
        suffix_reading: None,
        mark: MatchMark::default(),
        preference,
        alias: None,
    }
}

fn readings_dictionary() -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    dictionary.insert_candidates(
        "東京",
        ReadingCandidates::new(vec![candidate("도쿄", 10), candidate("동경", 0)]).unwrap(),
    );
    dictionary.insert_with_suffix("年代", "연대", "년대");
    dictionary.insert("連絡", "연락");
    dictionary
        .insert_alias("聯絡", "연락", "連絡", AliasKind::Lexical)
        .unwrap();
    dictionary
}

fn phrase_dictionary(conflict: bool) -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    dictionary.insert("無條件", "무조건");
    dictionary.insert_candidates(
        "降服",
        ReadingCandidates::new(vec![candidate("강복", 10), candidate("항복", 0)]).unwrap(),
    );
    let chunk = |key: &str, reading: &str| {
        PhraseChunk::new(
            reading.into(),
            vec![PhraseSpelling::new(
                key.into(),
                vec![PhraseCandidateLink::new(reading.into(), None)],
            )],
        )
    };
    let rule = |id, reading: &str| {
        PhraseRule::new(
            vec![WordId(id)],
            vec![chunk("無條件", "무조건"), chunk("降服", reading)],
        )
    };
    let mut rules = vec![rule(1, "항복")];
    if conflict {
        rules.push(rule(2, "강복"));
    }
    dictionary
        .set_phrase_evidence(PhraseEvidence::new(rules))
        .unwrap();
    dictionary
}

fn authorized_reference() -> MapDictionary {
    let mut dictionary = word("가온");
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
                            reading: "누리".into(),
                            suffix_reading: None,
                        },
                        kind: AuthorizationKind::Synonym,
                        witnesses: vec![RelationWitness {
                            source: SourceReference::Sense(SenseId(10)),
                            target: SourceReference::Sense(SenseId(20)),
                        }],
                    }],
                    diagnostics: vec![],
                }],
            )]),
        })
        .unwrap();
    dictionary
}

#[test]
fn shared_runtime_and_builtin_storage_have_compiler_verified_bounds() {
    fn shareable<T: Send + Sync + 'static>() {}
    shareable::<SharedConverter>();
    shareable::<MapDictionary>();
    shareable::<Error>();
    #[cfg(feature = "fst")]
    shareable::<gukhanmun::fst::FstDictionary>();
    #[cfg(feature = "cdb")]
    shareable::<gukhanmun::cdb::CdbDictionary>();
    #[cfg(feature = "stdict")]
    {
        shareable::<gukhanmun::stdict::KoKrDictionary>();
        SharedBuilder::new()
            .no_bundled_dictionaries()
            .push_dictionary(gukhanmun::stdict::ko_kr())
            .build()
            .unwrap();
    }
    #[cfg(feature = "opendict")]
    {
        SharedBuilder::new()
            .no_bundled_dictionaries()
            .push_dictionary(gukhanmun::opendict::north_korean())
            .build()
            .unwrap();
    }
    #[cfg(feature = "loanwords")]
    {
        SharedBuilder::new()
            .no_bundled_dictionaries()
            .push_dictionary(gukhanmun::loanwords::dictionary())
            .build()
            .unwrap();
    }
}

struct SharedDictionary(Arc<MapDictionary>);
impl HanjaDictionary for SharedDictionary {
    fn matches_at<'a>(&'a self, input: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        self.0.matches_at(input)
    }
}

#[test]
fn arc_backed_custom_dictionary_is_the_passing_twin_of_local_rc_storage() {
    let converter = SharedBuilder::new()
        .no_bundled_dictionaries()
        .push_dictionary(SharedDictionary(Arc::new(word("가온"))))
        .build()
        .unwrap();
    assert_eq!(converter.convert_text_to_string("宙名").unwrap(), "가온");
}

#[test]
fn policies_reference_filter_and_boxed_roles_preserve_selection() {
    for (policy, default) in [
        (SourcePolicy::ReferenceAuthorized, "가온"),
        (SourcePolicy::ReferenceFirst, "가온"),
        (SourcePolicy::LoanwordFirst, "누리"),
        (SourcePolicy::Ordered, "누리"),
    ] {
        for reference_only in [false, true] {
            let (local, shared) = pair!(
                .source_policy(policy).reference_only(reference_only)
                .push_boxed_dictionary_with_role(boxed_word("누리"), SourceRole::Loanword)
                .push_dictionary_with_role(word("가온"), SourceRole::Reference)
            );
            let expected = if reference_only && policy != SourcePolicy::Ordered {
                "가온"
            } else {
                default
            };
            assert_text(&local, &shared, "宙名", expected);
            assert_eq!(shared.options().source_policy, policy);
            assert_eq!(shared.options().reference_only, reference_only);
        }
    }
    let (local, shared) = pair!(
        .reference_only(true)
        .push_boxed_dictionary_with_role(boxed_word("누리"), SourceRole::Loanword)
        .push_dictionary_with_role(authorized_reference(), SourceRole::Reference)
    );
    assert_text(&local, &shared, "宙名", "누리");
    let (local, shared) = pair!(
        .source_policy(SourcePolicy::LoanwordFirst)
        .push_dictionary_with_role(word("누리"), SourceRole::Loanword)
        .push_boxed_dictionary(boxed_word("사용자"))
        .push_dictionary_with_role(word("가온"), SourceRole::Reference)
    );
    assert_text(&local, &shared, "宙名", "사용자");
}

#[test]
fn aliases_competing_readings_suffixes_and_literal_directives_match_local() {
    for policy in [SourcePolicy::Ordered, SourcePolicy::ReferenceAuthorized] {
        let (local, shared) = pair!(
            .source_policy(policy).push_dictionary(readings_dictionary())
            .directive("連絡", DirectiveAction::RequireHanja)
        );
        assert_text(
            &local,
            &shared,
            "東京을 東京를 年代 1990年代 連絡 聯絡",
            "동경을 도쿄를 연대 1990년대 연락(連絡) 연락(聯絡)",
        );
    }
    let (local, shared) = pair!(
        .push_dictionary(readings_dictionary())
        .directive("東京", DirectiveAction::SkipAnnotation)
        .rendering(RenderMode::HangulHanjaParens)
    );
    assert_text(&local, &shared, "東京을", "동경을");
}

#[test]
fn nested_source_phrase_proofs_conflicts_and_overrides_match_local() {
    for segmentation in [SegmentationStrategy::Lattice, SegmentationStrategy::Eager] {
        for conflict in [false, true] {
            let make_nested = || {
                let mut nested = gukhanmun::ChainDictionary::new();
                nested.push_with_role(MapDictionary::new(), SourceRole::Override);
                nested.push_with_role(phrase_dictionary(conflict), SourceRole::Reference);
                nested
            };
            let (local, shared) = pair!(
                .segmentation(segmentation).source_policy(SourcePolicy::ReferenceFirst)
                .push_dictionary_with_role(make_nested(), SourceRole::Reference)
                .push_dictionary_with_role(word("별도"), SourceRole::Reference)
            );
            let selected = if conflict {
                "무조건강복"
            } else {
                "무조건항복"
            };
            assert_text(&local, &shared, "무조건降服", selected);
            assert_text(&local, &shared, "降服", "강복");
            assert_text(&local, &shared, "무조건  降服", "무조건  강복");
            assert_text(&local, &shared, "宙名", "별도");
        }
        let make_overridden = || {
            let mut nested = gukhanmun::ChainDictionary::new()
                .with_source_policy(SourcePolicy::ReferenceAuthorized);
            let mut authored = MapDictionary::new();
            authored.insert("降服", "강복");
            nested.push_with_role(authored, SourceRole::Override);
            nested.push_with_role(phrase_dictionary(false), SourceRole::Reference);
            nested
        };
        let (local, shared) = pair!(
            .segmentation(segmentation).source_policy(SourcePolicy::ReferenceFirst)
            .push_dictionary_with_role(make_overridden(), SourceRole::Reference)
        );
        assert_text(&local, &shared, "무조건降服", "무조건강복");
    }
}

#[test]
fn settings_precedence_and_owned_inputs_survive_builder_scope() {
    let (local, shared) = pair!(
        .hanja_variant_set(HanjaVariantSet::Shinjitai)
        .rendering(RenderMode::Original)
    );
    assert_eq!(local.options().rendering, shared.options().rendering);
    assert_eq!(
        shared.options().rendering.hanja_variant_set,
        HanjaVariantSet::Shinjitai
    );
    let (local, shared) = pair!(
        .hanja_variant_set(HanjaVariantSet::Shinjitai)
        .rendering(RenderOptions {
            mode: RenderMode::Original,
            original_gloss: OriginalGloss::Ruby,
            hanja_variant_set: HanjaVariantSet::Simplified,
        })
    );
    assert_eq!(local.options().rendering, shared.options().rendering);
    assert_eq!(
        shared.options().rendering.hanja_variant_set,
        HanjaVariantSet::Simplified
    );
    let (local, shared) = pair!(
        .hanja_variant_set(HanjaVariantSet::Shinjitai)
        .rendering(RenderOptions {
            mode: RenderMode::Original,
            original_gloss: OriginalGloss::Ruby,
            hanja_variant_set: HanjaVariantSet::Simplified,
        })
        .rendering(RenderMode::HangulHanjaParens)
    );
    assert_eq!(local.options().rendering, shared.options().rendering);
    assert_eq!(
        shared.options().rendering.hanja_variant_set,
        HanjaVariantSet::AsDictionary
    );
    assert_eq!(
        shared.options().rendering.original_gloss,
        OriginalGloss::Parens
    );
    let converter = {
        let spelling = String::from("學校");
        let reading = String::from("학교");
        let mut dictionary = MapDictionary::new();
        dictionary.insert(spelling.clone(), reading);
        SharedBuilder::new()
            .no_bundled_dictionaries()
            .push_dictionary(dictionary)
            .directive(spelling, DirectiveAction::RequireHanja)
            .build()
            .unwrap()
    };
    assert_eq!(
        converter.convert_text_to_string("學校").unwrap(),
        "학교(學校)"
    );
    let (local, shared) = pair!(
        .numerals(NumeralStrategy::Smart).initial_sound_law(false)
        .collapse_redundant_parens(false).punctuation(PunctuationProfile::modern())
    );
    assert_text(&local, &shared, "二〇一六年。 다음", "2016년. 다음");
}

#[test]
fn dictionary_wide_homophones_and_literal_hangul_glosses_match_local() {
    let (local, shared) = pair!(
        .push_dictionary(state_dictionary())
        .homophone_window(ContextWindow::PerDocument)
        .homophone_detection(HomophoneDetection::DictionaryWide)
    );
    assert_text(&local, &shared, "假裝", "가장(假裝)");
    let (local, shared) = pair!(
        .push_dictionary(readings_dictionary())
        .rendering(RenderOptions {
            mode: RenderMode::Original,
            original_gloss: OriginalGloss::Ruby,
            hanja_variant_set: HanjaVariantSet::AsDictionary,
        })
        .directive("連絡", DirectiveAction::RequireHangul)
    );
    assert_text(&local, &shared, "聯絡", "聯絡(연락)");
}

#[test]
fn shared_presets_and_bundled_opt_out_work_without_optional_features() {
    for preset in [Preset::KoKr, Preset::KoKp] {
        let local = Builder::with_preset(preset)
            .no_bundled_dictionaries()
            .build()
            .unwrap();
        let shared = SharedBuilder::with_preset(preset)
            .no_bundled_dictionaries()
            .build()
            .unwrap();
        assert_eq!(local.options().engine, shared.options().engine);
        assert_eq!(
            local.options().homophone_window,
            shared.options().homophone_window
        );
        assert_eq!(
            local.convert_text_to_string("來日").unwrap(),
            shared.convert_text_to_string("來日").unwrap()
        );
    }
}

#[test]
fn invalid_punctuation_precedes_missing_bundled_feature_errors() {
    let local_default = Builder::new()
        .build()
        .map(|_| ())
        .map_err(|error| error.to_string());
    let shared_default = SharedBuilder::new()
        .build()
        .map(|_| ())
        .map_err(|error| error.to_string());
    assert_eq!(shared_default, local_default);
    if let Err(error) = shared_default {
        let first_missing = if cfg!(feature = "loanwords") {
            "stdict"
        } else {
            "loanwords"
        };
        assert!(error.contains(&format!("`{first_missing}` feature is disabled")));
    }
    let mut invalid = PunctuationProfile::preserve();
    invalid.period = Some(StopStyle::new('a', SpaceAfter::Single));
    for preset in [Preset::KoKr, Preset::KoKp] {
        let local = Builder::with_preset(preset)
            .punctuation(invalid)
            .build()
            .err()
            .expect("invalid local punctuation")
            .to_string();
        let shared = SharedBuilder::with_preset(preset)
            .punctuation(invalid)
            .build()
            .err()
            .expect("invalid shared punctuation")
            .to_string();
        assert_eq!(shared, local);
        assert!(shared.contains("invalid punctuation profile"));
    }
    #[cfg(not(feature = "loanwords"))]
    {
        let local = Builder::new()
            .no_bundled_dictionaries()
            .bundled_loanwords()
            .build()
            .err()
            .expect("missing loanwords")
            .to_string();
        let shared = SharedBuilder::new()
            .no_bundled_dictionaries()
            .bundled_loanwords()
            .build()
            .err()
            .expect("missing loanwords")
            .to_string();
        assert_eq!(shared, local);
        assert!(shared.contains("`loanwords` feature is disabled"));
    }
    #[cfg(not(feature = "stdict"))]
    {
        let local = Builder::new()
            .no_bundled_dictionaries()
            .bundled_stdict()
            .build()
            .err()
            .expect("missing stdict")
            .to_string();
        let shared = SharedBuilder::new()
            .no_bundled_dictionaries()
            .bundled_stdict()
            .build()
            .err()
            .expect("missing stdict")
            .to_string();
        assert_eq!(shared, local);
        assert!(shared.contains("`stdict` feature is disabled"));
    }
    #[cfg(not(feature = "opendict"))]
    {
        let local = Builder::with_preset(Preset::KoKp)
            .build()
            .err()
            .expect("missing opendict")
            .to_string();
        let shared = SharedBuilder::with_preset(Preset::KoKp)
            .build()
            .err()
            .expect("missing opendict")
            .to_string();
        assert_eq!(shared, local);
        assert!(shared.contains("`opendict` feature is disabled"));
    }
}

#[test]
fn each_bundled_disable_switch_preserves_preset_resolution() {
    macro_rules! compare_switch {
        ($switch:ident) => {
            for preset in [Preset::KoKr, Preset::KoKp] {
                let local = Builder::with_preset(preset)
                    .$switch()
                    .homophone_window(ContextWindow::Off)
                    .build()
                    .and_then(|converter| converter.convert_text_to_string("香港 來日"))
                    .map_err(|error| error.to_string());
                let shared = SharedBuilder::with_preset(preset)
                    .$switch()
                    .homophone_window(ContextWindow::Off)
                    .build()
                    .and_then(|converter| converter.convert_text_to_string("香港 來日"))
                    .map_err(|error| error.to_string());
                assert_eq!(shared, local, "{preset:?}: {}", stringify!($switch));
            }
        };
    }
    compare_switch!(no_bundled_stdict);
    compare_switch!(no_bundled_loanwords);
    compare_switch!(no_bundled_opendict);
    compare_switch!(no_bundled_dictionaries);
}

#[test]
fn render_modes_and_hanja_variant_axes_match_local() {
    for (variants, hanja) in [
        (HanjaVariantSet::AsDictionary, "學校"),
        (HanjaVariantSet::Kanxi, "學校"),
        (HanjaVariantSet::Shinjitai, "学校"),
        (HanjaVariantSet::Simplified, "学校"),
        (HanjaVariantSet::Asahimoji, "学校"),
    ] {
        for (mode, expected) in [
            (RenderMode::HangulOnly, "학교".to_owned()),
            (RenderMode::HangulHanjaParens, format!("학교({hanja})")),
            (RenderMode::HanjaHangulParens, format!("{hanja}(학교)")),
            (
                RenderMode::Ruby(RubyBase::OnHangul),
                format!("학교({hanja})"),
            ),
            (
                RenderMode::Ruby(RubyBase::OnHanja),
                format!("{hanja}(학교)"),
            ),
            (RenderMode::Original, hanja.to_owned()),
        ] {
            let make_dictionary = || {
                let mut dictionary = MapDictionary::new();
                dictionary.insert("學校", "학교");
                dictionary
            };
            let (local, shared) = pair!(
                .push_dictionary(make_dictionary()).rendering(mode).hanja_variant_set(variants)
            );
            assert_text(&local, &shared, "學校", &expected);
            #[cfg(feature = "html")]
            {
                let expected = match mode {
                    RenderMode::Ruby(RubyBase::OnHangul) => {
                        format!("<p><ruby>학교<rp>(</rp><rt>{hanja}</rt><rp>)</rp></ruby></p>")
                    }
                    RenderMode::Ruby(RubyBase::OnHanja) => {
                        format!("<p><ruby>{hanja}<rp>(</rp><rt>학교</rt><rp>)</rp></ruby></p>")
                    }
                    _ => format!("<p>{expected}</p>"),
                };
                assert_eq!(
                    local
                        .convert_html_fragment_to_string("<p>學校</p>")
                        .unwrap(),
                    expected
                );
                assert_eq!(
                    shared
                        .convert_html_fragment_to_string("<p>學校</p>")
                        .unwrap(),
                    expected
                );
            }
        }
    }
}

#[cfg(feature = "markdown")]
#[test]
fn gfm_extensions_and_commonmark_preserve_exact_writer_output() {
    let (local, shared) = pair!(.push_dictionary(readings_dictionary()));
    let input = "- [x] 東京을\n\n| 제목 |\n| --- |\n| 年代 |\n\n~~聯絡~~";
    for variant in [
        gukhanmun::markdown::MarkdownVariant::CommonMark,
        gukhanmun::markdown::MarkdownVariant::Gfm,
    ] {
        let expected = local.convert_markdown_to_string(input, variant).unwrap();
        let actual = shared.convert_markdown_to_string(input, variant).unwrap();
        assert_eq!(actual, expected);
        assert!(actual.contains("연대"));
        assert!(!actual.contains("東京"));
        assert!(!actual.contains("聯絡"));
    }
}

#[cfg(feature = "html")]
#[test]
fn html_preservation_punctuation_and_recovery_use_the_same_adapters() {
    let make_dictionary = || {
        let mut dictionary = MapDictionary::new();
        dictionary.insert("學校", "학교");
        dictionary
    };
    for recovery in [Recovery::Strict, Recovery::Lenient] {
        let (local, shared) = pair!(
            .push_dictionary(make_dictionary()).recovery(recovery)
            .punctuation(PunctuationProfile::modern())
            .html_preserve_when({
                let class = String::from("class=\"keep\"");
                move |info| info.raw_attributes.contains(&class)
            })
        );
        let input = "<p class=\"keep\"><b>學校。</b></p><p>學校。</p>";
        let expected = "<p class=\"keep\"><b>學校。</b></p><p>학교.</p>";
        assert_eq!(
            local.convert_html_fragment_to_string(input).unwrap(),
            expected
        );
        assert_eq!(
            shared.convert_html_fragment_to_string(input).unwrap(),
            expected
        );
        let malformed = "<p>學校 <1invalid> 學校";
        let local_result = local
            .convert_html_fragment_to_string(malformed)
            .map_err(|e| e.to_string());
        let shared_result = shared
            .convert_html_fragment_to_string(malformed)
            .map_err(|e| e.to_string());
        assert_eq!(local_result, shared_result);
        match recovery {
            Recovery::Strict => assert!(shared_result.is_err()),
            Recovery::Lenient => assert_eq!(shared_result.unwrap(), "<p>학교 <1invalid> 학교"),
        }
        assert_eq!(
            shared
                .convert_html_fragment_to_string("<p>學校</p>")
                .unwrap(),
            "<p>학교</p>"
        );
        let input = "앞</span data-x=。>뒤。다음";
        let expected = "앞</span data-x=。>뒤. 다음";
        assert_eq!(
            local.convert_html_fragment_to_string(input).unwrap(),
            expected
        );
        assert_eq!(
            shared.convert_html_fragment_to_string(input).unwrap(),
            expected
        );
    }
}

struct CountingLexemes {
    inner: MapDictionary,
    calls: Arc<AtomicUsize>,
}
impl HanjaDictionary for CountingLexemes {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        self.inner.matches_at(s)
    }
    fn candidate_matches_at<'a>(
        &'a self,
        s: &'a str,
    ) -> Box<dyn Iterator<Item = CandidateMatch> + 'a> {
        self.inner.candidate_matches_at(s)
    }
    fn lexemes(&self) -> Option<LexemeIndex> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.lexemes()
    }
}

#[test]
fn ordered_conversions_reuse_construction_time_lexemes() {
    let calls = Arc::new(AtomicUsize::new(0));
    let shared = SharedBuilder::new()
        .no_bundled_dictionaries()
        .source_policy(SourcePolicy::Ordered)
        .homophone_detection(HomophoneDetection::ContextLocal)
        .push_dictionary(CountingLexemes {
            inner: word("가온"),
            calls: Arc::clone(&calls),
        })
        .build()
        .unwrap();
    assert!(calls.load(Ordering::SeqCst) > 0);
    calls.store(0, Ordering::SeqCst);
    for _ in 0..3 {
        assert_eq!(shared.convert_text_to_string("宙名").unwrap(), "가온");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn cooperative_construction_and_context_local_conversion_do_not_enumerate() {
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
        let converter = SharedBuilder::new()
            .no_bundled_dictionaries()
            .source_policy(policy)
            .push_dictionary_with_role(LookupOnly(word("가온")), SourceRole::Reference)
            .build()
            .unwrap();
        assert_eq!(
            converter.convert_text_to_string("宙名 人").unwrap(),
            "가온 인"
        );
    }
}

fn state_dictionary() -> MapDictionary {
    let mut dictionary = MapDictionary::new();
    dictionary.insert_marked(
        "家長",
        "가장",
        MatchMark {
            require_hanja: true,
            require_hangul: false,
        },
    );
    dictionary.insert("假裝", "가장");
    dictionary.insert("學校", "학교");
    dictionary
}

#[test]
fn barrier_started_threads_share_one_runtime_without_sharing_document_state() {
    let (local, shared) = pair!(
        .push_dictionary(state_dictionary())
        .homophone_window(ContextWindow::PerDocument)
        .first_occurrence_window(ContextWindow::PerDocument)
    );
    let cases = [
        ("家長 家長", "가장(家長) 가장"),
        ("家長 假裝", "가장(家長) 가장(假裝)"),
        ("假裝", "가장"),
        ("家長", "가장(家長)"),
        ("學校", "학교"),
    ];
    #[cfg(feature = "markdown")]
    let markdown_serial = Arc::new(
        cases
            .iter()
            .map(|(input, expected)| {
                let variant = gukhanmun::markdown::MarkdownVariant::CommonMark;
                let serial = local.convert_markdown_to_string(input, variant).unwrap();
                assert_eq!(
                    shared.convert_markdown_to_string(input, variant).unwrap(),
                    serial
                );
                assert_eq!(serial.trim(), *expected);
                serial
            })
            .collect::<Vec<_>>(),
    );
    for (input, expected) in cases {
        assert_text(&local, &shared, input, expected);
        #[cfg(feature = "html")]
        {
            let input = format!("<p>{input}</p>");
            let expected = format!("<p>{expected}</p>");
            assert_eq!(
                local.convert_html_fragment_to_string(&input).unwrap(),
                expected
            );
            assert_eq!(
                shared.convert_html_fragment_to_string(&input).unwrap(),
                expected
            );
        }
    }
    let shared = Arc::new(shared);
    let barrier = Arc::new(Barrier::new(6));
    let workers = (0..6)
        .map(|worker| {
            let shared = Arc::clone(&shared);
            let barrier = Arc::clone(&barrier);
            #[cfg(feature = "markdown")]
            let markdown_serial = Arc::clone(&markdown_serial);
            std::thread::spawn(move || {
                barrier.wait();
                for round in 0..8 {
                    for offset in 0..cases.len() {
                        let index = (worker + round + offset) % cases.len();
                        let (input, expected) = cases[index];
                        assert_eq!(shared.convert_text_to_string(input).unwrap(), expected);
                        #[cfg(feature = "html")]
                        assert_eq!(
                            shared
                                .convert_html_fragment_to_string(&format!("<p>{input}</p>"))
                                .unwrap(),
                            format!("<p>{expected}</p>")
                        );
                        #[cfg(feature = "markdown")]
                        assert_eq!(
                            shared
                                .convert_markdown_to_string(
                                    input,
                                    gukhanmun::markdown::MarkdownVariant::CommonMark
                                )
                                .unwrap(),
                            markdown_serial[index]
                        );
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
}

#[cfg(feature = "html")]
#[test]
fn owned_html_predicate_can_run_concurrently_and_replace_an_earlier_predicate() {
    let calls = Arc::new(AtomicUsize::new(0));
    let old_calls = Arc::new(AtomicUsize::new(0));
    let shared = {
        let class = String::from("class=\"keep\"");
        let predicate_calls = Arc::clone(&calls);
        let replaced_calls = Arc::clone(&old_calls);
        SharedBuilder::new()
            .no_bundled_dictionaries()
            .push_dictionary(state_dictionary())
            .html_preserve_when(move |_| {
                replaced_calls.fetch_add(1, Ordering::SeqCst);
                true
            })
            .html_preserve_when(move |info| {
                predicate_calls.fetch_add(1, Ordering::SeqCst);
                info.raw_attributes.contains(&class)
            })
            .build()
            .unwrap()
    };
    let shared = Arc::new(shared);
    let barrier = Arc::new(Barrier::new(4));
    let workers = (0..4)
        .map(|_| {
            let shared = Arc::clone(&shared);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                assert_eq!(
                    shared
                        .convert_html_fragment_to_string(
                            "<p class=\"keep\"><b>學校</b></p><p>學校</p>"
                        )
                        .unwrap(),
                    "<p class=\"keep\"><b>學校</b></p><p>학교</p>"
                );
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(old_calls.load(Ordering::SeqCst), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 8);
}
