// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The activity log, revert, and the app's Undo and Redo.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::agent::AgentSession;
use crate::ops::common::{Actor, ActorKind, Via};

/// Which changes to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivityQuery {
    /// Only changes of this agent session; `"current"` is the caller's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Only changes by this actor, by name (case-insensitive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// Only changes by the person (`user`) or only those by agents
    /// (`agent`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_kind: Option<ActorKind>,
    /// Only changes in this project and its sub-projects: a key or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Only changes on or after this day: `YYYY-MM-DD`, or a natural form
    /// such as `today`, `yesterday` or `-7d`, in the caller's time zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Only changes on or before this day (included), in the same forms as
    /// `from`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Only the changes of this history group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Only changes to this task (key or ID): its own fields and
    /// description revisions, its links, its comments and its files,
    /// including files attached to its comments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// At most this many changes (default 50, at most 200).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// `next_cursor` of the previous page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Also list the agent sessions.
    #[serde(default)]
    pub sessions: bool,
}

/// A page of the activity log, newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivityPage {
    /// The changes.
    pub items: Vec<ActivityEntry>,
    /// How many changes match in all.
    pub total: u64,
    /// Pass as `cursor` for the next page; left out on the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// The agent sessions, newest first, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sessions: Option<Vec<AgentSession>>,
}

/// One changed item in the activity log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivityEntry {
    /// The change's sequence number.
    pub seq: i64,
    /// Its history group: everything one command changed.
    pub group: String,
    /// When (RFC 3339, UTC).
    pub at: String,
    /// The operation (`settings.update`).
    pub op: String,
    /// Who.
    pub actor: Actor,
    /// Through which program.
    pub via: Via,
    /// The agent session, for changes through MCP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The project of the changed item, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// What kind of item changed.
    pub entity: String,
    /// The item's ID.
    pub id: String,
    /// The changed fields, each with its value before and after.
    pub fields: BTreeMap<String, FieldChange>,
    /// The task the change belongs to, as it is now: the changed task
    /// itself, or the task of a changed comment, time entry or file. Left
    /// out for other items and for tasks deleted for good.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<ActivityTask>,
    /// The group that reverted this change, if one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverted_by: Option<String>,
}

/// A task as the activity log names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivityTask {
    /// The task's ID.
    pub id: String,
    /// Its key (`TM-41`).
    pub key: String,
    /// Its title.
    pub title: String,
}

/// One field's change. A side is left out when the field had no value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldChange {
    /// The value before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    /// The value after.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
    /// Whether a long text value was cut to its first 500 characters, as
    /// in every list; the item itself holds the whole text.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

/// Long text in lists is cut to this many characters (Unicode scalar
/// values), and the item says `truncated`.
pub const LIST_TEXT_LIMIT: usize = 500;

impl FieldChange {
    /// Cuts each text value longer than [`LIST_TEXT_LIMIT`] characters to
    /// that length and marks the change `truncated`.
    pub fn cut_long_text(&mut self) {
        for side in [&mut self.before, &mut self.after] {
            if let Some(Value::String(text)) = side
                && let Some((at, _)) = text.char_indices().nth(LIST_TEXT_LIMIT)
            {
                text.truncate(at);
                self.truncated = true;
            }
        }
    }
}

/// What to revert: history groups, or everything an agent session changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevertInput {
    /// History groups to revert.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    /// An agent session whose changes to revert.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Only report what would be reverted.
    #[serde(default)]
    pub dry_run: bool,
}

/// What a revert did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevertResult {
    /// The groups reverted (or that would be, in a dry run), newest first.
    pub reverted: Vec<String>,
    /// Groups left alone, with the reason.
    pub skipped: Vec<SkippedGroup>,
    /// The history group of the revert itself; left out in a dry run or when
    /// nothing was reverted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// A group a revert left alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkippedGroup {
    /// The group.
    pub group: String,
    /// Why: something changed it since, it was already reverted, and so on.
    pub reason: String,
    /// The reason's message ID, for showing it in another language.
    pub reason_id: String,
    /// The reason's arguments, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reason_args: BTreeMap<String, String>,
}

/// No input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UndoInput {}

/// What Undo or Redo did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UndoResult {
    /// The group that was undone or redone.
    pub target: String,
    /// The history group of the Undo or Redo itself.
    pub group: String,
    /// The operation that was undone or redone.
    pub op: String,
}

op!(
    /// The activity log.
    HistoryFeed,
    "history.feed",
    ActivityQuery,
    ActivityPage
);
op!(
    /// Revert history groups or an agent session.
    HistoryRevert,
    "history.revert",
    RevertInput,
    RevertResult
);
op!(
    /// Undo the app's last change.
    HistoryUndo,
    "history.undo",
    UndoInput,
    UndoResult
);
op!(
    /// Redo what Undo took back.
    HistoryRedo,
    "history.redo",
    UndoInput,
    UndoResult
);

pub(crate) const HISTORY_FEED: OpDescriptor = OpDescriptor::of::<HistoryFeed>(Meta {
    summary: "The activity log, newest first: who changed what, when and through which program; optionally the agent sessions.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("activity"),
    input_name: "ActivityQuery",
    output_name: "ActivityPage",
});

pub(crate) const HISTORY_REVERT: OpDescriptor = OpDescriptor::of::<HistoryRevert>(Meta {
    summary: "Revert history groups or everything an agent session changed. Groups that something changed since are skipped and listed.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("revert"),
    input_name: "RevertInput",
    output_name: "RevertResult",
});

pub(crate) const HISTORY_UNDO: OpDescriptor = OpDescriptor::of::<HistoryUndo>(Meta {
    summary: "Undo the last change made in this run of the app.",
    kind: OpKind::Command,
    safety: Safety::App,
    exposure: Exposure::App {
        reason: "Undo walks the app's own stack of changes made in this run; agents and the command line revert named groups instead.",
    },
    cli: None,
    input_name: "UndoInput",
    output_name: "UndoResult",
});

pub(crate) const HISTORY_REDO: OpDescriptor = OpDescriptor::of::<HistoryRedo>(Meta {
    summary: "Redo the change the last Undo took back.",
    kind: OpKind::Command,
    safety: Safety::App,
    exposure: Exposure::App {
        reason: "Redo walks the app's own stack of changes made in this run; agents and the command line revert named groups instead.",
    },
    cli: None,
    input_name: "UndoInput",
    output_name: "UndoResult",
});
