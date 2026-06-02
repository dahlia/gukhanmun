// Gukhanmun: Shared dictionary dump extraction helpers for Gukhanmun.
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

//! Shared extraction helpers for National Institute of Korean Language dumps.
//!
//! The Standard Korean Language Dictionary and 우리말샘 dumps differ in their
//! outer item shape, but both carry hanja source keys in compatible
//! `original_language_info` records.  This crate owns that shared key assembly
//! logic so source-specific extractors can focus on item walking and policy.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::Deserialize;

/// One `original_language_info` record from a dictionary dump item.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct OriginalLanguageInfo {
    /// Original-language spelling, such as `漢字`, `입`, or `Beijing[北京]`.
    pub original_language: Option<String>,

    /// Source language label, such as `한자`, `고유어`, or `/(병기)`.
    pub language_type: Option<String>,
}

/// Extracts all hanja-bearing lookup keys from `original_language_info`.
///
/// Native hanja and native Korean pieces are concatenated so mixed-script
/// source forms survive.  `/(병기)` boundaries flush alternate spellings into
/// separate keys.  Foreign-origin pieces contribute only standalone or
/// bracketed hanja spellings, discarding romanized text.
pub fn keys_from_originals(originals: &[OriginalLanguageInfo]) -> Option<Vec<String>> {
    let mut keys = Vec::new();
    let mut current = vec![PartialKey::default()];

    for original in originals {
        let language_type = original.language_type.as_deref().unwrap_or("");
        if language_type.contains("병기") {
            push_keys(&mut keys, &mut current);
            continue;
        }

        let pieces = original_language_pieces(original)?;
        let mut expanded = Vec::with_capacity(current.len() * pieces.alternatives.len());
        for prefix in &current {
            for piece in &pieces.alternatives {
                let mut key = prefix.key.clone();
                key.push_str(piece);
                expanded.push(PartialKey {
                    has_hanja: prefix.has_hanja || piece.chars().any(is_hanja),
                    key,
                });
            }
        }
        current = expanded;
        if pieces.boundary_after {
            push_keys(&mut keys, &mut current);
        }
    }

    push_keys(&mut keys, &mut current);
    if keys.is_empty() { None } else { Some(keys) }
}

/// Returns whether a foreign-origin source spelling contributes a hanja key.
///
/// A value made entirely of hanja is used as-is.  Otherwise bracketed hanja
/// spans such as `Beijing[北京]` are concatenated and returned; any non-hanja
/// bracket content rejects the spelling.
pub fn foreign_hanja_piece(input: &str) -> Option<String> {
    let normalized = normalize_original_language(input)?;
    if !normalized.is_empty() && normalized.chars().all(is_hanja) {
        return Some(normalized);
    }

    let mut output = String::new();
    let mut rest = normalized.as_str();
    while let Some(open) = rest.find('[') {
        let after_open = &rest[open + '['.len_utf8()..];
        let close = after_open.find(']')?;
        let candidate = &after_open[..close];
        if candidate.is_empty() || !candidate.chars().all(is_hanja) {
            return None;
        }
        output.push_str(candidate);
        rest = &after_open[close + ']'.len_utf8()..];
    }

    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

/// Normalizes a dictionary head word into the hangul reading stored in TSV.
///
/// Homograph digits, hyphen morpheme markers, and `^` spacing markers are
/// removed.  Callers that need to tolerate whitespace in source fields should
/// trim before calling this function.
pub fn normalize_word(word: &str) -> String {
    let without_number = word.trim_end_matches(|ch: char| ch.is_ascii_digit());
    without_number
        .chars()
        .filter(|ch| !matches!(ch, '-' | '^'))
        .collect()
}

/// Returns whether `ch` is treated as hanja by dictionary extractors.
pub fn is_hanja(ch: char) -> bool {
    matches!(
        ch,
        '\u{2F00}'..='\u{2FFF}'
            | '\u{3007}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
            | '\u{2CEB0}'..='\u{2EBEF}'
            | '\u{2EBF0}'..='\u{2EE5F}'
            | '\u{2F800}'..='\u{2FA1F}'
            | '\u{30000}'..='\u{3134F}'
            | '\u{31350}'..='\u{323AF}'
            | '\u{323B0}'..='\u{3347F}'
    )
}

#[derive(Clone, Debug, Default)]
struct PartialKey {
    key: String,
    has_hanja: bool,
}

#[derive(Clone, Debug)]
struct OriginalPieces {
    alternatives: Vec<String>,
    boundary_after: bool,
}

fn original_language_pieces(original: &OriginalLanguageInfo) -> Option<OriginalPieces> {
    let language_type = original.language_type.as_deref().unwrap_or("");
    let original_language = original.original_language.as_deref()?;

    match language_type {
        "한자" | "고유어" => native_original_pieces(original_language),
        _ => {
            if language_type.contains("병기") {
                Some(OriginalPieces {
                    alternatives: vec![String::new()],
                    boundary_after: true,
                })
            } else {
                foreign_hanja_piece(original_language).map(|piece| OriginalPieces {
                    alternatives: vec![piece],
                    boundary_after: false,
                })
            }
        }
    }
}

fn native_original_pieces(input: &str) -> Option<OriginalPieces> {
    let normalized = normalize_original_language(input)?;
    let boundary_after = normalized.ends_with('/');
    let normalized = normalized.trim_end_matches('/');
    Some(OriginalPieces {
        alternatives: split_inline_alternatives(normalized)?,
        boundary_after,
    })
}

fn push_keys(keys: &mut Vec<String>, current: &mut Vec<PartialKey>) {
    for key in current.drain(..) {
        if key.has_hanja && !key.key.is_empty() {
            keys.push(key.key);
        }
    }
    current.push(PartialKey::default());
}

fn normalize_original_language(input: &str) -> Option<String> {
    let mut output = String::new();
    let mut rest = input;

    while let Some(start) = rest.find("<equ>") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + "<equ>".len()..];
        let end = after_start.find("</equ>")?;
        output.push_str(decode_entity(after_start[..end].trim())?.encode_utf8(&mut [0; 4]));
        rest = &after_start[end + "</equ>".len()..];
    }

    output.push_str(rest);
    output.retain(|ch| ch != '▽');
    if output.contains('<') || output.contains('&') {
        return None;
    }
    Some(output)
}

fn split_inline_alternatives(input: &str) -> Option<Vec<String>> {
    let alternatives = input
        .split('/')
        .map(str::trim)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if alternatives.iter().any(String::is_empty) {
        None
    } else {
        Some(alternatives)
    }
}

fn decode_entity(entity: &str) -> Option<char> {
    let value = entity
        .strip_prefix("&#x")
        .and_then(|value| value.strip_suffix(';'))
        .and_then(|value| u32::from_str_radix(value, 16).ok())
        .or_else(|| {
            entity
                .strip_prefix("&#")
                .and_then(|value| value.strip_suffix(';'))
                .and_then(|value| value.parse().ok())
        })?;
    char::from_u32(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn original(original_language: &str, language_type: &str) -> OriginalLanguageInfo {
        OriginalLanguageInfo {
            original_language: Some(original_language.to_owned()),
            language_type: Some(language_type.to_owned()),
        }
    }

    #[test]
    fn native_and_hanja_pieces_form_mixed_script_keys() {
        let keys = keys_from_originals(&[
            original("입", "고유어"),
            original("口字", "한자"),
            original("집", "고유어"),
        ])
        .unwrap();

        assert_eq!(keys, ["입口字집"]);
    }

    #[test]
    fn byeonggi_boundaries_emit_alternate_keys() {
        let keys = keys_from_originals(&[
            original("溫暖", "한자"),
            original("/", "/(병기)"),
            original("溫煖", "한자"),
        ])
        .unwrap();

        assert_eq!(keys, ["溫暖", "溫煖"]);
    }

    #[test]
    fn foreign_bracketed_hanja_discards_romanized_text() {
        let keys = keys_from_originals(&[original("Beijing[北京]", "안 밝힘")]).unwrap();

        assert_eq!(keys, ["北京"]);
    }

    #[test]
    fn headword_markers_are_removed() {
        assert_eq!(normalize_word("힐난-조03"), "힐난조");
        assert_eq!(normalize_word("게임^이론"), "게임이론");
    }
}
