// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Drafts: the unsaved text of an editor, saved while the person types and
//! offered back after a crash or a restart.
//!
//! There is one draft per person and editor: a task's description, a new
//! comment on a task, an edit of a comment, a project's description, a wiki
//! page, or a new wiki page in a project. A draft belongs to the person who
//! wrote it, not to the workspace's data: it isn't in history, in the
//! activity feed or in exports, and agents and the command line can neither
//! see nor change it. Saving or discarding the text deletes its draft, and
//! a draft not touched for [`DRAFT_EXPIRY_DAYS`] days is forgotten.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};

/// How many days a draft is kept after it was last saved.
pub const DRAFT_EXPIRY_DAYS: i64 = 30;

/// Which editor a draft belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DraftKind {
    /// A task's description; the target is the task.
    Task,
    /// An edit of a comment; the target is the comment.
    Comment,
    /// A new comment on a task; the target is the task.
    NewComment,
    /// A project's description; the target is the project.
    Project,
    /// An edit of a wiki page; the target is the page.
    WikiPage,
    /// A new wiki page; the target is the project it goes in.
    NewWikiPage,
}

impl DraftKind {
    /// The name as stored.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Comment => "comment",
            Self::NewComment => "new_comment",
            Self::Project => "project",
            Self::WikiPage => "wiki_page",
            Self::NewWikiPage => "new_wiki_page",
        }
    }
}

/// The editor a draft belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftRef {
    /// The kind of editor.
    pub kind: DraftKind,
    /// What it edits: a task's key or ID for `task` and `new_comment`, a
    /// comment's ID, a project's key or ID for `project` and
    /// `new_wiki_page`, or a wiki page's ID.
    pub target: String,
}

/// The drafts to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftListInput {
    /// Only the drafts about this task (its key or ID): its description,
    /// a new comment on it and edits of its comments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
}

/// A draft to keep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftSaveInput {
    /// The kind of editor.
    pub kind: DraftKind,
    /// What it edits, as in [`DraftRef::target`].
    pub target: String,
    /// The unsaved text, Markdown.
    pub body: String,
    /// The unsaved title, for a wiki page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The revision the text was edited from, for texts with revisions;
    /// saving the text later names it, so changes made meanwhile are merged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<String>,
}

/// One kept draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    /// The kind of editor.
    pub kind: DraftKind,
    /// The ID of what it edits: the task, comment, project or page.
    pub target: String,
    /// The task's key, for drafts about a task or one of its comments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The unsaved text.
    pub body: String,
    /// The unsaved title, for a wiki page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The revision the text was edited from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<String>,
    /// When it was last saved (RFC 3339, UTC).
    pub updated_at: String,
}

/// A draft, or none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftOutput {
    /// The kept draft; left out when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<Draft>,
}

/// The person's drafts, most recently saved first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftList {
    /// The drafts.
    pub items: Vec<Draft>,
}

/// What a discard did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftDiscarded {
    /// Whether there was a draft to discard.
    pub discarded: bool,
}

op!(
    /// The person's draft of one editor.
    DraftGet,
    "draft.get",
    DraftRef,
    DraftOutput
);
op!(
    /// The person's drafts.
    DraftListOp,
    "draft.list",
    DraftListInput,
    DraftList
);
op!(
    /// Keeps the unsaved text of an editor.
    DraftSave,
    "draft.save",
    DraftSaveInput,
    DraftOutput
);
op!(
    /// Forgets the unsaved text of an editor.
    DraftDiscard,
    "draft.discard",
    DraftRef,
    DraftDiscarded
);

const REASON: &str = "A draft is the person's own unsaved text in an editor of the app, not the workspace's data; agents and scripts save text through the operations that change it.";

pub(crate) const DRAFT_GET: OpDescriptor = OpDescriptor::of::<DraftGet>(Meta {
    summary: "The person's unsaved text of one editor (a description, a comment, a new comment or a wiki page), kept from earlier.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::App { reason: REASON },
    cli: None,
    input_name: "DraftRef",
    output_name: "DraftOutput",
});

pub(crate) const DRAFT_LIST: OpDescriptor = OpDescriptor::of::<DraftListOp>(Meta {
    summary: "The person's kept drafts, most recently saved first; or only those about one task.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::App { reason: REASON },
    cli: None,
    input_name: "DraftListInput",
    output_name: "DraftList",
});

pub(crate) const DRAFT_SAVE: OpDescriptor = OpDescriptor::of::<DraftSave>(Meta {
    summary: "Keep the unsaved text of an editor, replacing the person's earlier draft of it. Drafts are not history and expire after 30 days.",
    kind: OpKind::Command,
    safety: Safety::App,
    exposure: Exposure::App { reason: REASON },
    cli: None,
    input_name: "DraftSaveInput",
    output_name: "DraftOutput",
});

pub(crate) const DRAFT_DISCARD: OpDescriptor = OpDescriptor::of::<DraftDiscard>(Meta {
    summary: "Forget the person's unsaved text of an editor.",
    kind: OpKind::Command,
    safety: Safety::App,
    exposure: Exposure::App { reason: REASON },
    cli: None,
    input_name: "DraftRef",
    output_name: "DraftDiscarded",
});
