// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The links that texts make: wiki pages, task descriptions and comments
//! link to pages with `[[Page]]`, `[[Page#Heading]]` or `[[KEY:Page]]`
//! (a page of project KEY), and to tasks by key (`TM-41`).
//!
//! A link is resolved when it is read, the way the wiki finds a page: by
//! its title (any case), slug, an old slug or its ID; a task by its key,
//! current or old. So a link keeps working after a page is renamed or
//! moved, stops resolving while its target is in the trash and works again
//! once it is restored. Links in texts that are in the trash don't count
//! until they come back.
//!
//! These are not the relations between tasks (blocks, relates…); those are
//! `relation.add` and `relation.remove`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::wiki::WikiPageRef;

/// A text that links somewhere: a wiki page, a task's description or a
/// comment on a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Backlink {
    /// A task's key, for a link in its description or in a comment on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The comment's ID, for a link in a comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// A page, for a link in its text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<WikiPageRef>,
    /// The key of the project the task or page is in.
    pub project: String,
    /// The title of the task or page.
    pub title: String,
    /// The heading the link names, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
}

/// What links to a page or a task: pages first (by title), then task
/// descriptions (by key), then comments (oldest first). Each text is listed
/// once, with the heading of its first link.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BacklinkList {
    /// The texts that link here.
    pub items: Vec<Backlink>,
    /// How many.
    pub total: u64,
}

/// What a link points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    /// A wiki page.
    Page,
    /// A task.
    Task,
}

/// One link in a text, resolved now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// A page or a task.
    pub kind: ReferenceKind,
    /// The page name or task key as written.
    pub target: String,
    /// The heading a page link names, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// Whether the target was found; false for a page that doesn't exist
    /// (yet), a task key that names nothing, or a target in the trash.
    pub found: bool,
    /// The page found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<WikiPageRef>,
    /// The task found: its current key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The key of the project of the page or task found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The title of the page or task found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The comment the link is in, when it is not in the description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// The links a text makes, in the order they first appear: for a task, its
/// description's, then its comments' (oldest first).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReferenceList {
    /// The links.
    pub items: Vec<Reference>,
    /// How many.
    pub total: u64,
}

/// A wiki page whose links to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiLinksInput {
    /// The page: its title (any case), slug or ID.
    pub page: String,
    /// The project (key or ID); needed unless `page` is an ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A task whose links to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskLinksInput {
    /// The task: its key (current or old) or ID.
    pub key: String,
}

op!(
    /// What links to a wiki page.
    WikiBacklinks,
    "wiki.backlinks",
    WikiLinksInput,
    BacklinkList
);
op!(
    /// What a wiki page links to.
    WikiReferences,
    "wiki.references",
    WikiLinksInput,
    ReferenceList
);
op!(
    /// What links to a task.
    TaskBacklinks,
    "task.backlinks",
    TaskLinksInput,
    BacklinkList
);
op!(
    /// What a task's description and comments link to.
    TaskReferences,
    "task.references",
    TaskLinksInput,
    ReferenceList
);

const fn read(
    summary: &'static str,
    cli: &'static str,
    input: &'static str,
    output: &'static str,
) -> Meta {
    Meta {
        summary,
        kind: OpKind::Query,
        safety: Safety::Read,
        exposure: Exposure::Data,
        cli: Some(cli),
        input_name: input,
        output_name: output,
    }
}

pub(crate) const WIKI_BACKLINKS: OpDescriptor = OpDescriptor::of::<WikiBacklinks>(read(
    "The wiki pages, task descriptions and comments that link to a wiki page, by its title, slug, an old slug or its ID.",
    "wiki backlinks <page>",
    "WikiLinksInput",
    "BacklinkList",
));
pub(crate) const WIKI_REFERENCES: OpDescriptor = OpDescriptor::of::<WikiReferences>(read(
    "The pages and tasks a wiki page's text links to, in order, each resolved now; links that find nothing are marked.",
    "wiki references <page>",
    "WikiLinksInput",
    "ReferenceList",
));
pub(crate) const TASK_BACKLINKS: OpDescriptor = OpDescriptor::of::<TaskBacklinks>(read(
    "The wiki pages, other tasks' descriptions and comments that mention a task by its key (current or old).",
    "task backlinks <key>",
    "TaskLinksInput",
    "BacklinkList",
));
pub(crate) const TASK_REFERENCES: OpDescriptor = OpDescriptor::of::<TaskReferences>(read(
    "The pages and tasks a task's description and comments link to, in order, each resolved now; links that find nothing are marked.",
    "task references <key>",
    "TaskLinksInput",
    "ReferenceList",
));
