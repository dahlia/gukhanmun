// Gukhanmun: WebAssembly binding via wasm-bindgen.
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

//! WebAssembly binding for the Gukhanmun hanja-to-hangul converter.
//!
//! Exposes [`WasmGukhanmun`] (owning converter) and [`WasmStream`] (chunked
//! streaming handle) to JavaScript via wasm-bindgen.  The public API mirrors
//! the TypeScript contract in `@gukhanmun/types`: options are passed as a
//! JSON-serialisable object, dictionaries as `{ format, bytes }` records, and
//! the format selector as a string or `{ format: "markdown", gfm?: boolean }`.
//!
//! All tracing calls are compiled out in release builds via
//! `release_max_level_off`.

use std::rc::Rc;

use js_sys::{Array, Object, Reflect, Uint8Array};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use gukhanmun::cdb::CdbDictionary;
use gukhanmun::fst::FstDictionary;
use gukhanmun::html::HtmlElementInfo;
use gukhanmun::markdown::MarkdownVariant;
use gukhanmun::{
    Builder, ContextWindow, Converter, DirectiveAction, NumeralStrategy, OriginalGloss, Preset,
    Recovery, RenderMode, RenderOptions, RubyBase, SegmentationStrategy,
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
    first_occurrence_window: Option<String>,
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

// ── Format selector ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum StreamFormat {
    Text,
    Html,
    Markdown { gfm: bool },
}

// ── Shared converter state ───────────────────────────────────────────────────

/// Shared state between [`WasmGukhanmun`] and any [`WasmStream`] it opens.
struct WasmInner {
    converter: Converter<'static>,
}

// ── Public WASM types ────────────────────────────────────────────────────────

/// Owning hanja-to-hangul converter exposed to JavaScript.
///
/// Construct via the `constructor` and keep the instance alive for the
/// duration of all conversions.  Each call to [`WasmGukhanmun::open_stream`]
/// borrows this instance via a reference count; calling `free()` on the JS
/// side drops the Rust value when the last stream is also freed.
#[wasm_bindgen]
pub struct WasmGukhanmun {
    inner: Rc<WasmInner>,
}

#[wasm_bindgen]
impl WasmGukhanmun {
    /// Creates a new converter from a JS options object and an array of
    /// dictionary records.
    ///
    /// `options` is a JSON-serialisable `GukhanmunOptions` object (may be
    /// `null` / `undefined` for all-defaults).  `dictionaries` is a JS `Array`
    /// where each element is `{ format: "fst", bytes: Uint8Array }`.
    ///
    /// Throws a `GukhanmunError`-shaped object on invalid input or a failed
    /// dictionary load.
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue, dictionaries: JsValue) -> Result<WasmGukhanmun, JsValue> {
        let opts: JsOptions = if options.is_null() || options.is_undefined() {
            JsOptions::default()
        } else {
            serde_wasm_bindgen::from_value(options)
                .map_err(|e| make_error("invalid-input", &e.to_string()))?
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
        if let Some(w) = &opts.first_occurrence_window {
            builder = builder.first_occurrence_window(parse_context_window(w)?);
        }
        if let Some(r) = &opts.recovery {
            builder = builder.recovery(parse_recovery(r)?);
        }

        // Directives
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

        // HTML preserve predicate
        if let Some(html_opts) = opts.html {
            let classes: Vec<String> = html_opts.preserve_classes;
            let attrs: Vec<JsPreserveAttr> = html_opts.preserve_attributes;
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

        // Dictionaries
        let dicts_arr = Array::from(&dictionaries);
        for i in 0..dicts_arr.length() {
            let entry = dicts_arr.get(i);
            let fmt = Reflect::get(&entry, &JsValue::from_str("format"))
                .ok()
                .and_then(|v| v.as_string())
                .ok_or_else(|| {
                    make_error(
                        "invalid-input",
                        "each dictionary must have a 'format' string field",
                    )
                })?;
            let bytes_val = Reflect::get(&entry, &JsValue::from_str("bytes")).map_err(|_| {
                make_error("invalid-input", "each dictionary must have a 'bytes' field")
            })?;
            let bytes = Uint8Array::new(&bytes_val).to_vec();

            match fmt.as_str() {
                "fst" => {
                    let dict = FstDictionary::from_bytes(&bytes)
                        .map_err(|e| make_error("dictionary-load", &e.to_string()))?;
                    builder = builder.push_dictionary(dict);
                }
                "cdb" => {
                    let dict = CdbDictionary::from_bytes(&bytes)
                        .map_err(|e| make_error("dictionary-load", &e.to_string()))?;
                    builder = builder.push_dictionary(dict);
                }
                other => {
                    return Err(make_error(
                        "unsupported-content-type",
                        &format!("unknown dictionary format: {other}"),
                    ));
                }
            }
        }

        let converter = builder.build().map_err(|e| map_gukhanmun_error(&e))?;

        Ok(WasmGukhanmun {
            inner: Rc::new(WasmInner { converter }),
        })
    }

    /// Converts `source` in one shot and returns the result string.
    ///
    /// `format` is `"text"` (default), `"html"`, `"markdown"`, or
    /// `{ format: "markdown", gfm?: boolean }`.  Throws a `GukhanmunError`
    /// on conversion failure.
    pub fn convert(&self, source: &str, format: JsValue) -> Result<String, JsValue> {
        let fmt = parse_format(&format)?;
        convert_with_format(&self.inner.converter, source, fmt)
    }

    /// Opens a streaming handle for chunked conversion.
    ///
    /// The caller must feed string chunks via [`WasmStream::push`] and call
    /// [`WasmStream::finish`] to flush the final output.  The batch-equivalence
    /// invariant holds: concatenating all `push` and `finish` return values
    /// equals the result of a single `convert` call on the concatenated input.
    pub fn open_stream(&self, format: JsValue) -> Result<WasmStream, JsValue> {
        let fmt = parse_format(&format)?;
        Ok(WasmStream {
            inner: Rc::clone(&self.inner),
            format: fmt,
            buffer: String::new(),
        })
    }
}

/// Chunked streaming handle produced by [`WasmGukhanmun::open_stream`].
///
/// Accumulates string chunks and performs the full conversion at
/// [`WasmStream::finish`].  This satisfies the batch-equivalence invariant:
/// every arbitrary chunk partition of an input produces the same final output
/// as a single [`WasmGukhanmun::convert`] call.
#[wasm_bindgen]
pub struct WasmStream {
    inner: Rc<WasmInner>,
    format: StreamFormat,
    buffer: String,
}

#[wasm_bindgen]
impl WasmStream {
    /// Appends `chunk` to the internal buffer.  Returns an empty string; all
    /// output is deferred to [`WasmStream::finish`].
    pub fn push(&mut self, chunk: &str) -> Result<String, JsValue> {
        self.buffer.push_str(chunk);
        Ok(String::new())
    }

    /// Converts the buffered input and returns the result.  Clears the buffer
    /// so the stream handle can be reused.
    pub fn finish(&mut self) -> Result<String, JsValue> {
        let result = convert_with_format(&self.inner.converter, &self.buffer, self.format)?;
        self.buffer.clear();
        Ok(result)
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn convert_with_format(
    converter: &Converter<'static>,
    source: &str,
    fmt: StreamFormat,
) -> Result<String, JsValue> {
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

fn parse_format(val: &JsValue) -> Result<StreamFormat, JsValue> {
    if val.is_null() || val.is_undefined() {
        return Ok(StreamFormat::Text);
    }
    if let Some(s) = val.as_string() {
        return match s.as_str() {
            "text" => Ok(StreamFormat::Text),
            "html" => Ok(StreamFormat::Html),
            "markdown" => Ok(StreamFormat::Markdown { gfm: false }),
            other => Err(make_error(
                "unsupported-content-type",
                &format!("unknown format: {other}"),
            )),
        };
    }
    let fmt_val = Reflect::get(val, &JsValue::from_str("format")).unwrap_or(JsValue::UNDEFINED);
    if fmt_val.as_string().as_deref() == Some("markdown") {
        let gfm = Reflect::get(val, &JsValue::from_str("gfm"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        return Ok(StreamFormat::Markdown { gfm });
    }
    Err(make_error(
        "unsupported-content-type",
        "invalid format value",
    ))
}

fn parse_preset(s: &str) -> Result<Preset, JsValue> {
    match s {
        "ko-kr" => Ok(Preset::KoKr),
        "ko-kp" => Ok(Preset::KoKp),
        other => Err(make_error(
            "invalid-input",
            &format!("unknown preset: {other}"),
        )),
    }
}

fn parse_render_mode(mode: &str, gloss: Option<&str>) -> Result<RenderOptions, JsValue> {
    let render_mode = match mode {
        "hangul-only" => RenderMode::HangulOnly,
        "hangul-hanja-parens" => RenderMode::HangulHanjaParens,
        "hanja-hangul-parens" => RenderMode::HanjaHangulParens,
        "ruby-on-hangul" => RenderMode::Ruby(RubyBase::OnHangul),
        "ruby-on-hanja" => RenderMode::Ruby(RubyBase::OnHanja),
        "original" => RenderMode::Original,
        other => {
            return Err(make_error(
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
                return Err(make_error(
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

fn parse_segmentation(s: &str) -> Result<SegmentationStrategy, JsValue> {
    match s {
        "lattice" => Ok(SegmentationStrategy::Lattice),
        "eager" => Ok(SegmentationStrategy::Eager),
        other => Err(make_error(
            "invalid-input",
            &format!("unknown segmentation strategy: {other}"),
        )),
    }
}

fn parse_numerals(s: &str) -> Result<NumeralStrategy, JsValue> {
    match s {
        "hangul-phonetic" => Ok(NumeralStrategy::HangulPhonetic),
        "positional-arabic" => Ok(NumeralStrategy::PositionalArabic),
        "additive-arabic" => Ok(NumeralStrategy::AdditiveArabic),
        "smart" => Ok(NumeralStrategy::Smart),
        other => Err(make_error(
            "invalid-input",
            &format!("unknown numeral strategy: {other}"),
        )),
    }
}

fn parse_context_window(s: &str) -> Result<ContextWindow, JsValue> {
    match s {
        "off" => Ok(ContextWindow::Off),
        "per-block" => Ok(ContextWindow::PerBlock),
        "per-section" => Ok(ContextWindow::PerSection),
        "per-document" => Ok(ContextWindow::PerDocument),
        other => Err(make_error(
            "invalid-input",
            &format!("unknown context window: {other}"),
        )),
    }
}

fn parse_recovery(s: &str) -> Result<Recovery, JsValue> {
    match s {
        "strict" => Ok(Recovery::Strict),
        "lenient" => Ok(Recovery::Lenient),
        other => Err(make_error(
            "invalid-input",
            &format!("unknown recovery policy: {other}"),
        )),
    }
}

fn make_error(code: &str, message: &str) -> JsValue {
    let obj = Object::new();
    let _ = Reflect::set(&obj, &JsValue::from_str("code"), &JsValue::from_str(code));
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("message"),
        &JsValue::from_str(message),
    );
    let _ = Reflect::set(&obj, &JsValue::from_str("chain"), &Array::new());
    obj.into()
}

fn map_gukhanmun_error(e: &gukhanmun::Error) -> JsValue {
    use gukhanmun::Error;
    let code = match e {
        Error::Core(_) => "segmentation",
        Error::Html(_) => "html-scan",
        Error::Markdown(_) => "markdown",
        Error::Fst(_) => "dictionary-load",
        Error::Io(_) => "io",
        Error::Config(_) => "invalid-input",
        _ => "internal",
    };
    make_error(code, &e.to_string())
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
