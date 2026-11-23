// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Today: what needs attention now, across projects, in the workspace's
//! time zone.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Actor;
use crate::ops::query::ListedTask;
use crate::ops::time::{DueReminder, Timer};

/// Which part of the workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TodayInput {
    /// Only this project (key or ID), with its sub-projects; default: every
    /// active project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// At most this many tasks per list, 1 to 200; default 50. The counts
    /// are always complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// A task that changed recently, with who changed it last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecentTask {
    /// The task.
    pub task: ListedTask,
    /// Who made its last change.
    pub changed_by: Actor,
    /// When (RFC 3339, UTC).
    pub changed_at: String,
}

/// How many items each list has, also beyond the limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TodayCounts {
    /// Tasks on Today: overdue, due today and in progress, each once. The
    /// sidebar shows it next to Today.
    pub today: u64,
    /// Open tasks overdue.
    pub overdue: u64,
    /// Open tasks due today.
    pub due_today: u64,
    /// Open tasks in an in-progress status, not due today or before.
    pub in_progress: u64,
    /// Personal reminders due today or missed, not acknowledged.
    pub reminders: u64,
    /// Tasks claimed by agents.
    pub claimed: u64,
    /// Tasks changed in the last 24 hours.
    pub recent: u64,
    /// Every open task of the active projects; the sidebar shows it next
    /// to All tasks.
    pub open: u64,
}

/// What needs attention today.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Today {
    /// Today's date in the workspace's time zone (`YYYY-MM-DD`).
    pub date: String,
    /// Open tasks due before today, the oldest first.
    pub overdue: Vec<ListedTask>,
    /// Open tasks due today, by priority.
    pub due_today: Vec<ListedTask>,
    /// Open tasks in an in-progress status that are not in the two lists
    /// above, by project and place on the board.
    pub in_progress: Vec<ListedTask>,
    /// Personal reminders that come due today or were missed and not
    /// acknowledged, the earliest first.
    pub reminders: Vec<DueReminder>,
    /// Tasks agents are working on (claimed), by when the claim ends.
    pub claimed: Vec<ListedTask>,
    /// Tasks changed in the last 24 hours, the newest first.
    pub recent: Vec<RecentTask>,
    /// The timer, while one runs or is paused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<Timer>,
    /// The size of every list.
    pub counts: TodayCounts,
}

op!(
    /// What needs attention today.
    TodayGet,
    "today.get",
    TodayInput,
    Today
);

pub(crate) const TODAY_GET: OpDescriptor = OpDescriptor::of::<TodayGet>(Meta {
    summary: "What needs attention today across projects: overdue tasks, tasks due today, tasks in progress, today's reminders, agents' claims, recent changes and the timer, with counts for the sidebar.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("today"),
    input_name: "TodayInput",
    output_name: "Today",
});
