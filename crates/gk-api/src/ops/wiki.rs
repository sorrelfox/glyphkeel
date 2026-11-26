// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Wiki pages: a tree of Markdown pages per project.
//!
//! A page is named by its title (any case), its slug or its ID; titles and
//! slugs need the project, an ID doesn't. Titles are unique within a
//! project ignoring case and can't hold `[`, `]`, `#` or `|`, because a
//! wiki link names a page by its title: `[[Release plan#Dates|the
//! dates]]`.
//!
//! A page's text is saved as revisions, the same way as task descriptions:
//! a save names the revision it was edited from (`base_revision`); when
//! another save came in between, the two are merged line by line, and when
//! they touch the same lines the late edit is kept as a conflict copy and
//! the page stays as it is.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Edge;
use crate::ops::description::{ConflictResolution, DescriptionSave, RevisionList, TextEdit};
pub use crate::ops::link::Backlink;
use crate::ops::link::Reference;
use crate::ops::query::TextRange;

/// A page as lists show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiPageSummary {
    /// The page's title.
    pub title: String,
    /// Its name in addresses, made from the title.
    pub slug: String,
    /// Its project's key.
    pub project: String,
    /// The parent page's ID; left out for a page at the top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// How many pages it is nested in.
    pub depth: u32,
    /// How many pages it holds directly.
    pub children: u32,
    /// Whether it is the project's instructions for agents.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub agent_brief: bool,
    /// Whether it is the wiki's home page, the page the wiki opens on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub home: bool,
    /// Whether it is a template, offered when a new page is made.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub template: bool,
    /// When it last changed (RFC 3339, UTC).
    pub updated_at: String,
    /// The page's ID.
    pub id: String,
}

/// A project's pages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiListInput {
    /// The project: its key or ID.
    pub project: String,
}

/// A project's pages as a tree: each page followed by the pages it holds,
/// in their order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiTree {
    /// The pages, in tree order.
    pub items: Vec<WikiPageSummary>,
    /// How many.
    pub total: u64,
}

/// One heading of a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiHeading {
    /// 1 to 6.
    pub level: u8,
    /// Its text; a link to it is `[[Page#text]]`.
    pub text: String,
    /// The line it starts on, from 1.
    pub line: u32,
}

/// Pages to find, for example to complete a `[[` link.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiSearchInput {
    /// Words to find in titles, headings and text. Written `Page#Heading`,
    /// it finds the headings of the pages whose titles match `Page`. Empty
    /// lists the most recently changed pages.
    #[serde(default)]
    pub text: String,
    /// Only this project (key or ID); default: every active project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// At most this many results, 1 to 200; default 20.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Where a page matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WikiMatch {
    /// The title.
    Title,
    /// A heading.
    Heading,
    /// The text.
    Text,
    /// Nothing; the text was empty.
    Recent,
}

/// One page found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiSearchHit {
    /// The page.
    pub page: WikiPageSummary,
    /// Where it matched.
    pub matched_in: WikiMatch,
    /// The matches in the page's title, in UTF-16 code units.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub title_matches: Vec<TextRange>,
    /// The heading that matched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<WikiHeading>,
    /// The matches in the heading's text, in UTF-16 code units.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heading_matches: Vec<TextRange>,
    /// A piece of the text around the matches, for a text match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// The matches in `snippet`, in UTF-16 code units.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snippet_matches: Vec<TextRange>,
}

/// The pages found, best first: title matches (beginnings first), then
/// headings, then text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiSearchResult {
    /// The results.
    pub items: Vec<WikiSearchHit>,
    /// How many results there are, also beyond `limit`.
    pub total: u64,
}

/// What `wiki.get` adds to a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WikiPart {
    /// The pages, task descriptions and comments that link to it.
    Backlinks,
    /// The pages and tasks its text links to.
    References,
    /// Its checklist items, numbered, for `checklist.update`.
    Checklist,
    /// The files attached to it.
    Files,
}

/// A page to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiGetInput {
    /// The page: its title (any case), slug or ID.
    pub page: String,
    /// The project (key or ID); needed unless `page` is an ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// An older revision to read instead of the current text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// What to add: `backlinks`, `references`, `checklist`, `files`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<WikiPart>,
}

/// Another page, as a page names its parent and children.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiPageRef {
    /// Its title.
    pub title: String,
    /// Its slug.
    pub slug: String,
    /// Its ID.
    pub id: String,
}

/// One wiki page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiPage {
    /// The page's title.
    pub title: String,
    /// Its name in addresses, made from the title.
    pub slug: String,
    /// Its project's key.
    pub project: String,
    /// The parent page; left out for a page at the top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<WikiPageRef>,
    /// The pages it is nested in, outermost first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<WikiPageRef>,
    /// The pages it holds, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<WikiPageRef>,
    /// Its text (Markdown).
    pub body: String,
    /// The revision of `body`; pass it as `base_revision` when you save a
    /// new text. Left out while the page has no text yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Whether `body` is the current text (false when an older revision was
    /// asked for).
    pub current: bool,
    /// The headings of `body`, for links to a heading.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headings: Vec<WikiHeading>,
    /// Edits that clashed with another and wait to be resolved (conflict
    /// copies).
    pub conflict_copies: u32,
    /// Whether it is the project's instructions for agents.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub agent_brief: bool,
    /// Whether it is the wiki's home page, the page the wiki opens on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub home: bool,
    /// Whether it is a template, offered when a new page is made.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub template: bool,
    /// What links to it, when asked for with `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backlinks: Option<Vec<Backlink>>,
    /// What its text links to, in order, when asked for with `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<Reference>>,
    /// Its checklist items, numbered, when asked for with `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checklist_items: Option<Vec<crate::ops::description::ChecklistItem>>,
    /// The files attached to it, oldest first, when asked for with
    /// `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<crate::ops::file::Attachment>>,
    /// When it was created (RFC 3339, UTC).
    pub created_at: String,
    /// When it last changed.
    pub updated_at: String,
    /// The page's ID.
    pub id: String,
}

/// A new page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiCreateInput {
    /// The project: its key or ID.
    pub project: String,
    /// The title, 1 to 200 characters, unique in the project ignoring case,
    /// without `[`, `]`, `#` or `|`.
    pub title: String,
    /// The text (Markdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Start from this page's text: a template (or any page) by title,
    /// slug, ID or `KEY:Page` for another project's; not with `body`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// The page to put it in (title, slug or ID); default: at the top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Place it before this page of the same parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Place it after this page of the same parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Place it first or last; default: last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
    /// A note saved with the first revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Your own ID for this request; repeating it within 24 hours returns
    /// the first answer instead of creating a second page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_request_id: Option<String>,
}

/// A page field that can be cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WikiField {
    /// The parent: the page moves to the top.
    Parent,
}

/// Changes to a page; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiUpdateInput {
    /// The page: its title (any case), slug or ID.
    pub page: String,
    /// The project (key or ID); needed unless `page` is an ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A new title. The old slug keeps finding the page, so links by the
    /// old title still resolve until another page takes that name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A new text (Markdown), replacing the old one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// The revision your new text was edited from. If somebody saved in
    /// between, the two edits are merged; when they changed the same lines,
    /// yours is kept as a conflict copy and the page stays as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<String>,
    /// Targeted changes to the text instead of a new one: each `old_text`
    /// must occur exactly once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<TextEdit>,
    /// Resolve a conflict copy of the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolve_conflict: Option<ConflictResolution>,
    /// A note saved with the new revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// A new parent page (title, slug or ID, same project).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Place it before this page of the same parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Place it after this page of the same parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Place it first or last among its parent's pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
    /// Mark it as the project's brief for agents (`true`; another page
    /// marked before is unmarked), or unmark it (`false`). Only people can
    /// change this, not agents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_brief: Option<bool>,
    /// Make it the wiki's home page (`true`; the page that was the home
    /// page before stops being it), or stop it being the home page
    /// (`false`). Without a home page the wiki opens on its first page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<bool>,
    /// Mark it as a template (`true`), which the app offers when a new page
    /// is made, or unmark it (`false`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,
    /// Fields to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<WikiField>,
}

/// A page changed, with the history group of the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiChange {
    /// The page after the change.
    pub page: WikiPage,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// What became of a new text: saved, merged with another edit, or kept
    /// as a conflict copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save: Option<DescriptionSave>,
}

/// A page whose revisions to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiHistoryInput {
    /// The page: its title (any case), slug or ID.
    pub page: String,
    /// The project (key or ID); needed unless `page` is an ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

op!(
    /// A project's pages as a tree.
    WikiList,
    "wiki.list",
    WikiListInput,
    WikiTree
);
op!(
    /// Pages by title, heading or text.
    WikiSearch,
    "wiki.search",
    WikiSearchInput,
    WikiSearchResult
);
op!(
    /// One page with its text and headings.
    WikiGet,
    "wiki.get",
    WikiGetInput,
    WikiPage
);
op!(
    /// A new page.
    WikiCreate,
    "wiki.create",
    WikiCreateInput,
    WikiChange
);
op!(
    /// Changes to a page.
    WikiUpdate,
    "wiki.update",
    WikiUpdateInput,
    WikiChange
);
op!(
    /// A page's revisions.
    WikiHistory,
    "wiki.history",
    WikiHistoryInput,
    RevisionList
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

const fn write(summary: &'static str, cli: &'static str, input: &'static str) -> Meta {
    Meta {
        summary,
        kind: OpKind::Command,
        safety: Safety::Write,
        exposure: Exposure::Data,
        cli: Some(cli),
        input_name: input,
        output_name: "WikiChange",
    }
}

pub(crate) const WIKI_LIST: OpDescriptor = OpDescriptor::of::<WikiList>(read(
    "A project's wiki pages as a tree: titles, slugs, nesting and when each changed.",
    "wiki list",
    "WikiListInput",
    "WikiTree",
));
pub(crate) const WIKI_SEARCH: OpDescriptor = OpDescriptor::of::<WikiSearch>(read(
    "Find wiki pages by title, heading or text, for example to link one; `Page#Heading` lists a page's headings.",
    "wiki search <text>",
    "WikiSearchInput",
    "WikiSearchResult",
));
pub(crate) const WIKI_GET: OpDescriptor = OpDescriptor::of::<WikiGet>(read(
    "One wiki page with its text, revision, headings, place in the tree and, on request, what links to it; or an older revision of it.",
    "wiki get <page>",
    "WikiGetInput",
    "WikiPage",
));
pub(crate) const WIKI_CREATE: OpDescriptor = OpDescriptor::of::<WikiCreate>(write(
    "Create a wiki page with a title unique in its project, optionally inside another page and with its text or a template's.",
    "wiki new",
    "WikiCreateInput",
));
pub(crate) const WIKI_UPDATE: OpDescriptor = OpDescriptor::of::<WikiUpdate>(write(
    "Change a wiki page: its text (whole, with a base revision, or by targeted edits), title, parent or place, whether it is the home page or a template; or resolve a conflict copy.",
    "wiki put <page>",
    "WikiUpdateInput",
));
/// Names one page to move to the trash.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiDeleteInput {
    /// The page: its title (any case), slug or ID.
    pub page: String,
    /// The project (key or ID); needed unless `page` is an ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

op!(
    /// A page to the trash.
    WikiDelete,
    "wiki.delete",
    WikiDeleteInput,
    crate::ops::trash::TrashChange
);

pub(crate) const WIKI_DELETE: OpDescriptor = OpDescriptor::of::<WikiDelete>(Meta {
    summary: "Move a wiki page, with the pages inside it, to the trash; links to it stop resolving until it is restored.",
    kind: OpKind::Command,
    safety: Safety::Trash,
    exposure: Exposure::Data,
    cli: Some("wiki delete <page>"),
    input_name: "WikiDeleteInput",
    output_name: "TrashChange",
});

pub(crate) const WIKI_HISTORY: OpDescriptor = OpDescriptor::of::<WikiHistory>(read(
    "List the revisions of a wiki page, newest first, conflict copies included; read one with wiki.get and its revision.",
    "wiki history <page>",
    "WikiHistoryInput",
    "RevisionList",
));

/// Where to move a page: into another parent, to the top, or to another
/// place among its siblings. Its slug stays, so links to it keep working.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiMoveInput {
    /// The page: its title (any case), slug or ID.
    pub page: String,
    /// The project (key or ID); needed unless `page` is an ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The new parent page (title, slug or ID, same project). Not the page
    /// itself or a page inside it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Move it to the top of the tree, out of its parent.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub top_level: bool,
    /// Place it before this page of the (new) parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Place it after this page of the (new) parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Place it first or last; default when the parent changes: last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
}

op!(
    /// A page to another place in the tree.
    WikiMove,
    "wiki.move",
    WikiMoveInput,
    WikiChange
);

pub(crate) const WIKI_MOVE: OpDescriptor = OpDescriptor::of::<WikiMove>(write(
    "Move a wiki page, with the pages inside it, into another parent page or to the top, and to a place among its siblings; refuses moves into itself; links to it keep working.",
    "wiki move <page>",
    "WikiMoveInput",
));

/// What a wiki export writes for each page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WikiTextFormat {
    /// Markdown files (`.md`), as the pages are written.
    #[default]
    Markdown,
    /// Web pages (`.html`) that open in any browser without the app,
    /// rendered as the wiki shows them, with an `index.html` listing the
    /// pages when there are several.
    Html,
}

/// A page, or a whole wiki, to export as Markdown files or web pages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiExportInput {
    /// The page: its title (any case), slug or ID. Left out: the project's
    /// whole wiki, every page in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// The project (key or ID); needed unless `page` is an ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Include the pages inside it, each in a folder named after its parent
    /// (always, for a whole wiki).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub subpages: bool,
    /// Where to write it: a new file or folder. For agents, a plain name
    /// goes into the workspace's `exports` folder; other paths must be
    /// inside a folder agents may use. Left out: a name made from the
    /// page's slug and the date, in the `exports` folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// A `.zip` file (the default) or a folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<crate::ops::transfer::ArchiveFormat>,
    /// Markdown files (the default) or web pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_format: Option<WikiTextFormat>,
}

/// A finished page export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiExportResult {
    /// The file or folder written.
    pub path: String,
    /// How it is stored.
    pub format: crate::ops::transfer::ArchiveFormat,
    /// The pages written: one Markdown file or web page each.
    pub pages: u64,
    /// The attached files copied.
    pub attachments: u64,
    /// Its size in bytes.
    pub size: u64,
}

op!(
    /// A page, and optionally the pages inside it, as Markdown files.
    WikiExport,
    "wiki.export",
    WikiExportInput,
    WikiExportResult
);

pub(crate) const WIKI_EXPORT: OpDescriptor = OpDescriptor::of::<WikiExport>(Meta {
    summary: "Export a wiki page, optionally with the pages inside it, or a project's whole wiki, as Markdown files or web pages with their attachments, in a .zip file or a folder; links between the exported pages and to the attachments become relative links.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("wiki export <page>"),
    input_name: "WikiExportInput",
    output_name: "WikiExportResult",
});

/// A folder of Markdown files to import as wiki pages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiImportInput {
    /// The project whose wiki the pages go to (key or ID).
    pub project: String,
    /// The folder (or a `.zip` file of one). For agents, a plain name is
    /// one in the workspace's `exports` folder; other paths must be inside
    /// a folder agents may use.
    pub path: String,
    /// The page to put the imported pages in (title, slug or ID); left out:
    /// at the top of the wiki.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Only check the folder and say what would be imported.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
}

/// What a folder import made, or would make.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiImportResult {
    /// Whether this was only a check.
    pub dry_run: bool,
    /// The pages made: one per Markdown file, and one per folder without
    /// its own file.
    pub pages: u64,
    /// The files attached to the pages: the images and other files the
    /// texts link to.
    pub attachments: u64,
    /// The pages made at the top of the import, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top: Vec<WikiPageRef>,
    /// Pages whose title was taken, with the title they got instead.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub renamed: Vec<WikiImportRename>,
    /// Files in the folder that were left out: neither Markdown nor linked
    /// from a page (paths in the folder).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
    /// The history group, for revert and Undo (moving every imported page
    /// to the trash); left out on a dry run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// A page that got another title than its file's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiImportRename {
    /// The file or folder in the import.
    pub path: String,
    /// The page's title.
    pub title: String,
}

op!(
    /// A folder of Markdown files as wiki pages.
    WikiImport,
    "wiki.import",
    WikiImportInput,
    WikiImportResult
);

pub(crate) const WIKI_IMPORT: OpDescriptor = OpDescriptor::of::<WikiImport>(Meta {
    summary: "Import a folder of Markdown files as wiki pages: sub-folders become pages inside pages, relative links between the files and `[[…]]` links become wiki links, and linked images and files are attached to their pages; one change that Undo takes back.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("wiki import <path>"),
    input_name: "WikiImportInput",
    output_name: "WikiImportResult",
});
