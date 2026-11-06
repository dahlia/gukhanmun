// Gukhanmun: Compound alias derivation from dictionary spelling groups.
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

//! Derives compound spelling alternatives from explicit dictionary groups.
//!
//! The source dumps list every spelling of a headword but print only the
//! basic spelling for a compound.  This module aligns the compound's marked
//! morphemes with its hanja string, substitutes a constituent that has an
//! explicit sibling spelling, and rejects donors whose `(hanja, reading)` pair
//! is shared by another headword without the same relation.

use std::collections::{BTreeMap, BTreeSet};

use gukhanmun_core::{AliasKind, is_initial_sound_law_pair};

/// One dictionary headword or sense used as a derivation source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AliasSource {
    /// Raw headword with `-`/`^` markers and homograph digits.
    pub word: String,
    /// Normalized hangul reading.
    pub reading: String,
    /// Every explicit hanja spelling of this headword.
    pub spellings: Vec<String>,
    /// Source record identifiers that contributed this row.
    pub source_ids: BTreeSet<u64>,
    /// Dictionary category tags such as `인명` or `역사`.
    pub categories: BTreeSet<String>,
}

/// One marked morpheme aligned with its hanja substring.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chunk {
    /// Character offset of the chunk in the compound's hanja string.
    pub start: usize,
    /// Hangul reading of the chunk.
    pub reading: String,
    /// Hanja (and possibly native) substring of the compound.
    pub hanja: String,
}

/// Limits applied to one compound's derived alternatives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AliasPolicy {
    /// Maximum aliases generated for one compound.
    pub max_per_compound: usize,
    /// Maximum substitutions enumerated before one group is skipped.
    ///
    /// Grid closure and multi-chunk substitution build a Cartesian product.
    /// A pathological source could grow it beyond memory, so a group whose
    /// product exceeds this bound is skipped instead of allocated.
    pub max_combinations: usize,
    /// Categories whose compounds are not derived automatically.
    pub excluded_categories: &'static [&'static str],
    /// Excluded-category compounds reviewed individually.
    ///
    /// The dictionary tags 鐵連枷 as `역사`, which the automatic pass skips
    /// because history entries are where the worst false aliases live.  A
    /// reviewed pair may still be admitted here, as `(reading, hanja)`.
    pub approved_compounds: &'static [(&'static str, &'static str)],
    /// Suffix readings admitted as single-character donors.
    ///
    /// A one-character donor cannot distinguish the musical-note 郞 from the
    /// official-rank 郞, so the automatic set rejects them.  These suffix
    /// headwords carry no such homograph risk and are reviewed by hand.
    pub single_character_suffix_donors: &'static [&'static str],
}

impl Default for AliasPolicy {
    fn default() -> Self {
        Self {
            max_per_compound: 8,
            max_combinations: 4096,
            excluded_categories: &["인명", "지명", "책명", "역사"],
            approved_compounds: &[("철련가", "鐵連枷"), ("철연가", "鐵連枷")],
            single_character_suffix_donors: &["욕", "암", "리"],
        }
    }
}

/// One generated compound alias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedAlias {
    /// Derived hanja spelling.
    pub hanja: String,
    /// Reading shared with the canonical compound.
    pub reading: String,
    /// Canonical compound spelling that was printed by the source.
    pub canonical: String,
    /// Relation kind; automatic derivations are recognition hints.
    pub kind: AliasKind,
    /// Substituted constituent and its replacement.
    pub via: (String, String),
    /// Source records of the compound.
    pub compound_source_ids: BTreeSet<u64>,
    /// Source records of the donor headword.
    pub donor_source_ids: BTreeSet<u64>,
}

type DonorKey = (String, String);
type DonorGroup = (BTreeSet<String>, BTreeSet<u64>);
type DonorMap = BTreeMap<DonorKey, DonorGroup>;

/// One sibling relation for an existing headword spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SiblingRelation {
    /// Non-first spelling of the headword.
    pub hanja: String,
    /// Shared reading.
    pub reading: String,
    /// First spelling of the headword.
    pub canonical: String,
    /// Source records of the headword.
    pub source_ids: BTreeSet<u64>,
}

/// Aligns a compound's marked chunks with its hanja string.
///
/// Returns `None` when a marker chunk is empty, a chunk contains a number, or
/// the chunk lengths do not sum to the hanja length.
pub fn align_morphemes(word: &str, hanja: &str) -> Option<Vec<Chunk>> {
    let hanja_chars = hanja.chars().count();
    let mut chunks = Vec::new();
    let mut offset = 0;
    for part in word.split(['-', '^']) {
        let trimmed = part.trim_end_matches(|ch: char| ch.is_ascii_digit());
        if trimmed.is_empty() {
            return None;
        }
        let len = trimmed.chars().count();
        if offset + len > hanja_chars {
            return None;
        }
        let chunk_hanja = hanja.chars().skip(offset).take(len).collect::<String>();
        chunks.push(Chunk {
            start: offset,
            reading: trimmed.to_owned(),
            hanja: chunk_hanja,
        });
        offset += len;
    }
    (offset == hanja_chars).then_some(chunks)
}

/// Derives missing position-wise combinations inside two-spelling headwords.
///
/// The dictionary prints the first and last combination of a two-component
/// spelling grid.  This function reconstructs the omitted intermediate
/// combinations only when the group has one reading, exactly two equal-length
/// spellings that differ in at least two positions, neither spelling is a
/// doubled character, and every spelling belongs to the same source-id set.
pub fn grid_aliases(
    sources: &[AliasSource],
    readings: &BTreeMap<String, BTreeSet<String>>,
    policy: &AliasPolicy,
) -> Vec<DerivedAlias> {
    let pair_ids = pair_source_ids(sources);
    let mut readings_by_set = BTreeMap::<BTreeSet<String>, BTreeSet<String>>::new();
    let mut groups =
        BTreeMap::<(String, BTreeSet<String>), (String, BTreeSet<u64>, BTreeSet<String>)>::new();
    for source in sources {
        let ordered = dedup(&source.spellings);
        if ordered.len() < 2 {
            continue;
        }
        let set = ordered.iter().cloned().collect::<BTreeSet<_>>();
        readings_by_set
            .entry(set.clone())
            .or_default()
            .insert(source.reading.clone());
        if ordered.len() != 2 {
            continue;
        }
        let length = ordered[0].chars().count();
        if length < 2 || ordered[1].chars().count() != length {
            continue;
        }
        let differing = ordered[0]
            .chars()
            .zip(ordered[1].chars())
            .filter(|(left, right)| left != right)
            .count();
        if differing < 2 || difference_is_doubled(&ordered[0], &ordered[1]) {
            continue;
        }
        let entry = groups
            .entry((source.reading.clone(), set))
            .or_insert_with(|| (ordered[0].clone(), BTreeSet::new(), BTreeSet::new()));
        entry.1.extend(source.source_ids.iter().copied());
        entry.2.extend(source.categories.iter().cloned());
    }
    let mut aliases = Vec::new();
    for ((reading, set), (canonical, source_ids, categories)) in groups {
        if readings_by_set
            .get(&set)
            .is_some_and(|readings| readings.len() > 1)
            || categories.iter().any(|category| {
                policy
                    .excluded_categories
                    .iter()
                    .any(|excluded| category == excluded)
            })
            || set.iter().any(|spelling| {
                pair_ids.get(&(reading.clone(), spelling.clone())) != Some(&source_ids)
            })
        {
            continue;
        }
        let spellings = set.iter().cloned().collect::<Vec<_>>();
        let columns = (0..canonical.chars().count())
            .map(|index| {
                spellings
                    .iter()
                    .filter_map(|spelling| spelling.chars().nth(index))
                    .collect::<BTreeSet<_>>()
            })
            .collect::<Vec<_>>();
        let mut combinations = 1usize;
        for column in &columns {
            combinations = combinations.saturating_mul(column.len());
            if combinations > policy.max_combinations {
                break;
            }
        }
        if combinations > policy.max_combinations {
            continue;
        }
        let mut products = vec![String::new()];
        for column in columns {
            products = products
                .into_iter()
                .flat_map(|prefix| {
                    column.iter().map(move |ch| {
                        let mut next = prefix.clone();
                        next.push(*ch);
                        next
                    })
                })
                .collect();
        }
        let mut group_aliases = Vec::new();
        for candidate in products {
            if set.contains(&candidate) || readings.contains_key(&candidate) {
                continue;
            }
            group_aliases.push(DerivedAlias {
                hanja: candidate.clone(),
                reading: reading.clone(),
                canonical: canonical.clone(),
                kind: AliasKind::Recognition,
                via: ("grid".into(), candidate),
                compound_source_ids: source_ids.clone(),
                donor_source_ids: source_ids.clone(),
            });
        }
        group_aliases.truncate(policy.max_per_compound);
        aliases.extend(group_aliases);
    }
    aliases
}

fn difference_is_doubled(left: &str, right: &str) -> bool {
    let differing = left
        .chars()
        .zip(right.chars())
        .filter(|(left, right)| left != right)
        .collect::<Vec<_>>();
    if differing.is_empty() {
        return false;
    }
    let left_first = differing[0].0;
    let right_first = differing[0].1;
    differing
        .iter()
        .all(|(left, right)| *left == left_first && *right == right_first)
}

/// Derives compound aliases and sibling relations from source rows.
///
/// `sources` should contain every admitted headword from one dictionary
/// category.  The function does not recurse: it only reads source spellings,
/// never its own output.
pub fn derive_aliases(
    sources: &[AliasSource],
    policy: &AliasPolicy,
) -> (Vec<DerivedAlias>, Vec<SiblingRelation>) {
    let readings = readings_by_spelling(sources);
    let mut aliases = grid_aliases(sources, &readings, policy);
    let (donors, ambiguous) = donor_groups(sources, policy);
    let by_hanja = donor_hanja_index(&donors, &ambiguous);
    let siblings = sibling_relations(sources, &donors, &ambiguous);
    for compound in sources {
        let spellings = dedup(&compound.spellings);
        if spellings.len() != 1 {
            continue;
        }
        let hanja = &spellings[0];
        let excluded = compound.categories.iter().any(|category| {
            policy
                .excluded_categories
                .iter()
                .any(|excluded| category == excluded)
        });
        if excluded
            && !policy
                .approved_compounds
                .contains(&(compound.reading.as_str(), hanja.as_str()))
        {
            continue;
        }
        let Some(chunks) = align_morphemes(&compound.word, hanja) else {
            continue;
        };
        let mut substitutable = Vec::new();
        for chunk in chunks {
            if chunk.hanja.is_empty() {
                continue;
            }
            let Some((group, ids)) = donor_lookup(&donors, &ambiguous, &by_hanja, &chunk) else {
                continue;
            };
            substitutable.push((chunk, group.clone(), ids.clone()));
        }
        if substitutable.is_empty() {
            continue;
        }
        let mut combinations = 1usize;
        for (_, group, _) in &substitutable {
            combinations = combinations.saturating_mul(group.len());
            if combinations > policy.max_combinations {
                break;
            }
        }
        if combinations > policy.max_combinations {
            continue;
        }
        // Each product carries the donors whose replacement it actually used,
        // so a partially substituted compound does not credit the unused ones.
        let mut products = vec![(
            hanja.clone(),
            Vec::<(String, String)>::new(),
            BTreeSet::new(),
        )];
        for (chunk, group, ids) in &substitutable {
            let mut next = Vec::new();
            for (current, via, used) in &products {
                next.push((current.clone(), via.clone(), used.clone()));
                for alternative in group {
                    if alternative == &chunk.hanja {
                        continue;
                    }
                    let mut replaced = via.clone();
                    replaced.push((chunk.hanja.clone(), alternative.clone()));
                    let mut credited = used.clone();
                    credited.extend(ids.iter().copied());
                    next.push((
                        replace_span(current, chunk.start, &chunk.hanja, alternative),
                        replaced,
                        credited,
                    ));
                }
            }
            products = next;
        }
        let mut for_compound = Vec::new();
        for (candidate, via, donor_ids) in products {
            if candidate == *hanja || via.is_empty() {
                continue;
            }
            if readings.contains_key(&candidate) {
                continue;
            }
            for_compound.push(DerivedAlias {
                hanja: candidate,
                reading: compound.reading.clone(),
                canonical: hanja.clone(),
                kind: AliasKind::Recognition,
                via: join_via(&via),
                compound_source_ids: compound.source_ids.clone(),
                donor_source_ids: donor_ids,
            });
        }
        for_compound.sort_by(|left, right| {
            (&left.hanja, &left.reading).cmp(&(&right.hanja, &right.reading))
        });
        for_compound
            .dedup_by(|left, right| left.hanja == right.hanja && left.reading == right.reading);
        for_compound.truncate(policy.max_per_compound);
        aliases.extend(for_compound);
    }
    aliases
        .sort_by(|left, right| (&left.hanja, &left.reading).cmp(&(&right.hanja, &right.reading)));
    // Two compounds can derive the same alias from different canonical
    // spellings.  Keeping either would make the canonical depend on input
    // order, so the key is quarantined instead.
    let mut canonicals = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for alias in &aliases {
        canonicals
            .entry((alias.hanja.clone(), alias.reading.clone()))
            .or_default()
            .insert(alias.canonical.clone());
    }
    aliases.retain(|alias| {
        canonicals
            .get(&(alias.hanja.clone(), alias.reading.clone()))
            .is_some_and(|set| set.len() == 1)
    });
    aliases.dedup_by(|left, right| left.hanja == right.hanja && left.reading == right.reading);
    (aliases, siblings)
}

/// Returns sibling relations for non-first spellings of one headword.
///
/// A relation is emitted only when every spelling of the headword carries the
/// same source-id set.  Otherwise the non-first spelling also belongs to a
/// separate homograph and must not be collapsed into the first spelling.
pub fn sibling_relations(
    sources: &[AliasSource],
    donors: &DonorMap,
    ambiguous: &BTreeSet<DonorKey>,
) -> Vec<SiblingRelation> {
    let pair_ids = pair_source_ids(sources);
    let mut relations = Vec::new();
    for source in sources {
        let spellings = dedup(&source.spellings);
        if spellings.len() < 2 {
            continue;
        }
        let Some(first) = spellings.first() else {
            continue;
        };
        let key = (source.reading.clone(), first.clone());
        if !donors.contains_key(&key) || ambiguous.contains(&key) {
            continue;
        }
        for spelling in spellings.iter().skip(1) {
            let Some(ids) = pair_ids.get(&(source.reading.clone(), spelling.clone())) else {
                continue;
            };
            if ids != &source.source_ids {
                continue;
            }
            relations.push(SiblingRelation {
                hanja: spelling.clone(),
                reading: source.reading.clone(),
                canonical: first.clone(),
                source_ids: source.source_ids.clone(),
            });
        }
    }
    relations
        .sort_by(|left, right| (&left.hanja, &left.reading).cmp(&(&right.hanja, &right.reading)));
    relations.dedup_by(|left, right| left.hanja == right.hanja && left.reading == right.reading);
    relations
}

type DonorHanjaIndex = BTreeMap<String, Vec<String>>;

/// Groups every donor and ambiguous reading by hanja.
///
/// The sound-law fallback only consults entries whose hanja matches the
/// chunk, so indexing once keeps the lookup proportional to that hanja rather
/// than to the whole dictionary.
fn donor_hanja_index(donors: &DonorMap, ambiguous: &BTreeSet<DonorKey>) -> DonorHanjaIndex {
    let mut index = DonorHanjaIndex::new();
    for (reading, hanja) in donors.keys().chain(ambiguous.iter()) {
        index
            .entry(hanja.clone())
            .or_default()
            .push(reading.clone());
    }
    index
}

/// Looks up the donor group that licenses substitutions for one chunk.
///
/// The exact `(reading, hanja)` key wins.  A non-initial chunk may instead
/// match a donor whose reading differs only by the word-initial sound law,
/// which is how 철-련가 (鐵連枷) reaches the 연가03 group (連枷/連耞).  Two
/// law-related donors, or one ambiguous donor, refuse the lookup.
fn donor_lookup<'a>(
    donors: &'a DonorMap,
    ambiguous: &BTreeSet<DonorKey>,
    by_hanja: &DonorHanjaIndex,
    chunk: &Chunk,
) -> Option<&'a DonorGroup> {
    let key = (chunk.reading.clone(), chunk.hanja.clone());
    if ambiguous.contains(&key) {
        return None;
    }
    if let Some(group) = donors.get(&key) {
        return Some(group);
    }
    if chunk.start == 0 {
        return None;
    }
    let law_related = |reading: &str| {
        is_initial_sound_law_pair(reading, &chunk.reading)
            || is_initial_sound_law_pair(&chunk.reading, reading)
    };
    let mut found = None;
    for reading in by_hanja.get(&chunk.hanja)? {
        if !law_related(reading) {
            continue;
        }
        let key = (reading.clone(), chunk.hanja.clone());
        if ambiguous.contains(&key) {
            return None;
        }
        if found.is_some() {
            return None;
        }
        found = donors.get(&key);
    }
    found
}

fn join_via(pairs: &[(String, String)]) -> (String, String) {
    let from = pairs
        .iter()
        .map(|pair| pair.0.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let to = pairs
        .iter()
        .map(|pair| pair.1.as_str())
        .collect::<Vec<_>>()
        .join(",");
    (from, to)
}

fn dedup(values: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .filter(|value| seen.insert((*value).clone()))
        .cloned()
        .collect()
}

fn readings_by_spelling(sources: &[AliasSource]) -> BTreeMap<String, BTreeSet<String>> {
    let mut readings = BTreeMap::<String, BTreeSet<String>>::new();
    for source in sources {
        for spelling in &source.spellings {
            readings
                .entry(spelling.clone())
                .or_default()
                .insert(source.reading.clone());
        }
    }
    readings
}

fn pair_source_ids(sources: &[AliasSource]) -> BTreeMap<(String, String), BTreeSet<u64>> {
    let mut ids = BTreeMap::<(String, String), BTreeSet<u64>>::new();
    for source in sources {
        for spelling in &source.spellings {
            ids.entry((source.reading.clone(), spelling.clone()))
                .or_default()
                .extend(source.source_ids.iter().copied());
        }
    }
    ids
}

fn donor_groups(sources: &[AliasSource], policy: &AliasPolicy) -> (DonorMap, BTreeSet<DonorKey>) {
    let pair_ids = pair_source_ids(sources);
    let mut donors = DonorMap::new();
    let mut ambiguous = BTreeSet::new();
    for source in sources {
        let spellings = dedup(&source.spellings);
        if spellings.len() < 2 {
            continue;
        }
        let length = spellings[0].chars().count();
        if spellings
            .iter()
            .any(|spelling| spelling.chars().count() != length)
        {
            continue;
        }
        if length < 2
            && !(source.word.starts_with('-')
                && policy
                    .single_character_suffix_donors
                    .contains(&source.reading.as_str()))
        {
            continue;
        }
        let group = spellings.iter().cloned().collect::<BTreeSet<_>>();
        for spelling in &spellings {
            let key = (source.reading.clone(), spelling.clone());
            if pair_ids.get(&key) != Some(&source.source_ids) {
                ambiguous.insert(key);
                continue;
            }
            donors.insert(key, (group.clone(), source.source_ids.clone()));
        }
    }
    (donors, ambiguous)
}

fn replace_span(hanja: &str, start: usize, original: &str, replacement: &str) -> String {
    let prefix = hanja.chars().take(start).collect::<String>();
    let suffix = hanja
        .chars()
        .skip(start + original.chars().count())
        .collect::<String>();
    format!("{prefix}{replacement}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(word: &str, reading: &str, spellings: &[&str], ids: &[u64]) -> AliasSource {
        AliasSource {
            word: word.into(),
            reading: reading.into(),
            spellings: spellings.iter().map(|value| (*value).into()).collect(),
            source_ids: ids.iter().copied().collect(),
            categories: BTreeSet::new(),
        }
    }

    #[test]
    fn aligns_marked_chunks_with_hanja() {
        let chunks = align_morphemes("기념-관", "紀念館").unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].reading, "기념");
        assert_eq!(chunks[0].hanja, "紀念");
        assert_eq!(chunks[1].reading, "관");
        assert_eq!(chunks[1].hanja, "館");
        assert!(align_morphemes("기념관", "紀念館").is_some());
        assert!(align_morphemes("기념-관", "紀念").is_none());
    }

    #[test]
    fn derives_a_reviewed_compound_alias() {
        let sources = vec![
            source("기념", "기념", &["紀念", "記念"], &[1]),
            source("기념-관", "기념관", &["紀念館"], &[2]),
        ];
        let (aliases, siblings) = derive_aliases(&sources, &AliasPolicy::default());
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].hanja, "記念館");
        assert_eq!(aliases[0].reading, "기념관");
        assert_eq!(aliases[0].kind, AliasKind::Recognition);
        assert_eq!(siblings.len(), 1);
        assert_eq!(siblings[0].hanja, "記念");
    }

    #[test]
    fn skips_an_ambiguous_donor() {
        let mut first = source("삼도", "삼도", &["三道"], &[10]);
        first.source_ids = [10].into();
        let second = source("삼도", "삼도", &["三道", "三途"], &[11]);
        let compound = source("삼남-삼도", "삼남삼도", &["三南三道"], &[20]);
        let (aliases, _) = derive_aliases(&[first, second, compound], &AliasPolicy::default());
        assert!(aliases.is_empty());
    }

    #[test]
    fn expands_every_substitutable_chunk() {
        let sources = vec![
            source("사기", "사기", &["沙器", "砂器"], &[1]),
            source("진주", "진주", &["眞珠", "眞朱"], &[2]),
            source("사기-진주", "사기진주", &["沙器眞珠"], &[3]),
        ];
        let (aliases, _) = derive_aliases(&sources, &AliasPolicy::default());
        let spellings = aliases
            .iter()
            .map(|alias| alias.hanja.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            spellings,
            BTreeSet::from(["沙器眞朱", "砂器眞珠", "砂器眞朱"])
        );
        let combined = aliases
            .iter()
            .find(|alias| alias.hanja == "砂器眞朱")
            .expect("combined alias");
        assert_eq!(combined.via, ("沙器,眞珠".into(), "砂器,眞朱".into()));
    }

    #[test]
    fn matches_a_non_initial_sound_law_donor() {
        let sources = vec![
            source("연가", "연가", &["連枷", "連耞"], &[1]),
            source("철-련가", "철련가", &["鐵連枷"], &[2]),
        ];
        let (aliases, _) = derive_aliases(&sources, &AliasPolicy::default());
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].hanja, "鐵連耞");
        assert_eq!(aliases[0].reading, "철련가");
        assert_eq!(aliases[0].canonical, "鐵連枷");
    }

    #[test]
    fn skips_products_that_exceed_the_combination_bound() {
        let left = "甲乙".repeat(13);
        let right = "丙丁".repeat(13);
        let sources = vec![source("가나", "가나", &[&left, &right], &[1])];
        let (aliases, _) = derive_aliases(&sources, &AliasPolicy::default());
        assert!(aliases.is_empty());

        let compound = vec!["가나"; 13].join("-");
        let hanja = "甲乙".repeat(13);
        // One differing position keeps grid closure out of the picture, so the
        // only possible aliases are the compound's 2^13 substitutions.
        let sources = vec![
            source("가나", "가나", &["甲乙", "甲丁"], &[1]),
            source(&compound, &compound.replace('-', ""), &[&hanja], &[2]),
        ];
        let (aliases, _) = derive_aliases(&sources, &AliasPolicy::default());
        assert!(aliases.is_empty());
    }

    #[test]
    fn refuses_an_ambiguous_donor_before_the_sound_law_fallback() {
        let sources = vec![
            source("련가04", "련가", &["連枷"], &[10]),
            source("련가05", "련가", &["連枷", "連耞"], &[11]),
            source("연가", "연가", &["連枷", "連耞"], &[1]),
            source("철-련가", "철련가", &["鐵連枷"], &[2]),
        ];
        let (aliases, _) = derive_aliases(&sources, &AliasPolicy::default());
        assert!(aliases.is_empty());

        let sources = vec![
            source("련가04", "련가", &["連枷"], &[10]),
            source("련가05", "련가", &["連枷", "連耞"], &[11]),
            source("련가-철", "련가철", &["連枷鐵"], &[2]),
        ];
        let (aliases, _) = derive_aliases(&sources, &AliasPolicy::default());
        assert!(aliases.is_empty());
    }

    #[test]
    fn quarantines_aliases_with_conflicting_canonical_spellings() {
        let donor = source("가나", "가나", &["甲乙", "丙丁", "戊己"], &[1]);
        let single = source("가나-무", "가나무", &["甲乙무"], &[2]);
        let (aliases, _) =
            derive_aliases(&[donor.clone(), single.clone()], &AliasPolicy::default());
        let spellings = aliases
            .iter()
            .map(|alias| (alias.hanja.as_str(), alias.canonical.as_str()))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            spellings,
            BTreeSet::from([("丙丁무", "甲乙무"), ("戊己무", "甲乙무")])
        );

        // The second compound derives 丙丁무 from a different canonical
        // spelling, so neither canonical claim can be trusted.
        let other = source("가나-무", "가나무", &["戊己무"], &[3]);
        let (aliases, _) = derive_aliases(&[donor, single, other], &AliasPolicy::default());
        assert!(aliases.is_empty());
    }

    #[test]
    fn credits_only_the_donors_a_product_uses() {
        let sources = vec![
            source("사기", "사기", &["沙器", "砂器"], &[1]),
            source("진주", "진주", &["眞珠", "眞朱"], &[2]),
            source("사기-진주", "사기진주", &["沙器眞珠"], &[3]),
        ];
        let (aliases, _) = derive_aliases(&sources, &AliasPolicy::default());
        let partial = aliases
            .iter()
            .find(|alias| alias.hanja == "砂器眞珠")
            .expect("partial alias");
        assert_eq!(partial.donor_source_ids, BTreeSet::from([1]));
        let combined = aliases
            .iter()
            .find(|alias| alias.hanja == "砂器眞朱")
            .expect("combined alias");
        assert_eq!(combined.donor_source_ids, BTreeSet::from([1, 2]));
    }

    #[test]
    fn admits_only_reviewed_excluded_compounds() {
        let donor = source("연가", "연가", &["連枷", "連耞"], &[1]);
        let mut compound = source("철-련가", "철련가", &["鐵連枷"], &[2]);
        compound.categories.insert("역사".into());
        let (aliases, _) = derive_aliases(&[donor.clone(), compound], &AliasPolicy::default());
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].hanja, "鐵連耞");

        let mut unreviewed = source("철-려가", "철려가", &["鐵連枷"], &[3]);
        unreviewed.categories.insert("역사".into());
        let (aliases, _) = derive_aliases(&[donor, unreviewed], &AliasPolicy::default());
        assert!(aliases.is_empty());
    }

    #[test]
    fn admits_reviewed_single_character_suffix_donors() {
        let donor = source("-욕", "욕", &["慾", "欲"], &[1]);
        let compound = source("탐-욕", "탐욕", &["貪慾"], &[2]);
        let (aliases, _) = derive_aliases(&[donor, compound], &AliasPolicy::default());
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].hanja, "貪欲");
        assert_eq!(aliases[0].reading, "탐욕");

        let unreviewed = source("-가", "가", &["價", "家"], &[3]);
        let compound = source("시-가", "시가", &["時價"], &[4]);
        let (aliases, _) = derive_aliases(&[unreviewed, compound], &AliasPolicy::default());
        assert!(aliases.is_empty());
    }

    #[test]
    fn skips_single_character_donors_and_excluded_categories() {
        let mut donor = source("랑", "랑", &["郞", "浪"], &[1]);
        donor.source_ids = [1].into();
        let compound = source("봉례-랑", "봉례랑", &["奉禮郞"], &[2]);
        let (aliases, _) = derive_aliases(&[donor, compound], &AliasPolicy::default());
        assert!(aliases.is_empty());

        let mut name = source("기념-관", "기념관", &["紀念館"], &[3]);
        name.categories.insert("인명".into());
        let donor = source("기념", "기념", &["紀念", "記念"], &[4]);
        let (aliases, _) = derive_aliases(&[donor, name], &AliasPolicy::default());
        assert!(aliases.is_empty());
    }
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    fn source(word: &str, reading: &str, spellings: &[&str], ids: &[u64]) -> AliasSource {
        AliasSource {
            word: word.into(),
            reading: reading.into(),
            spellings: spellings.iter().map(|value| (*value).into()).collect(),
            source_ids: ids.iter().copied().collect(),
            categories: BTreeSet::new(),
        }
    }

    #[test]
    fn grid_closure_recovers_omitted_combinations() {
        let sources = vec![source("마비", "마비", &["痲痹", "麻痺"], &[1])];
        let (aliases, _) = derive_aliases(&sources, &AliasPolicy::default());
        let spellings = aliases
            .iter()
            .map(|alias| alias.hanja.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(spellings, BTreeSet::from(["痲痺", "麻痹"]));
    }

    #[test]
    fn grid_closure_skips_multi_reading_groups() {
        let sources = vec![
            source("마비", "마비", &["痲痹", "麻痺"], &[1]),
            source("마비", "마비2", &["麻痺", "痲痹"], &[2]),
        ];
        let (aliases, _) = derive_aliases(&sources, &AliasPolicy::default());
        assert!(aliases.is_empty());
    }

    #[test]
    fn grid_closure_skips_three_spellings_and_doubled_words() {
        let three = vec![source(
            "정수정전",
            "정수정전",
            &["鄭秀貞傳", "鄭壽貞傳", "鄭水晶傳"],
            &[1],
        )];
        let (aliases, _) = derive_aliases(&three, &AliasPolicy::default());
        assert!(aliases.is_empty());
        for pair in [
            ("황황", "煌煌", "晃晃"),
            ("황황하다", "煌煌하다", "晃晃하다"),
            ("우우탄탄", "吁吁歎歎", "吁吁嘆嘆"),
        ] {
            let doubled = vec![source(pair.0, pair.0, &[pair.1, pair.2], &[2])];
            let (aliases, _) = derive_aliases(&doubled, &AliasPolicy::default());
            assert!(aliases.is_empty(), "{}", pair.0);
        }
    }

    #[test]
    fn grid_closure_skips_shared_homographs_and_excluded_categories() {
        let shared = vec![
            source("훈육", "훈육", &["薰育", "獯鬻"], &[1]),
            source("훈육", "훈육", &["薰育"], &[2]),
        ];
        let (aliases, _) = derive_aliases(&shared, &AliasPolicy::default());
        assert!(aliases.is_empty());
        let mut name = source("마비", "마비", &["痲痹", "麻痺"], &[3]);
        name.categories.insert("인명".into());
        let (aliases, _) = derive_aliases(&[name], &AliasPolicy::default());
        assert!(aliases.is_empty());
    }
}
