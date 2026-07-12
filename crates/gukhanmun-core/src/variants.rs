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
use alloc::vec;
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
    let chars = source
        .chars()
        .take(MAX_VARIANT_PREFIX_CHARS)
        .collect::<Vec<_>>();
    let mut candidates = vec![(String::new(), 0usize)];
    let mut source_byte_len = 0;
    for prefix_len in 1..=chars.len() {
        let source_char = chars[prefix_len - 1];
        source_byte_len += source_char.len_utf8();
        let alternatives = recognition_variants(source_char);
        let Some(next_capacity) = candidates.len().checked_mul(alternatives.len()) else {
            break;
        };
        if next_capacity > MAX_VARIANT_CANDIDATES {
            break;
        }
        let mut next = Vec::with_capacity(next_capacity);
        for (prefix, substitutions) in &candidates {
            for alternative in &alternatives {
                let mut candidate = prefix.clone();
                candidate.push(*alternative);
                next.push((
                    candidate,
                    substitutions + usize::from(*alternative != source_char),
                ));
            }
        }
        candidates = next;
        if exact
            .iter()
            .any(|matched| matched.byte_len == source_byte_len)
        {
            continue;
        }
        if candidates.len() == 1 {
            continue;
        }
        let source_prefix = &source[..source_byte_len];
        let mut matches = Vec::<(usize, String, Match)>::new();
        for (candidate, substitutions) in &candidates {
            if candidate == source_prefix {
                continue;
            }
            for matched in dictionary.matches_at(candidate) {
                if matched.byte_len == candidate.len() {
                    matches.push((*substitutions, candidate.clone(), matched));
                }
            }
        }
        matches.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
        matches.dedup_by(|left, right| left.1 == right.1 && left.2 == right.2);
        if matches.len() != 1 {
            continue;
        }
        if let Some((_, dictionary_hanja, matched)) = matches.into_iter().next() {
            resolved.push(ResolvedMatch {
                matched,
                source_byte_len,
                dictionary_hanja,
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

fn simplified_char(ch: char) -> char {
    let folded = compatibility_fold(ch);
    let directly_simplified = map_char(folded, SIMPLIFIED_FORMS);
    if directly_simplified != folded {
        return directly_simplified;
    }
    let old = unique_source_for_target(folded, JOYO_REVERSE_FORMS).unwrap_or(folded);
    let projected = map_char(old, SIMPLIFIED_FORMS);
    if projected == old { folded } else { projected }
}

fn kanxi_char(ch: char) -> char {
    let folded = compatibility_fold(ch);
    let old = unique_source_for_target(folded, JOYO_REVERSE_FORMS).unwrap_or(folded);
    let directly_traditional = map_char(old, TRADITIONAL_FORMS);
    let traditional = if directly_traditional == old {
        unique_source_for_target(old, SIMPLIFIED_REVERSE_FORMS).unwrap_or(old)
    } else {
        directly_traditional
    };
    map_char(compatibility_fold(traditional), Z_CANONICAL_FORMS)
}

fn asahimoji_char(ch: char) -> char {
    let asahi = map_char(ch, ASAHI_FORMS);
    if asahi != ch || is_table_target(ch, ASAHI_TARGETS) {
        return asahi;
    }
    shinjitai_char(ch)
}

fn push_unique(choices: &mut Vec<char>, ch: char) {
    if !choices.contains(&ch) {
        choices.push(ch);
    }
}

fn recognition_variants(ch: char) -> Vec<char> {
    let mut choices = Vec::with_capacity(9);
    for variant in [
        ch,
        compatibility_fold(ch),
        map_char(ch, Z_FORMS),
        shinjitai_char(ch),
        kanxi_char(ch),
        simplified_char(ch),
        asahimoji_char(ch),
    ] {
        push_unique(&mut choices, variant);
    }
    for table in [
        COMPATIBILITY_REVERSE_FORMS,
        Z_REVERSE_FORMS,
        ASAHI_REVERSE_FORMS,
    ] {
        if let Some(source) = unique_source_for_target(ch, table) {
            push_unique(&mut choices, source);
        }
    }
    choices
}

fn shinjitai_char(ch: char) -> char {
    let folded = compatibility_fold(ch);
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

pub(crate) fn compatibility_fold(ch: char) -> char {
    map_char(ch, COMPATIBILITY_FOLDS)
}

pub(crate) fn render_hanja(source: &str, variant_set: crate::HanjaVariantSet) -> Cow<'_, str> {
    if variant_set == crate::HanjaVariantSet::AsDictionary {
        return Cow::Borrowed(source);
    }
    Cow::Owned(
        source
            .chars()
            .map(|ch| match variant_set {
                crate::HanjaVariantSet::AsDictionary => unreachable!("handled above"),
                crate::HanjaVariantSet::Shinjitai => shinjitai_char(ch),
                crate::HanjaVariantSet::Kanxi => kanxi_char(ch),
                crate::HanjaVariantSet::Simplified => simplified_char(ch),
                crate::HanjaVariantSet::Asahimoji => asahimoji_char(ch),
            })
            .collect(),
    )
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
}
