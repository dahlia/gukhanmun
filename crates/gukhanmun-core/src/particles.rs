// Gukhanmun: Conservative particle evidence for dictionary reading selection.
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

use crate::{Match, ReadingCandidates, select_default_reading};

// Four scalars decide every supported suffix, including its boundary witness.
pub(crate) const LOOKAHEAD: usize = 4;

#[derive(Clone, Copy)]
enum Requirement {
    Consonant,
    Vowel,
    Euro,
    Ro,
}

fn witness(ch: char) -> bool {
    ch.is_whitespace() || ".,!?:;)]}。、！？，：；）】」』".contains(ch)
}

fn requirement(following: &str, final_end: bool) -> Option<Requirement> {
    use Requirement::*;
    [
        ("으로는", Euro),
        ("으로도", Euro),
        ("으로만", Euro),
        ("로는", Ro),
        ("로도", Ro),
        ("로만", Ro),
        ("과는", Consonant),
        ("과도", Consonant),
        ("과만", Consonant),
        ("와는", Vowel),
        ("와도", Vowel),
        ("와만", Vowel),
        ("으로", Euro),
        ("로", Ro),
        ("을", Consonant),
        ("은", Consonant),
        ("이", Consonant),
        ("과", Consonant),
        ("를", Vowel),
        ("는", Vowel),
        ("가", Vowel),
        ("와", Vowel),
    ]
    .into_iter()
    .find_map(|(particle, required)| {
        let rest = following.strip_prefix(particle)?;
        (rest.chars().next().map_or(final_end, witness)).then_some(required)
    })
}

fn compatible(reading: &str, required: Requirement) -> bool {
    let Some(last) = reading.chars().last() else {
        return true;
    };
    let syllable = last as u32;
    if !(0xac00..=0xd7a3).contains(&syllable) {
        return true;
    }
    let coda = (syllable - 0xac00) % 28;
    match required {
        Requirement::Consonant => coda != 0,
        Requirement::Vowel => coda == 0,
        Requirement::Euro => coda != 0 && coda != 8,
        Requirement::Ro => coda == 0 || coda == 8,
    }
}

pub(crate) fn select(
    candidates: &ReadingCandidates,
    following: &str,
    final_end: bool,
    byte_len: usize,
) -> Match {
    let selected = requirement(following, final_end)
        .and_then(|required| {
            candidates.as_slice().iter().find(|candidate| {
                compatible(&candidate.reading, required)
                    || candidate
                        .suffix_reading
                        .as_deref()
                        .is_some_and(|suffix| compatible(suffix, required))
            })
        })
        .unwrap_or_else(|| select_default_reading(candidates));
    Match {
        byte_len,
        reading: selected.reading.clone(),
        suffix_reading: selected.suffix_reading.clone(),
        mark: selected.mark,
    }
}
