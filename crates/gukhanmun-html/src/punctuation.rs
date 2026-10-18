// Gukhanmun: Incremental HTML character references for punctuation context.
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

//! WHATWG Data-state character references for punctuation context only.
use crate::entities::{ENTITIES, MAX_NAME_LEN};
use gukhanmun_core::{PunctuationTextCodec, PunctuationTextUnit};

/// Incremental HTML text codec preserving the spelling of untouched references.
///
/// Named references use the WHATWG snapshot, including legacy semicolonless
/// names. Numeric references use HTML replacement rules. Decoding happens once;
/// decoded markup is never parsed. Incomplete numeric references can buffer an
/// arbitrarily long spelling, but each source byte is scanned only once.
#[derive(Default)]
pub struct HtmlPunctuationTextCodec {
    pending: String,
    numeric: Option<Numeric>,
    named_len: usize,
}
#[derive(Default)]
struct Numeric {
    radix: u32,
    value: u32,
    digits: usize,
}
impl HtmlPunctuationTextCodec {
    /// Creates an empty codec for a contiguous stream of HTML text tokens.
    pub fn new() -> Self {
        Self::default()
    }
    fn literal(out: &mut Vec<PunctuationTextUnit>, text: &str) {
        out.extend(text.chars().map(|c| {
            let s = c.to_string();
            PunctuationTextUnit {
                raw: s.clone(),
                decoded: s,
            }
        }));
    }
    fn finish_pending(&mut self, out: &mut Vec<PunctuationTextUnit>) {
        if self.pending.is_empty() {
            return;
        }
        if let Some(n) = self.numeric.take() {
            if n.digits > 0 {
                out.push(PunctuationTextUnit {
                    raw: std::mem::take(&mut self.pending),
                    decoded: html_scalar(n.value).to_string(),
                });
            } else {
                Self::literal(out, &std::mem::take(&mut self.pending));
            }
        } else {
            let name = &self.pending[1..];
            let mut found = None;
            for end in (1..=name.len()).rev() {
                if !name.is_char_boundary(end) {
                    continue;
                }
                if let Ok(i) = ENTITIES.binary_search_by_key(&&name[..end], |&(n, _)| n) {
                    found = Some((end, ENTITIES[i].1));
                    break;
                }
            }
            if let Some((end, value)) = found {
                out.push(PunctuationTextUnit {
                    raw: self.pending[..end + 1].to_string(),
                    decoded: value.to_string(),
                });
                Self::literal(out, &self.pending[end + 1..]);
            } else {
                Self::literal(out, &self.pending);
            }
            self.pending.clear();
        }
        self.named_len = 0;
    }
    fn feed(&mut self, c: char, out: &mut Vec<PunctuationTextUnit>) {
        if self.pending.is_empty() {
            if c == '&' {
                self.pending.push(c);
            } else {
                Self::literal(out, &c.to_string());
            }
            return;
        }
        if self.pending == "&" && c == '#' {
            self.pending.push(c);
            self.numeric = Some(Numeric {
                radix: 10,
                ..Numeric::default()
            });
            return;
        }
        if let Some(n) = self.numeric.as_mut() {
            if self.pending == "&#" && matches!(c, 'x' | 'X') {
                n.radix = 16;
                self.pending.push(c);
                return;
            }
            if c.is_ascii()
                && let Some(d) = c.to_digit(n.radix)
            {
                n.value = n.value.saturating_mul(n.radix).saturating_add(d);
                n.digits += 1;
                self.pending.push(c);
                return;
            }
            if c == ';' && n.digits > 0 {
                self.pending.push(c);
                self.finish_pending(out);
                return;
            }
            self.finish_pending(out);
            self.feed(c, out);
            return;
        }
        if c.is_ascii_alphanumeric() || c == ';' {
            self.pending.push(c);
            self.named_len += 1;
            let name = &self.pending[1..];
            let i = ENTITIES.partition_point(|&(n, _)| n < name);
            let possible = ENTITIES.get(i).is_some_and(|&(n, _)| n.starts_with(name));
            if c == ';' || !possible || self.named_len >= MAX_NAME_LEN {
                self.finish_pending(out);
            }
        } else {
            self.finish_pending(out);
            self.feed(c, out);
        }
    }
}
impl PunctuationTextCodec for HtmlPunctuationTextCodec {
    fn decode(&mut self, text: &str, finish: bool) -> Vec<PunctuationTextUnit> {
        let mut out = Vec::new();
        for c in text.chars() {
            self.feed(c, &mut out)
        }
        if finish {
            self.finish_pending(&mut out);
        }
        out
    }
    fn encode(&self, text: &str) -> String {
        let mut out = String::new();
        for c in text.chars() {
            match c {
                '!' => out.push_str("&#33;"),
                '?' => out.push_str("&#63;"),
                '/' => out.push_str("&#47;"),
                ';' => out.push_str("&#59;"),
                '#' => out.push_str("&#35;"),
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                _ => out.push(c),
            }
        }
        out
    }
}
fn html_scalar(value: u32) -> char {
    const C1: [u32; 32] = [
        0x20ac, 0x81, 0x201a, 0x192, 0x201e, 0x2026, 0x2020, 0x2021, 0x2c6, 0x2030, 0x160, 0x2039,
        0x152, 0x8d, 0x17d, 0x8f, 0x90, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014,
        0x2dc, 0x2122, 0x161, 0x203a, 0x153, 0x9d, 0x17e, 0x178,
    ];
    let value = if (0x80..=0x9f).contains(&value) {
        C1[(value - 0x80) as usize]
    } else {
        value
    };
    if value == 0 {
        '\u{fffd}'
    } else {
        char::from_u32(value).unwrap_or('\u{fffd}')
    }
}
