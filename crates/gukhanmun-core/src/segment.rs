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

use alloc::string::String;
use alloc::vec::Vec;

use crate::fallback::{
    ArabicNumeralMatch, FallbackPart, FallbackState, arabic_numeral_at, can_start_arabic_numeral,
    is_hanja_numeral, is_numeral_unit, phoneticize_fallback_run_with_state, phoneticize_hanja_char,
    reading_matches_with_initial_sound_law,
};
use crate::{EngineOptions, HanjaDictionary, Match, MatchMark, SegmentationStrategy, is_hanja};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Segment {
    Dictionary {
        byte_start: usize,
        byte_end: usize,
        reading: String,
        suffix_reading: Option<String>,
        mark: MatchMark,
    },
    TrivialDictionary {
        byte_start: usize,
        byte_end: usize,
        reading: String,
        suffix_reading: Option<String>,
        mark: MatchMark,
    },
    Fallback {
        byte_start: usize,
        byte_end: usize,
    },
    NumeralText {
        byte_start: usize,
        byte_end: usize,
        text: String,
    },
    Text {
        byte_start: usize,
        byte_end: usize,
    },
}

pub(crate) fn is_trivial_single_char_match(
    source: &str,
    reading: &str,
    suffix_reading: Option<&str>,
    mark: MatchMark,
) -> bool {
    if mark != MatchMark::default() {
        return false;
    }
    if suffix_reading.is_some() {
        return false;
    }
    let mut source_chars = source.chars();
    let Some(ch) = source_chars.next() else {
        return false;
    };
    if source_chars.next().is_some() {
        return false;
    }
    if is_hanja_numeral(ch) {
        return false;
    }
    let Some(unihan) = phoneticize_hanja_char(ch) else {
        return false;
    };
    if reading == unihan {
        return true;
    }
    let mut reading_chars = reading.chars();
    let Some(dict_char) = reading_chars.next() else {
        return false;
    };
    if reading_chars.next().is_some() {
        return false;
    }
    reading_matches_with_initial_sound_law(unihan, dict_char)
}

fn segment_for_dictionary_match(
    source: &str,
    byte_start: usize,
    byte_end: usize,
    reading: String,
    suffix_reading: Option<String>,
    mark: MatchMark,
) -> Segment {
    if is_trivial_single_char_match(source, &reading, suffix_reading.as_deref(), mark) {
        Segment::TrivialDictionary {
            byte_start,
            byte_end,
            reading,
            suffix_reading,
            mark,
        }
    } else {
        Segment::Dictionary {
            byte_start,
            byte_end,
            reading,
            suffix_reading,
            mark,
        }
    }
}

#[derive(Clone, Debug)]
struct BestPath {
    score: Score,
    previous: usize,
    segment: Segment,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Score {
    fallback_chars: usize,
    numeral_chars: usize,
    dictionary_chars: usize,
    segments: usize,
}

impl Score {
    fn with_numeral(self, char_len: usize) -> Self {
        Self {
            fallback_chars: self.fallback_chars,
            numeral_chars: self.numeral_chars + char_len,
            dictionary_chars: self.dictionary_chars,
            segments: self.segments + 1,
        }
    }

    fn with_dictionary(self, char_len: usize) -> Self {
        Self {
            fallback_chars: self.fallback_chars,
            numeral_chars: self.numeral_chars,
            dictionary_chars: self.dictionary_chars + char_len,
            segments: self.segments + 1,
        }
    }

    fn with_fallback(self, char_len: usize) -> Self {
        Self {
            fallback_chars: self.fallback_chars + char_len,
            numeral_chars: self.numeral_chars,
            dictionary_chars: self.dictionary_chars,
            segments: self.segments + 1,
        }
    }

    fn with_text(self) -> Self {
        Self {
            fallback_chars: self.fallback_chars,
            numeral_chars: self.numeral_chars,
            dictionary_chars: self.dictionary_chars,
            segments: self.segments + 1,
        }
    }

    fn beats(self, other: Self) -> bool {
        if self.fallback_chars != other.fallback_chars {
            return self.fallback_chars < other.fallback_chars;
        }
        if self.numeral_chars != other.numeral_chars {
            return self.numeral_chars > other.numeral_chars;
        }
        if self.dictionary_chars != other.dictionary_chars {
            return self.dictionary_chars > other.dictionary_chars;
        }
        self.segments < other.segments
    }
}

pub(crate) fn segment_text<D>(text: &str, dictionary: &D, options: EngineOptions) -> Vec<Segment>
where
    D: HanjaDictionary + ?Sized,
{
    if !text.chars().any(is_hanja) {
        return text_segment(text);
    }

    let mut segments = Vec::new();
    let mut cursor = 0;

    while cursor < text.len() {
        let span_end = next_span_end(&text[cursor..], cursor);
        let span = &text[cursor..span_end];
        if span.chars().any(is_hanja) {
            segments.extend(segment_span_with_strategy(
                span, cursor, dictionary, options,
            ));
        } else {
            segments.push(Segment::Text {
                byte_start: cursor,
                byte_end: span_end,
            });
        }
        cursor = span_end;
    }

    segments
}

fn segment_span_with_strategy<D>(
    span: &str,
    byte_offset: usize,
    dictionary: &D,
    options: EngineOptions,
) -> Vec<Segment>
where
    D: HanjaDictionary + ?Sized,
{
    match options.segmentation {
        SegmentationStrategy::Lattice => {
            segment_span_lattice(span, byte_offset, dictionary, options)
        }
        SegmentationStrategy::Eager => segment_span_eager(span, byte_offset, dictionary, options),
    }
}

fn segment_span_lattice<D>(
    span: &str,
    byte_offset: usize,
    dictionary: &D,
    options: EngineOptions,
) -> Vec<Segment>
where
    D: HanjaDictionary + ?Sized,
{
    let boundaries = char_boundaries(span);
    let chars = span.chars().collect::<Vec<_>>();
    let char_count = boundaries.len().saturating_sub(1);
    let max_word_chars = dictionary.max_word_chars();
    let mut best = Vec::from_iter((0..=char_count).map(|_| None));
    best[0] = Some(BestPath {
        score: Score::default(),
        previous: 0,
        segment: Segment::Text {
            byte_start: 0,
            byte_end: 0,
        },
    });

    for start_char in 0..char_count {
        let Some(start_score) = best[start_char].as_ref().map(|path| path.score) else {
            continue;
        };
        let byte_start = boundaries[start_char];
        let lookup = lookup_suffix(span, &boundaries, start_char, max_word_chars);
        let dictionary_matches = if lookup.chars().any(is_hanja) {
            dictionary.matches_at(lookup).collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        if can_start_arabic_numeral(&chars, start_char)
            && let Some(matched) = arabic_numeral_at(&chars, start_char, options.numeral_strategy)
            && let Some(candidate) = arabic_numeral_segment(
                span,
                &boundaries,
                start_char,
                byte_start,
                matched,
                &dictionary_matches,
                options,
            )
        {
            let char_len = candidate.end_char - start_char;
            let score = start_score.with_numeral(char_len);
            propose(
                &mut best[candidate.end_char],
                score,
                start_char,
                Segment::NumeralText {
                    byte_start: byte_offset + byte_start,
                    byte_end: byte_offset + candidate.byte_end,
                    text: candidate.text,
                },
            );
        }

        if !dictionary_matches.is_empty() {
            for matched in dictionary_matches {
                let Some(byte_end) = byte_start.checked_add(matched.byte_len) else {
                    continue;
                };
                let Ok(end_char) = boundaries.binary_search(&byte_end) else {
                    continue;
                };
                if end_char <= start_char || end_char > char_count {
                    continue;
                }
                if !span[byte_start..byte_end].chars().any(is_hanja) {
                    continue;
                }
                let char_len = end_char - start_char;
                let score = start_score.with_dictionary(char_len);
                let source = &span[byte_start..byte_end];
                propose(
                    &mut best[end_char],
                    score,
                    start_char,
                    segment_for_dictionary_match(
                        source,
                        byte_offset + byte_start,
                        byte_offset + byte_end,
                        matched.reading,
                        matched.suffix_reading,
                        matched.mark,
                    ),
                );
            }
        }

        let current = span[byte_start..]
            .chars()
            .next()
            .expect("start_char is within the text");
        let end_char = start_char + 1;
        let byte_end = boundaries[end_char];
        if is_hanja(current) {
            let score = start_score.with_fallback(1);
            propose(
                &mut best[end_char],
                score,
                start_char,
                Segment::Fallback {
                    byte_start: byte_offset + byte_start,
                    byte_end: byte_offset + byte_end,
                },
            );
        } else {
            let score = start_score.with_text();
            propose(
                &mut best[end_char],
                score,
                start_char,
                Segment::Text {
                    byte_start: byte_offset + byte_start,
                    byte_end: byte_offset + byte_end,
                },
            );
        }
    }

    backtrack(&best)
}

fn segment_span_eager<D>(
    span: &str,
    byte_offset: usize,
    dictionary: &D,
    options: EngineOptions,
) -> Vec<Segment>
where
    D: HanjaDictionary + ?Sized,
{
    let boundaries = char_boundaries(span);
    let chars = span.chars().collect::<Vec<_>>();
    let char_count = boundaries.len().saturating_sub(1);
    let max_word_chars = dictionary.max_word_chars();
    let mut segments = Vec::new();
    let mut start_char = 0;

    while start_char < char_count {
        let byte_start = boundaries[start_char];
        let lookup = lookup_suffix(span, &boundaries, start_char, max_word_chars);

        if can_start_arabic_numeral(&chars, start_char)
            && let Some(matched) = arabic_numeral_at(&chars, start_char, options.numeral_strategy)
        {
            let dictionary_matches = if lookup.chars().any(is_hanja) {
                dictionary.matches_at(lookup).collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            if let Some(candidate) = arabic_numeral_segment(
                span,
                &boundaries,
                start_char,
                byte_start,
                matched,
                &dictionary_matches,
                options,
            ) {
                segments.push(Segment::NumeralText {
                    byte_start: byte_offset + byte_start,
                    byte_end: byte_offset + candidate.byte_end,
                    text: candidate.text,
                });
                start_char = candidate.end_char;
                continue;
            }
        }

        if lookup.chars().any(is_hanja)
            && let Some((matched, end_char)) =
                longest_match(span, &boundaries, start_char, lookup, dictionary)
        {
            let byte_end = byte_start + matched.byte_len;
            let source = &span[byte_start..byte_end];
            segments.push(segment_for_dictionary_match(
                source,
                byte_offset + byte_start,
                byte_offset + byte_end,
                matched.reading,
                matched.suffix_reading,
                matched.mark,
            ));
            start_char = end_char;
            continue;
        }

        let current = span[byte_start..]
            .chars()
            .next()
            .expect("start_char is within the text");
        let end_char = start_char + 1;
        let byte_end = boundaries[end_char];
        if is_hanja(current) {
            segments.push(Segment::Fallback {
                byte_start: byte_offset + byte_start,
                byte_end: byte_offset + byte_end,
            });
        } else {
            segments.push(Segment::Text {
                byte_start: byte_offset + byte_start,
                byte_end: byte_offset + byte_end,
            });
        }
        start_char = end_char;
    }

    segments
}

struct ArabicSegment {
    end_char: usize,
    byte_end: usize,
    text: String,
}

fn arabic_numeral_segment(
    span: &str,
    boundaries: &[usize],
    start_char: usize,
    byte_start: usize,
    matched: ArabicNumeralMatch,
    dictionary_matches: &[Match],
    options: EngineOptions,
) -> Option<ArabicSegment> {
    let char_count = boundaries.len().saturating_sub(1);
    let numeral_end_char = matched.next_index;
    if numeral_end_char > char_count {
        return None;
    }

    let numeral_byte_end = boundaries[numeral_end_char];
    if has_protected_dictionary_match(span, byte_start, numeral_byte_end, dictionary_matches) {
        return None;
    }

    let byte_end =
        longest_arabic_override_byte_end(span, byte_start, numeral_byte_end, dictionary_matches)
            .unwrap_or(numeral_byte_end);
    let end_char = boundaries.binary_search(&byte_end).ok()?;
    if end_char <= start_char {
        return None;
    }

    let mut text = matched.text;
    if byte_end > numeral_byte_end {
        let suffix = &span[numeral_byte_end..byte_end];
        let suffix_text = phoneticize_suffix_after_arabic_numeral(suffix, &text, options)?;
        text.push_str(&suffix_text);
    }

    Some(ArabicSegment {
        end_char,
        byte_end,
        text,
    })
}

fn longest_arabic_override_byte_end(
    span: &str,
    byte_start: usize,
    numeral_byte_end: usize,
    matches: &[Match],
) -> Option<usize> {
    matches
        .iter()
        .filter_map(|matched| {
            let byte_end = byte_start.checked_add(matched.byte_len)?;
            if byte_end < numeral_byte_end {
                return None;
            }
            let source = span.get(byte_start..byte_end)?;
            dictionary_match_allows_arabic_override(source, numeral_byte_end - byte_start)
                .then_some(byte_end)
        })
        .max()
}

fn phoneticize_suffix_after_arabic_numeral(
    suffix: &str,
    numeral_text: &str,
    options: EngineOptions,
) -> Option<String> {
    let mut state = FallbackState {
        starts_word: false,
        previous_reading: numeral_text.chars().last(),
    };
    let mut output = String::new();
    for part in phoneticize_fallback_run_with_state(suffix, options, &mut state) {
        match part {
            FallbackPart::Annotation { reading, .. } | FallbackPart::ReadingText(reading) => {
                output.push_str(&reading);
            }
            FallbackPart::Text(_) => return None,
        }
    }
    Some(output)
}

fn longest_match<D>(
    span: &str,
    boundaries: &[usize],
    start_char: usize,
    lookup: &str,
    dictionary: &D,
) -> Option<(Match, usize)>
where
    D: HanjaDictionary + ?Sized,
{
    let byte_start = boundaries[start_char];
    let char_count = boundaries.len().saturating_sub(1);
    let mut best: Option<(Match, usize)> = None;

    for matched in dictionary.matches_at(lookup) {
        let Some(byte_end) = byte_start.checked_add(matched.byte_len) else {
            continue;
        };
        let Ok(end_char) = boundaries.binary_search(&byte_end) else {
            continue;
        };
        if end_char <= start_char || end_char > char_count {
            continue;
        }
        if !span[byte_start..byte_end].chars().any(is_hanja) {
            continue;
        }
        if best
            .as_ref()
            .is_some_and(|(current, _)| current.byte_len >= matched.byte_len)
        {
            continue;
        }
        best = Some((matched, end_char));
    }

    best
}

fn has_protected_dictionary_match(
    span: &str,
    byte_start: usize,
    numeral_byte_end: usize,
    matches: &[Match],
) -> bool {
    matches.iter().any(|matched| {
        let Some(byte_end) = byte_start.checked_add(matched.byte_len) else {
            return false;
        };
        if byte_end <= numeral_byte_end {
            return false;
        }
        let source = &span[byte_start..byte_end];
        !dictionary_match_allows_arabic_override(source, numeral_byte_end - byte_start)
    })
}

fn dictionary_match_allows_arabic_override(source: &str, numeral_byte_len: usize) -> bool {
    if source.len() == numeral_byte_len {
        return true;
    }
    if !source.is_char_boundary(numeral_byte_len) {
        return false;
    }
    let suffix = &source[numeral_byte_len..];
    !suffix.is_empty() && suffix.chars().all(is_numeral_unit)
}

fn next_span_end(suffix: &str, byte_offset: usize) -> usize {
    let mut chars = suffix.char_indices();
    let Some((_, first)) = chars.next() else {
        return byte_offset;
    };
    let whitespace = first.is_whitespace();

    for (index, ch) in chars {
        if ch.is_whitespace() != whitespace {
            return byte_offset + index;
        }
    }

    byte_offset + suffix.len()
}

fn text_segment(text: &str) -> Vec<Segment> {
    if text.is_empty() {
        Vec::new()
    } else {
        Vec::from([Segment::Text {
            byte_start: 0,
            byte_end: text.len(),
        }])
    }
}

fn lookup_suffix<'a>(
    text: &'a str,
    boundaries: &[usize],
    start_char: usize,
    max_word_chars: Option<usize>,
) -> &'a str {
    let byte_start = boundaries[start_char];
    let char_limit = max_word_chars
        .map(|max| start_char.saturating_add(max).min(boundaries.len() - 1))
        .unwrap_or(boundaries.len() - 1);
    let mut end_char = start_char;

    while end_char < char_limit {
        let ch = text[boundaries[end_char]..boundaries[end_char + 1]]
            .chars()
            .next()
            .expect("char boundaries always contain complete characters");
        if ch.is_whitespace() {
            break;
        }
        end_char += 1;
    }

    &text[byte_start..boundaries[end_char]]
}

fn char_boundaries(s: &str) -> Vec<usize> {
    let mut boundaries = Vec::with_capacity(s.chars().count() + 1);
    boundaries.push(0);
    if s.is_empty() {
        return boundaries;
    }
    boundaries.extend(s.char_indices().skip(1).map(|(index, _)| index));
    boundaries.push(s.len());
    boundaries
}

fn propose(slot: &mut Option<BestPath>, score: Score, previous: usize, segment: Segment) {
    if slot
        .as_ref()
        .is_some_and(|current| !score.beats(current.score))
    {
        return;
    }

    *slot = Some(BestPath {
        score,
        previous,
        segment,
    });
}

fn backtrack(best: &[Option<BestPath>]) -> Vec<Segment> {
    let mut cursor = best.len().saturating_sub(1);
    let mut segments = Vec::new();

    while cursor > 0 {
        let Some(path) = &best[cursor] else {
            break;
        };
        segments.push(path.segment.clone());
        cursor = path.previous;
    }

    segments.reverse();
    segments
}

#[cfg(test)]
mod tests {
    use super::{Segment, segment_text};
    use crate::{EngineOptions, MapDictionary, SegmentationStrategy};
    use alloc::vec::Vec;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn lattice_segments_cover_the_input_without_gaps(input in "[가-힣一-龥]{0,8}") {
            let dict = MapDictionary::new();
            let segments = segment_text(
                &input,
                &dict,
                EngineOptions {
                    segmentation: SegmentationStrategy::Lattice,
                    ..EngineOptions::default()
                },
            );
            assert_segments_cover_input(&input, segments)?;
        }

        #[test]
        fn eager_segments_cover_the_input_without_gaps(input in "[가-힣一-龥]{0,8}") {
            let dict = MapDictionary::new();
            let segments = segment_text(
                &input,
                &dict,
                EngineOptions {
                    segmentation: SegmentationStrategy::Eager,
                    ..EngineOptions::default()
                },
            );
            assert_segments_cover_input(&input, segments)?;
        }
    }

    fn assert_segments_cover_input(
        input: &str,
        segments: Vec<Segment>,
    ) -> Result<(), TestCaseError> {
        let mut cursor = 0;

        for segment in segments {
            let (byte_start, byte_end) = match segment {
                Segment::Dictionary {
                    byte_start,
                    byte_end,
                    ..
                }
                | Segment::TrivialDictionary {
                    byte_start,
                    byte_end,
                    ..
                }
                | Segment::Fallback {
                    byte_start,
                    byte_end,
                }
                | Segment::NumeralText {
                    byte_start,
                    byte_end,
                    ..
                }
                | Segment::Text {
                    byte_start,
                    byte_end,
                } => (byte_start, byte_end),
            };

            prop_assert_eq!(byte_start, cursor);
            prop_assert!(byte_end > byte_start);
            cursor = byte_end;
        }

        prop_assert_eq!(cursor, input.len());
        Ok(())
    }
}
