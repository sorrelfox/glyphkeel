// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The MCP tools: written by hand, in the order agents see them, each
//! covering one or more catalog operations.
//!
//! A tool takes its input and output schemas from its main operation. It is
//! listed for an agent when the agent's profile allows the safety class of
//! every operation it covers and its toolset is switched on.

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

    /// The highest safety class among the operations it covers.
    #[must_use]
    pub fn safety(&self) -> Safety {
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
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": schema::portable(&(op.input_schema)().to_value()),
            "outputSchema": schema::portable(&(op.output_schema)().to_value()),
            "annotations": {
                "title": self.title,
                "readOnlyHint": safety == Safety::Read,
                "destructiveHint": matches!(safety, Safety::Trash | Safety::Permanent),
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
        assert_eq!(read_only, ["get_workspace", "get_activity"]);
        assert!(
            tools_for(Profile::Standard, &all)
                .iter()
                .any(|t| t.name == "revert_changes")
        );
        let defaults = tools_for(Profile::Full, &Toolset::defaults());
        assert!(defaults.iter().all(|t| t.toolset != Toolset::Admin));
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
