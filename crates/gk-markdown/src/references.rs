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
