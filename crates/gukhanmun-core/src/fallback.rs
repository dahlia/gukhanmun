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

use crate::generated::unihan_readings::KHANGUL_READINGS;
use crate::{EngineOptions, NumeralStrategy};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FallbackPart {
    Annotation { hanja: String, reading: String },
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
            state.previous_reading = numeral.reading.chars().last();
            parts.push(FallbackPart::Annotation {
                hanja: chars[index..numeral.next_index].iter().collect(),
                reading: numeral.reading,
            });
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
    reading: String,
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
    }
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
            reading,
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
        reading,
    })
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
    Some(match ch {
        '六' | '陸' | '陆' => "육",
        _ => return None,
    })
}

fn phoneticize_hanja_char(ch: char) -> Option<&'static str> {
    KHANGUL_READINGS
        .binary_search_by_key(&ch, |(hanja, _)| *hanja)
        .ok()
        .map(|index| KHANGUL_READINGS[index].1)
}

fn numeral_reading(ch: char) -> Option<&'static str> {
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
    Some(match ch {
        '零' => "영",
        '〇' => "공",
        '六' | '陸' | '陆' => "륙",
        _ => return numeral_reading(ch),
    })
}

fn positional_numeral_reading(ch: char) -> Option<&'static str> {
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
    matches!(
        ch,
        '零' | '〇'
            | '一'
            | '壹'
            | '壱'
            | '弌'
            | '夁'
            | '二'
            | '貳'
            | '贰'
            | '弐'
            | '弍'
            | '貮'
            | '三'
            | '參'
            | '叁'
            | '参'
            | '弎'
            | '叄'
            | '四'
            | '肆'
            | '䦉'
            | '五'
            | '伍'
            | '六'
            | '陸'
            | '陆'
            | '七'
            | '柒'
            | '漆'
            | '八'
            | '捌'
            | '九'
            | '玖'
    )
}

pub(crate) fn is_hanja_numeral(ch: char) -> bool {
    numeral_reading(ch).is_some()
}

fn should_apply_yeol_yul(previous_reading: Option<char>, current_reading: &str) -> bool {
    matches!(current_reading, "렬" | "률")
        && previous_reading.is_some_and(has_no_batchim_or_nieun_batchim)
}

fn has_no_batchim_or_nieun_batchim(ch: char) -> bool {
    let Some(final_index) = hangul_final_index(ch) else {
        return false;
    };

    final_index == 0 || final_index == 4
}

fn apply_initial_sound_law_to_first_syllable(reading: &str) -> String {
    let mut chars = reading.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    let mut output = String::new();
    output.push(convert_initial_sound_law(first));
    output.extend(chars);
    output
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
