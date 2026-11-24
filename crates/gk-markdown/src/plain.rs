// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Plain text for the search index.

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, parse_document};

use crate::text::{collapse_whitespace, decode_entities};

/// The words of `source` without Markdown syntax, for full-text search.
///
/// Each block becomes one or more lines, with whitespace collapsed and empty
/// lines left out. Link and image text is kept, their addresses are not; code,
/// math and the text inside raw HTML are kept, the tags are not.
///
/// ```
/// let text = gk_markdown::plain_text(
///     "# Plan\n\nShip **TM-41** via [the docs](https://example.com).\n\n- [ ] `cargo test`\n",
/// );
/// assert_eq!(text, "Plan\nShip TM-41 via the docs.\ncargo test\n");
/// ```
#[must_use]
pub fn plain_text(source: &str) -> String {
    let arena = Arena::new();
    let options = crate::options();
    let root = parse_document(&arena, source, &options);
    extract(root)
}

pub(crate) fn extract<'a>(root: &'a AstNode<'a>) -> String {
    let mut raw = String::new();
    walk(root, &mut raw);
    let mut out = String::with_capacity(raw.len());
    for line in raw.lines() {
        let line = collapse_whitespace(line);
        if !line.is_empty() {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// One step of the walk: a node to enter, or text to write once a node's
/// children are done.
enum Step<'a> {
    Enter(&'a AstNode<'a>),
    Close(&'static str),
}

/// Writes the text of `root` and its descendants in document order.
///
/// The walk keeps its own stack instead of recursing, so documents nested
/// thousands of levels deep (quotes, lists, emphasis) cannot exhaust the
/// thread's stack.
fn walk<'a>(root: &'a AstNode<'a>, out: &mut String) {
    let mut stack = vec![Step::Enter(root)];
    while let Some(step) = stack.pop() {
        let node = match step {
            Step::Close(text) => {
                out.push_str(text);
                continue;
            }
            Step::Enter(node) => node,
        };
        let close = match &node.data().value {
            NodeValue::Text(text) => {
                out.push_str(text);
                continue;
            }
            NodeValue::Code(code) => {
                out.push_str(&code.literal);
                continue;
            }
            NodeValue::Math(math) => {
                out.push_str(&math.literal);
                continue;
            }
            NodeValue::CodeBlock(block) => {
                out.push('\n');
                out.push_str(&block.literal);
                out.push('\n');
                continue;
            }
            NodeValue::HtmlBlock(block) => {
                out.push('\n');
                out.push_str(&decode_entities(&strip_tags(&block.literal)));
                out.push('\n');
                continue;
            }
            // The text between inline tags is in text nodes of its own.
            NodeValue::HtmlInline(_) => continue,
            NodeValue::SoftBreak | NodeValue::LineBreak => {
                out.push(' ');
                continue;
            }
            NodeValue::Alert(alert) => {
                out.push('\n');
                if let Some(title) = &alert.title {
                    out.push_str(title);
                    out.push('\n');
                }
                "\n"
            }
            NodeValue::TableCell => " ",
            value if value.block() => {
                out.push('\n');
                "\n"
            }
            _ => "",
        };
        if !close.is_empty() {
            stack.push(Step::Close(close));
        }
        // Pushed last to first, so the first child is taken next.
        let mut child = node.last_child();
        while let Some(node) = child {
            stack.push(Step::Enter(node));
            child = node.previous_sibling();
        }
    }
}

/// Replaces every tag, comment and declaration in raw HTML with a space.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let after = &rest[lt + 1..];
        let starts_markup = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'));
        if !starts_markup {
            out.push('<');
            rest = after;
            continue;
        }
        let end = if after.starts_with("!--") {
            after.find("-->").map(|i| i + 3)
        } else {
            after.find('>').map(|i| i + 1)
        };
        out.push(' ');
        match end {
            Some(end) => rest = &after[end..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_keeps_text() {
        assert_eq!(
            strip_tags("<details><summary>More</summary>a < b<!-- x > y --></details>"),
            "  More a < b  "
        );
        assert_eq!(strip_tags("<div unterminated"), " ");
    }
}
