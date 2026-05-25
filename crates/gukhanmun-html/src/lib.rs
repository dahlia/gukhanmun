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

use std::io::{self, Write};

use gukhanmun_core::{
    ContextWindow, EngineOptions, Error as CoreError, HanjaDictionary, InputToken,
    RecoverableInputError, Recovery, RenderOptions, RenderedToken, Scope, ScopeData,
    mark_homophones, process_tokens_iter_with_options, recover_input_tokens, render_tokens_iter,
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

    fn is_section_boundary(&self) -> bool {
        is_section_boundary_tag(&self.tag_name)
    }
}

/// Information about a freshly opened HTML element passed to a user-supplied
/// preserve predicate.
///
/// The view is borrowed; callers must not retain it past the predicate call.
/// `tag_name` is the canonical lowercase tag name, `raw_attributes` is the raw
/// attribute text of the start tag (with leading whitespace preserved, as on
/// [`HtmlScopeData::raw_attributes`]), and `lang` reflects the inherited
/// `lang` value after the adapter's normal inheritance has been applied.
#[derive(Clone, Copy, Debug)]
pub struct HtmlElementInfo<'a> {
    /// Canonical lowercase tag name.
    pub tag_name: &'a str,
    /// Raw attribute text from the start tag.
    pub raw_attributes: &'a str,
    /// Inherited `lang` value, if any.
    pub lang: Option<&'a str>,
}

type PreservePredicate<'a> = dyn Fn(&HtmlElementInfo<'_>) -> bool + 'a;

/// Caller-supplied configuration for the HTML reader.
///
/// The reader applies the hardcoded preserved-tag list and the inherited
/// `lang` rule unconditionally; [`HtmlReaderOptions::preserve_when`] adds a
/// user-defined predicate that runs in addition to those.  A predicate that
/// returns `true` for an element preserves that element and is inherited by
/// every descendant scope, matching how the built-in preserved tags propagate.
#[derive(Default)]
pub struct HtmlReaderOptions<'a> {
    preserve_when: Option<Box<PreservePredicate<'a>>>,
}

impl<'a> HtmlReaderOptions<'a> {
    /// Creates an options value with no user predicate.
    pub fn new() -> Self {
        Self {
            preserve_when: None,
        }
    }

    /// Attaches a predicate that flags elements for preservation.
    ///
    /// The predicate sees a [`HtmlElementInfo`] for every freshly opened
    /// element and returns `true` to preserve the element (and its
    /// descendants) verbatim.  Multiple calls replace the predicate; users who
    /// want OR-composition should combine their conditions inside the closure.
    pub fn preserve_when<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&HtmlElementInfo<'_>) -> bool + 'a,
    {
        self.preserve_when = Some(Box::new(predicate));
        self
    }

    fn evaluate(&self, info: &HtmlElementInfo<'_>) -> bool {
        self.preserve_when
            .as_ref()
            .is_some_and(|predicate| predicate(info))
    }
}

impl<'a> std::fmt::Debug for HtmlReaderOptions<'a> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HtmlReaderOptions")
            .field(
                "preserve_when",
                &self.preserve_when.as_ref().map(|_| "<fn>"),
            )
            .finish()
    }
}

/// Error returned while reading or writing HTML fragments.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HtmlError {
    /// A tag-like construct could not be parsed as an HTML tag.
    #[error("malformed HTML tag at byte {position}: {snippet}")]
    MalformedTag {
        /// Byte position of the malformed construct.
        position: usize,

        /// Source text for the malformed construct.
        snippet: String,
    },

    /// A construct that requires an explicit terminator reached end of input.
    #[error("unclosed HTML {construct} at byte {position}")]
    UnclosedConstruct {
        /// Human-readable construct name.
        construct: &'static str,

        /// Byte position where the construct started.
        position: usize,
    },
}

/// Incremental HTML fragment reader.
///
/// The reader accepts UTF-8 string chunks, preserves scanner state across
/// chunk boundaries, and emits fallible input tokens as soon as the current
/// buffer contains a complete text or markup region.  It intentionally remains
/// fragment-oriented rather than HTML5-conformant, matching the one-shot
/// reader's recovery and scope rules.
pub struct HtmlFragmentReader<'r, 'o> {
    buffer: String,
    base_position: usize,
    stack: Vec<ElementContext>,
    options: HtmlReaderOptionsSource<'r, 'o>,
}

enum HtmlReaderOptionsSource<'r, 'o> {
    Default,
    Borrowed(&'r HtmlReaderOptions<'o>),
}

impl HtmlReaderOptionsSource<'_, '_> {
    fn evaluate(&self, info: &HtmlElementInfo<'_>) -> bool {
        match self {
            Self::Default => false,
            Self::Borrowed(options) => options.evaluate(info),
        }
    }
}

impl HtmlFragmentReader<'static, 'static> {
    /// Creates a reader with default [`HtmlReaderOptions`].
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            base_position: 0,
            stack: Vec::new(),
            options: HtmlReaderOptionsSource::Default,
        }
    }
}

impl Default for HtmlFragmentReader<'static, 'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'r, 'o> HtmlFragmentReader<'r, 'o> {
    /// Creates a reader with caller-supplied [`HtmlReaderOptions`].
    pub fn with_options(options: &'r HtmlReaderOptions<'o>) -> Self {
        Self {
            buffer: String::new(),
            base_position: 0,
            stack: Vec::new(),
            options: HtmlReaderOptionsSource::Borrowed(options),
        }
    }

    /// Pushes another input chunk and returns every complete token available.
    ///
    /// Partial tags, quoted attributes, comments, CDATA regions, declarations,
    /// and raw-text end tags remain buffered until a later chunk or
    /// [`HtmlFragmentReader::finish`] resolves them.
    pub fn push_str(
        &mut self,
        input: &str,
    ) -> Vec<Result<InputToken<HtmlScopeData>, RecoverableInputError>> {
        self.buffer.push_str(input);
        self.scan_available(false)
    }

    /// Finishes the input stream and returns remaining tokens or recoverable
    /// errors for any unclosed construct still buffered.
    pub fn finish(mut self) -> Vec<Result<InputToken<HtmlScopeData>, RecoverableInputError>> {
        self.scan_available(true)
    }

    fn scan_available(&mut self, finish: bool) -> Vec<ScanItem> {
        let mut output = Vec::new();
        while !self.buffer.is_empty() {
            let progressed = if self.in_raw_text_element() {
                self.scan_raw_text_element(&mut output, finish)
            } else if self.buffer.starts_with('<') {
                self.scan_markup(&mut output, finish)
            } else {
                self.scan_text(&mut output)
            };
            if !progressed {
                break;
            }
        }
        output
    }

    fn in_raw_text_element(&self) -> bool {
        self.stack
            .last()
            .is_some_and(|context| is_raw_text_tag(&context.tag_name))
    }

    fn drain_to(&mut self, end: usize) -> String {
        let drained = self.buffer.drain(..end).collect::<String>();
        self.base_position += end;
        drained
    }

    fn push_recoverable(
        &mut self,
        output: &mut Vec<ScanItem>,
        original_len: usize,
        error: HtmlError,
    ) {
        tracing::trace!(
            position = self.base_position,
            "html scanner recovered a malformed region"
        );
        let original = self.drain_to(original_len);
        output.push(Err(RecoverableInputError::new(
            original,
            CoreError::Other(Box::new(error)),
        )));
    }

    fn scan_text(&mut self, output: &mut Vec<ScanItem>) -> bool {
        let end = self.buffer.find('<').unwrap_or(self.buffer.len());
        if end == 0 {
            return false;
        }
        let text = self.drain_to(end);
        push_text(output, text);
        true
    }

    fn scan_markup(&mut self, output: &mut Vec<ScanItem>, finish: bool) -> bool {
        if self.buffer.starts_with("<!--") {
            return self.scan_verbatim(output, "<!--", "-->", finish);
        }
        if self.buffer.starts_with("<![CDATA[") {
            return self.scan_verbatim(output, "<![CDATA[", "]]>", finish);
        }
        if self.buffer.starts_with("</") {
            return self.scan_end_tag(output, finish);
        }
        if self.buffer.starts_with("<!") || self.buffer.starts_with("<?") {
            return self.scan_declaration(output, finish);
        }
        self.scan_start_tag(output, finish)
    }

    fn scan_verbatim(
        &mut self,
        output: &mut Vec<ScanItem>,
        start: &'static str,
        end: &str,
        finish: bool,
    ) -> bool {
        if !self.buffer.starts_with(start) {
            return false;
        }
        let Some(end_offset) = self.buffer[start.len()..].find(end) else {
            if !finish {
                return false;
            }
            let position = self.base_position;
            self.push_recoverable(
                output,
                self.buffer.len(),
                HtmlError::UnclosedConstruct {
                    construct: start,
                    position,
                },
            );
            return true;
        };
        let end_position = start.len() + end_offset + end.len();
        output.push(Ok(InputToken::Verbatim(self.drain_to(end_position))));
        true
    }

    fn scan_declaration(&mut self, output: &mut Vec<ScanItem>, finish: bool) -> bool {
        let Some(end_position) = find_tag_end(&self.buffer, 0) else {
            if !finish {
                return false;
            }
            let position = self.base_position;
            self.push_recoverable(
                output,
                self.buffer.len(),
                HtmlError::UnclosedConstruct {
                    construct: "declaration",
                    position,
                },
            );
            return true;
        };
        output.push(Ok(InputToken::Verbatim(self.drain_to(end_position + 1))));
        true
    }

    fn scan_start_tag(&mut self, output: &mut Vec<ScanItem>, finish: bool) -> bool {
        if self.buffer == "<" && !finish {
            return false;
        }
        let Some((name_start, name_end)) = parse_start_tag_name(&self.buffer, 0) else {
            let error = malformed_tag(&self.buffer, 0, self.base_position);
            self.push_recoverable(output, 1, error);
            return true;
        };
        let Some(end_position) = find_tag_end(&self.buffer, 0) else {
            if !finish {
                return false;
            }
            let position = self.base_position;
            self.push_recoverable(
                output,
                self.buffer.len(),
                HtmlError::UnclosedConstruct {
                    construct: "start tag",
                    position,
                },
            );
            return true;
        };

        let tag_original = &self.buffer[name_start..name_end];
        let tag_name = tag_original.to_ascii_lowercase();
        let raw_start_tag = self.buffer[..=end_position].to_owned();
        let self_closing = is_self_closing_start_tag(&self.buffer, name_end, end_position);
        let raw_attributes = raw_attributes(&self.buffer, name_end, end_position, self_closing);
        let mut context = self.context_for(&tag_name, raw_attributes);
        let predicate_preserve_inherited = self
            .stack
            .last()
            .is_some_and(|parent| parent.predicate_preserve);
        let predicate_preserve_self = predicate_preserve_inherited
            || self.evaluate_preserve_predicate(&tag_name, raw_attributes, &context);
        context.predicate_preserve = predicate_preserve_self;
        let omit_end_tag = self_closing || is_void_tag(&tag_name);
        let scope = HtmlScopeData {
            tag_name: tag_name.clone(),
            raw_attributes: raw_attributes.to_owned(),
            raw_start_tag,
            end_tag_name: tag_original.to_owned(),
            omit_end_tag,
            preserve: context.preserve(),
            allows_inline_markup: !is_text_only_content_tag(&tag_name)
                && !context.text_only_ancestor,
            block_boundary: is_block_boundary_tag(&tag_name),
        };

        output.push(Ok(InputToken::Open(Scope::new(scope))));
        self.drain_to(end_position + 1);

        if !omit_end_tag {
            self.stack.push(ElementContext {
                tag_name: tag_name.clone(),
                tag_preserve: context.tag_preserve,
                predicate_preserve: predicate_preserve_self,
                text_only_ancestor: context.text_only_ancestor
                    || is_text_only_content_tag(&tag_name),
                lang: context.lang,
            });
        } else {
            output.push(Ok(InputToken::Close));
        }
        true
    }

    fn context_for(&self, tag_name: &str, raw_attributes: &str) -> ElementContext {
        let parent_tag_preserve = self
            .stack
            .last()
            .is_some_and(|context| context.tag_preserve);
        let parent_text_only_ancestor = self
            .stack
            .last()
            .is_some_and(|context| context.text_only_ancestor);
        let tag_preserve = parent_tag_preserve || is_preserved_tag(tag_name);
        let lang = extract_lang(raw_attributes).or_else(|| {
            self.stack
                .last()
                .and_then(|context| context.lang.as_ref().cloned())
        });
        ElementContext {
            tag_name: tag_name.to_owned(),
            tag_preserve,
            predicate_preserve: false,
            text_only_ancestor: parent_text_only_ancestor,
            lang,
        }
    }

    fn evaluate_preserve_predicate(
        &self,
        tag_name: &str,
        raw_attributes: &str,
        context: &ElementContext,
    ) -> bool {
        let info = HtmlElementInfo {
            tag_name,
            raw_attributes,
            lang: context.lang.as_deref(),
        };
        self.options.evaluate(&info)
    }

    fn scan_raw_text_element(&mut self, output: &mut Vec<ScanItem>, finish: bool) -> bool {
        let tag_name = self
            .stack
            .last()
            .expect("raw text mode has an open element")
            .tag_name
            .clone();
        let close_start = format!("</{tag_name}");
        let Some(close_offset) = find_raw_text_end_tag(&self.buffer, &tag_name) else {
            if finish {
                let position = self.base_position;
                self.push_recoverable(
                    output,
                    self.buffer.len(),
                    HtmlError::UnclosedConstruct {
                        construct: "raw text element",
                        position,
                    },
                );
                return true;
            }
            let keep = close_start.len().min(self.buffer.len());
            let emit_len =
                floor_char_boundary(&self.buffer, self.buffer.len().saturating_sub(keep));
            if emit_len == 0 {
                return false;
            }
            output.push(Ok(InputToken::Verbatim(self.drain_to(emit_len))));
            return true;
        };

        if close_offset > 0 {
            output.push(Ok(InputToken::Verbatim(self.drain_to(close_offset))));
            return true;
        }
        self.scan_end_tag(output, finish)
    }

    fn scan_end_tag(&mut self, output: &mut Vec<ScanItem>, finish: bool) -> bool {
        if self.buffer.len() <= 2 && self.buffer.starts_with("</") && !finish {
            return false;
        }
        let Some((name_start, name_end)) = parse_end_tag_name(&self.buffer, 0) else {
            let error = malformed_tag(&self.buffer, 0, self.base_position);
            self.push_recoverable(output, 1, error);
            return true;
        };
        let Some(end_position) = find_tag_end(&self.buffer, 0) else {
            if !finish {
                return false;
            }
            let position = self.base_position;
            self.push_recoverable(
                output,
                self.buffer.len(),
                HtmlError::UnclosedConstruct {
                    construct: "end tag",
                    position,
                },
            );
            return true;
        };

        let tag_name = self.buffer[name_start..name_end].to_ascii_lowercase();
        let Some(stack_position) = self
            .stack
            .iter()
            .rposition(|context| context.tag_name == tag_name)
        else {
            let text = self.drain_to(end_position + 1);
            push_text(output, text);
            return true;
        };

        while self.stack.len() > stack_position {
            self.stack.pop();
            output.push(Ok(InputToken::Close));
        }
        self.drain_to(end_position + 1);
        true
    }
}

/// Reads an HTML fragment into the core input-token stream.
///
/// The scanner is fragment-oriented and intentionally does not implement full
/// HTML5 tree construction.  It preserves raw start tags and non-text
/// constructs and computes effective preserve flags for scopes.  Malformed
/// constructs are recovered leniently: each is preserved as a
/// [`InputToken::Verbatim`] region (so its original bytes pass through
/// untouched) rather than reported as an error.  Use
/// [`try_read_html_fragment`] when malformed regions should be able to fail
/// the read.
pub fn read_html_fragment(input: &str) -> Vec<InputToken<HtmlScopeData>> {
    read_html_fragment_iter(input).collect()
}

/// Reads an HTML fragment as an iterator over core input tokens.
///
/// The current scanner still receives a complete fragment string, but callers
/// can compose the resulting token stream without depending on a `Vec` return
/// type.  Malformed regions are recovered leniently, as in
/// [`read_html_fragment`].
pub fn read_html_fragment_iter(input: &str) -> std::vec::IntoIter<InputToken<HtmlScopeData>> {
    let default_options = HtmlReaderOptions::default();
    read_html_fragment_iter_with_options(input, &default_options)
}

/// Reads an HTML fragment with caller-supplied [`HtmlReaderOptions`].
///
/// The options may attach a user predicate that participates in the adapter's
/// preserve decision alongside the hardcoded preserved-tag list and the
/// inherited `lang` rule.  A scope flagged by the predicate is preserved and
/// the flag is inherited by descendants, matching the existing preserved-tag
/// inheritance behavior.  Malformed regions are recovered leniently, as in
/// [`read_html_fragment`].
pub fn read_html_fragment_with_options(
    input: &str,
    options: &HtmlReaderOptions<'_>,
) -> Vec<InputToken<HtmlScopeData>> {
    read_html_fragment_iter_with_options(input, options).collect()
}

/// Iterator variant of [`read_html_fragment_with_options`].
pub fn read_html_fragment_iter_with_options(
    input: &str,
    options: &HtmlReaderOptions<'_>,
) -> std::vec::IntoIter<InputToken<HtmlScopeData>> {
    // Lenient recovery cannot fail: every `Err` becomes a `Verbatim` token, so
    // the infallible readers resolve the scanner's fallible stream this way.
    recover_input_tokens(
        try_read_html_fragment_iter_with_options(input, options),
        Recovery::Lenient,
    )
    .expect("lenient recovery of HTML tokens is infallible")
    .into_iter()
}

/// Reads an HTML fragment as a fallible token stream.
///
/// This is the recovery-neutral primitive that the one-shot and umbrella
/// readers build on.  Each well-formed region is yielded as `Ok(InputToken)`;
/// each malformed region the scanner can describe and preserve is yielded as
/// `Err(RecoverableInputError)` whose original text is the byte-for-byte source
/// of that region.  The caller chooses a policy by passing the stream to
/// [`recover_input_tokens`] (or the engine-level
/// [`process_fallible_tokens`](gukhanmun_core::process_fallible_tokens)).
pub fn try_read_html_fragment_iter(
    input: &str,
) -> std::vec::IntoIter<Result<InputToken<HtmlScopeData>, RecoverableInputError>> {
    let default_options = HtmlReaderOptions::default();
    try_read_html_fragment_iter_with_options(input, &default_options)
}

/// Fallible token-stream reader with caller-supplied [`HtmlReaderOptions`].
///
/// See [`try_read_html_fragment_iter`] for the recovery contract.
pub fn try_read_html_fragment_iter_with_options(
    input: &str,
    options: &HtmlReaderOptions<'_>,
) -> std::vec::IntoIter<Result<InputToken<HtmlScopeData>, RecoverableInputError>> {
    let mut reader = HtmlFragmentReader::with_options(options);
    let mut output = reader.push_str(input);
    output.extend(reader.finish());
    output.into_iter()
}

/// Reads an HTML fragment with an explicit recovery policy.
///
/// `Recovery::Strict` returns the first malformed region's cause as a
/// [`gukhanmun_core::Error`] (the HTML-specific [`HtmlError`] is preserved as
/// its boxed source).  `Recovery::Lenient` preserves each malformed region as a
/// verbatim token, logs it once at `warn` level, and continues.  Both modes
/// drive the shared [`recover_input_tokens`] primitive over
/// [`try_read_html_fragment_iter`].
///
/// The compatibility one-shot API collects the incremental reader's fallible
/// stream before applying the recovery policy.
pub fn try_read_html_fragment(
    input: &str,
    recovery: Recovery,
) -> Result<Vec<InputToken<HtmlScopeData>>, CoreError> {
    try_read_html_fragment_with_options(input, &HtmlReaderOptions::default(), recovery)
}

/// Reads an HTML fragment with caller-supplied options and an explicit recovery
/// policy.
///
/// See [`try_read_html_fragment`] for the recovery contract.
pub fn try_read_html_fragment_with_options(
    input: &str,
    options: &HtmlReaderOptions<'_>,
    recovery: Recovery,
) -> Result<Vec<InputToken<HtmlScopeData>>, CoreError> {
    recover_input_tokens(
        try_read_html_fragment_iter_with_options(input, options),
        recovery,
    )
}

/// Writes rendered HTML tokens back to a fragment string.
///
/// Start tags are emitted from the raw source text captured by the reader.
/// `Text` and `Verbatim` tokens are passed through without additional
/// escaping (the reader does not entity-encode `Text` either, so this matches
/// the original input form). Renderer-emitted `Ruby` tokens are wrapped in a
/// `<ruby><rt>...</rt></ruby>` element with HTML-special characters escaped
/// in both the base text and the `rt` gloss; that prevents any user- or
/// dictionary-supplied reading from breaking out of the markup.
pub fn write_html_fragment(
    tokens: impl IntoIterator<Item = RenderedToken<HtmlScopeData>>,
) -> String {
    let mut bytes = Vec::new();
    let mut writer = HtmlFragmentWriter::new(&mut bytes);
    for token in tokens {
        writer
            .write_token(token)
            .expect("writing HTML to an in-memory buffer cannot fail");
    }
    writer
        .finish()
        .expect("flushing an in-memory HTML buffer cannot fail");
    String::from_utf8(bytes).expect("HTML writer only emits UTF-8")
}

/// Streaming HTML fragment writer.
///
/// The writer serializes each rendered token as it arrives, keeping only the
/// open-scope stack needed to reconstruct end tags from reader-owned scope
/// data.
pub struct HtmlFragmentWriter<W> {
    output: W,
    scopes: Vec<HtmlScopeData>,
}

impl<W> HtmlFragmentWriter<W>
where
    W: Write,
{
    /// Creates a writer that serializes into `output`.
    pub fn new(output: W) -> Self {
        Self {
            output,
            scopes: Vec::new(),
        }
    }

    /// Writes one rendered token.
    pub fn write_token(&mut self, token: RenderedToken<HtmlScopeData>) -> io::Result<()> {
        match token {
            RenderedToken::Open(scope) => {
                self.output
                    .write_all(scope.data().raw_start_tag.as_bytes())?;
                self.scopes.push(scope.into_data());
            }
            RenderedToken::Close => {
                if let Some(scope) = self.scopes.pop()
                    && !scope.omit_end_tag
                {
                    self.output.write_all(b"</")?;
                    self.output.write_all(scope.end_tag_name.as_bytes())?;
                    self.output.write_all(b">")?;
                }
            }
            RenderedToken::Text(text) | RenderedToken::Verbatim(text) => {
                self.output.write_all(text.as_bytes())?;
            }
            RenderedToken::Ruby { base, rt } => {
                self.output.write_all(b"<ruby>")?;
                write_escaped_html_text(&mut self.output, &base)?;
                self.output.write_all(b"<rt>")?;
                write_escaped_html_text(&mut self.output, &rt)?;
                self.output.write_all(b"</rt></ruby>")?;
            }
        }
        Ok(())
    }

    /// Flushes the wrapped output without finishing the writer.
    pub fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }

    /// Flushes and returns the wrapped output value.
    pub fn finish(mut self) -> io::Result<W> {
        self.output.flush()?;
        Ok(self.output)
    }
}

/// Writes `input` to `output`, escaping characters that have special meaning
/// in HTML element content.
fn write_escaped_html_text(output: &mut impl Write, input: &str) -> io::Result<()> {
    for ch in input.chars() {
        match ch {
            '&' => output.write_all(b"&amp;")?,
            '<' => output.write_all(b"&lt;")?,
            '>' => output.write_all(b"&gt;")?,
            other => {
                let mut buffer = [0; 4];
                output.write_all(other.encode_utf8(&mut buffer).as_bytes())?;
            }
        }
    }
    Ok(())
}

/// Converts an HTML fragment with default engine options.
///
/// `render` accepts either a [`gukhanmun_core::RenderMode`] or a fully
/// constructed [`RenderOptions`] value (see
/// [`From<RenderMode> for RenderOptions`](RenderOptions#impl-From<RenderMode>-for-RenderOptions)).
pub fn convert_html_fragment<D, R>(input: &str, dictionary: &D, render: R) -> String
where
    D: HanjaDictionary + ?Sized,
    R: Into<RenderOptions>,
{
    convert_html_fragment_with_options(input, dictionary, render, EngineOptions::default())
}

/// Converts an HTML fragment with explicit engine options.
pub fn convert_html_fragment_with_options<D, R>(
    input: &str,
    dictionary: &D,
    render: R,
    options: EngineOptions,
) -> String
where
    D: HanjaDictionary + ?Sized,
    R: Into<RenderOptions>,
{
    let input_tokens = read_html_fragment(input);
    let output_tokens = process_tokens_iter_with_options(input_tokens, dictionary, options);
    let output_tokens = mark_homophones(output_tokens, dictionary, ContextWindow::PerBlock);
    let rendered_tokens = render_tokens_iter(output_tokens, render);
    write_html_fragment(rendered_tokens)
}

/// Converts an HTML fragment with an explicit recovery policy.
///
/// Reader errors surface as [`gukhanmun_core::Error`]; see
/// [`try_read_html_fragment`] for the recovery contract.
pub fn try_convert_html_fragment<D, R>(
    input: &str,
    dictionary: &D,
    render: R,
    recovery: Recovery,
) -> Result<String, CoreError>
where
    D: HanjaDictionary + ?Sized,
    R: Into<RenderOptions>,
{
    try_convert_html_fragment_with_options(
        input,
        dictionary,
        render,
        EngineOptions::default(),
        recovery,
    )
}

/// Converts an HTML fragment with explicit engine options and recovery policy.
///
/// Reader errors surface as [`gukhanmun_core::Error`]; see
/// [`try_read_html_fragment`] for the recovery contract.
pub fn try_convert_html_fragment_with_options<D, R>(
    input: &str,
    dictionary: &D,
    render: R,
    options: EngineOptions,
    recovery: Recovery,
) -> Result<String, CoreError>
where
    D: HanjaDictionary + ?Sized,
    R: Into<RenderOptions>,
{
    let input_tokens = try_read_html_fragment(input, recovery)?;
    let output_tokens = process_tokens_iter_with_options(input_tokens, dictionary, options);
    let output_tokens = mark_homophones(output_tokens, dictionary, ContextWindow::PerBlock);
    let rendered_tokens = render_tokens_iter(output_tokens, render);
    Ok(write_html_fragment(rendered_tokens))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ElementContext {
    tag_name: String,
    tag_preserve: bool,
    predicate_preserve: bool,
    text_only_ancestor: bool,
    lang: Option<String>,
}

/// One scanner output item: a well-formed token, or a recoverable malformed
/// region whose `original` text is the byte-for-byte source the lenient path
/// preserves as a verbatim token.
type ScanItem = Result<InputToken<HtmlScopeData>, RecoverableInputError>;

impl ElementContext {
    fn preserve(&self) -> bool {
        self.tag_preserve
            || self.predicate_preserve
            || self.lang.as_ref().is_some_and(|lang| !is_korean_lang(lang))
    }
}

fn parse_start_tag_name(input: &str, start: usize) -> Option<(usize, usize)> {
    let name_start = start.checked_add(1)?;
    parse_tag_name(input, name_start)
}

fn push_text(output: &mut Vec<ScanItem>, text: String) {
    if text.is_empty() {
        return;
    }
    match output.last_mut() {
        Some(Ok(InputToken::Text(existing))) => existing.push_str(&text),
        _ => output.push(Ok(InputToken::Text(text))),
    }
}

fn malformed_tag(input: &str, local_position: usize, absolute_position: usize) -> HtmlError {
    let source_end = input[local_position + 1..]
        .find('>')
        .map_or(input.len(), |offset| local_position + 1 + offset + 1);
    HtmlError::MalformedTag {
        position: absolute_position,
        snippet: input[local_position..source_end].to_owned(),
    }
}

fn floor_char_boundary(input: &str, mut index: usize) -> usize {
    while !input.is_char_boundary(index) {
        index -= 1;
    }
    index
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

/// Returns `true` when `lang` is a Korean BCP 47 primary or extended language
/// tag.
///
/// Recognised prefixes are `ko`, `kor`, `ko-*`, and `kor-*` (case-insensitive),
/// matching the predicate used by the HTML adapter's `lang` inheritance rule.
pub fn is_korean_lang(lang: &str) -> bool {
    let lang = lang.to_ascii_lowercase();
    lang == "ko" || lang == "kor" || lang.starts_with("ko-") || lang.starts_with("kor-")
}

/// Classification of a single inline HTML fragment as produced by
/// `pulldown-cmark`'s `Event::InlineHtml`.
///
/// `classify_inline_html` inspects the fragment and returns one of these
/// variants.  Callers use the result to decide how to handle the fragment in
/// the Markdown pipeline without duplicating HTML-scanner logic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InlineHtml {
    /// A start tag, including self-closing (`<br/>`) and void (`<br>`) forms.
    StartTag(InlineStartTag),
    /// An end tag (`</name>`).
    EndTag {
        /// Canonical lowercase tag name.
        tag_name: String,
    },
    /// A non-element construct: an HTML comment (`<!--…-->`), a CDATA section
    /// (`<![CDATA[…]]>`), a processing instruction (`<?…?>`), or a declaration
    /// (`<!…>`).  These must pass through verbatim without scope tracking.
    NonElement,
    /// A `<…>`-shaped fragment whose tag name cannot be parsed.  Callers
    /// should preserve it verbatim and, if desired, log a diagnostic.
    Malformed,
}

/// Parsed details of an inline HTML start tag.
///
/// All fields are extracted by the same scanner logic used in the HTML adapter,
/// so the Markdown adapter can share the HTML crate's rules for `lang`
/// inheritance, preserved tags, and void elements without duplicating code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineStartTag {
    /// Canonical lowercase tag name used for policy decisions.
    pub tag_name: String,
    /// Raw start-tag text from `<` through `>` (for serialisation).
    pub raw_start_tag: String,
    /// Raw attribute text (leading whitespace preserved, slash and `>` excluded).
    pub raw_attributes: String,
    /// Original-casing tag name for constructing the matching end tag.
    pub end_tag_name: String,
    /// `lang` attribute value from this tag only (lowercased, entities decoded).
    /// Ancestor `lang` inheritance is the caller's responsibility.
    pub lang: Option<String>,
    /// Whether the tag carries an explicit self-closing slash (`<br />`).
    pub self_closing: bool,
    /// Whether the end tag should be omitted (self-closing or void element).
    pub omit_end_tag: bool,
    /// Whether this is a preserved tag (`pre`, `code`, `kbd`, `script`,
    /// `style`, `textarea`).
    pub is_preserved_tag: bool,
    /// Whether this tag has a text-only content model (`title`, `option`).
    pub is_text_only_content: bool,
}

/// Classifies a single inline HTML fragment into its structural role.
///
/// The input should be the raw text of a single `pulldown-cmark`
/// `Event::InlineHtml` or `Event::Html` token — a complete single-tag string.
/// The function uses the same scanner primitives as the HTML adapter, so all
/// policy decisions (preserved tags, void elements, `lang` extraction) are
/// consistent with `HtmlFragmentReader`.
///
/// Note that this function only parses the tag itself; `lang` inheritance from
/// ancestor scopes remains the caller's responsibility.
pub fn classify_inline_html(html: &str) -> InlineHtml {
    if html.starts_with("<!--")
        || html.starts_with("<![CDATA[")
        || html.starts_with("<!")
        || html.starts_with("<?")
    {
        return InlineHtml::NonElement;
    }

    if html.starts_with("</") {
        if find_tag_end(html, 0).is_none() {
            return InlineHtml::Malformed;
        }
        return match parse_end_tag_name(html, 0) {
            Some((name_start, name_end)) => InlineHtml::EndTag {
                tag_name: html[name_start..name_end].to_ascii_lowercase(),
            },
            None => InlineHtml::Malformed,
        };
    }

    let Some((name_start, name_end)) = parse_start_tag_name(html, 0) else {
        return InlineHtml::Malformed;
    };
    let Some(end_position) = find_tag_end(html, 0) else {
        return InlineHtml::Malformed;
    };

    let end_tag_name = html[name_start..name_end].to_owned();
    let tag_name = end_tag_name.to_ascii_lowercase();
    let self_closing = is_self_closing_start_tag(html, name_end, end_position);
    let raw_attrs = raw_attributes(html, name_end, end_position, self_closing).to_owned();
    let lang = extract_lang(&raw_attrs);
    let omit_end_tag = self_closing || is_void_tag(&tag_name);

    InlineHtml::StartTag(InlineStartTag {
        raw_start_tag: html.to_owned(),
        is_preserved_tag: is_preserved_tag(&tag_name),
        is_text_only_content: is_text_only_content_tag(&tag_name),
        raw_attributes: raw_attrs,
        end_tag_name,
        lang,
        self_closing,
        omit_end_tag,
        tag_name,
    })
}

fn is_preserved_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "pre" | "code" | "kbd" | "script" | "style" | "textarea"
    )
}

/// HTML5 elements whose content model is text-only (no phrasing or flow
/// content). Text conversion is still safe inside them — the engine can map
/// `漢字` to `한자` — but inline markup such as `<ruby>` would produce invalid
/// content, so the scope reports `allows_inline_markup = false` and renderers
/// fall back to parens.
fn is_text_only_content_tag(tag_name: &str) -> bool {
    matches!(tag_name, "title" | "option")
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

fn is_section_boundary_tag(tag_name: &str) -> bool {
    matches!(tag_name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}
