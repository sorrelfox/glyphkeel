// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Comments on tasks, and their earlier texts.
//!
//! A comment is named by its ID. Its body is Markdown, at most 256 KB.
//! Editing keeps the earlier text as a revision; deleting moves it to the
//! trash.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::{Actor, Via};
use crate::ops::file::Attachment;

/// One comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Comment {
    /// The comment's ID.
    pub id: String,
    /// The task's key.
    pub task: String,
    /// The text (Markdown).
    pub body: String,
    /// The comment it answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// Who wrote it.
    pub author: Actor,
    /// The program it came through.
    pub via: Via,
    /// When it was written (RFC 3339, UTC).
    pub created_at: String,
    /// When it was last edited; left out when never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<String>,
    /// How many earlier texts it has.
    pub revision_count: u32,
    /// Its earlier texts, newest first, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revisions: Option<Vec<CommentRevision>>,
    /// Files attached to it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<Attachment>,
    /// Goes up by one with every edit; pass it as `expected_version`.
    pub version: i64,
}

/// An earlier text of a comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommentRevision {
    /// The text as it was.
    pub body: String,
    /// Who replaced it with a newer text.
    pub replaced_by: Actor,
    /// When it was replaced (RFC 3339, UTC).
    pub replaced_at: String,
}

/// A new comment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommentAddInput {
    /// The task: its key or ID.
    pub key: String,
    /// The text (Markdown), at most 256 KB.
    pub body: String,
    /// The ID of the comment this answers (same task).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// Your own ID for this request; repeating it within 24 hours returns
    /// the first answer instead of adding a second comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_request_id: Option<String>,
}

/// A comment's new text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommentUpdateInput {
    /// The comment's ID.
    pub comment: String,
    /// The new text (Markdown); the earlier one is kept.
    pub body: String,
    /// Refuse the change unless the comment still has this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
}

/// Names one comment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommentRef {
    /// The comment's ID.
    pub comment: String,
}

/// Which comments to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommentListInput {
    /// The task: its key or ID.
    pub key: String,
    /// Add each comment's earlier texts.
    #[serde(default)]
    pub revisions: bool,
}

/// A task's comments, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommentList {
    /// The comments.
    pub items: Vec<Comment>,
    /// How many.
    pub total: u64,
}

/// A comment changed, with the history group of the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommentChange {
    /// The comment after the change.
    pub comment: Comment,
    /// Whether it is in the trash now.
    #[serde(default)]
    pub deleted: bool,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// A task's comments.
    CommentListOp,
    "comment.list",
    CommentListInput,
    CommentList
);
op!(
    /// A new comment.
    CommentAdd,
    "comment.add",
    CommentAddInput,
    CommentChange
);
op!(
    /// A comment's new text.
    CommentUpdate,
    "comment.update",
    CommentUpdateInput,
    CommentChange
);
op!(
    /// A comment to the trash.
    CommentDelete,
    "comment.delete",
    CommentRef,
    CommentChange
);

pub(crate) const COMMENT_LIST: OpDescriptor = OpDescriptor::of::<CommentListOp>(Meta {
    summary: "A task's comments, oldest first, optionally with their earlier texts.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("comment list <key>"),
    input_name: "CommentListInput",
    output_name: "CommentList",
});

pub(crate) const COMMENT_ADD: OpDescriptor = OpDescriptor::of::<CommentAdd>(Meta {
    summary: "Add a Markdown comment to a task, optionally answering another.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("comment add <key>"),
    input_name: "CommentAddInput",
    output_name: "CommentChange",
});

pub(crate) const COMMENT_UPDATE: OpDescriptor = OpDescriptor::of::<CommentUpdate>(Meta {
    summary: "Change a comment's text; the earlier text is kept and can be viewed.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("comment edit <comment>"),
    input_name: "CommentUpdateInput",
    output_name: "CommentChange",
});

pub(crate) const COMMENT_DELETE: OpDescriptor = OpDescriptor::of::<CommentDelete>(Meta {
    summary: "Move a comment to the trash.",
    kind: OpKind::Command,
    safety: Safety::Trash,
    exposure: Exposure::Data,
    cli: Some("comment delete <comment>"),
    input_name: "CommentRef",
    output_name: "CommentChange",
});
