// Gukhanmun: Exact boundaries for mixed-script annotations.
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

use alloc::vec::Vec;
use core::ops::Range;

use crate::{is_hangul_syllable, is_hanja};

const SYLLABLE_BYTES: usize = 3;

fn runs(mut spelling: &str) -> impl Iterator<Item = (bool, &str)> {
    core::iter::from_fn(move || {
        let mut chars = spelling.char_indices();
        let (_, first) = chars.next()?;
        let hanja = is_hanja(first);
        let end = chars
            .find(|&(_, ch)| is_hanja(ch) != hanja)
            .map_or(spelling.len(), |(i, _)| i);
        let (run, rest) = spelling.split_at(end);
        spelling = rest;
        Some((hanja, run))
    })
}

pub(super) struct Alignment<'a> {
    spelling: &'a str,
    reading: &'a str,
    anchors: Vec<(&'a str, Range<usize>)>,
}

impl<'a> Alignment<'a> {
    /// Yields (is_hanja, spelling, reading). Only hanja runs need a gloss.
    pub(super) fn parts(&self) -> impl Iterator<Item = (bool, &'a str, &'a str)> + '_ {
        let mut anchors = self.anchors.iter().peekable();
        let mut cursor = 0;
        runs(self.spelling).map(move |(hanja, spelling)| {
            let end = if hanja {
                anchors
                    .peek()
                    .map_or(self.reading.len(), |(_, range)| range.start)
            } else {
                anchors.next().expect("one anchor per hangul run").1.end
            };
            let reading = &self.reading[cursor..end];
            cursor = end;
            (hanja, spelling, reading)
        })
    }
}

/// Infers boundaries only when both successive extremes of the exact
/// alignment coincide. No individual ideograph-to-syllable mapping is assumed.
pub(super) fn align<'a>(
    source: &str,
    canonical: &str,
    spelling: &'a str,
    reading: &'a str,
) -> Option<Alignment<'a>> {
    let valid = |text: &str| {
        text.chars()
            .all(|ch| is_hanja(ch) || is_hangul_syllable(ch))
    };
    if !valid(spelling)
        || (source != spelling && !valid(source))
        || (canonical != spelling && canonical != source && !valid(canonical))
    {
        return None;
    }
    let first = spelling.chars().next()?;
    let last = spelling.chars().next_back()?;
    if !spelling.chars().any(is_hanja) || !spelling.chars().any(is_hangul_syllable) {
        return None;
    }
    if reading.is_empty() || !reading.chars().all(is_hangul_syllable) {
        return None;
    }
    for other in [source, canonical] {
        if other != spelling
            && !runs(other)
                .map(|(hanja, run)| (!hanja).then_some(run))
                .eq(runs(spelling).map(|(hanja, run)| (!hanja).then_some(run)))
        {
            return None;
        }
    }

    let mut left = 0;
    let mut right = reading.len();
    // Consume fixed anchors up front so interior searches cannot overlap them.
    let head = if is_hanja(first) {
        None
    } else {
        let (_, head) = runs(spelling).next()?;
        reading.strip_prefix(head)?;
        left = head.len();
        Some(head)
    };
    let tail = if is_hanja(last) {
        None
    } else {
        let (_, tail) = runs(spelling).last()?;
        right = reading.strip_suffix(tail)?.len();
        Some(tail)
    };
    let middle = &spelling[left..spelling.len() - tail.map_or(0, str::len)];
    let mut anchors = Vec::new();
    let mut table = Vec::new();
    let mut cursor = left;
    for (hanja, run) in runs(middle) {
        if hanja {
            cursor = cursor.checked_add(SYLLABLE_BYTES)?;
            if cursor > right {
                return None;
            }
        } else {
            let offset = find::<false>(
                &reading.as_bytes()[cursor..right],
                run.as_bytes(),
                &mut table,
            )?;
            let start = cursor + offset;
            cursor = start + run.len();
            anchors.push((run, start..cursor));
        }
    }

    let mut cursor = right;
    for &(anchor, ref earliest) in anchors.iter().rev() {
        let limit = cursor.checked_sub(SYLLABLE_BYTES)?;
        let window = reading.as_bytes().get(left..limit)?;
        let offset = find::<true>(window, anchor.as_bytes(), &mut table)?;
        cursor = limit - offset - anchor.len();
        if cursor != earliest.start {
            // The forward scan proved a solution exists. The latest feasible
            // placement therefore exists too, even if we stop comparing now.
            return None;
        }
    }
    if let Some(head) = head {
        anchors.insert(0, (head, 0..left));
    }
    if let Some(tail) = tail {
        anchors.push((tail, right..reading.len()));
    }
    Some(Alignment {
        spelling,
        reading,
        anchors,
    })
}

// KMP in an indexed view; reverse both byte sequences without copying them.
// Exact matches of a valid UTF-8 needle still map to valid original offsets.
fn find<const REVERSE: bool>(text: &[u8], needle: &[u8], table: &mut Vec<usize>) -> Option<usize> {
    let at = |bytes: &[u8], i: usize| bytes[if REVERSE { bytes.len() - 1 - i } else { i }];
    table.clear();
    table.push(0);
    let mut matched = 0;
    for i in 1..needle.len() {
        while matched > 0 && at(needle, matched) != at(needle, i) {
            matched = table[matched - 1];
        }
        matched += usize::from(at(needle, matched) == at(needle, i));
        table.push(matched);
    }
    matched = 0;
    for i in 0..text.len() {
        let byte = at(text, i);
        while matched > 0 && at(needle, matched) != byte {
            matched = table[matched - 1];
        }
        matched += usize::from(at(needle, matched) == byte);
        if matched == needle.len() {
            return Some(i + 1 - matched);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::*;

    // Independent scalar-based oracle, with one choice for every admissible
    // positive reading length of each hanja run.
    fn enumerate(
        spelling: &[(bool, &str)],
        reading: &[char],
        lengths: &mut Vec<usize>,
        solutions: &mut Vec<Vec<usize>>,
    ) {
        let Some((&(hanja, text), rest)) = spelling.split_first() else {
            if reading.is_empty() {
                solutions.push(lengths.clone());
            }
            return;
        };
        if hanja {
            for n in 1..=reading.len() {
                lengths.push(n);
                enumerate(rest, &reading[n..], lengths, solutions);
                lengths.pop();
            }
        } else {
            let n = text.chars().count();
            if let Some(prefix) = reading.get(..n)
                && text.chars().eq(prefix.iter().copied())
            {
                lengths.push(n);
                enumerate(rest, &reading[n..], lengths, solutions);
                lengths.pop();
            }
        }
    }

    #[test]
    fn agrees_with_exhaustive_alignment() {
        for size in 2..=5 {
            for mut code in 0..3_usize.pow(size) {
                let spelling: String = (0..size)
                    .map(|_| {
                        let choice = ['가', '나', '甲'][code % 3];
                        code /= 3;
                        choice
                    })
                    .collect();
                if !spelling.chars().any(is_hanja) || spelling.chars().all(is_hanja) {
                    continue;
                }
                let parts: Vec<_> = runs(&spelling).collect();
                for size in 0..=6 {
                    for code in 0..1 << size {
                        let chars: Vec<_> = (0..size)
                            .map(|bit| if code & (1 << bit) == 0 { '가' } else { '나' })
                            .collect();
                        let reading: String = chars.iter().collect();
                        let mut solutions = Vec::new();
                        enumerate(&parts, &chars, &mut Vec::new(), &mut solutions);
                        let alignment = align(&spelling, &spelling, &spelling, &reading);
                        if let [expected] = solutions.as_slice() {
                            let alignment = alignment.unwrap();
                            let mut lengths = Vec::new();
                            let mut original = String::new();
                            let mut selected = String::new();
                            for (_, key, value) in alignment.parts() {
                                original.push_str(key);
                                selected.push_str(value);
                                lengths.push(value.chars().count());
                            }
                            assert_eq!(original, spelling);
                            assert_eq!(selected, reading);
                            assert_eq!(&lengths, expected);
                        } else {
                            assert!(alignment.is_none(), "{spelling:?} → {reading:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn validates_scripts_and_original_anchors() {
        let value = align("神달形", "神달形", "𠮷달形", "신달형").unwrap();
        assert_eq!(
            value.parts().collect::<Vec<_>>(),
            [(true, "𠮷", "신"), (false, "달", "달"), (true, "形", "형")]
        );
        for input in ["神달形", "神달形\u{e0100}", "神*달形", "神별形", "달神形"]
        {
            assert!(align(input, "神달形", "神달形", "신달형").is_none());
        }
        assert!(align("神달形", "神달形", "神달形", "신달ㅎ").is_none());
    }
}
