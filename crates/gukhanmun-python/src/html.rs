// Gukhanmun: Private Python extension with a safely shared conversion runtime.
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

//! Declarative subtree preservation without Python callbacks.

use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct Options {
    #[serde(default)]
    preserve_classes: Vec<String>,
    #[serde(default)]
    preserve_attributes: Vec<Attribute>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attribute {
    name: String,
    value: Option<String>,
}

impl Options {
    pub(super) fn matches(&self, raw: &str) -> bool {
        self.preserve_classes
            .iter()
            .any(|name| has_class(raw, name))
            || self
                .preserve_attributes
                .iter()
                .any(|attr| has_attribute(raw, &attr.name, attr.value.as_deref()))
    }
}

struct AttrIter<'a> {
    raw: &'a str,
    pos: usize,
}

impl<'a> AttrIter<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw, pos: 0 }
    }
}

impl<'a> Iterator for AttrIter<'a> {
    type Item = (&'a str, Option<&'a str>);

    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.raw.as_bytes();
        loop {
            while self.pos < bytes.len() && bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            if self.pos >= bytes.len() {
                return None;
            }
            let name_start = self.pos;
            while self.pos < bytes.len()
                && !bytes[self.pos].is_ascii_whitespace()
                && !matches!(bytes[self.pos], b'=' | b'/' | b'>' | b'\'' | b'"' | b'<')
            {
                self.pos += 1;
            }
            if self.pos == name_start {
                self.pos += 1;
                continue;
            }
            let name = &self.raw[name_start..self.pos];
            while self.pos < bytes.len() && bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            if bytes.get(self.pos) != Some(&b'=') {
                return Some((name, None));
            }
            self.pos += 1;
            while self.pos < bytes.len() && bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            let value = if matches!(bytes.get(self.pos), Some(b'\'' | b'"')) {
                let quote = bytes[self.pos];
                self.pos += 1;
                let value_start = self.pos;
                while self.pos < bytes.len() && bytes[self.pos] != quote {
                    self.pos += 1;
                }
                let v = &self.raw[value_start..self.pos];
                if self.pos < bytes.len() {
                    self.pos += 1;
                }
                v
            } else {
                let value_start = self.pos;
                while self.pos < bytes.len() && !bytes[self.pos].is_ascii_whitespace() {
                    self.pos += 1;
                }
                &self.raw[value_start..self.pos]
            };
            return Some((name, Some(value)));
        }
    }
}

/// Decodes HTML character references (`&amp;`, `&lt;`, `&#34;`, `&#x22;`,
/// etc.) in an attribute value, matching the CLI's `decode_html_attribute_value`
/// semantics.
fn decode_attr_value(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'&' {
            let next = raw[i..].find('&').map_or(raw.len(), |off| i + off);
            out.push_str(&raw[i..next]);
            i = next;
            continue;
        }
        if let Some(semi_rel) = raw[i + 1..].find(';') {
            let semi = i + 1 + semi_rel;
            let reference = &raw[i + 1..semi];
            let ch: Option<char> = match reference {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if reference.starts_with('#') => {
                    let digits = &reference[1..];
                    let code = if let Some(hex) = digits.strip_prefix(['x', 'X']) {
                        u32::from_str_radix(hex, 16).ok()
                    } else {
                        digits.parse::<u32>().ok()
                    };
                    code.and_then(char::from_u32)
                }
                _ => None,
            };
            if let Some(c) = ch {
                out.push(c);
                i = semi + 1;
            } else {
                out.push_str(&raw[i..=semi]);
                i = semi + 1;
            }
        } else {
            out.push_str(&raw[i..]);
            break;
        }
    }
    out
}

/// Returns `true` if `raw_attributes` contains a `class` attribute whose
/// whitespace-separated token list includes `class_name` (case-sensitive,
/// matching CSS class selector semantics).  Attribute values are decoded
/// before comparison.
fn has_class(raw_attributes: &str, class_name: &str) -> bool {
    for (name, value) in AttrIter::new(raw_attributes) {
        if name.eq_ignore_ascii_case("class") {
            let raw = value.unwrap_or("");
            let decoded = decode_attr_value(raw);
            return decoded
                .split_ascii_whitespace()
                .any(|tok| tok == class_name);
        }
    }
    false
}

/// Returns `true` if `raw_attributes` contains an attribute whose name
/// matches `attr_name` (case-insensitive) and, when `attr_value` is
/// `Some`, whose decoded value matches exactly (case-sensitive).
/// Boolean attributes (no `=` assignment) never match a value check.
fn has_attribute(raw_attributes: &str, attr_name: &str, attr_value: Option<&str>) -> bool {
    for (name, value) in AttrIter::new(raw_attributes) {
        if name.eq_ignore_ascii_case(attr_name) {
            return match attr_value {
                None => true,
                Some(required) => match value {
                    None => false,
                    Some(raw) => decode_attr_value(raw) == required,
                },
            };
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_complete_unicode_names_and_ascii_case_only() {
        assert!(!has_class("한class='keep'", "keep"));
        assert!(has_class("CLASS='keep'", "keep"));
        assert!(has_attribute("data-한='yes'", "DATA-한", Some("yes")));
        assert!(!has_attribute("한class='keep'", "class", Some("keep")));
        assert!(!has_attribute("cläss='keep'", "ÄSS", None));
        assert!(has_attribute("한='yes'", "한", Some("yes")));
    }

    #[test]
    fn first_duplicate_and_decoded_values_match_existing_contract() {
        assert!(!has_class("class='skip' CLASS='keep'", "keep"));
        assert!(has_class("class='keep&#32;more'", "more"));
        assert!(has_attribute("data-a='a&amp;b'", "data-a", Some("a&b")));
        assert!(has_attribute("data-a=a&#x26;b", "data-a", Some("a&b")));
        assert!(has_attribute("data-a", "data-a", None));
        assert!(!has_attribute("data-a", "data-a", Some("")));
    }
}
