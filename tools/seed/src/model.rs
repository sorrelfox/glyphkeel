// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The records of the archive, one type per line kind.
//!
//! Field names follow the columns of the workspace database. Derived columns
//! that the engine recomputes from the text (`*_fold`) are left out. IDs are
//! UUIDv7 strings, moments are milliseconds since 1970 UTC, days are
//! `YYYY-MM-DD`, durations are minutes, ranks are fractional-index strings.
//!
//! Every type rejects unknown fields, so reading a file back through these
//! types is the schema check.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Version of the archive layout and of these records.
pub const FORMAT_VERSION: u32 = 1;
/// Value of [`Manifest::format`].
pub const FORMAT_NAME: &str = "glyphkeel-archive";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub app_version: String,
    pub workspace_id: String,
    pub exported_at: i64,
    pub generator: Generator,
    pub projects: Vec<ManifestProject>,
    pub counts: Counts,
    /// SHA-256 of every other file in the archive, by path.
    pub hashes: BTreeMap<String, String>,
}

/// How a generated archive was made. Archives exported by the app leave it
/// out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generator {
    pub tool: String,
    pub set: String,
    pub seed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestProject {
    pub id: String,
    pub key: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    pub projects: u64,
    pub tasks: u64,
    pub open_tasks: u64,
    pub dated_tasks: u64,
    pub relations: u64,
    pub blocks_relations: u64,
    pub comments: u64,
    pub time_entries: u64,
    pub revisions: u64,
    pub wiki_pages: u64,
    pub attachments: u64,
    pub blobs: u64,
    pub activity: u64,
    pub agent_sessions: u64,
}

/// `agent_sessions.jsonl` at the archive root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSession {
    pub id: String,
    pub client_id: Option<String>,
    pub client_version: Option<String>,
    pub display_name: String,
    pub profile: String,
    pub scope_project: Option<String>,
    pub pid: Option<i64>,
    pub started_at: i64,
    pub last_seen_at: i64,
    pub ended_at: Option<i64>,
    pub reads: i64,
    pub writes: i64,
}

/// `projects/<KEY>/project.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFile {
    pub project: Project,
    pub statuses: Vec<Status>,
    pub types: Vec<TaskType>,
    pub labels: Vec<Label>,
    pub views: Vec<serde_json::Value>,
    pub layouts: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub parent_id: Option<String>,
    pub rank: String,
    pub key: String,
    pub name: String,
    pub description: String,
    pub description_rev: Option<String>,
    pub hue: String,
    pub icon: Option<String>,
    pub state: String,
    pub modules: Modules,
    pub next_number: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub deleted_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modules {
    pub board: u8,
    pub list: u8,
    pub gantt: u8,
    pub wiki: u8,
    pub time: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub id: String,
    pub name: String,
    /// `todo`, `in_progress`, `done` or `cancelled`.
    pub category: String,
    pub hue: String,
    pub rank: String,
    pub wip_limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskType {
    pub id: String,
    pub name: String,
    /// `task`, `bug`, `feature` or `milestone`.
    pub glyph: String,
    pub hue: String,
    pub rank: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    pub id: String,
    pub name: String,
    pub hue: String,
    pub rank: String,
}

/// One line of `projects/<KEY>/tasks.jsonl`. The description itself is in
/// `projects/<KEY>/tasks/<KEY>-<number>.md` (absent when empty) and in
/// `revisions.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub number: i64,
    pub key: String,
    pub title: String,
    pub description_rev: Option<String>,
    pub status_id: String,
    pub status_entered_at: i64,
    pub type_id: String,
    /// 0 none, 1 low, 2 normal, 3 high, 4 urgent.
    pub priority: u8,
    pub parent_id: Option<String>,
    pub rank: String,
    pub start_date: Option<String>,
    pub due_date: Option<String>,
    pub estimate_min: Option<i64>,
    pub progress_override: Option<u8>,
    pub checklist_done: i64,
    pub checklist_total: i64,
    pub spent_min: i64,
    pub claim: Option<Claim>,
    pub labels: Vec<String>,
    /// Relations stored on this task (`from_task` is this task).
    pub relations: Vec<Relation>,
    pub created_at: i64,
    pub created_by_kind: String,
    pub created_by_name: String,
    pub edited_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
    pub deleted_at: Option<i64>,
    pub deleted_group: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub session_id: String,
    pub name: String,
    pub until: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    pub id: String,
    pub to_task: String,
    /// `blocks`, `relates`, `duplicates` or `clones`.
    pub kind: String,
    pub lag_days: Option<i64>,
    pub created_at: i64,
    pub created_by_kind: String,
    pub created_by_name: String,
}

/// One line of `projects/<KEY>/comments.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Comment {
    pub id: String,
    pub task_id: String,
    pub body: String,
    pub reply_to: Option<String>,
    pub actor_kind: String,
    pub actor_name: String,
    pub session_id: Option<String>,
    pub via: String,
    pub created_at: i64,
    pub edited_at: Option<i64>,
    pub deleted_at: Option<i64>,
    pub deleted_group: Option<String>,
    pub version: i64,
    /// Earlier texts, oldest first.
    pub revisions: Vec<CommentRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommentRevision {
    pub id: String,
    pub body: String,
    pub actor_kind: String,
    pub actor_name: String,
    pub created_at: i64,
}

/// One line of `projects/<KEY>/time.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeEntry {
    pub id: String,
    pub task_id: String,
    pub work_date: String,
    pub minutes: i64,
    pub note: String,
    pub activity: Option<String>,
    pub actor_kind: String,
    pub actor_name: String,
    pub session_id: Option<String>,
    pub created_at: i64,
    pub edited_at: Option<i64>,
    pub deleted_at: Option<i64>,
    pub deleted_group: Option<String>,
    pub version: i64,
}

/// One line of `projects/<KEY>/revisions.jsonl`: description history of
/// tasks and the project, and the revisions of wiki pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub id: String,
    /// `task`, `project` or `wiki_page`.
    pub owner_kind: String,
    pub owner_id: String,
    pub body: String,
    pub base_rev: Option<String>,
    /// `normal`, `merged`, `conflict`, `restored` or `resolved`.
    pub kind: String,
    pub message: Option<String>,
    pub actor_kind: String,
    pub actor_name: String,
    pub session_id: Option<String>,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
}

/// One line of `projects/<KEY>/wiki.jsonl`. The current text is also in
/// `projects/<KEY>/wiki/<path>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WikiPage {
    pub id: String,
    pub project_id: String,
    pub parent_id: Option<String>,
    pub rank: String,
    pub title: String,
    pub slug: String,
    /// Path of the Markdown file below `projects/<KEY>/wiki/`.
    pub path: String,
    pub current_rev: String,
    pub is_agent_brief: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub deleted_group: Option<String>,
}

/// One line of `projects/<KEY>/files.jsonl`. The bytes are in
/// `files/<sha256>` at the archive root, once per distinct content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub id: String,
    /// `task`, `comment`, `wiki_page` or `project`.
    pub owner_kind: String,
    pub owner_id: String,
    pub file_name: String,
    pub mime: String,
    pub size: u64,
    pub sha256: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub actor_kind: String,
    pub actor_name: String,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    pub deleted_group: Option<String>,
}

/// One line of `projects/<KEY>/activity.jsonl`. `seq` is strictly increasing
/// across the whole archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub seq: i64,
    pub group_id: String,
    pub at: i64,
    pub op: String,
    pub actor_kind: String,
    pub actor_name: String,
    pub via: String,
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub entity_kind: String,
    pub entity_id: String,
    pub changes: serde_json::Value,
    pub inverse: Option<serde_json::Value>,
    pub reverted_by: Option<String>,
}

/// `fixtures.json` at the root of the hostile-content archive: what each
/// task in it is meant to exercise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub key: String,
    pub name: String,
    pub targets: Vec<String>,
    pub purpose: String,
    pub expect: String,
}
