// Gukhanmun: Node.js (napi-rs v3) binding.
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

//! Node.js (napi-rs v3) binding for the Gukhanmun hanja-to-hangul converter.
//!
//! Exposes [`NapiGukhanmun`] (owning converter) to JavaScript via napi-rs.
//! Streaming is supported through [`NapiGukhanmun::open_stream`] /
//! [`NapiGukhanmun::stream_push`] / [`NapiGukhanmun::stream_finish`], where
//! the stream state lives in an [`External`] handle that is assembled into a
//! platform `TransformStream` by the TypeScript wrapper layer.
//!
//! Errors are encoded as JSON in the napi `reason` field so the TypeScript
//! wrapper can reconstruct a `GukhanmunError` with `code` and `chain`.

#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde::Deserialize;

use gukhanmun::fst::FstDictionary;
use gukhanmun::html::HtmlElementInfo;
use gukhanmun::markdown::MarkdownVariant;
use gukhanmun::{
    Builder, ContextWindow, Converter, DirectiveAction, HomophoneDetection, NumeralStrategy,
    OriginalGloss, Preset, Recovery, RenderMode, RenderOptions, RubyBase, SegmentationStrategy,
};

// ── Option deserialization structs ──────────────────────────────────────────

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct JsOptions {
    preset: Option<String>,
    rendering: Option<String>,
    original_gloss: Option<String>,
    segmentation: Option<String>,
    numerals: Option<String>,
    initial_sound_law: Option<bool>,
    homophone_window: Option<String>,
    homophone_detection: Option<String>,
    first_occurrence_window: Option<String>,
    collapse_redundant_parens: Option<bool>,
    recovery: Option<String>,
    directives: Option<JsDirectives>,
    html: Option<JsHtmlOptions>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct JsDirectives {
    #[serde(default)]
    require_hanja: Vec<String>,
    #[serde(default)]
    require_hangul: Vec<String>,
    #[serde(default)]
    skip_annotation: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct JsHtmlOptions {
    #[serde(default)]
    preserve_classes: Vec<String>,
    #[serde(default)]
    preserve_attributes: Vec<JsPreserveAttr>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum JsPreserveAttr {
    Name(String),
    NameValue { name: String, value: Option<String> },
}

// ── Dictionary input ─────────────────────────────────────────────────────────

/// Raw dictionary record passed from JavaScript: `{ format: "fst"|"cdb", bytes: Uint8Array }`.
#[napi(object)]
pub struct RawDictInput {
    /// Dictionary format: `"fst"` or `"cdb"`.
    pub format: String,
    /// Raw binary data of the dictionary file.
    pub bytes: Buffer,
}

// ── Format selector ──────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum StreamFormatJson {
    // { format: "markdown", gfm?: boolean }
    MarkdownObj { format: String, gfm: Option<bool> },
}

#[derive(Clone, Copy)]
enum StreamFormat {
    Text,
    Html,
    Markdown { gfm: bool },
}

// ── Public NAPI struct ───────────────────────────────────────────────────────

/// Owning hanja-to-hangul converter exposed to Node.js via napi-rs.
///
/// Construct via the [`NapiGukhanmun::load`] factory.  All methods are
/// synchronous; the TypeScript wrapper exposes the async `load()` contract
/// by wrapping the result in a resolved `Promise`.
#[napi]
pub struct NapiGukhanmun {
    converter: Converter<'static>,
}

// SAFETY: napi-rs synchronous callbacks always execute on the Node.js main
// thread; there is no concurrent access across threads.
unsafe impl Send for NapiGukhanmun {}
unsafe impl Sync for NapiGukhanmun {}

#[napi]
impl NapiGukhanmun {
    /// Creates a new converter from a JSON options string and an array of
    /// pre-resolved dictionary records.
    ///
    /// `options_json` is a JSON-serialised `GukhanmunOptions` object (or
    /// `null`/`undefined` for all defaults).  `dictionaries` is an array of
    /// `{ format: "fst" | "cdb", bytes: Uint8Array }` records where the bytes
    /// have already been loaded by the TypeScript wrapper.
    ///
    /// Throws a JSON-encoded `GukhanmunError` on invalid options or a failed
    /// dictionary load.
    #[napi(factory)]
    pub fn load(
        options_json: Option<String>,
        dictionaries: Option<Vec<RawDictInput>>,
    ) -> napi::Result<NapiGukhanmun> {
        let opts: JsOptions = match options_json.as_deref() {
            None | Some("null") | Some("undefined") => JsOptions::default(),
            Some(json) => {
                serde_json::from_str(json).map_err(|e| napi_err("invalid-input", &e.to_string()))?
            }
        };

        let preset = parse_preset(opts.preset.as_deref().unwrap_or("ko-kr"))?;
        let mut builder = Builder::with_preset(preset).no_bundled_stdict();

        if let Some(r) = &opts.rendering {
            let mode = parse_render_mode(r, opts.original_gloss.as_deref())?;
            builder = builder.rendering(mode);
        }
        if let Some(s) = &opts.segmentation {
            builder = builder.segmentation(parse_segmentation(s)?);
        }
        if let Some(n) = &opts.numerals {
            builder = builder.numerals(parse_numerals(n)?);
        }
        if let Some(v) = opts.initial_sound_law {
            builder = builder.initial_sound_law(v);
        }
        if let Some(w) = &opts.homophone_window {
            builder = builder.homophone_window(parse_context_window(w)?);
        }
        if let Some(d) = &opts.homophone_detection {
            builder = builder.homophone_detection(parse_homophone_detection(d)?);
        }
        if let Some(w) = &opts.first_occurrence_window {
            builder = builder.first_occurrence_window(parse_context_window(w)?);
        }
        if let Some(v) = opts.collapse_redundant_parens {
            builder = builder.collapse_redundant_parens(v);
        }
        if let Some(r) = &opts.recovery {
            builder = builder.recovery(parse_recovery(r)?);
        }

        if let Some(dirs) = opts.directives {
            for h in dirs.require_hanja {
                builder = builder.directive(h, DirectiveAction::RequireHanja);
            }
            for h in dirs.require_hangul {
                builder = builder.directive(h, DirectiveAction::RequireHangul);
            }
            for h in dirs.skip_annotation {
                builder = builder.directive(h, DirectiveAction::SkipAnnotation);
            }
        }

        if let Some(html_opts) = opts.html {
            let classes = html_opts.preserve_classes;
            let attrs = html_opts.preserve_attributes;
            builder = builder.html_preserve_when(move |info: &HtmlElementInfo<'_>| {
                for cls in &classes {
                    if has_class(info.raw_attributes, cls) {
                        return true;
                    }
                }
                for attr in &attrs {
                    match attr {
                        JsPreserveAttr::Name(name) => {
                            if has_attribute(info.raw_attributes, name, None) {
                                return true;
                            }
                        }
                        JsPreserveAttr::NameValue { name, value } => {
                            if has_attribute(info.raw_attributes, name, value.as_deref()) {
                                return true;
                            }
                        }
                    }
                }
                false
            });
        }

        for dict in dictionaries.unwrap_or_default() {
            let bytes = dict.bytes.as_ref();
            match dict.format.as_str() {
                "fst" => {
                    let d = FstDictionary::from_bytes(bytes)
                        .map_err(|e| napi_err("dictionary-load", &e.to_string()))?;
                    builder = builder.push_dictionary(d);
                }
                "cdb" => {
                    use gukhanmun::cdb::CdbDictionary;
                    let d = CdbDictionary::from_bytes(bytes)
                        .map_err(|e| napi_err("dictionary-load", &e.to_string()))?;
                    builder = builder.push_dictionary(d);
                }
                other => {
                    return Err(napi_err(
                        "unsupported-content-type",
                        &format!("unknown dictionary format: {other}"),
                    ));
                }
            }
        }

        let converter = builder.build().map_err(|e| map_gukhanmun_error(&e))?;

        Ok(NapiGukhanmun { converter })
    }

    /// Converts `source` in one shot.
    ///
    /// `format_json` is the JSON-serialised format selector: `"\"text\""`,
    /// `"\"html\""`, `"\"markdown\""`, or `"{\"format\":\"markdown\",\"gfm\":true}"`.
    /// When `null` / `undefined`, defaults to `"text"`.
    ///
    /// Throws a JSON-encoded `GukhanmunError` on conversion failure.
    #[napi]
    pub fn convert(&self, source: String, format_json: Option<String>) -> napi::Result<String> {
        let fmt = parse_format_json(format_json.as_deref())?;
        convert_with_format(&self.converter, &source, fmt)
    }

    /// Opens a streaming handle.
    ///
    /// Returns an opaque [`External`] handle that must be passed back to
    /// [`NapiGukhanmun::stream_push`] and [`NapiGukhanmun::stream_finish`].
    /// The handle is automatically freed when the JavaScript value is
    /// garbage-collected.
    #[napi]
    pub fn open_stream(&self, format_json: Option<String>) -> napi::Result<External<StreamState>> {
        let fmt = parse_format_json(format_json.as_deref())?;
        Ok(External::new(StreamState {
            buffer: String::new(),
            format: fmt,
        }))
    }

    /// Appends `chunk` to the stream's internal buffer.  Returns an empty
    /// string; all output is deferred to [`NapiGukhanmun::stream_finish`].
    #[napi]
    pub fn stream_push(
        &self,
        stream: &mut External<StreamState>,
        chunk: String,
    ) -> napi::Result<String> {
        stream.buffer.push_str(&chunk);
        Ok(String::new())
    }

    /// Converts the buffered input and returns the result.
    ///
    /// Throws a JSON-encoded `GukhanmunError` on conversion failure.
    #[napi]
    pub fn stream_finish(&self, stream: &mut External<StreamState>) -> napi::Result<String> {
        let result = convert_with_format(&self.converter, &stream.buffer, stream.format)?;
        stream.buffer.clear();
        Ok(result)
    }
}

// ── StreamState ──────────────────────────────────────────────────────────────

/// Internal stream state held in an `External` handle.
pub struct StreamState {
    buffer: String,
    format: StreamFormat,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn convert_with_format(
    converter: &Converter<'static>,
    source: &str,
    fmt: StreamFormat,
) -> napi::Result<String> {
    match fmt {
        StreamFormat::Text => converter
            .convert_text_to_string(source)
            .map_err(|e| map_gukhanmun_error(&e)),
        StreamFormat::Html => converter
            .convert_html_fragment_to_string(source)
            .map_err(|e| map_gukhanmun_error(&e)),
        StreamFormat::Markdown { gfm } => {
            let variant = if gfm {
                MarkdownVariant::Gfm
            } else {
                MarkdownVariant::CommonMark
            };
            converter
                .convert_markdown_to_string(source, variant)
                .map_err(|e| map_gukhanmun_error(&e))
        }
    }
}

fn parse_format_json(json: Option<&str>) -> napi::Result<StreamFormat> {
    match json {
        None | Some("null") | Some("undefined") => return Ok(StreamFormat::Text),
        _ => {}
    }
    let raw = json.unwrap();
    // String literal forms: "\"text\"", "\"html\"", "\"markdown\""
    if let Ok(s) = serde_json::from_str::<String>(raw) {
        return match s.as_str() {
            "text" => Ok(StreamFormat::Text),
            "html" => Ok(StreamFormat::Html),
            "markdown" => Ok(StreamFormat::Markdown { gfm: false }),
            other => Err(napi_err(
                "unsupported-content-type",
                &format!("unknown format: {other}"),
            )),
        };
    }
    // Object form: {"format":"markdown","gfm":true}
    if let Ok(obj) = serde_json::from_str::<StreamFormatJson>(raw) {
        let StreamFormatJson::MarkdownObj { format, gfm } = obj;
        if format == "markdown" {
            return Ok(StreamFormat::Markdown {
                gfm: gfm.unwrap_or(false),
            });
        }
        return Err(napi_err(
            "unsupported-content-type",
            &format!("unknown format in object: {format}"),
        ));
    }
    Err(napi_err("unsupported-content-type", "invalid format value"))
}

fn parse_preset(s: &str) -> napi::Result<Preset> {
    match s {
        "ko-kr" => Ok(Preset::KoKr),
        "ko-kp" => Ok(Preset::KoKp),
        other => Err(napi_err(
            "invalid-input",
            &format!("unknown preset: {other}"),
        )),
    }
}

fn parse_render_mode(mode: &str, gloss: Option<&str>) -> napi::Result<RenderOptions> {
    let render_mode = match mode {
        "hangul-only" => RenderMode::HangulOnly,
        "hangul-hanja-parens" => RenderMode::HangulHanjaParens,
        "hanja-hangul-parens" => RenderMode::HanjaHangulParens,
        "ruby-on-hangul" => RenderMode::Ruby(RubyBase::OnHangul),
        "ruby-on-hanja" => RenderMode::Ruby(RubyBase::OnHanja),
        "original" => RenderMode::Original,
        other => {
            return Err(napi_err(
                "invalid-input",
                &format!("unknown rendering mode: {other}"),
            ));
        }
    };
    let original_gloss = if mode == "original" {
        match gloss.unwrap_or("parens") {
            "parens" => OriginalGloss::Parens,
            "ruby" => OriginalGloss::Ruby,
            other => {
                return Err(napi_err(
                    "invalid-input",
                    &format!("unknown originalGloss: {other}"),
                ));
            }
        }
    } else {
        OriginalGloss::Parens
    };
    Ok(RenderOptions {
        mode: render_mode,
        original_gloss,
    })
}

fn parse_segmentation(s: &str) -> napi::Result<SegmentationStrategy> {
    match s {
        "lattice" => Ok(SegmentationStrategy::Lattice),
        "eager" => Ok(SegmentationStrategy::Eager),
        other => Err(napi_err(
            "invalid-input",
            &format!("unknown segmentation strategy: {other}"),
        )),
    }
}

fn parse_numerals(s: &str) -> napi::Result<NumeralStrategy> {
    match s {
        "hangul-phonetic" => Ok(NumeralStrategy::HangulPhonetic),
        "positional-arabic" => Ok(NumeralStrategy::PositionalArabic),
        "additive-arabic" => Ok(NumeralStrategy::AdditiveArabic),
        "smart" => Ok(NumeralStrategy::Smart),
        other => Err(napi_err(
            "invalid-input",
            &format!("unknown numeral strategy: {other}"),
        )),
    }
}

fn parse_context_window(s: &str) -> napi::Result<ContextWindow> {
    match s {
        "off" => Ok(ContextWindow::Off),
        "per-block" => Ok(ContextWindow::PerBlock),
        "per-section" => Ok(ContextWindow::PerSection),
        "per-document" => Ok(ContextWindow::PerDocument),
        other => Err(napi_err(
            "invalid-input",
            &format!("unknown context window: {other}"),
        )),
    }
}

fn parse_homophone_detection(s: &str) -> napi::Result<HomophoneDetection> {
    match s {
        "context-local" => Ok(HomophoneDetection::ContextLocal),
        "dictionary-wide" => Ok(HomophoneDetection::DictionaryWide),
        other => Err(napi_err(
            "invalid-input",
            &format!("unknown homophone detection: {other}"),
        )),
    }
}

fn parse_recovery(s: &str) -> napi::Result<Recovery> {
    match s {
        "strict" => Ok(Recovery::Strict),
        "lenient" => Ok(Recovery::Lenient),
        other => Err(napi_err(
            "invalid-input",
            &format!("unknown recovery policy: {other}"),
        )),
    }
}

/// Creates a JSON-encoded error reason that the TypeScript wrapper parses
/// into a `GukhanmunError`.
fn napi_err(code: &str, message: &str) -> napi::Error {
    let reason = serde_json::json!({
        "code": code,
        "message": message,
        "chain": []
    })
    .to_string();
    napi::Error::from_reason(reason)
}

fn map_gukhanmun_error(e: &gukhanmun::Error) -> napi::Error {
    use gukhanmun::Error;
    use std::error::Error as StdError;
    let code = match e {
        Error::Core(_) => "segmentation",
        Error::Html(_) => "html-scan",
        Error::Markdown(_) => "markdown",
        Error::Fst(_) => "dictionary-load",
        Error::Cdb(_) => "dictionary-load",
        Error::Io(_) => "io",
        Error::Config(_) => "invalid-input",
        _ => "internal",
    };
    let mut chain: Vec<serde_json::Value> = Vec::new();
    let mut src: Option<&(dyn StdError + 'static)> = e.source();
    while let Some(s) = src {
        chain.push(serde_json::json!({ "code": "internal", "message": s.to_string() }));
        src = s.source();
    }
    chain.reverse();
    let reason = serde_json::json!({
        "code": code,
        "message": e.to_string(),
        "chain": chain,
    })
    .to_string();
    napi::Error::from_reason(reason)
}

/// Iterates over `(name, value)` pairs parsed from a raw HTML attribute
/// string.  Names are returned verbatim (compare with
/// `eq_ignore_ascii_case`); values are returned verbatim without entity
/// decoding (sufficient for CSS class and data-attribute matching).
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
                && (bytes[self.pos].is_ascii_alphanumeric()
                    || matches!(bytes[self.pos], b'-' | b':' | b'_' | b'.'))
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
