// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The trash: what was deleted, bringing it back, and deleting it for good.
//!
//! Deleting a task, comment, file, time entry, wiki page or project moves it
//! to the trash together with what hangs below it (subtasks, child pages,
//! sub-projects). One entry of the trash is one such item with everything
//! that went with it, so restoring it brings all of that back. Items stay
//! in the trash for the `trash_days` setting and are then purged; purging
//! also removes their text from the history.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Actor;

/// What kind of item an entry of the trash is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrashKind {
    /// A project, with its sub-projects and everything in them.
    Project,
    /// A task, with its subtasks.
    Task,
    /// A comment.
    Comment,
    /// An attached file.
    File,
    /// A time entry.
    TimeEntry,
    /// A wiki page, with the pages inside it.
    WikiPage,
}

impl TrashKind {
    /// Every kind, in the order the trash lists them when sorted by kind.
    pub const ALL: [Self; 6] = [
        Self::Project,
        Self::Task,
        Self::Comment,
        Self::File,
        Self::TimeEntry,
        Self::WikiPage,
    ];

    /// The name used in inputs and outputs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Task => "task",
            Self::Comment => "comment",
            Self::File => "file",
            Self::TimeEntry => "time_entry",
            Self::WikiPage => "wiki_page",
        }
    }
}

/// One entry of the trash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrashEntry {
    /// The item's ID; pass it to restore or purge the entry.
    pub id: String,
    /// What it is.
    pub kind: TrashKind,
    /// Its title: a task's or page's title, a project's name, a file's
    /// name, the beginning of a comment, a time entry's note.
    pub title: String,
    /// A task's or project's key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// The key of the project it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// For a comment, file or time entry: the task's key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// For a time entry: its minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u32>,
    /// For a file: its size in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// When it was deleted (RFC 3339, UTC).
    pub deleted_at: String,
    /// Who deleted it, when history knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_by: Option<Actor>,
    /// When it will be purged (RFC 3339, UTC).
    pub purge_at: String,
    /// How many other items went to the trash with it: subtasks, pages
    /// inside a page, sub-projects.
    pub contains: u64,
    /// The history group of the deletion.
    pub group: String,
}

/// Which entries of the trash to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrashListInput {
    /// Only entries of this project (key or ID), trashed projects
    /// included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Only these kinds; all when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<TrashKind>,
    /// Entries per page, 1 to 200 (50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// The `next_cursor` of the page before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Entries of the trash, newest deletion first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrashList {
    /// The entries.
    pub items: Vec<TrashEntry>,
    /// How many entries match, on all pages.
    pub total: u64,
    /// The cursor of the next page; left out on the last one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Days an item stays in the trash (the `trash_days` setting).
    pub trash_days: u32,
}

/// Entries of the trash to restore.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrashRestoreInput {
    /// The entries' IDs, 1 to 100.
    pub items: Vec<String>,
}

/// Entries of the trash to delete for good.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrashPurgeInput {
    /// The entries' IDs, up to 100.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<String>,
    /// Empty the whole trash instead (not for agents).
    #[serde(default)]
    pub all: bool,
}

/// What a deletion, restore or purge touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrashChange {
    /// The entries, as the trash lists them (for a purge: as they were).
    pub items: Vec<TrashEntry>,
    /// How many items moved, counting what went with each entry.
    pub total: u64,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// The trash's entries.
    TrashListOp,
    "trash.list",
    TrashListInput,
    TrashList
);
op!(
    /// Entries back out of the trash.
    TrashRestore,
    "trash.restore",
    TrashRestoreInput,
    TrashChange
);
op!(
    /// Entries deleted for good.
    TrashPurge,
    "trash.purge",
    TrashPurgeInput,
    TrashChange
);

pub(crate) const TRASH_LIST: OpDescriptor = OpDescriptor::of::<TrashListOp>(Meta {
    summary: "The trash: deleted items with what went with them, who deleted them and when they will be purged, newest first.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("trash list"),
    input_name: "TrashListInput",
    output_name: "TrashList",
});

pub(crate) const TRASH_RESTORE: OpDescriptor = OpDescriptor::of::<TrashRestore>(Meta {
    summary: "Bring items back out of the trash, with everything that was deleted with them.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("trash restore <items>"),
    input_name: "TrashRestoreInput",
    output_name: "TrashChange",
});

pub(crate) const TRASH_PURGE: OpDescriptor = OpDescriptor::of::<TrashPurge>(Meta {
    summary: "Delete items in the trash for good, and their text from the history. Can't be undone.",
    kind: OpKind::Command,
    safety: Safety::Permanent,
    exposure: Exposure::Data,
    cli: Some("trash purge <items>"),
    input_name: "TrashPurgeInput",
    output_name: "TrashChange",
});
