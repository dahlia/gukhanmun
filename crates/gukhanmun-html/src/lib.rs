// Gukhanmun: HTML fragment adapter for Gukhanmun.
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

//! HTML fragment reader and writer for Gukhanmun.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use gukhanmun_core::{
    ContextWindow, EngineOptions, HanjaDictionary, InputToken, RenderMode, RenderedToken, Scope,
    ScopeData, mark_homophones, process_tokens_with_options, render_tokens,
};

/// Adapter-owned scope data for HTML fragments.
///
/// The value preserves the original start tag for serialization and stores the
/// effective policy flags computed by the HTML adapter.  Inherited properties
/// such as ancestor preserved tags and `lang` attributes are resolved before
/// the value is sent to the core engine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmlScopeData {
    tag_name: String,
    raw_attributes: String,
    raw_start_tag: String,
    end_tag_name: String,
    omit_end_tag: bool,
    preserve: bool,
    allows_inline_markup: bool,
    block_boundary: bool,
}

impl HtmlScopeData {
    /// Returns the canonical lowercase tag name used for adapter policy.
    pub fn tag_name(&self) -> &str {
        &self.tag_name
    }

    /// Returns the raw attribute text from the start tag.
    ///
    /// The leading whitespace, if present in the source, is preserved.
    pub fn raw_attributes(&self) -> &str {
        &self.raw_attributes
    }

    /// Returns whether text in this scope should pass through unchanged.
    pub fn is_preserve(&self) -> bool {
        self.preserve
    }
}

impl ScopeData for HtmlScopeData {
    fn is_preserve(&self) -> bool {
        self.preserve
    }

    fn allows_inline_markup(&self) -> bool {
        self.allows_inline_markup
    }

    fn is_block_boundary(&self) -> bool {
        self.block_boundary
    }
}

/// Reads an HTML fragment into the core input-token stream.
///
/// The scanner is fragment-oriented and intentionally does not implement full
/// HTML5 tree construction.  It preserves raw start tags and non-text
/// constructs, computes effective preserve flags for scopes, and treats
/// malformed constructs as ordinary text.
pub fn read_html_fragment(input: &str) -> Vec<InputToken<HtmlScopeData>> {
    Scanner::new(input).scan()
}

/// Writes rendered HTML tokens back to a fragment string.
///
/// Start tags are emitted from the raw source text captured by the reader.
/// `Text` and `Verbatim` tokens are passed through without additional escaping.
pub fn write_html_fragment(
    tokens: impl IntoIterator<Item = RenderedToken<HtmlScopeData>>,
) -> String {
    let mut output = String::new();
    let mut scopes = Vec::new();

    for token in tokens {
        match token {
            RenderedToken::Open(scope) => {
                output.push_str(&scope.data().raw_start_tag);
                scopes.push(scope.into_data());
            }
            RenderedToken::Close => {
                if let Some(scope) = scopes.pop()
                    && !scope.omit_end_tag
                {
                    output.push_str("</");
                    output.push_str(&scope.end_tag_name);
                    output.push('>');
                }
            }
            RenderedToken::Text(text) | RenderedToken::Verbatim(text) => output.push_str(&text),
        }
    }

    output
}

/// Converts an HTML fragment with default engine options.
pub fn convert_html_fragment<D>(input: &str, dictionary: &D, mode: RenderMode) -> String
where
    D: HanjaDictionary + ?Sized,
{
    convert_html_fragment_with_options(input, dictionary, mode, EngineOptions::default())
}

/// Converts an HTML fragment with explicit engine options.
pub fn convert_html_fragment_with_options<D>(
    input: &str,
    dictionary: &D,
    mode: RenderMode,
    options: EngineOptions,
) -> String
where
    D: HanjaDictionary + ?Sized,
{
    let input_tokens = read_html_fragment(input);
    let output_tokens = process_tokens_with_options(input_tokens, dictionary, options);
    let output_tokens = mark_homophones(output_tokens, ContextWindow::PerBlock);
    let rendered_tokens = render_tokens(output_tokens, mode);
    write_html_fragment(rendered_tokens)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ElementContext {
    tag_name: String,
    tag_preserve: bool,
    lang: Option<String>,
}

#[derive(Clone, Debug)]
struct Scanner<'a> {
    input: &'a str,
    position: usize,
    stack: Vec<ElementContext>,
    output: Vec<InputToken<HtmlScopeData>>,
}

impl<'a> Scanner<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            position: 0,
            stack: Vec::new(),
            output: Vec::new(),
        }
    }

    fn scan(mut self) -> Vec<InputToken<HtmlScopeData>> {
        while self.position < self.input.len() {
            if self.input[self.position..].starts_with('<') {
                self.scan_markup();
            } else {
                self.scan_text();
            }
        }
        self.output
    }

    fn scan_text(&mut self) {
        let next_markup = self.input[self.position..]
            .find('<')
            .map_or(self.input.len(), |offset| self.position + offset);
        self.push_text(&self.input[self.position..next_markup]);
        self.position = next_markup;
    }

    fn scan_markup(&mut self) {
        if self.scan_verbatim("<!--", "-->") || self.scan_verbatim("<![CDATA[", "]]>") {
            return;
        }
        if self.input[self.position..].starts_with("</") {
            self.scan_end_tag();
            return;
        }
        if self.input[self.position..].starts_with("<!")
            || self.input[self.position..].starts_with("<?")
        {
            self.scan_declaration();
            return;
        }
        self.scan_start_tag();
    }

    fn scan_verbatim(&mut self, start: &str, end: &str) -> bool {
        if !self.input[self.position..].starts_with(start) {
            return false;
        }
        let Some(end_offset) = self.input[self.position + start.len()..].find(end) else {
            self.push_text(&self.input[self.position..]);
            self.position = self.input.len();
            return true;
        };
        let end_position = self.position + start.len() + end_offset + end.len();
        self.output.push(InputToken::Verbatim(
            self.input[self.position..end_position].to_owned(),
        ));
        self.position = end_position;
        true
    }

    fn scan_declaration(&mut self) {
        let Some(end_position) = find_tag_end(self.input, self.position) else {
            self.push_text(&self.input[self.position..]);
            self.position = self.input.len();
            return;
        };
        self.output.push(InputToken::Verbatim(
            self.input[self.position..=end_position].to_owned(),
        ));
        self.position = end_position + 1;
    }

    fn scan_start_tag(&mut self) {
        let start = self.position;
        let Some((name_start, name_end)) = parse_start_tag_name(self.input, start) else {
            self.push_text("<");
            self.position += 1;
            return;
        };
        let Some(end_position) = find_tag_end(self.input, start) else {
            self.push_text(&self.input[start..]);
            self.position = self.input.len();
            return;
        };

        let tag_original = &self.input[name_start..name_end];
        let tag_name = tag_original.to_ascii_lowercase();
        let raw_start_tag = self.input[start..=end_position].to_owned();
        let self_closing = is_self_closing_start_tag(self.input, name_end, end_position);
        let raw_attributes = raw_attributes(self.input, name_end, end_position, self_closing);
        let context = self.context_for(&tag_name, raw_attributes);
        let omit_end_tag = self_closing || is_void_tag(&tag_name);
        let scope = HtmlScopeData {
            tag_name: tag_name.clone(),
            raw_attributes: raw_attributes.to_owned(),
            raw_start_tag,
            end_tag_name: tag_original.to_owned(),
            omit_end_tag,
            preserve: context.preserve(),
            allows_inline_markup: !context.preserve(),
            block_boundary: is_block_boundary_tag(&tag_name),
        };

        self.output.push(InputToken::Open(Scope::new(scope)));
        self.position = end_position + 1;

        if !omit_end_tag {
            self.stack.push(ElementContext {
                tag_name: tag_name.clone(),
                tag_preserve: context.tag_preserve,
                lang: context.lang,
            });
            if is_raw_text_tag(&tag_name) {
                self.scan_raw_text_element(&tag_name);
            }
        } else {
            self.output.push(InputToken::Close);
        }
    }

    fn context_for(&self, tag_name: &str, raw_attributes: &str) -> ElementContext {
        let parent_tag_preserve = self
            .stack
            .last()
            .is_some_and(|context| context.tag_preserve);
        let tag_preserve = parent_tag_preserve || is_preserved_tag(tag_name);
        let lang = extract_lang(raw_attributes).or_else(|| {
            self.stack
                .last()
                .and_then(|context| context.lang.as_ref().cloned())
        });
        ElementContext {
            tag_name: tag_name.to_owned(),
            tag_preserve,
            lang,
        }
    }

    fn scan_raw_text_element(&mut self, tag_name: &str) {
        let Some(close_offset) = find_raw_text_end_tag(&self.input[self.position..], tag_name)
        else {
            self.output
                .push(InputToken::Verbatim(self.input[self.position..].to_owned()));
            self.position = self.input.len();
            return;
        };
        let raw_end = self.position + close_offset;
        self.output.push(InputToken::Verbatim(
            self.input[self.position..raw_end].to_owned(),
        ));
        self.position = raw_end;
        self.scan_end_tag();
    }

    fn scan_end_tag(&mut self) {
        let start = self.position;
        let Some((name_start, name_end)) = parse_end_tag_name(self.input, start) else {
            self.push_text("<");
            self.position += 1;
            return;
        };
        let Some(end_position) = find_tag_end(self.input, start) else {
            self.push_text(&self.input[start..]);
            self.position = self.input.len();
            return;
        };

        let tag_name = self.input[name_start..name_end].to_ascii_lowercase();
        let Some(stack_position) = self
            .stack
            .iter()
            .rposition(|context| context.tag_name == tag_name)
        else {
            self.push_text(&self.input[start..=end_position]);
            self.position = end_position + 1;
            return;
        };

        while self.stack.len() > stack_position {
            self.stack.pop();
            self.output.push(InputToken::Close);
        }
        self.position = end_position + 1;
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        match self.output.last_mut() {
            Some(InputToken::Text(existing)) => existing.push_str(text),
            _ => self.output.push(InputToken::Text(text.to_owned())),
        }
    }
}

impl ElementContext {
    fn preserve(&self) -> bool {
        self.tag_preserve || self.lang.as_ref().is_some_and(|lang| !is_korean_lang(lang))
    }
}

fn parse_start_tag_name(input: &str, start: usize) -> Option<(usize, usize)> {
    let name_start = start.checked_add(1)?;
    parse_tag_name(input, name_start)
}

fn parse_end_tag_name(input: &str, start: usize) -> Option<(usize, usize)> {
    let name_start = start.checked_add(2)?;
    parse_tag_name(input, name_start)
}

fn parse_tag_name(input: &str, name_start: usize) -> Option<(usize, usize)> {
    let bytes = input.as_bytes();
    let first = *bytes.get(name_start)?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let mut end = name_start + 1;
    while let Some(byte) = bytes.get(end)
        && (byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b':' | b'_'))
    {
        end += 1;
    }
    Some((name_start, end))
}

fn find_tag_end(input: &str, start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut quote = None;
    let mut index = start + 1;
    while let Some(byte) = bytes.get(index).copied() {
        match (quote, byte) {
            (Some(active), current) if active == current => quote = None,
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b'>') => return Some(index),
            _ => {}
        }
        index += 1;
    }
    None
}

fn is_self_closing_start_tag(input: &str, name_end: usize, end_position: usize) -> bool {
    let bytes = input.as_bytes();
    let mut slash_position = end_position;
    while slash_position > name_end && bytes[slash_position - 1].is_ascii_whitespace() {
        slash_position -= 1;
    }
    if slash_position <= name_end || bytes[slash_position - 1] != b'/' {
        return false;
    }

    let slash_index = slash_position - 1;
    if input[name_end..slash_index].trim().is_empty() {
        return true;
    }

    let previous = bytes[slash_index - 1];
    previous.is_ascii_whitespace() || matches!(previous, b'\'' | b'"')
}

fn raw_attributes(input: &str, name_end: usize, end_position: usize, self_closing: bool) -> &str {
    let mut attr_end = end_position;
    if self_closing {
        while attr_end > name_end && input.as_bytes()[attr_end - 1].is_ascii_whitespace() {
            attr_end -= 1;
        }
        if attr_end > name_end && input.as_bytes()[attr_end - 1] == b'/' {
            attr_end -= 1;
        }
    }
    &input[name_end..attr_end]
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    })
}

fn find_raw_text_end_tag(input: &str, tag_name: &str) -> Option<usize> {
    let close_start = format!("</{tag_name}");
    let mut search_start = 0;

    while search_start < input.len() {
        let offset =
            search_start + find_ascii_case_insensitive(&input[search_start..], &close_start)?;
        let delimiter_index = offset + close_start.len();
        if input
            .as_bytes()
            .get(delimiter_index)
            .is_some_and(|byte| is_raw_text_end_tag_delimiter(*byte))
        {
            return Some(offset);
        }
        search_start = delimiter_index;
    }

    None
}

fn is_raw_text_end_tag_delimiter(byte: u8) -> bool {
    byte == b'>' || byte == b'/' || byte.is_ascii_whitespace()
}

fn extract_lang(raw_attributes: &str) -> Option<String> {
    let bytes = raw_attributes.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'-' | b':' | b'_'))
        {
            index += 1;
        }
        if name_start == index {
            index += 1;
            continue;
        }
        let name = &raw_attributes[name_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let value = if matches!(bytes.get(index), Some(b'\'' | b'"')) {
            let quote = bytes[index];
            index += 1;
            let value_start = index;
            while index < bytes.len() && bytes[index] != quote {
                index += 1;
            }
            let value = &raw_attributes[value_start..index];
            if index < bytes.len() {
                index += 1;
            }
            value
        } else {
            let value_start = index;
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            &raw_attributes[value_start..index]
        };
        if name.eq_ignore_ascii_case("lang") {
            return Some(decode_basic_entities(value.trim()).to_ascii_lowercase());
        }
    }
    None
}

fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn is_korean_lang(lang: &str) -> bool {
    let lang = lang.to_ascii_lowercase();
    lang == "ko" || lang == "kor" || lang.starts_with("ko-") || lang.starts_with("kor-")
}

fn is_preserved_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "pre" | "code" | "kbd" | "script" | "style" | "textarea"
    )
}

fn is_raw_text_tag(tag_name: &str) -> bool {
    matches!(tag_name, "script" | "style" | "textarea")
}

fn is_void_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn is_block_boundary_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "figcaption"
            | "figure"
            | "footer"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "section"
            | "table"
            | "td"
            | "th"
            | "tr"
            | "ul"
    )
}
