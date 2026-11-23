// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The MCP tools: written by hand, in the order agents see them, each
//! covering one or more catalog operations.
//!
//! A tool takes its input and output schemas from its main operation. It is
//! listed for an agent when the agent's profile allows its main operation's
//! safety class and its toolset is switched on. A tool that covers several
//! operations runs the parts an input asks for as one change, and the engine
//! checks each part against its own class: `update_project` is listed for
//! the standard profile, but removing a status through it needs the full
//! one.

use serde_json::{Value, json};

use crate::catalog::{CATALOG, OpDescriptor, Safety, descriptor};
use crate::profile::Profile;
use crate::schema;

/// A group of tools an agent can switch on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Toolset {
    /// Overview, global search, activity, revert.
    Workspace,
    /// Projects, sub-projects and their configuration.
    Projects,
    /// Tasks, checklists, links, claims.
    Tasks,
    /// Comments.
    Comments,
    /// Time entries, report, timer, reminders.
    Time,
    /// Today, the board, saved views, personal layout.
    Views,
    /// Wiki pages and the agent brief.
    Wiki,
    /// Schedule data.
    Schedule,
    /// Attachments.
    Files,
    /// Trash, backups, export and import, workspace settings. Off by
    /// default.
    Admin,
}

impl Toolset {
    /// Every toolset, in the order tools are listed.
    pub const ALL: [Self; 10] = [
        Self::Workspace,
        Self::Projects,
        Self::Tasks,
        Self::Comments,
        Self::Time,
        Self::Views,
        Self::Wiki,
        Self::Schedule,
        Self::Files,
        Self::Admin,
    ];

    /// The name used by `--toolsets`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Projects => "projects",
            Self::Tasks => "tasks",
            Self::Comments => "comments",
            Self::Time => "time",
            Self::Views => "views",
            Self::Wiki => "wiki",
            Self::Schedule => "schedule",
            Self::Files => "files",
            Self::Admin => "admin",
        }
    }

    /// Whether the toolset is on unless `--toolsets` says otherwise.
    #[must_use]
    pub const fn on_by_default(self) -> bool {
        !matches!(self, Self::Admin)
    }

    /// The toolsets that are on by default.
    #[must_use]
    pub fn defaults() -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|t| t.on_by_default())
            .collect()
    }

    /// The toolset named `name`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == name)
    }
}

/// One MCP tool.
#[derive(Debug, Clone, Copy)]
pub struct McpTool {
    /// The tool's name, `snake_case`.
    pub name: &'static str,
    /// Its toolset.
    pub toolset: Toolset,
    /// A short title for people.
    pub title: &'static str,
    /// What it does, when to use it and when not, and one example. Written
    /// for agents.
    pub description: &'static str,
    /// The operation whose input and output the tool takes.
    pub main: &'static str,
    /// Every operation the tool covers, the main one first.
    pub covers: &'static [&'static str],
    /// Whether calling it twice with the same input has the same effect as
    /// calling it once.
    pub idempotent: bool,
}

impl McpTool {
    /// The main operation's descriptor.
    ///
    /// # Panics
    /// If the main operation is not in the catalog; the catalog's tests
    /// rule that out.
    #[must_use]
    pub fn main_op(&self) -> &'static OpDescriptor {
        descriptor(self.main)
            .unwrap_or_else(|| panic!("{} covers unknown {}", self.name, self.main))
    }

    /// The safety class that decides whether the tool is listed: its main
    /// operation's.
    #[must_use]
    pub fn safety(&self) -> Safety {
        self.main_op().safety
    }

    /// The highest safety class among the operations it covers, which its
    /// annotations report.
    #[must_use]
    pub fn max_safety(&self) -> Safety {
        self.covers
            .iter()
            .filter_map(|id| descriptor(id))
            .map(|d| d.safety)
            .max()
            .unwrap_or(Safety::Read)
    }

    /// Whether an agent with `profile` sees the tool.
    #[must_use]
    pub fn allowed_for(&self, profile: Profile) -> bool {
        profile.allows(self.safety())
    }

    /// The tool as `tools/list` returns it, with portable schemas.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let op = self.main_op();
        let safety = self.safety();
        let max = self.max_safety();
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": schema::portable(&(op.input_schema)().to_value()),
            "outputSchema": schema::portable(&(op.output_schema)().to_value()),
            "annotations": {
                "title": self.title,
                "readOnlyHint": safety == Safety::Read,
                "destructiveHint": matches!(max, Safety::Trash | Safety::Permanent),
                "idempotentHint": self.idempotent,
                "openWorldHint": false,
            },
        })
    }
}

/// Every tool, in the order agents see them: by toolset, then by how often
/// it is needed.
pub static TOOLS: &[McpTool] = &[
    McpTool {
        name: "get_workspace",
        toolset: Toolset::Workspace,
        title: "Workspace overview",
        description: "Start here. Returns the workspace's name, your own name, profile and session, \
the workspace settings (working hours per day, first day of the week, trash retention) and a short \
guide to keys and references. Call it once at the start of a session; it changes nothing. \
Example: get_workspace {}",
        main: "workspace.get",
        covers: &["workspace.get", "settings.get"],
        idempotent: true,
    },
    McpTool {
        name: "get_activity",
        toolset: Toolset::Workspace,
        title: "Activity log",
        description: "Lists recent changes, newest first: who changed what, when, and through which \
program, with each field's value before and after. Filter by `session` (\"current\" for your own \
changes), `actor` or `group`; page with `cursor`. Set `sessions: true` to also list agent sessions. \
Use it to check what you or another agent changed before reverting. \
Example: get_activity {\"session\": \"current\", \"limit\": 20}",
        main: "history.feed",
        covers: &["history.feed", "agent.sessions"],
        idempotent: true,
    },
    McpTool {
        name: "revert_changes",
        toolset: Toolset::Workspace,
        title: "Revert changes",
        description: "Reverts history groups (the `group` of a change, from get_activity or from a \
tool's result) or everything an agent session changed. Groups that were changed again since are \
skipped and listed with the reason, so a newer change is never overwritten. Run with \
`dry_run: true` first to see what would be reverted. \
Example: revert_changes {\"session\": \"current\", \"dry_run\": true}",
        main: "history.revert",
        covers: &["history.revert"],
        idempotent: false,
    },
    McpTool {
        name: "list_projects",
        toolset: Toolset::Projects,
        title: "List projects",
        description: "Lists the project tree in sidebar order: each project's key, name, path, parent, \
depth and open-task counts (alone and with its sub-projects). Archived projects only with \
`include_archived: true`. Use the keys it returns in every other tool. \
Example: list_projects {}",
        main: "project.list",
        covers: &["project.list"],
        idempotent: true,
    },
    McpTool {
        name: "get_project",
        toolset: Toolset::Projects,
        title: "Get a project",
        description: "Returns one project with its configuration: statuses in board order (with \
category and WIP limit), task types, labels, modules, sub-projects, old keys and task counts. Read \
it before creating tasks so you use status, type and label names that exist. \
Example: get_project {\"project\": \"TM\"}",
        main: "project.get",
        covers: &["project.get"],
        idempotent: true,
    },
    McpTool {
        name: "create_project",
        toolset: Toolset::Projects,
        title: "Create a project",
        description: "Creates a project, or a sub-project with `parent` (at most 3 levels), which \
copies its parent's statuses, types and labels. The key is 2 to 10 capital letters and digits \
starting with a letter, and starts every task key. Pass `client_request_id` so a retry doesn't \
create a second project. \
Example: create_project {\"name\": \"Website\", \"key\": \"WEB\"}",
        main: "project.create",
        covers: &["project.create"],
        idempotent: false,
    },
    McpTool {
        name: "update_project",
        toolset: Toolset::Projects,
        title: "Update a project",
        description: "Changes a project in one step: `name`, `description`, `hue`, `icon`, `modules`, \
a new `key` (old keys keep working), a new `parent` or place (`before`/`after`/`at`), `archived`, \
and lists of `statuses`, `types` and `labels` to add (no `status`/`type`/`label` given), change \
or `remove`. Removing a status needs `move_tasks_to` when it has tasks; removals can't be undone \
and need the full profile. To clear a field, list it in `clear`. \
Example: update_project {\"project\": \"TM\", \"statuses\": [{\"name\": \"Blocked\", \"category\": \
\"in_progress\", \"after\": \"In progress\"}]}",
        main: "project.update",
        covers: &[
            "project.update",
            "project.move",
            "project.change_key",
            "project.archive",
            "project.unarchive",
            "status.create",
            "status.update",
            "status.reorder",
            "status.delete",
            "type.create",
            "type.update",
            "type.delete",
            "label.create",
            "label.update",
            "label.delete",
        ],
        idempotent: false,
    },
    McpTool {
        name: "get_task",
        toolset: Toolset::Tasks,
        title: "Get tasks",
        description: "Returns one to 20 tasks by key (`TM-41`; old keys work too) with every field, \
the description, labels and `version`. Pass the version as `expected_version` to update_task or \
move_task so you never overwrite a change you haven't seen. \
Example: get_task {\"keys\": [\"TM-41\"]}",
        main: "task.get",
        covers: &["task.get"],
        idempotent: true,
    },
    McpTool {
        name: "create_task",
        toolset: Toolset::Tasks,
        title: "Create a task",
        description: "Creates a task in a project from a title alone, or with `description`, \
`status`, `type`, `priority` (none, low, normal, high, urgent), `labels`, `parent`, `start_date`, \
`due_date` (YYYY-MM-DD) and `estimate` (`1d 4h`, `90m`). It lands in the first to-do status at \
the bottom unless told otherwise, and gets the project's next key. Pass `client_request_id` so \
a retry doesn't create a second task. \
Example: create_task {\"project\": \"TM\", \"title\": \"Write the release notes\", \"due_date\": \
\"2026-12-01\"}",
        main: "task.create",
        covers: &["task.create"],
        idempotent: false,
    },
    McpTool {
        name: "update_task",
        toolset: Toolset::Tasks,
        title: "Update a task",
        description: "Changes the given fields of a task only: `title`, `description` (replaces \
it), `type`, `priority`, `labels` (replace) or `add_labels`/`remove_labels`, `parent`, \
`start_date`, `due_date`, `estimate`, `progress`. To clear a field, list it in `clear`. Status \
and position change with move_task. Pass `expected_version` from get_task. \
Example: update_task {\"key\": \"TM-41\", \"priority\": \"high\", \"clear\": [\"due_date\"]}",
        main: "task.update",
        covers: &["task.update"],
        idempotent: true,
    },
    McpTool {
        name: "move_task",
        toolset: Toolset::Tasks,
        title: "Move a task",
        description: "Moves a task to another `status` (by name), to a place in its column \
(`before` or `after` another task's key, or `at`: top or bottom), or to another `project`, where \
it gets a new key while the old one keeps finding it. Entering a done or cancelled status closes \
the task. \
Example: move_task {\"key\": \"TM-41\", \"status\": \"In progress\", \"at\": \"top\"}",
        main: "task.move",
        covers: &["task.move"],
        idempotent: true,
    },
    McpTool {
        name: "get_board",
        toolset: Toolset::Views,
        title: "Get a board",
        description: "Returns a project's board as the person sees it: columns in their order with \
hidden ones flagged (and their cards left out), WIP limits and whether a column is over it, and \
compact cards. `include_subprojects: true` adds the sub-projects' tasks, matching columns by \
status name. Closed cards older than the layout's limit are counted, not listed, unless \
`show_older: true`. \
Example: get_board {\"project\": \"TM\"}",
        main: "board.get",
        covers: &["board.get", "layout.get"],
        idempotent: true,
    },
    McpTool {
        name: "set_board_layout",
        toolset: Toolset::Views,
        title: "Set the board layout",
        description: "Changes the person's own layout of a project's board: `hide`, `show`, \
`collapse`, `expand` columns (by status name), set their `order`, the `sort_mode` (manual, \
priority, due, updated) or per-column `column_sort`, `card_fields`, `hide_done_after_days`, \
`include_subprojects`, or `reset` to the project's layout. It never changes the project's \
statuses. Only change the layout when the person asked. \
Example: set_board_layout {\"project\": \"TM\", \"hide\": [\"Backlog\"]}",
        main: "layout.update",
        covers: &["layout.update"],
        idempotent: true,
    },
    McpTool {
        name: "update_settings",
        toolset: Toolset::Admin,
        title: "Update workspace settings",
        description: "Changes workspace settings: `trash_days` (1 to 3650), `reminder_time` (HH:MM), \
`hours_per_day` (1 to 24), `days_per_week` (1 to 7), `week_start`. Fields left out keep their value. \
Only change settings the person asked you to. Settings about agents can't be changed here. \
Example: update_settings {\"hours_per_day\": 6}",
        main: "settings.update",
        covers: &["settings.update"],
        idempotent: true,
    },
];

/// The tool that covers operation `id`, if one does.
#[must_use]
pub fn tool_for(id: &str) -> Option<&'static McpTool> {
    TOOLS.iter().find(|t| t.covers.contains(&id))
}

/// The tools an agent sees with `profile` and the given toolsets, in order.
#[must_use]
pub fn tools_for(profile: Profile, toolsets: &[Toolset]) -> Vec<&'static McpTool> {
    TOOLS
        .iter()
        .filter(|t| toolsets.contains(&t.toolset) && t.allowed_for(profile))
        .collect()
}

/// The golden tool list: every tool as `tools/list` returns it, the
/// toolsets with their defaults, and which tools each profile sees with the
/// default toolsets.
#[must_use]
pub fn golden_document() -> Value {
    let toolsets: serde_json::Map<String, Value> = Toolset::ALL
        .iter()
        .map(|t| {
            (
                t.as_str().to_owned(),
                json!({ "on_by_default": t.on_by_default() }),
            )
        })
        .collect();
    let defaults = Toolset::defaults();
    let profiles: serde_json::Map<String, Value> = Profile::ALL
        .iter()
        .map(|p| {
            let names: Vec<&str> = tools_for(*p, &defaults).iter().map(|t| t.name).collect();
            (p.as_str().to_owned(), json!(names))
        })
        .collect();
    let tools: Vec<Value> = TOOLS
        .iter()
        .map(|t| {
            let mut v = t.to_json();
            v["toolset"] = json!(t.toolset.as_str());
            v["covers"] = json!(t.covers);
            v
        })
        .collect();
    json!({ "toolsets": toolsets, "profiles": profiles, "tools": tools })
}

/// Data operations that no tool covers. Must stay empty: every data
/// operation is reachable by agents. An entry needs a recorded equivalence.
pub const PARITY_EXCEPTIONS: &[(&str, &str)] = &[];

/// Data operations no tool covers and that are not listed as exceptions.
#[must_use]
pub fn parity_gaps() -> Vec<&'static str> {
    CATALOG
        .iter()
        .filter(|d| !d.is_app_only())
        .filter(|d| tool_for(d.id).is_none())
        .filter(|d| !PARITY_EXCEPTIONS.iter().any(|(id, _)| *id == d.id))
        .map(|d| d.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_data_operation_has_a_tool() {
        assert_eq!(parity_gaps(), Vec::<&str>::new());
    }

    #[test]
    fn tools_cover_only_data_operations_that_exist() {
        let mut names = HashSet::new();
        for t in TOOLS {
            assert!(names.insert(t.name), "{} is listed twice", t.name);
            assert_eq!(t.covers.first(), Some(&t.main), "{}: main first", t.name);
            for id in t.covers {
                let d = descriptor(id).unwrap_or_else(|| panic!("{}: unknown {id}", t.name));
                assert!(!d.is_app_only(), "{}: {id} is app-only", t.name);
            }
            assert!(
                t.name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{}",
                t.name
            );
        }
    }

    #[test]
    fn tools_are_in_toolset_order() {
        let order: Vec<usize> = TOOLS
            .iter()
            .map(|t| {
                Toolset::ALL
                    .iter()
                    .position(|s| *s == t.toolset)
                    .unwrap_or(usize::MAX)
            })
            .collect();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(order, sorted);
    }

    #[test]
    fn profiles_filter_tools() {
        let all = Toolset::ALL;
        let read_only: Vec<&str> = tools_for(Profile::ReadOnly, &all)
            .iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(
            read_only,
            [
                "get_workspace",
                "get_activity",
                "list_projects",
                "get_project",
                "get_task",
                "get_board"
            ]
        );
        assert!(
            tools_for(Profile::Standard, &all)
                .iter()
                .any(|t| t.name == "revert_changes")
        );
        let defaults = tools_for(Profile::Full, &Toolset::defaults());
        assert!(defaults.iter().all(|t| t.toolset != Toolset::Admin));
    }

    #[test]
    fn a_tool_is_listed_by_its_main_operation_and_warns_of_its_riskiest_part() {
        let update = TOOLS
            .iter()
            .find(|t| t.name == "update_project")
            .expect("update_project");
        assert_eq!(update.safety(), Safety::Write);
        assert_eq!(update.max_safety(), Safety::Permanent);
        assert!(update.allowed_for(Profile::Standard));
        assert_eq!(update.to_json()["annotations"]["destructiveHint"], true);
    }

    #[test]
    fn tool_schemas_are_portable() {
        for t in TOOLS {
            let v = t.to_json();
            assert!(schema::is_portable(&v["inputSchema"]), "{}", t.name);
            assert!(schema::is_portable(&v["outputSchema"]), "{}", t.name);
            assert_eq!(v["inputSchema"]["type"], "object", "{}", t.name);
            assert_eq!(v["outputSchema"]["type"], "object", "{}", t.name);
        }
    }
}
