// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The records of an export archive, one type per line kind; format
//! version 1.
//!
//! Field names follow the columns of the workspace database. Derived columns
//! that the engine recomputes from the text (`*_fold`, checklist counts,
//! spent time) are carried but recomputed on import. IDs are UUIDv7 strings,
//! moments are milliseconds since 1970 UTC, days are `YYYY-MM-DD`,
//! durations are minutes, ranks are fractional-index strings.
//!
//! Every type rejects unknown fields, so reading a file through these types
//! is the schema check. The development tool that writes test data writes
//! the same records; fields it doesn't know (aliases, redirects, subtask
//! order, views and layouts) are optional here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub(crate) format: String,
    pub(crate) format_version: u32,
    pub(crate) app_version: String,
    pub(crate) workspace_id: String,
    pub(crate) exported_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) generator: Option<Generator>,
    pub(crate) projects: Vec<ManifestProject>,
    pub(crate) counts: Counts,
    /// SHA-256 of every other file in the archive, by path.
    pub(crate) hashes: BTreeMap<String, String>,
}

/// How a generated archive was made. Archives exported by the app leave it
/// out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Generator {
    pub(crate) tool: String,
    pub(crate) set: String,
    pub(crate) seed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestProject {
    pub(crate) id: String,
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Counts {
    pub(crate) projects: u64,
    pub(crate) tasks: u64,
    pub(crate) open_tasks: u64,
    pub(crate) dated_tasks: u64,
    pub(crate) relations: u64,
    pub(crate) blocks_relations: u64,
    pub(crate) comments: u64,
    pub(crate) time_entries: u64,
    pub(crate) revisions: u64,
    pub(crate) wiki_pages: u64,
    pub(crate) attachments: u64,
    pub(crate) blobs: u64,
    pub(crate) activity: u64,
    pub(crate) agent_sessions: u64,
}

/// `agent_sessions.jsonl` at the archive root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentSession {
    pub(crate) id: String,
    pub(crate) client_id: Option<String>,
    pub(crate) client_version: Option<String>,
    pub(crate) display_name: String,
    pub(crate) profile: String,
    pub(crate) scope_project: Option<String>,
    pub(crate) pid: Option<i64>,
    pub(crate) started_at: i64,
    pub(crate) last_seen_at: i64,
    pub(crate) ended_at: Option<i64>,
    pub(crate) reads: i64,
    pub(crate) writes: i64,
}

/// `projects/<KEY>/project.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectFile {
    pub(crate) project: Project,
    pub(crate) statuses: Vec<Status>,
    pub(crate) types: Vec<TaskType>,
    pub(crate) labels: Vec<Label>,
    pub(crate) views: Vec<SavedView>,
    pub(crate) layouts: Vec<Layout>,
    /// Earlier keys of the project, which still find it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) key_aliases: Vec<String>,
}

/// A saved view of the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SavedView {
    pub(crate) id: String,
    pub(crate) view_kind: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) query: String,
    pub(crate) columns: Option<serde_json::Value>,
    pub(crate) sort: Option<serde_json::Value>,
    pub(crate) group_by: Option<serde_json::Value>,
    pub(crate) totals: Option<serde_json::Value>,
    pub(crate) options: Option<serde_json::Value>,
    pub(crate) is_default: bool,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

/// A personal layout of one of the project's views (board columns, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Layout {
    pub(crate) view_kind: String,
    pub(crate) settings: serde_json::Value,
    pub(crate) updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Project {
    pub(crate) id: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) rank: String,
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) description_rev: Option<String>,
    pub(crate) hue: String,
    pub(crate) icon: Option<String>,
    pub(crate) state: String,
    pub(crate) modules: Modules,
    pub(crate) next_number: i64,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) deleted_at: Option<i64>,
    pub(crate) deleted_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Modules {
    pub(crate) board: u8,
    pub(crate) list: u8,
    pub(crate) gantt: u8,
    pub(crate) wiki: u8,
    pub(crate) time: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Status {
    pub(crate) id: String,
    pub(crate) name: String,
    /// `todo`, `in_progress`, `done` or `cancelled`.
    pub(crate) category: String,
    pub(crate) hue: String,
    pub(crate) rank: String,
    pub(crate) wip_limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaskType {
    pub(crate) id: String,
    pub(crate) name: String,
    /// `task`, `bug`, `feature` or `milestone`.
    pub(crate) glyph: String,
    pub(crate) hue: String,
    pub(crate) rank: String,
    pub(crate) is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Label {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) hue: String,
    pub(crate) rank: String,
}

/// One line of `projects/<KEY>/tasks.jsonl`. The description itself is in
/// `projects/<KEY>/tasks/<KEY>-<number>.md` (absent when empty) and in
/// `revisions.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Task {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) number: i64,
    pub(crate) key: String,
    pub(crate) title: String,
    pub(crate) description_rev: Option<String>,
    pub(crate) status_id: String,
    pub(crate) status_entered_at: i64,
    pub(crate) type_id: String,
    /// 0 none, 1 low, 2 normal, 3 high, 4 urgent.
    pub(crate) priority: u8,
    pub(crate) parent_id: Option<String>,
    pub(crate) rank: String,
    pub(crate) start_date: Option<String>,
    pub(crate) due_date: Option<String>,
    pub(crate) estimate_min: Option<i64>,
    pub(crate) progress_override: Option<u8>,
    pub(crate) checklist_done: i64,
    pub(crate) checklist_total: i64,
    pub(crate) spent_min: i64,
    pub(crate) claim: Option<Claim>,
    pub(crate) labels: Vec<String>,
    /// Relations stored on this task (`from_task` is this task).
    pub(crate) relations: Vec<Relation>,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) edited_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) closed_at: Option<i64>,
    pub(crate) deleted_at: Option<i64>,
    pub(crate) deleted_group: Option<String>,
    pub(crate) version: i64,
    /// The order among its parent's subtasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) subtask_rank: Option<String>,
    /// Earlier keys of the task (after moves), which still find it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) key_aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Claim {
    pub(crate) session_id: String,
    pub(crate) name: String,
    pub(crate) until: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Relation {
    pub(crate) id: String,
    pub(crate) to_task: String,
    /// `blocks`, `relates`, `duplicates` or `clones`.
    pub(crate) kind: String,
    pub(crate) lag_days: Option<i64>,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
}

/// One line of `projects/<KEY>/comments.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Comment {
    pub(crate) id: String,
    pub(crate) task_id: String,
    pub(crate) body: String,
    pub(crate) reply_to: Option<String>,
    pub(crate) actor_kind: String,
    pub(crate) actor_name: String,
    pub(crate) session_id: Option<String>,
    pub(crate) via: String,
    pub(crate) created_at: i64,
    pub(crate) edited_at: Option<i64>,
    pub(crate) deleted_at: Option<i64>,
    pub(crate) deleted_group: Option<String>,
    pub(crate) version: i64,
    /// Earlier texts, oldest first.
    pub(crate) revisions: Vec<CommentRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommentRevision {
    pub(crate) id: String,
    pub(crate) body: String,
    pub(crate) actor_kind: String,
    pub(crate) actor_name: String,
    pub(crate) created_at: i64,
}

/// One line of `projects/<KEY>/time.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TimeEntry {
    pub(crate) id: String,
    pub(crate) task_id: String,
    pub(crate) work_date: String,
    pub(crate) minutes: i64,
    pub(crate) note: String,
    pub(crate) activity: Option<String>,
    pub(crate) actor_kind: String,
    pub(crate) actor_name: String,
    pub(crate) session_id: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) edited_at: Option<i64>,
    pub(crate) deleted_at: Option<i64>,
    pub(crate) deleted_group: Option<String>,
    pub(crate) version: i64,
}

/// One line of `projects/<KEY>/revisions.jsonl`: description history of
/// tasks and the project, and the revisions of wiki pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Revision {
    pub(crate) id: String,
    /// `task`, `project` or `wiki_page`.
    pub(crate) owner_kind: String,
    pub(crate) owner_id: String,
    pub(crate) body: String,
    pub(crate) base_rev: Option<String>,
    /// `normal`, `merged`, `conflict`, `restored` or `resolved`.
    pub(crate) kind: String,
    pub(crate) message: Option<String>,
    pub(crate) actor_kind: String,
    pub(crate) actor_name: String,
    pub(crate) session_id: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) resolved_at: Option<i64>,
}

/// One line of `projects/<KEY>/wiki.jsonl`. The current text is also in
/// `projects/<KEY>/wiki/<path>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WikiPage {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) rank: String,
    pub(crate) title: String,
    pub(crate) slug: String,
    /// Path of the Markdown file below `projects/<KEY>/wiki/`.
    pub(crate) path: String,
    pub(crate) current_rev: String,
    pub(crate) is_agent_brief: bool,
    /// Whether it is the wiki's home page; left out when not.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) is_home: bool,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) deleted_at: Option<i64>,
    pub(crate) deleted_group: Option<String>,
    /// Earlier slugs, which still lead to the page.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) redirects: Vec<String>,
}

/// One line of `projects/<KEY>/files.jsonl`. The bytes are in
/// `files/<sha256>` at the archive root, once per distinct content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Attachment {
    pub(crate) id: String,
    /// `task`, `comment`, `wiki_page` or `project`.
    pub(crate) owner_kind: String,
    pub(crate) owner_id: String,
    pub(crate) file_name: String,
    pub(crate) mime: String,
    pub(crate) size: u64,
    pub(crate) sha256: String,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) actor_kind: String,
    pub(crate) actor_name: String,
    pub(crate) created_at: i64,
    pub(crate) deleted_at: Option<i64>,
    pub(crate) deleted_group: Option<String>,
}

/// One line of `projects/<KEY>/activity.jsonl`. `seq` is strictly increasing
/// across the whole archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Activity {
    pub(crate) seq: i64,
    pub(crate) group_id: String,
    pub(crate) at: i64,
    pub(crate) op: String,
    pub(crate) actor_kind: String,
    pub(crate) actor_name: String,
    pub(crate) via: String,
    pub(crate) session_id: Option<String>,
    pub(crate) project_id: Option<String>,
    pub(crate) entity_kind: String,
    pub(crate) entity_id: String,
    pub(crate) changes: serde_json::Value,
    pub(crate) inverse: Option<serde_json::Value>,
    pub(crate) reverted_by: Option<String>,
}
