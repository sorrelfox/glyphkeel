// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Global search: words and fragments across task titles and descriptions,
//! comments, wiki pages, and file names and the text inside the files
//! (text, PDF and Office documents), ranked, with the matches marked.
//!
//! Ranking: a match in a title comes before one in a task's description or
//! a page's text, which comes before one in a comment, a file name or a
//! file's text; matches only on fragments of words (`CONN` in `ERR_CONN`)
//! come last.
//! Within each group, better matches first, then newer ones. A task key
//! (`TM-41`) puts that task first; a project key alone offers the project.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Category;
use crate::ops::query::TextRange;

/// What a search result is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    /// A project, found by its key.
    Project,
    /// A task: its title or description.
    Task,
    /// A comment on a task.
    Comment,
    /// A wiki page: its title or text.
    Wiki,
    /// A file attached to a task, comment or wiki page: by its name, or by
    /// the text inside it (`matched_in` is then `text`, with a snippet).
    File,
}

/// What to search for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchInput {
    /// Words (each found as a whole word or a word's beginning) and
    /// "quoted phrases"; all must occur. A task key such as `TM-41` also
    /// finds that task.
    pub text: String,
    /// Only these kinds; default: all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<SearchKind>,
    /// Only this project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// With `project`: its sub-projects too.
    #[serde(default)]
    pub include_subprojects: bool,
    /// Only tasks matching this task query (`is:open label:bug`), with
    /// their comments and files; wiki pages are left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Results per page, 1 to 200; default 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// The `next_cursor` of the page before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Where a result matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchedIn {
    /// The key.
    Key,
    /// The title (a file's name).
    Title,
    /// The text: a description, a page's text, a comment or the text
    /// inside a file.
    Text,
    /// Only fragments of words, anywhere.
    Fragment,
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchHit {
    /// What it is.
    pub kind: SearchKind,
    /// Where it matched; results are grouped by it, best first.
    pub matched_in: MatchedIn,
    /// The project's key.
    pub project: String,
    /// The task's key, for tasks and their comments and files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The title as shown: the task's title, the page's title, the file's
    /// name, the project's name; a comment shows its task's title.
    pub title: String,
    /// The matches in `title`, in UTF-16 code units.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub title_matches: Vec<TextRange>,
    /// A piece of the text around the matches; left out when only the
    /// title matched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// The matches in `snippet`, in UTF-16 code units.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snippet_matches: Vec<TextRange>,
    /// The task's status name, for tasks and their comments and files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The task's status category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    /// The wiki page's slug.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// The wiki page's ID, for a file attached to a page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// When it last changed (RFC 3339, UTC).
    pub updated_at: String,
    /// The ID of what was found: the task, comment, page, file or project.
    pub id: String,
}

/// One page of search results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchResult {
    /// The results of this page, best first.
    pub items: Vec<SearchHit>,
    /// How many results there are.
    pub total: u64,
    /// Pass as `cursor` for the next page; left out on the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

op!(
    /// Full-text search across tasks, comments, wiki pages and files.
    SearchQuery,
    "search.query",
    SearchInput,
    SearchResult
);

pub(crate) const SEARCH_QUERY: OpDescriptor = OpDescriptor::of::<SearchQuery>(Meta {
    summary: "Search task titles and descriptions, comments, wiki pages, file names and the text inside attached files for words and fragments, ranked, with the matches marked; a task key finds that task.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("search <text>"),
    input_name: "SearchInput",
    output_name: "SearchResult",
});
