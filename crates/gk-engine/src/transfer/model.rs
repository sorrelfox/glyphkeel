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
    /// The project's versions (releases).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) versions: Vec<Version>,
    /// The project's sprints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) sprints: Vec<Sprint>,
    pub(crate) views: Vec<SavedView>,
    pub(crate) layouts: Vec<Layout>,
    /// Earlier keys of the project, which still find it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) key_aliases: Vec<String>,
    /// The project's own custom fields, and the fields of every project
    /// that its tasks have values of (`shared`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) custom_fields: Vec<CustomField>,
    /// The project's task templates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) templates: Vec<TemplateLine>,
    /// The project's recurring task series.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) series: Vec<SeriesLine>,
    /// The project's automation rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) automations: Vec<AutomationLine>,
    /// The dashboards pinned to the project.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) dashboards: Vec<DashboardLine>,
}

/// A dashboard: one pinned to a project in its `project.json`, the
/// workspace's own in `dashboards.jsonl`. A widget names a project by ID
/// and looks at tasks with a query, as the workspace keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DashboardLine {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) description: String,
    pub(crate) widgets: Vec<serde_json::Value>,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) updated_at: i64,
}

/// An automation rule: a project's in `project.json`, the workspace's in
/// `automations.jsonl`. Its run log and run state are not carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AutomationLine {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) trigger: gk_api::ops::automation::AutomationTrigger,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) condition: String,
    pub(crate) actions: Vec<gk_api::ops::automation::AutomationAction>,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) updated_at: i64,
    pub(crate) updated_by_kind: String,
}

/// A custom field, in `project.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CustomField {
    pub(crate) id: String,
    pub(crate) name: String,
    /// `text`, `markdown`, `integer`, `decimal`, `date`, `boolean`,
    /// `select`, `multi_select` or `url`.
    pub(crate) kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) default: Option<serde_json::Value>,
    /// The task types it shows for, by name; empty: every type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) types: Vec<String>,
    pub(crate) required: bool,
    pub(crate) rank: String,
    /// A field of every project rather than the project's own.
    #[serde(default)]
    pub(crate) shared: bool,
    pub(crate) created_at: i64,
}

/// One stored value of a custom field on a task: text, or a number (a
/// whole number for whole numbers and yes/no).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CustomValue {
    pub(crate) field_id: String,
    pub(crate) value: serde_json::Value,
}

/// A template: a task template in `project.json`, a project template in
/// `templates.jsonl` at the archive root. Its content names statuses,
/// types and labels by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemplateLine {
    pub(crate) id: String,
    /// `task` or `project`.
    pub(crate) kind: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: String,
    pub(crate) content: serde_json::Value,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) updated_at: i64,
}

/// A recurring task series of the project: its rule, the details new
/// occurrences get, and its current occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SeriesLine {
    pub(crate) id: String,
    pub(crate) rule: serde_json::Value,
    pub(crate) pattern: serde_json::Value,
    pub(crate) current_task: Option<String>,
    pub(crate) next_date: Option<String>,
    pub(crate) made: u32,
    pub(crate) ended_at: Option<i64>,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) updated_at: i64,
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
    /// The rules for agents; left out when there are none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent_rules: Option<serde_json::Value>,
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
    /// The fewest tasks the column should hold; left out when no minimum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) wip_min: Option<i64>,
    /// The status's rules; left out when it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) rules: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaskType {
    pub(crate) id: String,
    pub(crate) name: String,
    /// `task`, `bug`, `feature`, `milestone`, `epic` or `story`.
    pub(crate) glyph: String,
    pub(crate) hue: String,
    pub(crate) rank: String,
    pub(crate) is_default: bool,
    /// New tasks of the type are due this many days after they are made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) due_offset_days: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Label {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) hue: String,
    pub(crate) rank: String,
}

/// A version (release) of the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Version {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) description: String,
    /// `open`, `locked` or `closed`.
    pub(crate) status: String,
    pub(crate) start_date: Option<String>,
    pub(crate) due_date: Option<String>,
    /// `none` or `descendants`.
    pub(crate) sharing: String,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

/// A sprint of the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Sprint {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) goal: String,
    /// `planned`, `active` or `closed`.
    pub(crate) state: String,
    pub(crate) start_date: Option<String>,
    pub(crate) end_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) started_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) completed_at: Option<i64>,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
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
    /// The recurring series the task is an occurrence of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) series_id: Option<String>,
    /// The version it is planned for: one of its project's or one an
    /// ancestor shares.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fix_version_id: Option<String>,
    /// The sprint it is in: one of its project's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sprint_id: Option<String>,
    /// The image attached to it that its board card shows as its cover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cover_id: Option<String>,
    /// Its custom field values, one per stored value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) custom_values: Vec<CustomValue>,
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
    /// The kind of work, by name.
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
    /// Whether it is a template; left out when not.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) is_template: bool,
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
