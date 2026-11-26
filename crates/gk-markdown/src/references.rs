// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The task keys and wiki links a document mentions, for backlinks.

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, parse_document};

use crate::keys;

/// What a document refers to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct References {
    /// Task keys as written (`TM-41`), each once, in order of first mention.
    /// Keys in code, link text and wiki links are not counted, the same as
    /// in the reader. Whether a key exists is for the caller to check.
    pub task_keys: Vec<String>,
    /// Wiki links, each target once, in order of first mention.
    pub wiki_links: Vec<WikiLink>,
}

/// A `[[Page]]`, `[[Page#Heading]]` or `[[Page#Heading|label]]` link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    /// The page name, trimmed. Empty for a link to a heading of the same page
    /// (`[[#Heading]]`).
    pub page: String,
    /// The heading after `#`, trimmed, if there is one.
    pub heading: Option<String>,
}

/// Where a wiki link is written in a document, for rewriting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLinkSpan {
    /// The bytes of the whole link, `[[` to `]]`.
    pub range: std::ops::Range<usize>,
    /// The page and heading it names.
    pub link: WikiLink,
    /// The text after `|`, as written (Markdown), if there is one.
    pub label: Option<String>,
}

/// Every wiki link in `source`, in reading order, with where it is written.
/// Links in code are not links, so they aren't listed.
///
/// ```
/// let text = "See [[Release plan#Dates|the dates]] and [[Home]].";
/// let spans = gk_markdown::wiki_link_spans(text);
/// assert_eq!(&text[spans[0].range.clone()], "[[Release plan#Dates|the dates]]");
/// assert_eq!(spans[0].label.as_deref(), Some("the dates"));
/// assert_eq!(spans[1].link.page, "Home");
/// ```
#[must_use]
pub fn wiki_link_spans(source: &str) -> Vec<WikiLinkSpan> {
    let arena = Arena::new();
    let options = crate::options();
    let root = parse_document(&arena, source, &options);
    let lines = crate::checklist::LineIndex::new(source);
    let mut out = Vec::new();
    for node in root.descendants() {
        let NodeValue::WikiLink(link) = &node.data().value else {
            continue;
        };
        let pos = node.data().sourcepos;
        let start = lines.offset(pos.start.line, pos.start.column);
        let end = (lines.offset(pos.end.line, pos.end.column) + 1).min(source.len());
        // Only a span that reads as the link is rewritten.
        let Some(written) = source.get(start..end) else {
            continue;
        };
        if !(written.starts_with("[[") && written.ends_with("]]")) {
            continue;
        }
        let (page, heading) = match link.url.split_once('#') {
            Some((page, heading)) => (page.trim(), Some(heading.trim().to_owned())),
            None => (link.url.trim(), None),
        };
        let label = written[2..written.len() - 2]
            .split_once('|')
            .map(|(_, label)| label.trim().to_owned())
            .filter(|l| !l.is_empty());
        out.push(WikiLinkSpan {
            range: start..end,
            link: WikiLink {
                page: page.to_owned(),
                heading,
            },
            label,
        });
    }
    out
}

/// Where a Markdown link (`[text](url)`) or image (`![alt](url)`) is
/// written, for rewriting it. Only links written inline are listed, not
/// reference links or autolinks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSpan {
    /// The bytes of the whole link, `[` (or `![`) to `)`.
    pub range: std::ops::Range<usize>,
    /// The bytes of the text between the brackets.
    pub text: std::ops::Range<usize>,
    /// The bytes of the destination as written (without `<` and `>`).
    pub destination: std::ops::Range<usize>,
    /// The destination as parsed: escapes and entities resolved.
    pub url: String,
    /// Whether it is an image.
    pub image: bool,
}

/// Every inline link and image in `source`, in reading order, with where it
/// is written. Links in code are not links, so they aren't listed.
///
/// ```
/// let text = "See [the plan](plan.md#dates) and ![chart](img/c.png \"Chart\").";
/// let spans = gk_markdown::link_spans(text);
/// assert_eq!(&text[spans[0].destination.clone()], "plan.md#dates");
/// assert_eq!(&text[spans[0].text.clone()], "the plan");
/// assert!(spans[1].image);
/// assert_eq!(spans[1].url, "img/c.png");
/// assert_eq!(&text[spans[1].range.clone()], "![chart](img/c.png \"Chart\")");
/// ```
#[must_use]
pub fn link_spans(source: &str) -> Vec<LinkSpan> {
    let arena = Arena::new();
    let options = crate::options();
    let root = parse_document(&arena, source, &options);
    let lines = crate::checklist::LineIndex::new(source);
    let mut out = Vec::new();
    for node in root.descendants() {
        let (url, image) = match &node.data().value {
            NodeValue::Link(link) => (link.url.clone(), false),
            NodeValue::Image(link) => (link.url.clone(), true),
            _ => continue,
        };
        let pos = node.data().sourcepos;
        let start = lines.offset(pos.start.line, pos.start.column);
        let end = (lines.offset(pos.end.line, pos.end.column) + 1).min(source.len());
        let Some(written) = source.get(start..end) else {
            continue;
        };
        let opening = if image { "![" } else { "[" };
        if !(written.starts_with(opening) && written.ends_with(')')) {
            continue;
        }
        let Some(close) = written.rfind("](") else {
            continue;
        };
        let inside = &written[close + 2..written.len() - 1];
        let lead = inside.len() - inside.trim_start().len();
        let rest = &inside[lead..];
        let (from, len) = if let Some(quoted) = rest.strip_prefix('<') {
            match quoted.find('>') {
                Some(n) => (lead + 1, n),
                None => continue,
            }
        } else {
            (lead, rest.find(char::is_whitespace).unwrap_or(rest.len()))
        };
        let dest_start = start + close + 2 + from;
        out.push(LinkSpan {
            range: start..end,
            text: start + opening.len()..start + close,
            destination: dest_start..dest_start + len,
            url,
            image,
        });
    }
    out
}

/// Finds the task keys and wiki links in `source`.
///
/// ```
/// let refs = gk_markdown::references(
///     "TM-4 blocks TM-9; see [[Release plan#Dates|the dates]], TM-4 again, `TM-5`.",
/// );
/// assert_eq!(refs.task_keys, ["TM-4", "TM-9"]);
/// assert_eq!(refs.wiki_links[0].page, "Release plan");
/// assert_eq!(refs.wiki_links[0].heading.as_deref(), Some("Dates"));
/// ```
#[must_use]
pub fn references(source: &str) -> References {
    let arena = Arena::new();
    let options = crate::options();
    let root = parse_document(&arena, source, &options);
    collect(root)
}

pub(crate) fn collect<'a>(root: &'a AstNode<'a>) -> References {
    let mut refs = References::default();
    for node in root.descendants() {
        match &node.data().value {
            NodeValue::Text(text) if !inside_link(node) => {
                for range in keys::find(text) {
                    let key = &text[range];
                    if !refs.task_keys.iter().any(|k| k == key) {
                        refs.task_keys.push(key.to_owned());
                    }
                }
            }
            NodeValue::WikiLink(link) => {
                let (page, heading) = match link.url.split_once('#') {
                    Some((page, heading)) => (page.trim(), Some(heading.trim().to_owned())),
                    None => (link.url.trim(), None),
                };
                let wiki = WikiLink {
                    page: page.to_owned(),
                    heading,
                };
                if !refs.wiki_links.contains(&wiki) {
                    refs.wiki_links.push(wiki);
                }
            }
            _ => {}
        }
    }
    refs
}

/// Whether a text node is part of a link, an image or a wiki link, where a
/// task key is shown as written.
pub(crate) fn inside_link<'a>(node: &'a AstNode<'a>) -> bool {
    node.ancestors().skip(1).any(|a| {
        matches!(
            a.data().value,
            NodeValue::Link(_) | NodeValue::Image(_) | NodeValue::WikiLink(_)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::wiki_link_spans;

    #[test]
    fn spans_cover_each_link_as_written_and_skip_code() {
        let text = "# Über\n\n> Ein [[Größe#Maß|die **Maße**]] hier.\n\n`[[Code]]`\n\n- [[Home]]\n";
        let spans = wiki_link_spans(text);
        let written: Vec<&str> = spans.iter().map(|s| &text[s.range.clone()]).collect();
        assert_eq!(written, ["[[Größe#Maß|die **Maße**]]", "[[Home]]"]);
        assert_eq!(spans[0].link.page, "Größe");
        assert_eq!(spans[0].link.heading.as_deref(), Some("Maß"));
        assert_eq!(spans[0].label.as_deref(), Some("die **Maße**"));
        assert_eq!(spans[1].label, None);
    }
}
