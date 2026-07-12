// Gukhanmun: Core IR, engine, dictionary traits, and fallback logic for Gukhanmun.
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

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::generated::unihan_readings::{KHANGUL_ALL_READINGS, KHANGUL_READINGS};
use crate::{EngineOptions, NumeralStrategy, is_hanja};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FallbackPart {
    Annotation { hanja: String, reading: String },
    ReadingText(String),
    Text(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FallbackState {
    pub(crate) starts_word: bool,
    pub(crate) previous_reading: Option<char>,
}

impl Default for FallbackState {
    fn default() -> Self {
        Self {
            starts_word: true,
            previous_reading: None,
        }
    }
}

pub(crate) fn phoneticize_fallback_run_with_state(
    run: &str,
    options: EngineOptions,
    state: &mut FallbackState,
) -> Vec<FallbackPart> {
    let chars = run.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut parts = Vec::new();

    while index < chars.len() {
        if let Some(numeral) = numeral_at(
            &chars,
            index,
            options.numeral_strategy,
            options.initial_sound_law,
        ) {
            state.starts_word = false;
            match numeral.part {
                NumeralPart::AnnotationReading(reading) => {
                    state.previous_reading = reading.chars().last();
                    parts.push(FallbackPart::Annotation {
                        hanja: chars[index..numeral.next_index].iter().collect(),
                        reading,
                    });
                }
                NumeralPart::Text(text) => {
                    state.previous_reading = text.chars().last();
                    push_reading_text(&mut parts, text);
                }
            }
            index = numeral.next_index;
            continue;
        }

        let start = index;
        while index < chars.len()
            && numeral_at(
                &chars,
                index,
                options.numeral_strategy,
                options.initial_sound_law,
            )
            .is_none()
        {
            index += 1;
        }
        let chunk = &chars[start..index];
        let chunk_parts = phoneticize_non_numeral_chunk(chunk, options.initial_sound_law, state);
        parts.extend(chunk_parts);
    }

    parts
}

pub(crate) fn fallback_reading_for_run(run: &str, options: EngineOptions) -> Option<String> {
    let mut state = FallbackState::default();
    let mut output = String::new();

    for part in phoneticize_fallback_run_with_state(run, options, &mut state) {
        match part {
            FallbackPart::Annotation { reading, .. } => output.push_str(&reading),
            FallbackPart::ReadingText(text) => output.push_str(&text),
            FallbackPart::Text(_) => return None,
        }
    }

    (!output.is_empty()).then_some(output)
}

fn phoneticize_non_numeral_chunk(
    chars: &[char],
    initial_sound_law: bool,
    state: &mut FallbackState,
) -> Vec<FallbackPart> {
    let mut parts = Vec::new();
    let mut hanja = String::new();
    let mut reading = String::new();

    for &ch in chars {
        let Some(mut char_reading) = phoneticize_hanja_char(ch).map(ToString::to_string) else {
            flush_annotation(&mut parts, &mut hanja, &mut reading);
            push_text(&mut parts, ch.to_string());
            state.starts_word = false;
            state.previous_reading = None;
            continue;
        };

        if initial_sound_law
            && (state.starts_word || should_apply_yeol_yul(state.previous_reading, &char_reading))
        {
            char_reading = apply_initial_sound_law_to_first_syllable(&char_reading);
        }

        hanja.push(ch);
        reading.push_str(&char_reading);
        state.previous_reading = char_reading.chars().last();
        state.starts_word = false;
    }

    flush_annotation(&mut parts, &mut hanja, &mut reading);
    parts
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NumeralMatch {
    next_index: usize,
    part: NumeralPart,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArabicNumeralMatch {
    pub(crate) next_index: usize,
    pub(crate) text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum NumeralPart {
    AnnotationReading(String),
    Text(String),
}

fn numeral_at(
    chars: &[char],
    index: usize,
    strategy: NumeralStrategy,
    initial_sound_law: bool,
) -> Option<NumeralMatch> {
    match strategy {
        NumeralStrategy::HangulPhonetic => {
            hangul_phonetic_numeral_at(chars, index, initial_sound_law)
        }
        NumeralStrategy::PositionalArabic
        | NumeralStrategy::AdditiveArabic
        | NumeralStrategy::Smart => {
            let arabic = if can_start_arabic_numeral(chars, index) {
                arabic_numeral_at(chars, index, strategy)
            } else {
                None
            };
            arabic
                .map(|matched| NumeralMatch {
                    next_index: matched.next_index,
                    part: NumeralPart::Text(matched.text),
                })
                .or_else(|| hangul_phonetic_numeral_at(chars, index, initial_sound_law))
        }
    }
}

pub(crate) fn arabic_numeral_at(
    chars: &[char],
    index: usize,
    strategy: NumeralStrategy,
) -> Option<ArabicNumeralMatch> {
    let matched = match strategy {
        NumeralStrategy::HangulPhonetic => return None,
        NumeralStrategy::PositionalArabic => positional_arabic_numeral_at(chars, index),
        NumeralStrategy::AdditiveArabic => additive_arabic_numeral_at(chars, index),
        NumeralStrategy::Smart => smart_additive_arabic_numeral_at(chars, index)
            .or_else(|| smart_positional_arabic_numeral_at(chars, index)),
    }?;

    let NumeralMatch { next_index, part } = matched;
    match part {
        NumeralPart::Text(text) => Some(ArabicNumeralMatch { next_index, text }),
        NumeralPart::AnnotationReading(_) => None,
    }
}

/// Returns `true` for unit/counter hanja whose presence after a short digit
/// run should trigger Arabic conversion in the `Smart` strategy.
///
/// Covers the set enumerated in the design document's numeral section
/// (`年月日時分秒號世紀`). Extend this function—and nowhere else—when the set
/// needs to grow.
pub(crate) fn is_numeral_unit(ch: char) -> bool {
    matches!(
        ch,
        '年' | '月' | '日' | '時' | '分' | '秒' | '號' | '世' | '紀'
    )
}

pub(crate) fn is_hanja_place_marker(ch: char) -> bool {
    small_place_value(ch).is_some() || large_place_value(ch).is_some()
}

pub(crate) fn can_start_arabic_numeral(chars: &[char], start_char: usize) -> bool {
    let Some(&current) = chars.get(start_char) else {
        return false;
    };
    let Some(&previous) = start_char.checked_sub(1).and_then(|index| chars.get(index)) else {
        return true;
    };
    if is_hanja_numeral(previous) || previous == '第' {
        return false;
    }
    !(is_hanja_place_marker(current) && is_hanja(previous))
}

fn smart_positional_arabic_numeral_at(chars: &[char], index: usize) -> Option<NumeralMatch> {
    let matched = positional_arabic_numeral_at(chars, index)?;
    let digit_count = matched.next_index - index;
    let followed_by_unit = chars
        .get(matched.next_index)
        .is_some_and(|&next| is_numeral_unit(next));
    (digit_count >= 4 || followed_by_unit).then_some(matched)
}

fn smart_additive_arabic_numeral_at(chars: &[char], index: usize) -> Option<NumeralMatch> {
    let first = *chars.get(index)?;
    if digit_value(first).is_some() {
        return additive_arabic_numeral_at(chars, index);
    }
    small_place_value(first)?;

    let matched = additive_arabic_numeral_at(chars, index)?;
    if matched.next_index > index + 1 {
        return Some(matched);
    }
    let followed_by_non_unit_hanja = chars
        .get(matched.next_index)
        .is_some_and(|&next| is_hanja(next) && !is_numeral_unit(next));
    (!followed_by_non_unit_hanja).then_some(matched)
}

fn hangul_phonetic_numeral_at(
    chars: &[char],
    index: usize,
    initial_sound_law: bool,
) -> Option<NumeralMatch> {
    let ch = *chars.get(index)?;
    if ch == '第'
        && chars
            .get(index + 1)
            .is_some_and(|&next| is_hanja_numeral(next))
    {
        let mut end = index + 1;
        while chars
            .get(end)
            .is_some_and(|&current| is_hanja_numeral(current))
        {
            end += 1;
        }
        let mut reading = String::from("제");
        push_numeral_readings(&mut reading, &chars[index + 1..end], initial_sound_law);
        return Some(NumeralMatch {
            next_index: end,
            part: NumeralPart::AnnotationReading(reading),
        });
    }

    if !is_hanja_numeral(ch) {
        return None;
    }

    let mut end = index + 1;
    while chars
        .get(end)
        .is_some_and(|&current| is_hanja_numeral(current))
    {
        end += 1;
    }
    let mut reading = String::new();
    push_numeral_readings(&mut reading, &chars[index..end], initial_sound_law);
    Some(NumeralMatch {
        next_index: end,
        part: NumeralPart::AnnotationReading(reading),
    })
}

fn positional_arabic_numeral_at(chars: &[char], index: usize) -> Option<NumeralMatch> {
    positional_arabic_numeral_at_min_len(chars, index, 1)
}

fn positional_arabic_numeral_at_min_len(
    chars: &[char],
    index: usize,
    min_len: usize,
) -> Option<NumeralMatch> {
    let ch = *chars.get(index)?;
    digit_value(ch)?;

    let mut end = index + 1;
    while chars
        .get(end)
        .and_then(|&current| digit_value(current))
        .is_some()
    {
        end += 1;
    }
    let len = end - index;
    if len < min_len {
        return None;
    }
    if chars
        .get(end)
        .is_some_and(|&current| is_hanja_numeral(current))
    {
        return None;
    }

    let mut text = String::new();
    for &ch in &chars[index..end] {
        let digit = digit_value(ch).expect("checked by digit-only scan");
        text.push(char::from_digit(digit.into(), 10).expect("hanja digit values are decimal"));
    }
    Some(NumeralMatch {
        next_index: end,
        part: NumeralPart::Text(text),
    })
}

fn additive_arabic_numeral_at(chars: &[char], index: usize) -> Option<NumeralMatch> {
    let ch = *chars.get(index)?;
    if !is_hanja_numeral(ch) {
        return None;
    }

    let mut end = index + 1;
    while chars
        .get(end)
        .is_some_and(|&current| is_hanja_numeral(current))
    {
        end += 1;
    }
    let value = parse_additive_numeral(&chars[index..end])?;
    Some(NumeralMatch {
        next_index: end,
        part: NumeralPart::Text(value.to_string()),
    })
}

fn parse_additive_numeral(chars: &[char]) -> Option<u128> {
    let mut total = 0u128;
    let mut section = 0u128;
    let mut current_digit = None;
    let mut last_small_unit = None;
    let mut last_large_unit = None;
    let mut has_place_marker = false;

    for &ch in chars {
        if let Some(digit) = digit_value(ch) {
            let digit = u128::from(digit);
            if let Some(previous) = current_digit
                && previous != 0
            {
                return None;
            }
            current_digit = Some(digit);
            continue;
        }

        if let Some(unit) = small_place_value(ch) {
            if last_small_unit.is_some_and(|previous| unit >= previous) {
                return None;
            }
            let digit = current_digit.take().unwrap_or(1);
            if digit == 0 {
                return None;
            }
            section = section.checked_add(digit.checked_mul(unit)?)?;
            last_small_unit = Some(unit);
            has_place_marker = true;
            continue;
        }

        if let Some(unit) = large_place_value(ch) {
            if last_large_unit.is_some_and(|previous| unit >= previous) {
                return None;
            }
            let digit = current_digit.take();
            if digit == Some(0) {
                return None;
            }
            section = section.checked_add(digit.unwrap_or(0))?;
            if section == 0 {
                section = 1;
            }
            total = total.checked_add(section.checked_mul(unit)?)?;
            section = 0;
            last_small_unit = None;
            last_large_unit = Some(unit);
            has_place_marker = true;
            continue;
        }

        return None;
    }

    if !has_place_marker {
        return None;
    }

    section = section.checked_add(current_digit.unwrap_or(0))?;
    total.checked_add(section)
}

fn push_numeral_readings(output: &mut String, chars: &[char], initial_sound_law: bool) {
    let positional = chars.iter().all(|&ch| is_positional_numeral(ch));
    for (index, &ch) in chars.iter().enumerate() {
        let mut reading = if positional {
            positional_numeral_reading(ch)
        } else if initial_sound_law {
            numeral_reading(ch)
        } else {
            canonical_numeral_reading(ch)
        }
        .expect("checked by is_hanja_numeral");

        if positional && index == 0 && initial_sound_law {
            reading = initial_sound_numeral_reading(ch).unwrap_or(reading);
        }
        output.push_str(reading);
    }
}

fn initial_sound_numeral_reading(ch: char) -> Option<&'static str> {
    let ch = normalize_hanja_char(ch);
    Some(match ch {
        '六' | '陸' | '陆' => "육",
        _ => return None,
    })
}

fn digit_value(ch: char) -> Option<u8> {
    let ch = normalize_hanja_char(ch);
    Some(match ch {
        '零' | '〇' => 0,
        '一' | '壹' | '壱' | '弌' | '夁' => 1,
        '二' | '貳' | '贰' | '弐' | '弍' | '貮' => 2,
        '三' | '參' | '叁' | '参' | '弎' | '叄' => 3,
        '四' | '肆' | '䦉' => 4,
        '五' | '伍' => 5,
        '六' | '陸' | '陆' => 6,
        '七' | '柒' | '漆' => 7,
        '八' | '捌' => 8,
        '九' | '玖' => 9,
        _ => return None,
    })
}

fn small_place_value(ch: char) -> Option<u128> {
    let ch = normalize_hanja_char(ch);
    Some(match ch {
        '十' | '拾' => 10,
        '百' | '佰' | '陌' => 100,
        '千' | '仟' | '阡' => 1000,
        _ => return None,
    })
}

fn large_place_value(ch: char) -> Option<u128> {
    let ch = normalize_hanja_char(ch);
    let exponent = match ch {
        '萬' | '万' => 4,
        '億' => 8,
        '兆' => 12,
        '京' => 16,
        '垓' => 20,
        '秭' => 24,
        '穰' => 28,
        '溝' => 32,
        '澗' => 36,
        _ => return None,
    };
    10u128.checked_pow(exponent)
}

/// Normalizes compatibility ideographs before numeral or Unihan lookup.
///
/// Gukhanmun treats them as presentation variants of their unified code
/// points, not as separate lexical characters. Compatibility-specific
/// `kHangul` values, including distinctions inherited from KS X 1001/1002,
/// are therefore ignored intentionally.
fn normalize_hanja_char(ch: char) -> char {
    crate::variants::compatibility_fold(ch)
}

pub(crate) fn phoneticize_hanja_char(ch: char) -> Option<&'static str> {
    let ch = normalize_hanja_char(ch);
    KHANGUL_READINGS
        .binary_search_by_key(&ch, |(hanja, _)| *hanja)
        .ok()
        .map(|index| KHANGUL_READINGS[index].1)
}

/// Returns every distinct Sino-Korean reading Unihan records for `ch`, with the
/// canonical (fallback) reading first.
///
/// Unlike [`phoneticize_hanja_char`], which yields only the single reading used
/// for character-by-character conversion, this exposes the full reading set so
/// the parenthetical-annotation collapser can recognise an author-supplied
/// alternative reading (for example `數字(수자)`, where `數` also reads `삭`, or
/// `議論(의론)` versus `議論(의논)`). Returns an empty slice when `ch` has no
/// recorded reading.
pub(crate) fn khangul_all_readings(ch: char) -> &'static [&'static str] {
    let ch = normalize_hanja_char(ch);
    KHANGUL_ALL_READINGS
        .binary_search_by_key(&ch, |(hanja, _)| *hanja)
        .ok()
        .map(|index| KHANGUL_ALL_READINGS[index].1)
        .unwrap_or(&[])
}

fn numeral_reading(ch: char) -> Option<&'static str> {
    let ch = normalize_hanja_char(ch);
    Some(match ch {
        '零' | '〇' => "영",
        '一' | '壹' | '壱' | '弌' | '夁' => "일",
        '二' | '貳' | '贰' | '弐' | '弍' | '貮' => "이",
        '三' | '參' | '叁' | '参' | '弎' | '叄' => "삼",
        '四' | '肆' | '䦉' => "사",
        '五' | '伍' => "오",
        '六' | '陸' | '陆' => "육",
        '七' | '柒' | '漆' => "칠",
        '八' | '捌' => "팔",
        '九' | '玖' => "구",
        '十' | '拾' => "십",
        '百' | '佰' | '陌' => "백",
        '千' | '仟' | '阡' => "천",
        '萬' | '万' => "만",
        '億' => "억",
        '兆' => "조",
        '京' => "경",
        '垓' => "해",
        '秭' => "자",
        '穰' => "양",
        '溝' => "구",
        '澗' => "간",
        _ => return None,
    })
}

fn canonical_numeral_reading(ch: char) -> Option<&'static str> {
    let ch = normalize_hanja_char(ch);
    Some(match ch {
        '零' => "영",
        '〇' => "공",
        '六' | '陸' | '陆' => "륙",
        _ => return numeral_reading(ch),
    })
}

fn positional_numeral_reading(ch: char) -> Option<&'static str> {
    let ch = normalize_hanja_char(ch);
    Some(match ch {
        '零' => "영",
        '〇' => "공",
        '一' | '壹' | '壱' | '弌' | '夁' => "일",
        '二' | '貳' | '贰' | '弐' | '弍' | '貮' => "이",
        '三' | '參' | '叁' | '参' | '弎' | '叄' => "삼",
        '四' | '肆' | '䦉' => "사",
        '五' | '伍' => "오",
        '六' | '陸' | '陆' => "륙",
        '七' | '柒' | '漆' => "칠",
        '八' | '捌' => "팔",
        '九' | '玖' => "구",
        _ => return numeral_reading(ch),
    })
}

fn is_positional_numeral(ch: char) -> bool {
    digit_value(ch).is_some()
}

pub(crate) fn is_hanja_numeral(ch: char) -> bool {
    numeral_reading(ch).is_some()
}

pub(crate) fn should_apply_yeol_yul(previous_reading: Option<char>, current_reading: &str) -> bool {
    matches!(current_reading, "렬" | "률")
        && previous_reading.is_some_and(has_no_batchim_or_nieun_batchim)
}

fn has_no_batchim_or_nieun_batchim(ch: char) -> bool {
    let Some(final_index) = hangul_final_index(ch) else {
        return false;
    };

    final_index == 0 || final_index == 4
}

pub(crate) fn apply_initial_sound_law_to_first_syllable(reading: &str) -> String {
    let mut chars = reading.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    let mut output = String::new();
    output.push(convert_initial_sound_law(first));
    output.extend(chars);
    output
}

/// Returns whether the single-syllable `reading`, after the initial sound law,
/// equals `syllable`, without allocating.
///
/// This is the allocation-free counterpart to comparing a `syllable` against
/// [`apply_initial_sound_law_to_first_syllable`]; it is used on the
/// reading-validation hot path of the parenthetical collapser.  Multi-syllable
/// readings never match, mirroring the single-syllable comparison there.
pub(crate) fn reading_matches_with_initial_sound_law(reading: &str, syllable: char) -> bool {
    let mut chars = reading.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if chars.next().is_some() {
        return false;
    }
    convert_initial_sound_law(first) == syllable
}

fn convert_initial_sound_law(sound: char) -> char {
    let Some((base, final_index)) = hangul_base_and_final(sound) else {
        return sound;
    };

    let converted_base = match base {
        '녀' => '여',
        '뇨' => '요',
        '뉴' => '유',
        '니' => '이',
        '랴' => '야',
        '려' => '여',
        '례' => '예',
        '료' => '요',
        '류' => '유',
        '리' => '이',
        '라' => '나',
        '래' => '내',
        '로' => '노',
        '뢰' => '뇌',
        '루' => '누',
        '르' => '느',
        _ => return sound,
    };

    compose_with_final(converted_base, final_index).unwrap_or(sound)
}

fn hangul_base_and_final(ch: char) -> Option<(char, u32)> {
    let code = ch as u32;
    if !(0xac00..=0xd7a3).contains(&code) {
        return None;
    }

    let syllable_index = code - 0xac00;
    let final_index = syllable_index % 28;
    let base = char::from_u32(code - final_index)?;
    Some((base, final_index))
}

fn hangul_final_index(ch: char) -> Option<u32> {
    hangul_base_and_final(ch).map(|(_, final_index)| final_index)
}

fn compose_with_final(base: char, final_index: u32) -> Option<char> {
    char::from_u32(base as u32 + final_index)
}

fn flush_annotation(parts: &mut Vec<FallbackPart>, hanja: &mut String, reading: &mut String) {
    if hanja.is_empty() {
        return;
    }

    parts.push(FallbackPart::Annotation {
        hanja: core::mem::take(hanja),
        reading: core::mem::take(reading),
    });
}

fn push_text(parts: &mut Vec<FallbackPart>, text: String) {
    if text.is_empty() {
        return;
    }

    match parts.last_mut() {
        Some(FallbackPart::Text(existing)) => existing.push_str(&text),
        _ => parts.push(FallbackPart::Text(text)),
    }
}

fn push_reading_text(parts: &mut Vec<FallbackPart>, text: String) {
    if text.is_empty() {
        return;
    }

    match parts.last_mut() {
        Some(FallbackPart::ReadingText(existing)) => existing.push_str(&text),
        _ => parts.push(FallbackPart::ReadingText(text)),
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_initial_sound_law_to_first_syllable, convert_initial_sound_law};

    #[test]
    fn initial_sound_law_preserves_batchim() {
        assert_eq!(convert_initial_sound_law('념'), '염');
        assert_eq!(convert_initial_sound_law('림'), '임');
        assert_eq!(convert_initial_sound_law('가'), '가');
    }

    #[test]
    fn initial_sound_law_changes_only_first_syllable() {
        assert_eq!(apply_initial_sound_law_to_first_syllable("량질"), "양질");
        assert_eq!(apply_initial_sound_law_to_first_syllable("미래"), "미래");
    }
}
