// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The digest: what changed without the person since they last looked.
//! Agent sessions, automation rules, reminders and due dates, and changes
//! made through the command line, each grouped, from history.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::error::OpError;
use crate::ops::automation::AutomationRun;
use crate::ops::history::ActivityTask;
use crate::ops::time::DueReminder;

/// The default number of items in each of the digest's lists.
pub const DIGEST_DEFAULT_LIMIT: u32 = 20;
/// The most items each of the digest's lists returns.
pub const DIGEST_MAX_LIMIT: u32 = 100;
/// The most history groups the digest names for one rule or for the
/// command line, for reverting them.
pub const DIGEST_MAX_GROUPS: usize = 500;

/// Which changes the digest covers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigestInput {
    /// From when: a moment (RFC 3339) or a day in words (`yesterday`,
    /// `-3d`, `2026-11-20`: from its start, in the caller's time zone).
    /// Left out: for the person, the moment they last dismissed the
    /// digest; for an agent, the end of its previous session of the same
    /// name; without either, the last 24 hours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// Only this project (key or ID), with its sub-projects; default: every
    /// project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// At most this many items in each list, 1 to 100; default 20. The
    /// counts are always complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Where the digest's starting moment came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DigestSince {
    /// The caller's `since`.
    Given,
    /// The moment the person last dismissed the digest.
    Marker,
    /// The end of the agent's previous session of the same name.
    PreviousSession,
    /// The last 24 hours: nothing else was known.
    LastDay,
}

/// One agent session's changes since the digest's start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigestSession {
    /// The session's ID, for reverting it or opening it in the activity
    /// log.
    pub id: String,
    /// The agent's display name.
    pub name: String,
    /// The client's own ID (`claude-code`), when it sent one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    /// Whether the session still runs.
    pub running: bool,
    /// When it started (RFC 3339, UTC).
    pub started_at: String,
    /// When it made its latest change.
    pub last_at: String,
    /// Changed items since the start, counting each item once per command.
    pub changes: u64,
    /// Changes since the start that were reverted since.
    pub reverted: u64,
    /// Commands (history groups) since the start.
    pub groups: u64,
    /// The tasks it touched, the most recently touched first, up to the
    /// limit.
    pub tasks: Vec<ActivityTask>,
    /// How many tasks it touched in all.
    pub task_count: u64,
}

/// One automation rule's runs since the digest's start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigestRule {
    /// The rule's ID.
    pub id: String,
    /// Its name (as of its latest run).
    pub name: String,
    /// Its project, by key; left out for a rule of the whole workspace or a
    /// rule deleted since.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Runs since the start.
    pub runs: u64,
    /// Runs that failed.
    pub failures: u64,
    /// Runs stopped by a limit.
    pub limited: u64,
    /// Items its runs changed.
    pub changes: u64,
    /// When it ran last (RFC 3339, UTC).
    pub last_at: String,
    /// Why its latest failure failed, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<OpError>,
    /// The history groups of its runs that changed something, newest
    /// first (at most 500), for reverting them.
    pub groups: Vec<String>,
    /// The tasks its runs acted on, the latest first, up to the limit.
    pub tasks: Vec<ActivityTask>,
    /// How many tasks its runs acted on.
    pub task_count: u64,
    /// Its latest runs, newest first, up to the limit: the run log.
    pub recent: Vec<AutomationRun>,
}

/// A task whose due date came while the person was away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigestDueTask {
    /// The task.
    pub task: ActivityTask,
    /// Its due date (`YYYY-MM-DD`).
    pub due_date: String,
    /// Whether the due date has passed (else it is today).
    pub overdue: bool,
}

/// Changes the person made through one program since the start.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigestCli {
    /// Changed items.
    pub changes: u64,
    /// Commands (history groups).
    pub group_count: u64,
    /// The history groups, newest first (at most 500), for reverting them.
    pub groups: Vec<String>,
    /// The tasks touched, the most recently touched first, up to the limit.
    pub tasks: Vec<ActivityTask>,
    /// How many tasks were touched in all.
    pub task_count: u64,
    /// When the latest change was made; left out without changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_at: Option<String>,
}

/// How much the digest holds, also beyond the limits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigestCounts {
    /// Agent sessions that changed something.
    pub sessions: u64,
    /// Items agents changed.
    pub agent_changes: u64,
    /// Automation rules that ran.
    pub rules: u64,
    /// Runs of automation rules.
    pub runs: u64,
    /// Runs that failed.
    pub failures: u64,
    /// Reminders that came due and weren't acknowledged.
    pub reminders: u64,
    /// Open tasks whose due day began since the start (due today or
    /// passed since).
    pub became_due: u64,
    /// Items changed through the command line.
    pub cli_changes: u64,
    /// Items the person changed in the app; counted for agents only.
    pub app_changes: u64,
    /// The entries of the digest: each session, rule, reminder and task
    /// that became due, the command line as one and, for agents, the
    /// person's changes in the app as one. The sidebar shows it
    /// next to Inbox.
    pub items: u64,
}

/// What changed without the person since the digest's start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Digest {
    /// The start (RFC 3339, UTC).
    pub since: String,
    /// Where the start came from.
    pub since_from: DigestSince,
    /// The end: the moment the digest was read. Pass it to the dismissal,
    /// so that what changed after it still shows next time.
    pub until: String,
    /// The agent sessions that changed something, the latest first; for an
    /// agent, its own session is left out.
    pub agents: Vec<DigestSession>,
    /// The automation rules that ran, the latest first.
    pub automation: Vec<DigestRule>,
    /// Reminders that came due since the start and weren't acknowledged,
    /// the earliest first.
    pub reminders: Vec<DueReminder>,
    /// Open tasks whose due day began since the start, the earliest due
    /// date first; tasks already due at the start are left out.
    pub became_due: Vec<DigestDueTask>,
    /// Changes the person made through the command line.
    pub cli: DigestCli,
    /// Changes the person made in the app, for an agent catching up; empty
    /// for the person, who saw them.
    pub app: DigestCli,
    /// The size of every list.
    pub counts: DigestCounts,
}

/// When the person saw the digest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigestDismissInput {
    /// The `until` of the digest the person saw (RFC 3339); default: now.
    /// The marker never moves back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

/// The person's marker after dismissing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigestDismissed {
    /// The moment up to which the person has seen the digest (RFC 3339,
    /// UTC); the next digest starts there.
    pub marker: String,
}

op!(
    /// What changed without the person since they last looked.
    DigestGet,
    "digest.get",
    DigestInput,
    Digest
);
op!(
    /// Mark the digest as seen.
    DigestDismiss,
    "digest.dismiss",
    DigestDismissInput,
    DigestDismissed
);

pub(crate) const DIGEST_GET: OpDescriptor = OpDescriptor::of::<DigestGet>(Meta {
    summary: "What changed without the person since a moment (by default since they last looked): agent sessions with their changes and tasks, automation rules with their runs and failures, reminders and tasks that came due, and changes through the command line (for an agent, also the person's changes in the app), with counts.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("digest show"),
    input_name: "DigestInput",
    output_name: "Digest",
});

pub(crate) const DIGEST_DISMISS: OpDescriptor = OpDescriptor::of::<DigestDismiss>(Meta {
    summary: "Mark the digest as seen up to a moment, so that the next one starts there.",
    kind: OpKind::Command,
    safety: Safety::App,
    exposure: Exposure::App {
        reason: "It moves the person's own marker of what they have seen; an agent that moved it would hide its changes from the person.",
    },
    cli: Some("digest dismiss"),
    input_name: "DigestDismissInput",
    output_name: "DigestDismissed",
});
