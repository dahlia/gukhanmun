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

use gukhanmun_core::AliasKind;

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
    /// Categories whose compounds are not derived automatically.
    pub excluded_categories: &'static [&'static str],
}

impl Default for AliasPolicy {
    fn default() -> Self {
        Self {
            max_per_compound: 8,
            excluded_categories: &["인명", "지명", "책명", "역사"],
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
    let (donors, ambiguous) = donor_groups(sources);
    let siblings = sibling_relations(sources, &donors, &ambiguous);
    for compound in sources {
        if compound.categories.iter().any(|category| {
            policy
                .excluded_categories
                .iter()
                .any(|excluded| category == excluded)
        }) {
            continue;
        }
        let spellings = dedup(&compound.spellings);
        if spellings.len() != 1 {
            continue;
        }
        let hanja = &spellings[0];
        let Some(chunks) = align_morphemes(&compound.word, hanja) else {
            continue;
        };
        let mut for_compound = Vec::new();
        for chunk in chunks {
            if chunk.hanja.chars().count() < 2 {
                continue;
            }
            let key = (chunk.reading.clone(), chunk.hanja.clone());
            let Some((group, donor_ids)) = donors.get(&key) else {
                continue;
            };
            if ambiguous.contains(&key) {
                continue;
            }
            for alternative in group {
                let candidate = replace_span(hanja, chunk.start, &chunk.hanja, alternative);
                if candidate == *hanja {
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
                    via: (chunk.hanja.clone(), alternative.clone()),
                    compound_source_ids: compound.source_ids.clone(),
                    donor_source_ids: donor_ids.clone(),
                });
            }
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

fn donor_groups(sources: &[AliasSource]) -> (DonorMap, BTreeSet<DonorKey>) {
    let pair_ids = pair_source_ids(sources);
    let mut donors = DonorMap::new();
    let mut ambiguous = BTreeSet::new();
    for source in sources {
        let spellings = dedup(&source.spellings);
        if spellings.len() < 2 {
            continue;
        }
        let length = spellings[0].chars().count();
        if length < 2
            || spellings
                .iter()
                .any(|spelling| spelling.chars().count() != length)
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
