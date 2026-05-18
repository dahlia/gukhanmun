use alloc::string::String;
use alloc::vec::Vec;

use crate::{HanjaDictionary, MatchMark, is_hanja};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Segment {
    Dictionary {
        byte_start: usize,
        byte_end: usize,
        reading: String,
        mark: MatchMark,
    },
    Fallback {
        byte_start: usize,
        byte_end: usize,
    },
    Text {
        byte_start: usize,
        byte_end: usize,
    },
}

#[derive(Clone, Debug)]
struct BestPath {
    score: Score,
    previous: usize,
    segment: Segment,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Score {
    dictionary_chars: usize,
    segments: usize,
}

impl Score {
    fn with_dictionary(self, char_len: usize) -> Self {
        Self {
            dictionary_chars: self.dictionary_chars + char_len,
            segments: self.segments + 1,
        }
    }

    fn with_fallback(self) -> Self {
        Self {
            dictionary_chars: self.dictionary_chars,
            segments: self.segments + 1,
        }
    }

    fn with_text(self) -> Self {
        Self {
            dictionary_chars: self.dictionary_chars,
            segments: self.segments + 1,
        }
    }

    fn beats(self, other: Self) -> bool {
        self.dictionary_chars > other.dictionary_chars
            || (self.dictionary_chars == other.dictionary_chars && self.segments < other.segments)
    }
}

pub(crate) fn segment_text<D>(text: &str, dictionary: &D) -> Vec<Segment>
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
            segments.extend(segment_span(span, cursor, dictionary));
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

fn segment_span<D>(span: &str, byte_offset: usize, dictionary: &D) -> Vec<Segment>
where
    D: HanjaDictionary + ?Sized,
{
    let boundaries = char_boundaries(span);
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

        if lookup.chars().any(is_hanja) {
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
                let char_len = end_char - start_char;
                let score = start_score.with_dictionary(char_len);
                propose(
                    &mut best[end_char],
                    score,
                    start_char,
                    Segment::Dictionary {
                        byte_start: byte_offset + byte_start,
                        byte_end: byte_offset + byte_end,
                        reading: matched.reading,
                        mark: matched.mark,
                    },
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
            let score = start_score.with_fallback();
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
    use crate::MapDictionary;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn segments_cover_the_input_without_gaps(input in "[가-힣一-龥]{0,8}") {
            let dict = MapDictionary::new();
            let segments = segment_text(&input, &dict);
            let mut cursor = 0;

            for segment in segments {
                let (byte_start, byte_end) = match segment {
                    Segment::Dictionary { byte_start, byte_end, .. }
                    | Segment::Fallback { byte_start, byte_end }
                    | Segment::Text { byte_start, byte_end } => (byte_start, byte_end),
                };

                prop_assert_eq!(byte_start, cursor);
                prop_assert!(byte_end > byte_start);
                cursor = byte_end;
            }

            prop_assert_eq!(cursor, input.len());
        }
    }
}
