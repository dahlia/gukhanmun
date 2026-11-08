// Gukhanmun: Leaf-owned phrase pools across composite dictionaries.
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

use crate::{
    ChainDictionary, HanjaDictionary, SourceGroup, SourceId, SourceMatch, SourcePolicy, SourceRole,
};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;

// A composite registered as a loanword is one opaque supplier at this boundary,
// as in the legacy source policy. Its effective candidate projection must not
// export descendant reference roles or borrow their phrase authority. A leaf
// exposing its own evidence remains a directly owned source.
pub(crate) fn opaque_loanword<D: HanjaDictionary + ?Sized>(
    dictionary: &D,
    role: SourceRole,
) -> bool {
    role == SourceRole::Loanword && dictionary.phrase_evidence().is_none()
}

// Remap every ownership-bearing field together, leaving semantic identities intact.
fn remap(mut matched: SourceMatch, offset: usize, role: SourceRole, leaf: bool) -> SourceMatch {
    for group in &mut matched.groups {
        group.source_id.0 += offset;
        if leaf || role == SourceRole::Override {
            group.role = role;
        }
    }
    let remap_choices = |choices: &[crate::SourceChoice]| -> Arc<[crate::SourceChoice]> {
        choices
            .iter()
            .cloned()
            .map(|mut choice| {
                if role == SourceRole::Override {
                    choice.reason = crate::ChoiceReason::Override;
                }
                choice.anchor.source_id.0 += offset;
                if let Some(source) = &mut choice.form_source {
                    source.0 += offset;
                }
                choice
            })
            .collect()
    };
    // Preserve both complete views without searching or splicing identities.
    let legacy_is_full = matched
        .legacy_selection
        .as_ref()
        .is_some_and(|legacy| Arc::ptr_eq(&legacy.choices, &matched.ranked));
    matched.ranked = remap_choices(&matched.ranked);
    if let Some(legacy) = &mut matched.legacy_selection {
        legacy.choices = if legacy_is_full {
            matched.ranked.clone()
        } else {
            remap_choices(&legacy.choices)
        };
    }
    matched.diagnostics = matched
        .diagnostics
        .iter()
        .cloned()
        .map(|mut diagnostic| {
            match &mut diagnostic {
                crate::SelectionDiagnostic::UnknownEvidence { source_id, .. }
                | crate::SelectionDiagnostic::ConflictingRedirect { source_id, .. }
                | crate::SelectionDiagnostic::ConflictingAlias { source_id, .. }
                | crate::SelectionDiagnostic::OpaqueLegacyProjection { source_id, .. } => {
                    source_id.0 += offset
                }
            }
            diagnostic
        })
        .collect::<Vec<_>>()
        .into();

    // Nested policies may exclude owners. Outer cooperation must not resurrect them.
    let active: BTreeSet<_> = matched
        .ranked
        .iter()
        .flat_map(|choice| core::iter::once(choice.anchor.source_id).chain(choice.form_source))
        .collect();
    matched
        .groups
        .retain(|group| active.contains(&group.source_id));
    matched
}

fn legacy<D: HanjaDictionary>(
    dictionary: &D,
    matched: crate::CandidateMatch,
    key: &str,
    role: SourceRole,
) -> Option<SourceMatch> {
    let group = SourceGroup::from_dictionary(
        dictionary,
        SourceId(0),
        role,
        key.into(),
        matched.candidates,
    );
    SourceMatch::rank(
        matched.byte_len,
        key.into(),
        alloc::vec![group],
        SourcePolicy::ReferenceFirst,
        false,
    )
    .map(|mut matched| {
        matched.legacy_selection = Some(crate::LegacySelection {
            choices: matched.ranked.clone(),
            merge_eligible: true,
        });
        matched
    })
}

fn combine_proof<D: HanjaDictionary>(
    chain: &ChainDictionary<D>,
    pools: Vec<(SourceMatch, bool)>,
) -> Vec<SourceMatch> {
    if chain.source_policy == SourcePolicy::Ordered {
        let mut first = BTreeMap::new();
        for (matched, _) in pools {
            first.entry(matched.byte_len).or_insert(matched);
        }
        return first.into_values().collect();
    }
    let mut groups = BTreeMap::<usize, Vec<SourceGroup>>::new();
    let mut surfaces = BTreeMap::new();
    let mut permitted = BTreeMap::<(usize, SourceId), Vec<crate::SourceChoice>>::new();
    let mut overrides = BTreeMap::new();
    for (matched, nested) in pools {
        let len = matched.byte_len;
        if matched
            .groups
            .iter()
            .any(|group| group.role == SourceRole::Override)
        {
            // An authored composite supplies its already resolved complete pool,
            // including promoted forms. Re-ranking its first raw group loses it.
            overrides.entry(len).or_insert_with(|| matched.clone());
        }
        if nested {
            // An inner policy licenses complete choices, not every candidate in
            // an owner used solely as a spelling supplier. Preserve that mask.
            for group in &matched.groups {
                permitted.entry((len, group.source_id)).or_default();
            }
            for choice in matched.ranked.iter() {
                permitted
                    .entry((len, choice.anchor.source_id))
                    .or_default()
                    .push(choice.clone());
            }
        }
        surfaces.entry(len).or_insert(matched.hanja);
        groups.entry(len).or_default().extend(matched.groups);
    }
    groups
        .into_iter()
        .filter_map(|(len, groups)| {
            if let Some(mut matched) = overrides.remove(&len) {
                matched.legacy_selection = None;
                return Some(matched);
            }
            let mut matched = SourceMatch::rank(
                len,
                surfaces.remove(&len).unwrap_or_default(),
                groups,
                chain.source_policy,
                chain.reference_only,
            )?;
            let ranked: Vec<_> = matched
                .ranked
                .iter()
                .filter(|choice| {
                    permitted
                        .get(&(len, choice.anchor.source_id))
                        .is_none_or(|allowed| {
                            allowed.iter().any(|eligible| {
                                eligible.anchor == choice.anchor
                                    && eligible.form == choice.form
                                    && eligible.form_source == choice.form_source
                                    && eligible.mark == choice.mark
                            })
                        })
                })
                .cloned()
                .collect();
            if ranked.is_empty() {
                return None;
            }
            matched.ranked = ranked.into();
            Some(matched)
        })
        .collect()
}

#[derive(Default)]
struct FallbackPool {
    hanja: alloc::string::String,
    groups: Vec<SourceGroup>,
    opaque_ranges: BTreeMap<SourceId, usize>,
}

// Mirror the child's public candidate API using its already computed view.
fn public_projection(matched: &SourceMatch) -> crate::CandidateMatch {
    let choices = matched.selection_choices();
    let choices = if matched
        .legacy_selection
        .as_ref()
        .is_some_and(|legacy| legacy.merge_eligible)
    {
        choices
    } else {
        &choices[..1]
    };
    crate::CandidateMatch {
        byte_len: matched.byte_len,
        candidates: crate::ReadingCandidates::from_source_choices(choices),
    }
}

// Project the already computed child view once, avoiding recursive re-queries.
// A composite projection is opaque; only its public candidates and its own
// source evidence participate in the parent's original policy computation.
fn add_fallback<D: HanjaDictionary>(
    pools: &mut BTreeMap<usize, FallbackPool>,
    dictionary: &D,
    matched: &SourceMatch,
    key: &str,
    offset: usize,
    role: SourceRole,
    opaque: bool,
) {
    if matched.selection_choices().is_empty() {
        return;
    }
    let pool = pools.entry(matched.byte_len).or_default();
    if pool.groups.is_empty() {
        pool.hanja = matched.hanja.clone();
    }
    pool.groups.push(SourceGroup::from_dictionary(
        dictionary,
        SourceId(offset),
        role,
        key.into(),
        public_projection(matched).candidates,
    ));
    if opaque {
        pool.opaque_ranges
            .insert(SourceId(offset), dictionary.source_count());
    }
}

fn combine<D: HanjaDictionary>(
    chain: &ChainDictionary<D>,
    pools: Vec<(SourceMatch, bool)>,
    fallback: BTreeMap<usize, FallbackPool>,
) -> Vec<SourceMatch> {
    let mut proof: BTreeMap<_, _> = combine_proof(chain, pools)
        .into_iter()
        .map(|matched| (matched.byte_len, matched))
        .collect();
    fallback
        .into_iter()
        .filter_map(|(len, mut fallback)| {
            let ordered = chain.source_policy == SourcePolicy::Ordered;
            if ordered {
                fallback.groups.truncate(1);
            }
            let legacy = SourceMatch::rank(
                len,
                fallback.hanja,
                fallback.groups,
                if ordered {
                    SourcePolicy::ReferenceFirst
                } else {
                    chain.source_policy
                },
                !ordered && chain.reference_only,
            )?;
            let mut matched = proof.remove(&len).unwrap_or_else(|| {
                let mut opaque = legacy.clone();
                opaque.ranked = Arc::from([]);
                opaque.groups.clear();
                opaque.diagnostics = Arc::from([]);
                opaque
            });
            // Opaque ranges describe this parent's fallback projection, not
            // descendant projections carried by a retained proof-pool clone.
            let mut diagnostics: Vec<_> = matched
                .diagnostics
                .iter()
                .filter(|diagnostic| {
                    !matches!(
                        diagnostic,
                        crate::SelectionDiagnostic::OpaqueLegacyProjection { .. }
                    )
                })
                .cloned()
                .collect();
            diagnostics.extend(legacy.diagnostics.iter().cloned());
            // Rank first: discarded groups supply neither legacy identities nor
            // forms, so their opaque projection must not appear in diagnostics.
            let participating: BTreeSet<_> = legacy
                .ranked
                .iter()
                .flat_map(|choice| {
                    core::iter::once(choice.anchor.source_id).chain(choice.form_source)
                })
                .collect();
            diagnostics.extend(fallback.opaque_ranges.into_iter().filter_map(
                |(source_id, source_count)| {
                    participating.contains(&source_id).then_some(
                        crate::SelectionDiagnostic::OpaqueLegacyProjection {
                            source_id,
                            source_count,
                        },
                    )
                },
            ));
            // Both views may describe the same issue. Keep its first occurrence
            // without conflating distinct source IDs, forms or diagnostic kinds.
            let mut seen = BTreeSet::new();
            diagnostics.retain(|diagnostic| seen.insert(diagnostic.clone()));
            matched.diagnostics = diagnostics.into();
            matched.legacy_selection = Some(crate::LegacySelection {
                choices: legacy.ranked,
                merge_eligible: ordered,
            });
            Some(matched)
        })
        .collect()
}

pub(crate) fn matches_at<D: HanjaDictionary>(
    chain: &ChainDictionary<D>,
    source: &str,
) -> Vec<SourceMatch> {
    let mut pools = Vec::new();
    let mut fallback = BTreeMap::new();
    let mut offset = 0;
    for (index, dictionary) in chain.dictionaries.iter().enumerate() {
        let role = chain.roles[index];
        let leaf = dictionary.phrase_evidence().is_some();
        let opaque_registration = opaque_loanword(dictionary, role);
        let source_matches = dictionary.source_matches_at(source);
        let source_aware = source_matches.is_some();
        let opaque = !leaf && (source_aware || dictionary.source_count() > 1);
        if let Some(matches) = source_matches {
            for matched in matches {
                let Some(key) = source.get(..matched.byte_len) else {
                    continue;
                };
                if matched.selection_choices().is_empty() {
                    continue;
                }
                // A registered opaque supplier exports only its public legacy
                // projection. Do not query again or retain descendant authority.
                let matched = if opaque_registration {
                    let Some(projected) =
                        legacy(dictionary, public_projection(&matched), key, role)
                    else {
                        continue;
                    };
                    projected
                } else {
                    matched
                };
                add_fallback(
                    &mut fallback,
                    dictionary,
                    &matched,
                    key,
                    offset,
                    role,
                    opaque,
                );
                pools.push((
                    remap(matched, offset, role, leaf || opaque_registration),
                    !leaf && !opaque_registration,
                ));
            }
        } else {
            for candidate in dictionary.candidate_matches_at(source) {
                let Some(key) = source.get(..candidate.byte_len) else {
                    continue;
                };
                if let Some(matched) = legacy(dictionary, candidate, key, role) {
                    add_fallback(
                        &mut fallback,
                        dictionary,
                        &matched,
                        key,
                        offset,
                        role,
                        opaque,
                    );
                    pools.push((remap(matched, offset, role, true), false));
                }
            }
        }
        offset += dictionary.source_count();
    }
    combine(chain, pools, fallback)
}

pub(crate) fn matches_spellings<D: HanjaDictionary>(
    chain: &ChainDictionary<D>,
    spellings: &[&str],
) -> Vec<(usize, SourceMatch)> {
    let mut pools = Vec::new();
    let mut fallback = BTreeMap::new();
    let mut offset = 0;
    for (index, dictionary) in chain.dictionaries.iter().enumerate() {
        let role = chain.roles[index];
        let leaf = dictionary.phrase_evidence().is_some();
        let opaque_registration = opaque_loanword(dictionary, role);
        let source_matches = dictionary.source_matches_at_spellings(spellings);
        let source_aware = source_matches.is_some();
        let nested = source_aware && !leaf && !opaque_registration;
        let opaque = !leaf && (source_aware || dictionary.source_count() > 1);
        let mut matches = source_matches.unwrap_or_else(|| {
            dictionary
                .candidate_matches_at_spellings(spellings)
                .into_iter()
                .filter_map(|(index, matched)| {
                    let key = *spellings.get(index)?;
                    legacy(dictionary, matched, key, role).map(|matched| (index, matched))
                })
                .collect()
        });
        matches.retain(|(index, matched)| {
            let spelling = if source_aware {
                spellings.first()
            } else {
                spellings.get(*index)
            };
            spelling.is_some_and(|key| key.len() == matched.byte_len)
        });
        if matches.iter().any(|(index, _)| *index == 0) {
            matches.retain(|(index, _)| *index == 0);
        } else if matches.len() > 1 {
            matches.clear();
        }
        for (_, mut matched) in matches {
            matched.byte_len = spellings.first().map_or(0, |key| key.len());
            matched.hanja = spellings.first().copied().unwrap_or_default().into();
            if matched.selection_choices().is_empty() {
                continue;
            }
            let key = matched.default_choice().anchor.key.clone();
            let mut matched = if source_aware && opaque_registration {
                let Some(projected) = legacy(dictionary, public_projection(&matched), &key, role)
                else {
                    continue;
                };
                projected
            } else {
                matched
            };
            // The projected group's key can have a different UTF-8 width.
            // Retain the queried surface before a fallback-only pool is built.
            matched.byte_len = spellings.first().map_or(0, |key| key.len());
            matched.hanja = spellings.first().copied().unwrap_or_default().into();
            add_fallback(
                &mut fallback,
                dictionary,
                &matched,
                &key,
                offset,
                role,
                opaque,
            );
            let matched = remap(
                matched,
                offset,
                role,
                leaf || !source_aware || opaque_registration,
            );
            pools.push((matched, nested));
        }
        offset += dictionary.source_count();
    }
    combine(chain, pools, fallback)
        .into_iter()
        .map(|matched| {
            let index = spellings
                .iter()
                .position(|key| *key == matched.default_choice().anchor.key)
                .unwrap_or(0);
            (index, matched)
        })
        .collect()
}

// Source-aware lookup promises variants::matches_at that whole-spelling glyph
// alternatives have already been considered, even for a single evidence leaf.
pub(crate) fn complete_variants<D: HanjaDictionary + ?Sized>(
    dictionary: &D,
    source: &str,
    matches: Vec<SourceMatch>,
) -> Vec<SourceMatch> {
    let mut result: BTreeMap<_, _> = matches
        .into_iter()
        .map(|matched| (matched.byte_len, matched))
        .collect();
    let mut prefix_end = 0;
    let mut prefixes = alloc::vec![alloc::string::String::new()];
    for ch in source
        .chars()
        .take(crate::variants::MAX_VARIANT_PREFIX_CHARS)
    {
        prefix_end += ch.len_utf8();
        if crate::variants::extend_spellings(&mut prefixes, ch).is_none() {
            break;
        }
        if prefixes.len() == 1 {
            continue;
        }
        let spellings: Vec<_> = prefixes.iter().map(alloc::string::String::as_str).collect();
        if let Some((_, mut matched)) = dictionary
            .source_matches_at_spellings(&spellings)
            .and_then(|matches| matches.into_iter().next())
        {
            matched.byte_len = prefix_end;
            matched.hanja = source[..prefix_end].into();
            result.insert(prefix_end, matched);
        }
    }
    result.into_values().collect()
}
