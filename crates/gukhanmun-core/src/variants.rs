// Gukhanmun: Resolves and renders Han character variants.
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

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use crate::generated::hanja_variants::{
    ASAHI_FORMS, ASAHI_REVERSE_FORMS, ASAHI_TARGETS, COMPATIBILITY_FOLDS,
    COMPATIBILITY_REVERSE_FORMS, JOYO_FORMS, JOYO_REVERSE_FORMS, JOYO_TARGETS, SIMPLIFIED_FORMS,
    SIMPLIFIED_REVERSE_FORMS, TRADITIONAL_FORMS, Z_CANONICAL_FORMS, Z_FORMS, Z_REVERSE_FORMS,
};
use crate::{HanjaDictionary, Match};

pub(crate) const MAX_VARIANT_PREFIX_CHARS: usize = 32;
pub(crate) const MAX_VARIANT_CANDIDATES: usize = 256;

#[derive(Clone, Debug)]
pub(crate) struct ResolvedMatch {
    pub(crate) matched: Match,
    pub(crate) source_byte_len: usize,
    pub(crate) dictionary_hanja: String,
}

pub(crate) fn matches_at<D>(source: &str, dictionary: &D) -> Vec<ResolvedMatch>
where
    D: HanjaDictionary + ?Sized,
{
    let exact = dictionary.matches_at(source).collect::<Vec<_>>();
    let mut resolved = exact
        .iter()
        .cloned()
        .filter_map(|matched| {
            source.get(..matched.byte_len).map(|hanja| ResolvedMatch {
                source_byte_len: matched.byte_len,
                dictionary_hanja: hanja.into(),
                matched,
            })
        })
        .collect::<Vec<_>>();
    let mut candidates: Option<Vec<(String, usize)>> = None;
    let mut source_byte_len = 0;
    for source_char in source.chars().take(MAX_VARIANT_PREFIX_CHARS) {
        source_byte_len += source_char.len_utf8();
        let alternatives = recognition_variants(source_char);
        let alternatives = alternatives.as_slice();
        if let Some(current) = candidates.take() {
            if alternatives.len() == 1 {
                candidates = Some(
                    current
                        .into_iter()
                        .map(|(mut candidate, substitutions)| {
                            candidate.push(source_char);
                            (candidate, substitutions)
                        })
                        .collect(),
                );
            } else {
                let Some(next_capacity) = current.len().checked_mul(alternatives.len()) else {
                    break;
                };
                if next_capacity > MAX_VARIANT_CANDIDATES {
                    break;
                }
                let mut next = Vec::with_capacity(next_capacity);
                let (last_alternative, other_alternatives) = alternatives
                    .split_last()
                    .expect("recognition variants include the source character");
                for (mut prefix, substitutions) in current {
                    for alternative in other_alternatives {
                        let mut candidate = prefix.clone();
                        candidate.push(*alternative);
                        next.push((
                            candidate,
                            substitutions + usize::from(*alternative != source_char),
                        ));
                    }
                    prefix.push(*last_alternative);
                    next.push((
                        prefix,
                        substitutions + usize::from(*last_alternative != source_char),
                    ));
                }
                candidates = Some(next);
            }
        } else if alternatives.len() > 1 {
            let source_prefix = &source[..source_byte_len - source_char.len_utf8()];
            candidates = Some(
                alternatives
                    .iter()
                    .map(|alternative| {
                        let mut candidate = String::with_capacity(source_prefix.len() + 4);
                        candidate.push_str(source_prefix);
                        candidate.push(*alternative);
                        (candidate, usize::from(*alternative != source_char))
                    })
                    .collect(),
            );
        } else {
            continue;
        }
        let candidates = candidates
            .as_ref()
            .expect("variant candidates exist after a nontrivial alternative");
        let source_prefix = &source[..source_byte_len];
        debug_assert_eq!(
            candidates.first().map(|(candidate, _)| candidate.as_str()),
            Some(source_prefix)
        );
        let spellings = candidates
            .iter()
            .map(|(candidate, _)| candidate.as_str())
            .collect::<Vec<_>>();
        let selected = dictionary.matches_at_spellings(&spellings);
        if selected.is_empty() {
            continue;
        }
        resolved.retain(|matched| matched.source_byte_len != source_byte_len);
        if selected.iter().any(|(index, _)| *index == 0) {
            resolved.extend(selected.into_iter().map(|(_, matched)| ResolvedMatch {
                matched,
                source_byte_len,
                dictionary_hanja: source_prefix.into(),
            }));
            continue;
        }
        let mut matches = Vec::<(usize, &str, Match)>::new();
        for (candidate_index, matched) in selected {
            let (candidate, substitutions) = &candidates[candidate_index];
            matches.push((*substitutions, candidate.as_str(), matched));
        }
        matches.sort_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));
        matches.dedup_by(|left, right| left.1 == right.1 && left.2 == right.2);
        if matches.len() != 1 {
            continue;
        }
        if let Some((_, dictionary_hanja, matched)) = matches.into_iter().next() {
            resolved.push(ResolvedMatch {
                matched,
                source_byte_len,
                dictionary_hanja: dictionary_hanja.into(),
            });
        }
    }
    resolved
}

fn map_char(ch: char, table: &[(char, char)]) -> char {
    table
        .binary_search_by_key(&ch, |(from, _)| *from)
        .ok()
        .map_or(ch, |index| table[index].1)
}

fn unique_source_for_target(ch: char, reverse_table: &[(char, char)]) -> Option<char> {
    reverse_table
        .binary_search_by_key(&ch, |(target, _)| *target)
        .ok()
        .map(|index| reverse_table[index].1)
}

fn is_table_target(ch: char, targets: &[char]) -> bool {
    targets.binary_search(&ch).is_ok()
}

fn simplified_char_folded(folded: char) -> char {
    let directly_simplified = map_char(folded, SIMPLIFIED_FORMS);
    if directly_simplified != folded {
        return directly_simplified;
    }
    let old = unique_source_for_target(folded, JOYO_REVERSE_FORMS).unwrap_or(folded);
    let projected = map_char(old, SIMPLIFIED_FORMS);
    if projected == old { folded } else { projected }
}

fn simplified_char(ch: char) -> char {
    simplified_char_folded(compatibility_fold(ch))
}

fn kanxi_char_folded(folded: char) -> char {
    let old = unique_source_for_target(folded, JOYO_REVERSE_FORMS).unwrap_or(folded);
    let directly_traditional = map_char(old, TRADITIONAL_FORMS);
    let traditional = if directly_traditional == old {
        unique_source_for_target(old, SIMPLIFIED_REVERSE_FORMS).unwrap_or(old)
    } else {
        directly_traditional
    };
    map_char(compatibility_fold(traditional), Z_CANONICAL_FORMS)
}

fn kanxi_char(ch: char) -> char {
    kanxi_char_folded(compatibility_fold(ch))
}

fn asahimoji_char_folded(ch: char, folded: char) -> char {
    let asahi = map_char(ch, ASAHI_FORMS);
    if asahi != ch || is_table_target(ch, ASAHI_TARGETS) {
        return asahi;
    }
    shinjitai_char_folded(folded)
}

fn asahimoji_char(ch: char) -> char {
    asahimoji_char_folded(ch, compatibility_fold(ch))
}

const MAX_RECOGNITION_VARIANTS: usize = 10;

struct RecognitionVariants {
    values: [char; MAX_RECOGNITION_VARIANTS],
    len: usize,
}

impl RecognitionVariants {
    fn new() -> Self {
        Self {
            values: ['\0'; MAX_RECOGNITION_VARIANTS],
            len: 0,
        }
    }

    fn push_unique(&mut self, ch: char) {
        if !self.as_slice().contains(&ch) {
            debug_assert!(self.len < MAX_RECOGNITION_VARIANTS);
            self.values[self.len] = ch;
            self.len += 1;
        }
    }

    fn as_slice(&self) -> &[char] {
        &self.values[..self.len]
    }
}

fn recognition_variants(ch: char) -> RecognitionVariants {
    let mut choices = RecognitionVariants::new();
    let folded = compatibility_fold(ch);
    for variant in [
        ch,
        folded,
        map_char(ch, Z_FORMS),
        shinjitai_char_folded(folded),
        kanxi_char_folded(folded),
        simplified_char_folded(folded),
        asahimoji_char_folded(ch, folded),
    ] {
        choices.push_unique(variant);
    }
    for table in [
        COMPATIBILITY_REVERSE_FORMS,
        Z_REVERSE_FORMS,
        ASAHI_REVERSE_FORMS,
    ] {
        if let Some(source) = unique_source_for_target(ch, table) {
            choices.push_unique(source);
        }
    }
    choices
}

fn shinjitai_char_folded(folded: char) -> char {
    let directly_joyo = map_char(folded, JOYO_FORMS);
    if directly_joyo != folded || is_table_target(folded, JOYO_TARGETS) {
        return directly_joyo;
    }
    let directly_traditional = map_char(folded, TRADITIONAL_FORMS);
    let traditional = if directly_traditional == folded {
        unique_source_for_target(folded, SIMPLIFIED_REVERSE_FORMS).unwrap_or(folded)
    } else {
        directly_traditional
    };
    let joyo = map_char(traditional, JOYO_FORMS);
    if joyo != traditional {
        return joyo;
    }
    let simplified = map_char(folded, SIMPLIFIED_FORMS);
    if is_table_target(simplified, JOYO_TARGETS) {
        simplified
    } else {
        traditional
    }
}

fn shinjitai_char(ch: char) -> char {
    shinjitai_char_folded(compatibility_fold(ch))
}

pub(crate) fn compatibility_fold(ch: char) -> char {
    map_char(ch, COMPATIBILITY_FOLDS)
}

fn render_hanja_char(ch: char, variant_set: crate::HanjaVariantSet) -> char {
    match variant_set {
        crate::HanjaVariantSet::AsDictionary => unreachable!("handled by render_hanja"),
        crate::HanjaVariantSet::Shinjitai => shinjitai_char(ch),
        crate::HanjaVariantSet::Kanxi => kanxi_char(ch),
        crate::HanjaVariantSet::Simplified => simplified_char(ch),
        crate::HanjaVariantSet::Asahimoji => asahimoji_char(ch),
    }
}

pub(crate) fn render_hanja(source: &str, variant_set: crate::HanjaVariantSet) -> Cow<'_, str> {
    if variant_set == crate::HanjaVariantSet::AsDictionary {
        return Cow::Borrowed(source);
    }
    let mut chars = source.char_indices();
    while let Some((byte_index, ch)) = chars.next() {
        let rendered = render_hanja_char(ch, variant_set);
        if rendered == ch {
            continue;
        }
        let mut output = String::with_capacity(source.len());
        output.push_str(&source[..byte_index]);
        output.push(rendered);
        output.extend(chars.map(|(_, ch)| render_hanja_char(ch, variant_set)));
        return Cow::Owned(output);
    }
    Cow::Borrowed(source)
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;

    use super::render_hanja;
    use crate::HanjaVariantSet;

    #[test]
    fn as_dictionary_borrows_the_source() {
        assert!(matches!(
            render_hanja("漢字", HanjaVariantSet::AsDictionary),
            Cow::Borrowed("漢字")
        ));
    }

    #[test]
    fn normalized_variant_set_borrows_the_source() {
        assert!(matches!(
            render_hanja("芸術", HanjaVariantSet::Shinjitai),
            Cow::Borrowed("芸術")
        ));
    }

    #[test]
    fn changed_variant_set_owns_the_rendered_source() {
        assert!(matches!(
            render_hanja("藝術", HanjaVariantSet::Shinjitai),
            Cow::Owned(rendered) if rendered == "芸術"
        ));
    }
}
