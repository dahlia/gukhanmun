use alloc::string::String;
use alloc::vec::Vec;

use crate::{HanjaDictionary, MatchMark};

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

    fn beats(self, other: Self) -> bool {
        self.dictionary_chars > other.dictionary_chars
            || (self.dictionary_chars == other.dictionary_chars && self.segments < other.segments)
    }
}

pub(crate) fn segment_hanja_run<D>(run: &str, dictionary: &D) -> Vec<Segment>
where
    D: HanjaDictionary + ?Sized,
{
    let boundaries = char_boundaries(run);
    let char_count = boundaries.len().saturating_sub(1);
    let mut best = Vec::from_iter((0..=char_count).map(|_| None));
    best[0] = Some(BestPath {
        score: Score::default(),
        previous: 0,
        segment: Segment::Fallback {
            byte_start: 0,
            byte_end: 0,
        },
    });

    for start_char in 0..char_count {
        let Some(start_score) = best[start_char].as_ref().map(|path| path.score) else {
            continue;
        };
        let byte_start = boundaries[start_char];

        for matched in dictionary.matches_at(&run[byte_start..]) {
            let Some(byte_end) = byte_start.checked_add(matched.byte_len) else {
                continue;
            };
            let Ok(end_char) = boundaries.binary_search(&byte_end) else {
                continue;
            };
            if end_char <= start_char || end_char > char_count {
                continue;
            }
            let char_len = end_char - start_char;
            let score = start_score.with_dictionary(char_len);
            propose(
                &mut best[end_char],
                score,
                start_char,
                Segment::Dictionary {
                    byte_start,
                    byte_end,
                    reading: matched.reading,
                    mark: matched.mark,
                },
            );
        }

        let end_char = start_char + 1;
        let score = start_score.with_fallback();
        propose(
            &mut best[end_char],
            score,
            start_char,
            Segment::Fallback {
                byte_start,
                byte_end: boundaries[end_char],
            },
        );
    }

    backtrack(&best)
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
    use super::{Segment, segment_hanja_run};
    use crate::MapDictionary;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn segments_cover_the_input_without_gaps(input in "[一-龥]{0,8}") {
            let dict = MapDictionary::new();
            let segments = segment_hanja_run(&input, &dict);
            let mut cursor = 0;

            for segment in segments {
                let (byte_start, byte_end) = match segment {
                    Segment::Dictionary { byte_start, byte_end, .. }
                    | Segment::Fallback { byte_start, byte_end } => (byte_start, byte_end),
                };

                prop_assert_eq!(byte_start, cursor);
                prop_assert!(byte_end > byte_start);
                cursor = byte_end;
            }

            prop_assert_eq!(cursor, input.len());
        }
    }
}
