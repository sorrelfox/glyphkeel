// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Descriptions: their revisions, conflict copies, checklists, and the
//! reader's HTML.
//!
//! Every saved description of a task or project is a revision. A save
//! names the revision it was edited from (`base_revision`); when another
//! save came in between, the two edits are merged line by line, and when
//! they touch the same lines the late edit is kept as a **conflict copy**
//! for the person to resolve, while the stored text stays as it is.
//!
//! Checklist items are named by their number (from 1, in reading order)
//! plus their text, so a stale number never ticks the wrong item.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Actor;
use crate::ops::task::{Checklist, TaskChange};

/// A targeted change to a text: `old_text` must occur exactly once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextEdit {
    /// The text to replace; it must occur exactly once.
    pub old_text: String,
    /// What replaces it.
    pub new_text: String,
}

/// How a revision came about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RevisionKind {
    /// A save.
    Normal,
    /// Two edits of the same revision, merged.
    Merged,
    /// An edit that clashed with another; kept aside, not the description.
    Conflict,
    /// An earlier revision brought back (by Undo or a revert).
    Restored,
    /// The result of resolving a conflict copy.
    Resolved,
}

/// What became of a description that was saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SaveOutcome {
    /// Saved as the new description.
    Saved,
    /// Somebody saved in between; both edits were merged.
    Merged,
    /// Somebody changed the same lines in between: the description stays
    /// as they left it, and this edit was kept as a conflict copy.
    Conflict,
    /// The text was already that.
    Unchanged,
}

/// The result of saving a description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DescriptionSave {
    /// What became of the edit.
    pub outcome: SaveOutcome,
    /// The description's revision now; pass it as `base_revision` next
    /// time. Left out while the description has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// The conflict copy that holds the edit, after a conflict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_copy: Option<String>,
}

/// One revision of a description, without its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevisionInfo {
    /// The revision's ID.
    pub id: String,
    /// How it came about.
    pub kind: RevisionKind,
    /// The revision it was edited from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// Who saved it.
    pub author: Actor,
    /// When (RFC 3339, UTC).
    pub created_at: String,
    /// A note saved with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// For a conflict copy: when it was resolved; left out while it is
    /// open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
    /// Whether it is the description now.
    pub current: bool,
    /// Its size in bytes.
    pub bytes: u64,
}

/// Whose description history to list: a task's or a project's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DescriptionHistoryInput {
    /// The task: its key or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The project: its key or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A description's revisions, newest first, conflict copies included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevisionList {
    /// The revisions.
    pub items: Vec<RevisionInfo>,
    /// How many.
    pub total: u64,
}

/// One revision to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DescriptionRevisionInput {
    /// The revision's ID.
    pub revision: String,
    /// Another revision of the same description: adds the differences
    /// from it to this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_from: Option<String>,
}

/// One revision with its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevisionDetail {
    /// The revision.
    pub revision: RevisionInfo,
    /// The task's or project's key, or the wiki page's title.
    pub owner: String,
    /// The text (Markdown).
    pub body: String,
    /// The differences from `diff_from` as a unified diff; empty when the
    /// texts are the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// What to do with a conflict copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictAction {
    /// Keep the description as it is and close the copy.
    KeepCurrent,
    /// Make the copy the description.
    UseCopy,
    /// Save `text` (for example both combined by hand) as the description.
    UseText,
}

/// How to resolve a conflict copy, inside `task.update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConflictResolution {
    /// The conflict copy's revision ID.
    pub copy: String,
    /// What to do with it.
    pub action: ConflictAction,
    /// The text to save, for `use_text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Resolving a task's conflict copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskResolveInput {
    /// The task: its key or ID.
    pub key: String,
    /// The conflict copy's revision ID.
    pub copy: String,
    /// What to do with it.
    pub action: ConflictAction,
    /// The text to save, for `use_text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Refuse unless the description is still at this revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<String>,
}

/// One checklist item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistItem {
    /// Its number, from 1, in reading order; the reader's checkboxes carry
    /// the same number.
    pub number: u32,
    /// Whether it is ticked.
    pub checked: bool,
    /// Its text, without formatting.
    pub text: String,
    /// How many items it is nested in.
    pub depth: u32,
}

/// Tick or clear one item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistToggle {
    /// The item's number.
    pub number: u32,
    /// The item's text as you read it.
    pub text: String,
    /// Tick (`true`) or clear (`false`); left out, the item flips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
}

/// A new item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistAdd {
    /// Its text, one line.
    pub text: String,
    /// Whether it starts ticked.
    #[serde(default)]
    pub checked: bool,
    /// Put it after this item (number, as read before this change); default:
    /// after the last top-level item, or in a new list at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u32>,
}

/// An item to remove, with its nested items.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistRemove {
    /// The item's number.
    pub number: u32,
    /// The item's text as you read it.
    pub text: String,
}

/// Changes to the checklist in a task's description or a comment. Numbers
/// are as read before the change. When an item's number now points at other
/// text, the item with that text is used if there is exactly one; otherwise
/// nothing changes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistUpdateInput {
    /// The task whose description holds the checklist: its key or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Or the comment that holds it: its ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Items to tick or clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub toggle: Vec<ChecklistToggle>,
    /// Items to add.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<ChecklistAdd>,
    /// Items to remove.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<ChecklistRemove>,
}

/// A checklist after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistChange {
    /// The task's key.
    pub key: String,
    /// The comment's ID, for a comment's checklist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// The items now.
    pub items: Vec<ChecklistItem>,
    /// Ticked and all items.
    pub checklist: Checklist,
    /// The task's (or comment's) version now.
    pub version: i64,
    /// The description's revision now, for a task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// One text to render. Give exactly one of `source`, `task`, `comment`,
/// `project` and `revision`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderSource {
    /// Markdown to render as it is, such as an editor's preview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// A task's description: its key or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// A comment's body: its ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// A project's description: its key or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A description revision: its ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// The id that keeps this text's footnote anchors apart from others on
    /// the page; default: the item's ID (`preview` for `source`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
}

/// Texts to render for the reader.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderInput {
    /// One to 200 texts.
    pub documents: Vec<RenderSource>,
}

/// One rendered text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rendered {
    /// Sanitized HTML, safe to insert as it is.
    pub html: String,
    /// The id its footnote anchors are made from.
    pub document_id: String,
    /// Its checklist; the HTML's checkboxes carry the same numbers.
    pub checklist: Vec<ChecklistItem>,
    /// The description's revision that was rendered, for a task or
    /// project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// The item's version (task or comment) that was rendered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i64>,
}

/// The rendered texts, in the order asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderOutput {
    /// The texts.
    pub items: Vec<Rendered>,
}

op!(
    /// Resolves a conflict copy of a task's description.
    TaskResolveConflict,
    "task.resolve_conflict",
    TaskResolveInput,
    TaskChange
);
op!(
    /// Ticks, clears, adds or removes checklist items.
    ChecklistUpdate,
    "checklist.update",
    ChecklistUpdateInput,
    ChecklistChange
);
op!(
    /// A description's revisions.
    DescriptionHistory,
    "description.history",
    DescriptionHistoryInput,
    RevisionList
);
op!(
    /// One revision with its text.
    DescriptionRevision,
    "description.revision",
    DescriptionRevisionInput,
    RevisionDetail
);
op!(
    /// The reader's HTML for Markdown texts.
    MarkdownRender,
    "markdown.render",
    RenderInput,
    RenderOutput
);

pub(crate) const TASK_RESOLVE_CONFLICT: OpDescriptor = OpDescriptor::of::<TaskResolveConflict>(
    Meta {
        summary: "Resolve a conflict copy of a task's description: keep the description, use the copy, or save a combined text.",
        kind: OpKind::Command,
        safety: Safety::Write,
        exposure: Exposure::Data,
        cli: Some("task resolve <key>"),
        input_name: "TaskResolveInput",
        output_name: "TaskChange",
    },
);

pub(crate) const CHECKLIST_UPDATE: OpDescriptor = OpDescriptor::of::<ChecklistUpdate>(Meta {
    summary: "Tick, clear, add or remove checklist items in a task's description or a comment, by number plus expected text.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("task check <key>"),
    input_name: "ChecklistUpdateInput",
    output_name: "ChecklistChange",
});

pub(crate) const DESCRIPTION_HISTORY: OpDescriptor = OpDescriptor::of::<DescriptionHistory>(Meta {
    summary: "List the revisions of a task's or project's description, newest first, conflict copies included.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("description history"),
    input_name: "DescriptionHistoryInput",
    output_name: "RevisionList",
});

pub(crate) const DESCRIPTION_REVISION: OpDescriptor = OpDescriptor::of::<DescriptionRevision>(
    Meta {
        summary: "One description revision with its text, and the differences from another revision on request.",
        kind: OpKind::Query,
        safety: Safety::Read,
        exposure: Exposure::Data,
        cli: Some("description show <revision>"),
        input_name: "DescriptionRevisionInput",
        output_name: "RevisionDetail",
    },
);

pub(crate) const MARKDOWN_RENDER: OpDescriptor = OpDescriptor::of::<MarkdownRender>(Meta {
    summary: "Render descriptions, comments or editor text to the reader's sanitized HTML, with numbered checkboxes and linked task keys.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::App {
        reason: "It produces HTML for the app's reader; agents and the command line read the Markdown itself.",
    },
    cli: None,
    input_name: "RenderInput",
    output_name: "RenderOutput",
});
