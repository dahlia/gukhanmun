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

use std::fmt;

use gukhanmun_core::{
    ContextWindow, EngineOptions, HanjaDictionary, InputToken, RenderOptions, RenderedToken, Scope,
    ScopeData, mark_homophones, process_tokens_iter_with_options, render_tokens_iter,
};
use gukhanmun_html::{InlineHtml, classify_inline_html, is_korean_lang};
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
    /// CommonMark only—no GFM extensions.
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
    let mut output = String::new();
    write_markdown_to_fmt(tokens, &mut output)?;
    Ok(output)
}

/// Writes rendered Markdown tokens to a [`fmt::Write`] sink.
///
/// `pulldown-cmark` parses from a complete `&str`, so the Markdown reader is
/// not fully incremental.  This writer still consumes the rendered token stream
/// lazily and avoids collecting an intermediate event vector before
/// serialization.
pub fn write_markdown_to_fmt(
    tokens: impl IntoIterator<Item = RenderedToken<MarkdownScopeData>>,
    output: impl fmt::Write,
) -> Result<(), MarkdownError> {
    pulldown_cmark_to_cmark::cmark(rendered_tokens_to_events(tokens), output)?;
    Ok(())
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
        match classify_inline_html(html) {
            InlineHtml::NonElement => {
                self.push_leaf(LeafNode::InlineHtml(html.to_owned()));
            }
            InlineHtml::EndTag { .. } => self.push_inline_html_end(html),
            InlineHtml::StartTag(_) => self.push_inline_html_start(html),
            InlineHtml::Malformed => {
                tracing::debug!(html, "malformed inline HTML fragment");
                self.push_leaf(LeafNode::InlineHtml(html.to_owned()));
            }
        }
    }

    fn push_inline_html_start(&mut self, html: &str) {
        self.flush_pending_reopen();
        let tag = match classify_inline_html(html) {
            InlineHtml::StartTag(tag) => tag,
            _ => {
                tracing::debug!(html, "malformed inline HTML start tag");
                self.push_leaf(LeafNode::InlineHtml(html.to_owned()));
                return;
            }
        };
        let context = self.context_for(
            &tag.tag_name,
            tag.lang.as_deref(),
            tag.is_preserved_tag,
            tag.is_text_only_content,
        );
        let scope = MarkdownScopeData {
            preserve: context.preserve(),
            // Decoupled from `preserve` for the same reason as in the HTML
            // adapter: preserve disables text conversion, but does not
            // restrict markup at deeper positions where conversion resumes.
            allows_inline_markup: !tag.is_text_only_content && !context.text_only_ancestor,
            block_boundary: false,
            node: MarkdownNode::InlineHtmlElement {
                raw_start: tag.raw_start_tag,
                end_tag_name: tag.end_tag_name,
                omit_end_tag: tag.omit_end_tag,
            },
        };

        self.output.push(InputToken::Open(Scope::new(scope)));
        if tag.omit_end_tag {
            self.output.push(InputToken::Close);
        } else {
            self.html_stack.push(context.clone());
            self.open_scopes.push(OpenScope::InlineHtml(context));
        }
    }

    fn push_inline_html_end(&mut self, html: &str) {
        self.flush_pending_reopen();
        let tag_name = match classify_inline_html(html) {
            InlineHtml::EndTag { tag_name } => tag_name,
            _ => {
                tracing::debug!(html, "malformed inline HTML end tag: unparseable tag name");
                self.push_leaf(LeafNode::InlineHtml(html.to_owned()));
                return;
            }
        };
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

    fn context_for(
        &self,
        tag_name: &str,
        tag_lang: Option<&str>,
        tag_is_preserved: bool,
        tag_is_text_only: bool,
    ) -> HtmlContext {
        let parent_tag_preserve = self
            .html_stack
            .last()
            .is_some_and(|context| context.tag_preserve);
        let parent_text_only_ancestor = self
            .html_stack
            .last()
            .is_some_and(|context| context.text_only_ancestor);
        let tag_preserve = parent_tag_preserve || tag_is_preserved;
        let lang = tag_lang.map(|s| s.to_owned()).or_else(|| {
            self.html_stack
                .last()
                .and_then(|context| context.lang.as_ref().cloned())
        });
        HtmlContext {
            tag_name: tag_name.to_owned(),
            tag_preserve,
            text_only_ancestor: parent_text_only_ancestor || tag_is_text_only,
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
) -> impl Iterator<Item = Event<'static>> {
    RenderedEvents {
        tokens: tokens.into_iter(),
        stack: Vec::new(),
    }
}

struct RenderedEvents<I> {
    tokens: I,
    stack: Vec<MarkdownScopeData>,
}

impl<I> Iterator for RenderedEvents<I>
where
    I: Iterator<Item = RenderedToken<MarkdownScopeData>>,
{
    type Item = Event<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let token = self.tokens.next()?;
            match token {
                RenderedToken::Open(scope) => {
                    let data = scope.into_data();
                    let event = open_event(&data);
                    self.stack.push(data);
                    return Some(event);
                }
                RenderedToken::Close => {
                    if let Some(data) = self.stack.pop()
                        && let Some(event) = close_event(&data)
                    {
                        return Some(event);
                    }
                }
                RenderedToken::Text(text) => return Some(Event::Text(CowStr::from(text))),
                RenderedToken::Verbatim(text) => {
                    return Some(Event::InlineHtml(CowStr::from(text)));
                }
                RenderedToken::Ruby { base, rt } => {
                    let mut markup = String::with_capacity(base.len() + rt.len() + 45);
                    markup.push_str("<ruby>");
                    push_escaped_html_text(&mut markup, &base);
                    markup.push_str("<rp>(</rp><rt>");
                    push_escaped_html_text(&mut markup, &rt);
                    markup.push_str("</rt><rp>)</rp></ruby>");
                    return Some(Event::InlineHtml(CowStr::from(markup)));
                }
            }
        }
    }
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

fn open_event(data: &MarkdownScopeData) -> Event<'static> {
    match &data.node {
        MarkdownNode::Container(tag) => Event::Start(tag.clone()),
        MarkdownNode::Leaf(node) => leaf_to_event(node),
        MarkdownNode::InlineHtmlElement { raw_start, .. } => {
            Event::InlineHtml(CowStr::from(raw_start.clone()))
        }
    }
}

fn close_event(data: &MarkdownScopeData) -> Option<Event<'static>> {
    match &data.node {
        MarkdownNode::Container(tag) => Some(Event::End(tag.to_end())),
        MarkdownNode::Leaf(_) => None,
        MarkdownNode::InlineHtmlElement {
            end_tag_name,
            omit_end_tag,
            ..
        } => {
            if !omit_end_tag {
                Some(Event::InlineHtml(CowStr::from(format!(
                    "</{end_tag_name}>"
                ))))
            } else {
                None
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
