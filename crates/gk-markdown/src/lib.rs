// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Markdown for the engine and the reader: one parser and renderer, sanitized
//! HTML, checklist numbering and toggling, references to tasks and wiki
//! pages, and plain text for search.
//!
//! Every job parses with the same dialect ([comrak] with CommonMark, GitHub
//! Flavored Markdown, callouts, math and wiki links), so a checkbox has the
//! same number in the engine, the reader's HTML, the command line and MCP.
//!
//! - [`render`] turns a document into HTML for the reader. The HTML is
//!   sanitized here and is safe to insert as is; math, diagrams and code
//!   highlighting are drawn later in the page from the markers it carries.
//! - [`checklist`] lists the checkboxes, and [`toggle`] and [`set_checked`]
//!   tick or clear one by changing exactly one byte of the text.
//! - [`references`] finds the task keys and wiki links a document mentions,
//!   for backlinks.
//! - [`plain_text`] extracts the words for the search index.
//! - [`analyze`] does the last three jobs from a single parse.
//!
//! ```
//! use gk_markdown::{RenderOptions, checklist, render, toggle};
//!
//! let text = "Release TM-41:\n\n- [ ] write notes\n- [x] tag\n";
//! let known = |key: &str| key == "TM-41";
//! let html = render(text, &RenderOptions::new("doc1").with_task_keys(&known));
//! assert!(html.contains(r#"<a data-ref="TM-41""#));
//! assert!(html.contains(r#"<input type="checkbox" data-item="1">"#));
//!
//! let items = checklist(text);
//! assert_eq!(items[0].text, "write notes");
//! let ticked = toggle(text, 1, "write notes").unwrap();
//! assert_eq!(ticked, "Release TM-41:\n\n- [x] write notes\n- [x] tag\n");
//! ```
//!
//! # The reader's HTML
//!
//! The HTML is guarded by two allowlists ([`allowlist`]):
//!
//! - Raw HTML that a user types may only use [`allowlist::USER_TAGS`]. Other
//!   tags are renamed before sanitizing, so they are removed; their text
//!   stays.
//! - The finished HTML may only contain [`allowlist::READER_TAGS`], each with
//!   its own attributes, and links only to the web, to mail addresses, to
//!   footnotes and to attachments (`gk-file://<id>`, nothing more). Images load only from `gk-file:` attachments; a web image
//!   becomes an `<img>` without `src` that carries the address in
//!   `data-external-src`, for the reader to load on request.
//!
//! The attributes the reader acts on (`data-item`, `data-ref`, `data-wiki`,
//! `data-math`, `data-lang`, `data-external-src` and `class`) are written
//! only by the renderer: each render marks its own with a fresh random value,
//! and the sanitizer drops every such attribute without that mark. Raw HTML
//! can't forge a checkbox, a task link or a diagram.
//!
//! [comrak]: https://docs.rs/comrak

mod checklist;
mod headings;
mod keys;
mod plain;
mod raw_html;
mod references;
mod render;
mod sanitize;
mod text;

pub use checklist::{ChecklistItem, ToggleError, checklist, set_checked, toggle};
pub use headings::{Heading, headings};
pub use plain::plain_text;
pub use references::{References, WikiLink, references};
pub use render::{RenderOptions, render};
pub use sanitize::allowlist;

use comrak::{Arena, Options, parse_document};

/// The parser options every job uses. Changing one changes how checkboxes
/// are numbered, so they are set in this one place.
fn options() -> Options<'static> {
    let mut options = Options::default();
    let extension = &mut options.extension;
    extension.strikethrough = true;
    extension.table = true;
    extension.autolink = true;
    extension.tasklist = true;
    extension.footnotes = true;
    // GitHub-style callouts: `> [!NOTE]`.
    extension.alerts = true;
    // `$…$` and `$$…$$`; a ```` ```math ```` block is handled by the renderer.
    extension.math_dollars = true;
    // `[[Page#Heading|label]]`.
    extension.wikilinks_title_after_pipe = true;
    // Raw HTML reaches the renderer, which filters it and then sanitizes the
    // whole result.
    options.render.r#unsafe = true;
    options
}

/// What the engine stores for a document: its checklist, references and
/// search text, from one parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    /// The checkboxes, as [`checklist`] returns them.
    pub checklist: Vec<ChecklistItem>,
    /// The task keys and wiki links, as [`references`] returns them.
    pub references: References,
    /// The text for the search index, as [`plain_text`] returns it.
    pub plain_text: String,
}

/// Parses `source` once and returns its checklist, references and plain
/// text.
///
/// ```
/// let a = gk_markdown::analyze("- [ ] ask TM-3 about [[Release plan]]\n");
/// assert_eq!(a.checklist.len(), 1);
/// assert_eq!(a.references.task_keys, ["TM-3"]);
/// assert_eq!(a.references.wiki_links[0].page, "Release plan");
/// assert_eq!(a.plain_text, "ask TM-3 about Release plan\n");
/// ```
#[must_use]
pub fn analyze(source: &str) -> Analysis {
    let arena = Arena::new();
    let options = options();
    let root = parse_document(&arena, source, &options);
    Analysis {
        checklist: checklist::items(root, source),
        references: references::collect(root),
        plain_text: plain::extract(root),
    }
}
