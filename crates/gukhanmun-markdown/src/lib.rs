// Gukhanmun: Markdown adapter for Gukhanmun.
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

//! Markdown reader and writer for Gukhanmun.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use gukhanmun_core::{
    ContextWindow, EngineOptions, HanjaDictionary, InputToken, RenderOptions, RenderedToken, Scope,
    ScopeData, mark_homophones, process_tokens_iter_with_options, render_tokens_iter,
};
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};

/// Adapter-owned scope data for Markdown documents.
///
/// The value stores the Markdown event or inline-HTML element represented by
/// the scope plus the effective policy flags consumed by the core pipeline.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkdownScopeData {
    node: MarkdownNode,
    preserve: bool,
    allows_inline_markup: bool,
    block_boundary: bool,
}

impl MarkdownScopeData {
    /// Returns whether text in this scope should pass through unchanged.
    pub fn is_preserve(&self) -> bool {
        self.preserve
    }

    /// Returns whether this scope resets block-oriented middleware state.
    pub fn is_block_boundary(&self) -> bool {
        self.block_boundary
    }
}

impl ScopeData for MarkdownScopeData {
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
        matches!(&self.node, MarkdownNode::Container(Tag::Heading { .. }))
    }
}

/// Error returned when Markdown reading or serialization fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MarkdownError {
    /// Markdown serialization failed.
    #[error("failed to serialize Markdown: {source}")]
    Serialize {
        /// Underlying serializer error.
        #[source]
        source: pulldown_cmark_to_cmark::Error,
    },
}

impl From<pulldown_cmark_to_cmark::Error> for MarkdownError {
    fn from(source: pulldown_cmark_to_cmark::Error) -> Self {
        Self::Serialize { source }
    }
}

/// Selects the Markdown dialect the parser recognises.
///
/// The variant controls which pulldown-cmark extensions are enabled.  Use
/// [`MarkdownVariant::CommonMark`] for strict CommonMark input and
/// [`MarkdownVariant::Gfm`] to also parse tables, footnotes, strikethrough,
/// and task lists as defined by GitHub Flavored Markdown.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MarkdownVariant {
    /// CommonMark only — no GFM extensions.
    #[default]
    CommonMark,
    /// GitHub Flavored Markdown: tables, footnotes, strikethrough, task lists.
    Gfm,
}

/// Reads Markdown into the core input-token stream.
///
/// Inline HTML tags are represented as scopes so their `lang` and
/// preserved-tag policy affects text until the corresponding inline HTML close
/// tag is read.  Pass [`MarkdownVariant::Gfm`] to enable GFM extensions.
///
/// # Recovery
///
/// The Markdown reader surfaces **no** recoverable reader errors, so it has no
/// fallible (`Result`-yielding) variant and ignores the
/// [`Recovery`](gukhanmun_core::Recovery) policy: `pulldown-cmark` is a total
/// parser that never rejects input, and malformed inline HTML is preserved
/// verbatim rather than reported.  A future extension could lift inline-HTML
/// malformations by routing `Event::Html` / `Event::InlineHtml` through the
/// HTML crate's fallible
/// [`try_read_html_fragment_iter`](https://docs.rs/gukhanmun-html) and
/// re-emitting the resulting [`RecoverableInputError`](gukhanmun_core::RecoverableInputError)s;
/// that is intentionally not done today.
pub fn read_markdown(input: &str, variant: MarkdownVariant) -> Vec<InputToken<MarkdownScopeData>> {
    read_markdown_iter(input, variant).collect()
}

/// Reads Markdown as an iterator over core input tokens.
///
/// `pulldown-cmark` still parses from a complete `&str`, but this API lets the
/// rest of the Gukhanmun pipeline compose token streams without a `Vec`
/// boundary at the adapter edge.
pub fn read_markdown_iter(
    input: &str,
    variant: MarkdownVariant,
) -> std::vec::IntoIter<InputToken<MarkdownScopeData>> {
    Reader::new(input, variant).read().into_iter()
}

/// Writes rendered Markdown tokens back to Markdown text.
///
/// The writer serializes through `pulldown-cmark-to-cmark`, so Markdown syntax
/// is preserved semantically rather than byte-for-byte.
pub fn write_markdown(
    tokens: impl IntoIterator<Item = RenderedToken<MarkdownScopeData>>,
) -> Result<String, MarkdownError> {
    let events = rendered_tokens_to_events(tokens);
    let mut output = String::new();
    pulldown_cmark_to_cmark::cmark(events.iter(), &mut output)?;
    Ok(output)
}

/// Converts Markdown with default engine options.
///
/// `render` accepts either a [`gukhanmun_core::RenderMode`] or a fully
/// constructed [`RenderOptions`] value.
pub fn convert_markdown<D, R>(
    input: &str,
    dictionary: &D,
    render: R,
    variant: MarkdownVariant,
) -> Result<String, MarkdownError>
where
    D: HanjaDictionary + ?Sized,
    R: Into<RenderOptions>,
{
    convert_markdown_with_options(input, dictionary, render, EngineOptions::default(), variant)
}

/// Converts Markdown with explicit engine options.
pub fn convert_markdown_with_options<D, R>(
    input: &str,
    dictionary: &D,
    render: R,
    options: EngineOptions,
    variant: MarkdownVariant,
) -> Result<String, MarkdownError>
where
    D: HanjaDictionary + ?Sized,
    R: Into<RenderOptions>,
{
    let input_tokens = read_markdown(input, variant);
    let output_tokens = process_tokens_iter_with_options(input_tokens, dictionary, options);
    let output_tokens = mark_homophones(output_tokens, dictionary, ContextWindow::PerBlock);
    let rendered_tokens = render_tokens_iter(output_tokens, render);
    write_markdown(rendered_tokens)
}

#[derive(Clone, Debug, PartialEq)]
enum MarkdownNode {
    Container(Tag<'static>),
    Leaf(LeafNode),
    InlineHtmlElement {
        raw_start: String,
        end_tag_name: String,
        omit_end_tag: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum LeafNode {
    Code(String),
    Html(String),
    InlineHtml(String),
    InlineMath(String),
    DisplayMath(String),
    FootnoteReference(String),
    SoftBreak,
    HardBreak,
    Rule,
    TaskListMarker(bool),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HtmlContext {
    tag_name: String,
    tag_preserve: bool,
    text_only_ancestor: bool,
    lang: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum OpenScope {
    Container(Tag<'static>),
    InlineHtml(HtmlContext),
}

#[derive(Clone, Debug)]
struct Reader<'a> {
    input: &'a str,
    variant: MarkdownVariant,
    html_stack: Vec<HtmlContext>,
    open_scopes: Vec<OpenScope>,
    pending_reopen: Vec<Tag<'static>>,
    output: Vec<InputToken<MarkdownScopeData>>,
}

impl<'a> Reader<'a> {
    fn new(input: &'a str, variant: MarkdownVariant) -> Self {
        Self {
            input,
            variant,
            html_stack: Vec::new(),
            open_scopes: Vec::new(),
            pending_reopen: Vec::new(),
            output: Vec::new(),
        }
    }

    fn read(mut self) -> Vec<InputToken<MarkdownScopeData>> {
        for event in Parser::new_ext(self.input, markdown_options(self.variant)) {
            match event {
                Event::Start(tag) => self.push_container(tag.into_static()),
                Event::End(tag) => self.push_container_end(tag),
                Event::Text(text) => self.push_text(&text),
                Event::Code(text) => self.push_leaf(LeafNode::Code(text.to_string())),
                Event::Html(text) => self.push_leaf(LeafNode::Html(text.to_string())),
                Event::InlineHtml(html) => self.push_inline_html(&html),
                Event::InlineMath(text) => self.push_leaf(LeafNode::InlineMath(text.to_string())),
                Event::DisplayMath(text) => {
                    self.push_leaf(LeafNode::DisplayMath(text.to_string()));
                }
                Event::FootnoteReference(text) => {
                    self.push_leaf(LeafNode::FootnoteReference(text.to_string()));
                }
                Event::SoftBreak => self.push_leaf(LeafNode::SoftBreak),
                Event::HardBreak => self.push_leaf(LeafNode::HardBreak),
                Event::Rule => self.push_leaf(LeafNode::Rule),
                Event::TaskListMarker(checked) => self.push_leaf(LeafNode::TaskListMarker(checked)),
            }
        }
        self.output
    }

    fn push_container(&mut self, tag: Tag<'static>) {
        self.flush_pending_reopen();
        self.open_container(tag);
    }

    fn open_container(&mut self, tag: Tag<'static>) {
        let intrinsic_preserve = matches!(tag, Tag::CodeBlock(_) | Tag::HtmlBlock);
        let preserve = intrinsic_preserve || self.active_html_preserve();
        // Markup permission is independent of `preserve`: preserve-only
        // ancestors (code blocks, non-Korean lang scopes) skip text
        // conversion, so no annotation arises that would emit markup, but
        // they do not structurally forbid markup at deeper allow-markup
        // positions. The actual restriction is HTML5's text-only content
        // model, inherited from ancestor inline HTML such as `<title>` or
        // `<option>`.
        let allows_inline_markup = !self.active_html_text_only_ancestor();
        let scope = MarkdownScopeData {
            preserve,
            allows_inline_markup,
            block_boundary: is_markdown_block_boundary(&tag),
            node: MarkdownNode::Container(tag.clone()),
        };
        self.output.push(InputToken::Open(Scope::new(scope)));
        self.open_scopes.push(OpenScope::Container(tag));
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if !self.pending_reopen.is_empty() {
            let leading_whitespace_end = text
                .char_indices()
                .find_map(|(index, character)| (!character.is_whitespace()).then_some(index))
                .unwrap_or(text.len());
            if leading_whitespace_end > 0 {
                self.push_text_immediate(&text[..leading_whitespace_end]);
            }
            if leading_whitespace_end == text.len() {
                return;
            }
            self.flush_pending_reopen();
            self.push_text_immediate(&text[leading_whitespace_end..]);
            return;
        }
        self.push_text_immediate(text);
    }

    fn push_text_immediate(&mut self, text: &str) {
        match self.output.last_mut() {
            Some(InputToken::Text(existing)) => existing.push_str(text),
            _ => self.output.push(InputToken::Text(text.to_owned())),
        }
    }

    fn push_container_end(&mut self, tag: TagEnd) {
        if self.close_pending_container(tag) {
            return;
        }
        if is_markdown_block_end(tag) {
            self.pending_reopen.clear();
            self.close_active_html_scopes();
        }
        self.close_markdown_container(tag);
    }

    fn close_active_html_scopes(&mut self) {
        while let Some(position) = self
            .open_scopes
            .iter()
            .rposition(|scope| matches!(scope, OpenScope::InlineHtml(_)))
        {
            self.close_html_scope_at(position, false);
        }
    }

    fn active_html_preserve(&self) -> bool {
        self.html_stack.last().is_some_and(HtmlContext::preserve)
    }

    /// Returns `true` when any enclosing inline HTML element has a text-only
    /// content model. The lookup walks the whole stack rather than only the
    /// top because Markdown containers (emphasis, list items, …) may sit
    /// between the text-only element and the current cursor.
    fn active_html_text_only_ancestor(&self) -> bool {
        self.html_stack
            .iter()
            .any(|context| context.text_only_ancestor)
    }

    fn push_leaf(&mut self, node: LeafNode) {
        self.flush_pending_reopen();
        let scope = MarkdownScopeData {
            preserve: true,
            allows_inline_markup: false,
            block_boundary: false,
            node: MarkdownNode::Leaf(node),
        };
        self.output.push(InputToken::Open(Scope::new(scope)));
        self.output.push(InputToken::Close);
    }

    fn push_inline_html(&mut self, html: &str) {
        if is_non_element_inline_html(html) {
            self.push_leaf(LeafNode::InlineHtml(html.to_owned()));
            return;
        }
        if html.starts_with("</") {
            self.push_inline_html_end(html);
        } else {
            self.push_inline_html_start(html);
        }
    }

    fn push_inline_html_start(&mut self, html: &str) {
        self.flush_pending_reopen();
        let Some((name_start, name_end)) = parse_start_tag_name(html, 0) else {
            tracing::debug!(
                html,
                "malformed inline HTML start tag: unparseable tag name"
            );
            self.push_leaf(LeafNode::InlineHtml(html.to_owned()));
            return;
        };
        let Some(end_position) = find_tag_end(html, 0) else {
            tracing::debug!(
                html,
                "malformed inline HTML start tag: missing closing bracket"
            );
            self.push_leaf(LeafNode::InlineHtml(html.to_owned()));
            return;
        };

        let tag_original = &html[name_start..name_end];
        let tag_name = tag_original.to_ascii_lowercase();
        let self_closing = is_self_closing_start_tag(html, name_end, end_position);
        let raw_attributes = raw_attributes(html, name_end, end_position, self_closing);
        let context = self.context_for(&tag_name, raw_attributes);
        let omit_end_tag = self_closing || is_void_tag(&tag_name);
        let scope = MarkdownScopeData {
            preserve: context.preserve(),
            // Decoupled from `preserve` for the same reason as in the HTML
            // adapter: preserve disables text conversion, but does not
            // restrict markup at deeper positions where conversion resumes.
            allows_inline_markup: !is_text_only_content_tag(&tag_name)
                && !context.text_only_ancestor,
            block_boundary: false,
            node: MarkdownNode::InlineHtmlElement {
                raw_start: html.to_owned(),
                end_tag_name: tag_original.to_owned(),
                omit_end_tag,
            },
        };

        self.output.push(InputToken::Open(Scope::new(scope)));
        if omit_end_tag {
            self.output.push(InputToken::Close);
        } else {
            self.html_stack.push(context.clone());
            self.open_scopes.push(OpenScope::InlineHtml(context));
        }
    }

    fn push_inline_html_end(&mut self, html: &str) {
        self.flush_pending_reopen();
        let Some((name_start, name_end)) = parse_end_tag_name(html, 0) else {
            tracing::debug!(html, "malformed inline HTML end tag: unparseable tag name");
            self.push_leaf(LeafNode::InlineHtml(html.to_owned()));
            return;
        };
        let tag_name = html[name_start..name_end].to_ascii_lowercase();
        let Some(stack_position) = self.open_scopes.iter().rposition(
            |scope| matches!(scope, OpenScope::InlineHtml(context) if context.tag_name == tag_name),
        ) else {
            tracing::debug!(
                html,
                "unmatched inline HTML close tag: no matching open scope"
            );
            self.push_leaf(LeafNode::InlineHtml(html.to_owned()));
            return;
        };

        self.close_html_scope_at(stack_position, true);
    }

    fn context_for(&self, tag_name: &str, raw_attributes: &str) -> HtmlContext {
        let parent_tag_preserve = self
            .html_stack
            .last()
            .is_some_and(|context| context.tag_preserve);
        let parent_text_only_ancestor = self
            .html_stack
            .last()
            .is_some_and(|context| context.text_only_ancestor);
        let tag_preserve = parent_tag_preserve || is_preserved_tag(tag_name);
        let lang = extract_lang(raw_attributes).or_else(|| {
            self.html_stack
                .last()
                .and_then(|context| context.lang.as_ref().cloned())
        });
        HtmlContext {
            tag_name: tag_name.to_owned(),
            tag_preserve,
            text_only_ancestor: parent_text_only_ancestor || is_text_only_content_tag(tag_name),
            lang,
        }
    }

    fn close_markdown_container(&mut self, tag: TagEnd) {
        let Some(stack_position) = self.open_scopes.iter().rposition(|scope| match scope {
            OpenScope::Container(open_tag) => open_tag.to_end() == tag,
            OpenScope::InlineHtml(_) => false,
        }) else {
            return;
        };

        while self.open_scopes.len() > stack_position {
            match self
                .open_scopes
                .pop()
                .expect("open scope stack is non-empty")
            {
                OpenScope::Container(_) => self.output.push(InputToken::Close),
                OpenScope::InlineHtml(_) => {
                    self.html_stack.pop();
                    self.output.push(InputToken::Close);
                }
            }
        }
    }

    fn close_html_scope_at(&mut self, stack_position: usize, reopen_markdown: bool) {
        let mut reopen = Vec::new();
        while self.open_scopes.len() > stack_position {
            match self
                .open_scopes
                .pop()
                .expect("open scope stack is non-empty")
            {
                OpenScope::Container(tag) => {
                    self.output.push(InputToken::Close);
                    if reopen_markdown {
                        reopen.push(tag);
                    }
                }
                OpenScope::InlineHtml(_) => {
                    self.html_stack.pop();
                    self.output.push(InputToken::Close);
                }
            }
        }

        for tag in reopen.into_iter().rev() {
            self.pending_reopen.push(tag);
        }
    }

    fn flush_pending_reopen(&mut self) {
        if self.pending_reopen.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_reopen);
        for tag in pending {
            self.open_container(tag);
        }
    }

    fn close_pending_container(&mut self, tag: TagEnd) -> bool {
        let Some(position) = self
            .pending_reopen
            .iter()
            .rposition(|open_tag| open_tag.to_end() == tag)
        else {
            return false;
        };
        self.pending_reopen.truncate(position);
        true
    }
}

impl HtmlContext {
    fn preserve(&self) -> bool {
        self.tag_preserve || self.lang.as_ref().is_some_and(|lang| !is_korean_lang(lang))
    }
}

fn rendered_tokens_to_events(
    tokens: impl IntoIterator<Item = RenderedToken<MarkdownScopeData>>,
) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    let mut stack = Vec::new();

    for token in tokens {
        match token {
            RenderedToken::Open(scope) => {
                let data = scope.into_data();
                emit_open(&data, &mut events);
                stack.push(data);
            }
            RenderedToken::Close => {
                if let Some(data) = stack.pop() {
                    emit_close(&data, &mut events);
                }
            }
            RenderedToken::Text(text) => events.push(Event::Text(CowStr::from(text))),
            RenderedToken::Verbatim(text) => events.push(Event::InlineHtml(CowStr::from(text))),
            RenderedToken::Ruby { base, rt } => {
                let mut markup = String::with_capacity(base.len() + rt.len() + 25);
                markup.push_str("<ruby>");
                push_escaped_html_text(&mut markup, &base);
                markup.push_str("<rt>");
                push_escaped_html_text(&mut markup, &rt);
                markup.push_str("</rt></ruby>");
                events.push(Event::InlineHtml(CowStr::from(markup)));
            }
        }
    }

    events
}

/// Appends `input` to `output`, escaping characters that have special meaning
/// in HTML element content. Mirrors the escape rules in the HTML adapter so
/// that inline-HTML emitted from a `RenderedToken::Ruby` token cannot be
/// broken out of by hostile dictionary readings or source text.
fn push_escaped_html_text(output: &mut String, input: &str) {
    for ch in input.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            other => output.push(other),
        }
    }
}

fn emit_open(data: &MarkdownScopeData, events: &mut Vec<Event<'static>>) {
    match &data.node {
        MarkdownNode::Container(tag) => events.push(Event::Start(tag.clone())),
        MarkdownNode::Leaf(node) => events.push(leaf_to_event(node)),
        MarkdownNode::InlineHtmlElement { raw_start, .. } => {
            events.push(Event::InlineHtml(CowStr::from(raw_start.clone())));
        }
    }
}

fn emit_close(data: &MarkdownScopeData, events: &mut Vec<Event<'static>>) {
    match &data.node {
        MarkdownNode::Container(tag) => events.push(Event::End(tag.to_end())),
        MarkdownNode::Leaf(_) => {}
        MarkdownNode::InlineHtmlElement {
            end_tag_name,
            omit_end_tag,
            ..
        } => {
            if !omit_end_tag {
                events.push(Event::InlineHtml(CowStr::from(format!(
                    "</{end_tag_name}>"
                ))));
            }
        }
    }
}

fn leaf_to_event(node: &LeafNode) -> Event<'static> {
    match node {
        LeafNode::Code(text) => Event::Code(CowStr::from(text.clone())),
        LeafNode::Html(text) => Event::Html(CowStr::from(text.clone())),
        LeafNode::InlineHtml(text) => Event::InlineHtml(CowStr::from(text.clone())),
        LeafNode::InlineMath(text) => Event::InlineMath(CowStr::from(text.clone())),
        LeafNode::DisplayMath(text) => Event::DisplayMath(CowStr::from(text.clone())),
        LeafNode::FootnoteReference(text) => Event::FootnoteReference(CowStr::from(text.clone())),
        LeafNode::SoftBreak => Event::SoftBreak,
        LeafNode::HardBreak => Event::HardBreak,
        LeafNode::Rule => Event::Rule,
        LeafNode::TaskListMarker(checked) => Event::TaskListMarker(*checked),
    }
}

fn markdown_options(variant: MarkdownVariant) -> Options {
    match variant {
        MarkdownVariant::CommonMark => Options::empty(),
        // In pulldown-cmark 0.13.x, ENABLE_GFM is a single flag (alerts/callouts)
        // rather than the full combination.  Each GFM extension must be listed
        // individually.
        MarkdownVariant::Gfm => {
            Options::ENABLE_TABLES
                | Options::ENABLE_FOOTNOTES
                | Options::ENABLE_STRIKETHROUGH
                | Options::ENABLE_TASKLISTS
                | Options::ENABLE_GFM
        }
    }
}

fn is_markdown_block_boundary(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::Item
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::Table(_)
            | Tag::TableCell
            | Tag::FootnoteDefinition(_)
    )
}

fn is_markdown_block_end(tag: TagEnd) -> bool {
    !matches!(
        tag,
        TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::Link
            | TagEnd::Image
    )
}

fn is_non_element_inline_html(html: &str) -> bool {
    html.starts_with("<!--")
        || html.starts_with("<!")
        || html.starts_with("<?")
        || html.starts_with("<![CDATA[")
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

/// HTML5 elements whose content model is text-only. Inline markup such as
/// `<ruby>` is invalid inside them, so scopes wrapping these tags report
/// `allows_inline_markup = false` and renderers fall back to parens.
fn is_text_only_content_tag(tag_name: &str) -> bool {
    matches!(tag_name, "title" | "option")
}

fn is_preserved_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "pre" | "code" | "kbd" | "script" | "style" | "textarea"
    )
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
