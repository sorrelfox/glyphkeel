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

fn walk<'a>(node: &'a AstNode<'a>, out: &mut String) {
    match &node.data().value {
        NodeValue::Text(text) => out.push_str(text),
        NodeValue::Code(code) => out.push_str(&code.literal),
        NodeValue::Math(math) => out.push_str(&math.literal),
        NodeValue::CodeBlock(block) => {
            out.push('\n');
            out.push_str(&block.literal);
            out.push('\n');
        }
        NodeValue::HtmlBlock(block) => {
            out.push('\n');
            out.push_str(&decode_entities(&strip_tags(&block.literal)));
            out.push('\n');
        }
        // The text between inline tags is in text nodes of its own.
        NodeValue::HtmlInline(_) => {}
        NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
        NodeValue::Alert(alert) => {
            out.push('\n');
            if let Some(title) = &alert.title {
                out.push_str(title);
                out.push('\n');
            }
            walk_children(node, out);
            out.push('\n');
        }
        NodeValue::TableCell => {
            walk_children(node, out);
            out.push(' ');
        }
        value => {
            let block = value.block();
            if block {
                out.push('\n');
            }
            walk_children(node, out);
            if block {
                out.push('\n');
            }
        }
    }
}

fn walk_children<'a>(node: &'a AstNode<'a>, out: &mut String) {
    for child in node.children() {
        walk(child, out);
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
