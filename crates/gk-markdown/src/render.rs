// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The reader's HTML.

use std::collections::HashSet;
use std::fmt;

use comrak::nodes::{AstNode, NodeLink, NodeValue};
use comrak::{Arena, parse_document};

use crate::references::inside_link;
use crate::{keys, sanitize};

/// How to render a document.
#[derive(Clone, Copy)]
pub struct RenderOptions<'a> {
    document_id: &'a str,
    task_keys: Option<&'a dyn Fn(&str) -> bool>,
}

impl<'a> RenderOptions<'a> {
    /// Options for the document `document_id`. The id makes the document's
    /// footnote anchors unique when several rendered documents share a page:
    /// pass something unique on that page, such as the task's or comment's
    /// id. Letters, digits and `_` are used as they are; every other
    /// character becomes `_`.
    ///
    /// No task keys are linked until [`with_task_keys`](Self::with_task_keys)
    /// says which exist.
    #[must_use]
    pub fn new(document_id: &'a str) -> Self {
        Self {
            document_id,
            task_keys: None,
        }
    }

    /// Links the task keys in the text for which `exists` returns true. Keys
    /// it returns false for, such as unknown keys or tasks the reader may not
    /// see, stay plain text.
    #[must_use]
    pub fn with_task_keys(mut self, exists: &'a dyn Fn(&str) -> bool) -> Self {
        self.task_keys = Some(exists);
        self
    }
}

impl fmt::Debug for RenderOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderOptions")
            .field("document_id", &self.document_id)
            .field("task_keys", &self.task_keys.is_some())
            .finish()
    }
}

/// Renders `source` to sanitized HTML for the reader.
///
/// What the reader finds in it:
///
/// - checkboxes: `<input type="checkbox" data-item="3">`, numbered as in
///   [`checklist`](crate::checklist), in `<li class="task-item">`;
/// - task keys: `<a data-ref="TM-41">`; a link to a comment
///   (`TM-41#c-1a2b3c4d`) adds `data-comment="1a2b3c4d"`; wiki links:
///   `<a data-wiki="Page#Heading">`;
/// - math: `<span data-math="inline">` or `"display"` with the formula as
///   text, also for a ```` ```math ```` block;
/// - code blocks: `<pre data-lang="rust"><code>`, with the first word of the
///   info string; diagrams are `<pre data-lang="mermaid">`;
/// - callouts: `<div class="markdown-alert markdown-alert-note">` with a
///   `<p class="markdown-alert-title">`;
/// - images: `<img src="gk-file:…">` for attachments; a web image has no
///   `src`, only `data-external-src`; any other image has neither;
/// - footnotes: ids `fn-<document>-<name>` and `fnref-<document>-<name>`.
///
/// ```
/// use gk_markdown::{RenderOptions, render};
///
/// let html = render(
///     "Hi <script>alert(1)</script>[x](javascript:alert(1)) $e^x$",
///     &RenderOptions::new("c1"),
/// );
/// assert_eq!(html, "<p>Hi <a rel=\"noopener noreferrer\">x</a> <span data-math=\"inline\">e^x</span></p>\n");
/// ```
#[must_use]
pub fn render(source: &str, options: &RenderOptions<'_>) -> String {
    let arena = Arena::new();
    let parse_options = crate::options();
    let root = parse_document(&arena, source, &parse_options);
    let key_links = match options.task_keys {
        Some(exists) => link_task_keys(&arena, root, exists),
        None => HashSet::new(),
    };
    let footnote_prefix = footnote_prefix(options.document_id);
    prefix_footnotes(root, &footnote_prefix);
    let mark = new_mark();
    let mut html = String::with_capacity(source.len() * 2);
    let state = State {
        mark: mark.clone(),
        items: 0,
        key_links,
    };
    // Writing to a String can't fail, and the formatter adds no errors of
    // its own.
    if HtmlFormatter::format_document(root, &parse_options, &mut html, state).is_err() {
        html.clear();
    }
    sanitize::clean(&html, &mark, &footnote_prefix)
}

/// Replaces the task keys in text nodes with links, except in code, links,
/// images and wiki links, and returns the new links' addresses
/// ([`node_address`]). The formatter recognises them by address: nothing a
/// user writes can produce one.
fn link_task_keys<'a>(
    arena: &'a Arena<'a>,
    root: &'a AstNode<'a>,
    exists: &dyn Fn(&str) -> bool,
) -> HashSet<usize> {
    let mut links = HashSet::new();
    let texts: Vec<_> = root
        .descendants()
        .filter(|n| matches!(n.data().value, NodeValue::Text(_)) && !inside_link(n))
        .collect();
    for node in texts {
        let text = match &node.data().value {
            NodeValue::Text(text) => text.to_string(),
            _ => continue,
        };
        let mut done = 0;
        for range in keys::find(&text) {
            let key = &text[range.clone()];
            if !exists(key) {
                continue;
            }
            let end = range.end + keys::comment_anchor(&text[range.end..]).unwrap_or(0);
            let range = range.start..end;
            let target = &text[range.clone()];
            if range.start > done {
                let before = NodeValue::Text(text[done..range.start].to_owned().into());
                node.insert_before(arena.alloc(AstNode::from(before)));
            }
            let link = arena.alloc(AstNode::from(NodeValue::Link(Box::new(NodeLink {
                url: target.to_owned(),
                title: String::new(),
            }))));
            link.append(arena.alloc(AstNode::from(NodeValue::Text(target.to_owned().into()))));
            node.insert_before(link);
            links.insert(node_address(link));
            done = range.end;
        }
        if done == 0 {
            continue;
        }
        if done == text.len() {
            node.detach();
        } else {
            node.data_mut().value = NodeValue::Text(text[done..].to_owned().into());
        }
    }
    links
}

/// Identifies a node of the document being rendered.
fn node_address(node: &AstNode<'_>) -> usize {
    std::ptr::from_ref(node).addr()
}

/// The document id as a footnote id prefix: letters, digits and `_` only, so
/// the first `-` after it ends it and two documents can't share an id.
fn footnote_prefix(document_id: &str) -> String {
    let prefix: String = document_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if prefix.is_empty() {
        "_".to_owned()
    } else {
        prefix
    }
}

/// Renames the footnotes to `<prefix>-<name>`, so comrak writes the ids
/// `fn-<prefix>-<name>` and `fnref-<prefix>-<name>`.
fn prefix_footnotes<'a>(root: &'a AstNode<'a>, prefix: &str) {
    for node in root.descendants() {
        match &mut node.data_mut().value {
            NodeValue::FootnoteDefinition(definition) => {
                definition.name = format!("{prefix}-{}", definition.name);
            }
            NodeValue::FootnoteReference(reference) => {
                reference.name = format!("{prefix}-{}", reference.name);
            }
            _ => {}
        }
    }
}

/// A fresh random mark for one render's own attributes. It needs to be
/// unguessable by whoever writes the document, not cryptographically strong:
/// `RandomState` is seeded from the operating system's random source.
fn new_mark() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut halves = [0u64; 2];
    for (i, half) in halves.iter_mut().enumerate() {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(count);
        hasher.write_usize(i);
        *half = hasher.finish();
    }
    format!("gk{:016x}{:016x}", halves[0], halves[1])
}

/// What the formatter carries through a render. (The formatter macro needs
/// `Default`; renders always pass a state with a fresh mark.)
#[derive(Default)]
struct State {
    /// The render's mark, written before the value of each renderer-only
    /// attribute as `<mark>:<value>`.
    mark: String,
    /// Checkboxes written so far.
    items: usize,
    /// The task-key links added to the document, by [`node_address`].
    key_links: HashSet<usize>,
}

/// Appends `"<mark>:<value>"` (escaped, in quotes) to the output.
fn marked(mark: &str, value: &str) -> String {
    let mut out = String::with_capacity(mark.len() + value.len() + 3);
    out.push('"');
    out.push_str(mark);
    out.push(':');
    crate::text::push_attribute(value, &mut out);
    out.push('"');
    out
}

/// The language of a code block: the first word of its info string, in
/// lower case, without control characters.
fn code_language(info: &str) -> String {
    info.split_whitespace()
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .flat_map(char::to_lowercase)
        .collect()
}

mod formatter {
    #![allow(missing_debug_implementations, unreachable_pub)]

    use std::fmt::Write as _;

    use comrak::create_formatter;
    use comrak::html::{ChildRendering, format_node_default};
    use comrak::nodes::NodeValue;
    use comrak::options::AlertStyleType;

    use super::{State, code_language, marked, node_address};
    use crate::raw_html;
    use crate::text::push_inline_text;

    create_formatter!(HtmlFormatter<State>, {
        NodeValue::TaskItem(ref task) => |context, node, entering| {
            let in_list = node
                .parent()
                .is_some_and(|p| matches!(p.data().value, NodeValue::List(_)));
            if entering {
                context.user.items += 1;
                let number = context.user.items.to_string();
                let item_class = marked(&context.user.mark, "task-item");
                let data_item = marked(&context.user.mark, &number);
                context.cr()?;
                if in_list {
                    write!(context, "<li class={item_class}>")?;
                }
                write!(context, "<input type=\"checkbox\" data-item={data_item}")?;
                if task.symbol.is_some() {
                    context.write_str(" checked=\"\"")?;
                }
                context.write_str(" /> ")?;
            } else if in_list {
                context.write_str("</li>\n")?;
            }
        },
        NodeValue::Math(ref math) => |context, entering| {
            if entering {
                let kind = if math.display_math { "display" } else { "inline" };
                let attribute = marked(&context.user.mark, kind);
                write!(context, "<span data-math={attribute}>")?;
                context.escape(&math.literal)?;
                context.write_str("</span>")?;
            }
        },
        NodeValue::WikiLink(ref link) => |context, entering| {
            if entering {
                let attribute = marked(&context.user.mark, &link.url);
                write!(context, "<a data-wiki={attribute}>")?;
            } else {
                context.write_str("</a>")?;
            }
        },
        NodeValue::Link(ref link) => |context, node, entering| {
            if !context.user.key_links.contains(&node_address(node)) {
                return format_node_default(context, node, entering);
            }
            if entering {
                let (key, comment) = match link.url.split_once(crate::keys::COMMENT_ANCHOR) {
                    Some((key, comment)) => (key, Some(comment)),
                    None => (link.url.as_str(), None),
                };
                let attribute = marked(&context.user.mark, key);
                write!(context, "<a data-ref={attribute}")?;
                if let Some(comment) = comment {
                    let attribute = marked(&context.user.mark, comment);
                    write!(context, " data-comment={attribute}")?;
                }
                context.write_str(">")?;
            } else {
                context.write_str("</a>")?;
            }
        },
        NodeValue::Image(ref image) => |context, node, entering| {
            let url = image.url.trim();
            let scheme = url.split_once(':').map(|(s, _)| s.to_ascii_lowercase());
            if scheme.as_deref() == Some(crate::allowlist::IMAGE_SCHEME) {
                return format_node_default(context, node, entering);
            }
            // Any other image gets no `src`: the reader shows a placeholder
            // and loads a web image only when asked to.
            if entering {
                context.write_str("<img")?;
                if matches!(scheme.as_deref(), Some("http" | "https")) {
                    let attribute = marked(&context.user.mark, url);
                    write!(context, " data-external-src={attribute}")?;
                }
                let mut alt = String::new();
                push_inline_text(node, &mut alt);
                context.write_str(" alt=\"")?;
                context.escape(&alt)?;
                context.write_str("\" />")?;
            }
            return Ok(ChildRendering::Skip);
        },
        NodeValue::CodeBlock(ref block) => |context, entering| {
            if entering {
                let language = code_language(&block.info);
                context.cr()?;
                if language == "math" {
                    let attribute = marked(&context.user.mark, "display");
                    write!(context, "<p><span data-math={attribute}>")?;
                    context.escape(&block.literal)?;
                    context.write_str("</span></p>\n")?;
                } else {
                    if language.is_empty() {
                        context.write_str("<pre><code>")?;
                    } else {
                        let attribute = marked(&context.user.mark, &language);
                        write!(context, "<pre data-lang={attribute}><code>")?;
                    }
                    context.escape(&block.literal)?;
                    context.write_str("</code></pre>\n")?;
                }
            }
        },
        NodeValue::Alert(ref alert) => |context, entering| {
            if entering {
                let kind = alert.alert_type.css_class(AlertStyleType::Specific);
                let class = marked(&context.user.mark, &format!("markdown-alert {kind}"));
                let title_class = marked(&context.user.mark, "markdown-alert-title");
                context.cr()?;
                write!(context, "<div class={class}>\n<p class={title_class}>")?;
                match alert.title {
                    Some(ref title) => context.escape(title)?,
                    None => context.write_str(alert.alert_type.default_title())?,
                }
                context.write_str("</p>\n")?;
            } else {
                context.cr()?;
                context.write_str("</div>\n")?;
            }
        },
        NodeValue::HtmlInline(ref raw) => |context, entering| {
            if entering {
                let mut filtered = String::with_capacity(raw.len());
                raw_html::filter(raw, &mut filtered);
                context.write_str(&filtered)?;
            }
        },
        NodeValue::HtmlBlock(ref block) => |context, entering| {
            if entering {
                let mut filtered = String::with_capacity(block.literal.len());
                raw_html::filter(&block.literal, &mut filtered);
                context.cr()?;
                context.write_str(&filtered)?;
                context.cr()?;
            }
        },
    });
}

use formatter::HtmlFormatter;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_are_fresh() {
        let a = new_mark();
        let b = new_mark();
        assert_ne!(a, b);
        assert_eq!(a.len(), 34);
    }

    #[test]
    fn footnote_prefixes_are_plain() {
        assert_eq!(footnote_prefix("task-1.desc"), "task_1_desc");
        assert_eq!(footnote_prefix(""), "_");
        assert_eq!(footnote_prefix("é"), "_");
    }

    #[test]
    fn code_language_keeps_non_ascii() {
        assert_eq!(code_language("Rust ignore"), "rust");
        assert_eq!(code_language("C♯"), "c♯");
        assert_eq!(code_language("日本語 x"), "日本語");
        assert_eq!(code_language(""), "");
    }
}
