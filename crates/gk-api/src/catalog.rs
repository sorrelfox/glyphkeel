// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The list of operations and what is known about each one.

use schemars::{JsonSchema, Schema};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::ops::{
    agent, backup, board, bulk, comment, description, file, history, project, query, search,
    settings, task, time, today, transfer, trash, view, wiki, workspace,
};
use crate::schema;

/// One operation of the catalog, as a type: its name and its input and
/// output types.
pub trait Operation {
    /// The operation's name, `area.verb`.
    const ID: &'static str;
    /// What the caller sends. Unknown fields are refused.
    type Input: DeserializeOwned + Serialize + JsonSchema;
    /// What the operation returns.
    type Output: DeserializeOwned + Serialize + JsonSchema;
}

/// How much an operation can change, which decides who may call it and
/// whether it is confirmed first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Safety {
    /// Changes nothing.
    Read,
    /// Creates or changes data; can be undone.
    Write,
    /// Moves data into the trash; can be restored.
    Trash,
    /// Can't be undone inside the app (a backup still has the data). The
    /// person confirms it first.
    Permanent,
    /// Changes the application rather than the data. Never offered to
    /// agents.
    App,
}

impl Safety {
    /// The name used in schemas, logs and the generated client.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Trash => "trash",
            Self::Permanent => "permanent",
            Self::App => "app",
        }
    }
}

/// Whether an operation reads or changes the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    /// Runs on a read connection, in one snapshot.
    Query,
    /// Runs in one write transaction, with history and a change event.
    Command,
}

/// Where an operation can be reached from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exposure {
    /// The app, the command line and MCP. The default.
    Data,
    /// The app only (some also from the command line), never agents. The
    /// reason says why the operation is not about the workspace's data; code
    /// review checks that it holds.
    App {
        /// Why agents can't reach this operation.
        reason: &'static str,
    },
}

/// What every adapter needs to know about one operation.
#[derive(Debug, Clone, Copy)]
pub struct OpDescriptor {
    /// `area.verb`.
    pub id: &'static str,
    /// One sentence: what the operation does.
    pub summary: &'static str,
    /// Query or command.
    pub kind: OpKind,
    /// The safety class.
    pub safety: Safety,
    /// Who can reach it.
    pub exposure: Exposure,
    /// The `glyphkeel` command, without the program name; `None` while it has
    /// none yet.
    pub cli: Option<&'static str>,
    /// Name of the input type in schemas and the generated client.
    pub input_name: &'static str,
    /// Name of the output type in schemas and the generated client.
    pub output_name: &'static str,
    /// The input's JSON Schema, as deserialization reads it.
    pub input_schema: fn() -> Schema,
    /// The output's JSON Schema, as serialization writes it.
    pub output_schema: fn() -> Schema,
}

impl OpDescriptor {
    /// The descriptor of `O` with the given metadata.
    #[must_use]
    pub const fn of<O: Operation>(meta: Meta) -> Self {
        Self {
            id: O::ID,
            summary: meta.summary,
            kind: meta.kind,
            safety: meta.safety,
            exposure: meta.exposure,
            cli: meta.cli,
            input_name: meta.input_name,
            output_name: meta.output_name,
            input_schema: schema::input::<O::Input>,
            output_schema: schema::output::<O::Output>,
        }
    }

    /// Whether agents can never reach this operation.
    #[must_use]
    pub const fn is_app_only(&self) -> bool {
        matches!(self.exposure, Exposure::App { .. }) || matches!(self.safety, Safety::App)
    }
}

/// The metadata of an operation, given to [`OpDescriptor::of`].
#[derive(Debug, Clone, Copy)]
pub struct Meta {
    /// See [`OpDescriptor::summary`].
    pub summary: &'static str,
    /// See [`OpDescriptor::kind`].
    pub kind: OpKind,
    /// See [`OpDescriptor::safety`].
    pub safety: Safety,
    /// See [`OpDescriptor::exposure`].
    pub exposure: Exposure,
    /// See [`OpDescriptor::cli`].
    pub cli: Option<&'static str>,
    /// See [`OpDescriptor::input_name`].
    pub input_name: &'static str,
    /// See [`OpDescriptor::output_name`].
    pub output_name: &'static str,
}

/// Every operation, in catalog order: by area, then by how often it is
/// needed. Adapters and generated files keep this order.
pub static CATALOG: &[OpDescriptor] = &[
    workspace::WORKSPACE_GET,
    settings::SETTINGS_GET,
    settings::SETTINGS_UPDATE,
    search::SEARCH_QUERY,
    project::PROJECT_LIST,
    project::PROJECT_GET,
    project::PROJECT_CREATE,
    project::PROJECT_UPDATE,
    project::PROJECT_MOVE,
    project::PROJECT_CHANGE_KEY,
    project::PROJECT_ARCHIVE,
    project::PROJECT_UNARCHIVE,
    project::PROJECT_DELETE,
    project::PROJECT_BRIEF,
    project::STATUS_CREATE,
    project::STATUS_UPDATE,
    project::STATUS_REORDER,
    project::STATUS_DELETE,
    project::TYPE_CREATE,
    project::TYPE_UPDATE,
    project::TYPE_DELETE,
    project::LABEL_CREATE,
    project::LABEL_UPDATE,
    project::LABEL_DELETE,
    query::TASK_SEARCH,
    task::TASK_GET,
    task::TASK_CREATE,
    task::TASK_UPDATE,
    task::TASK_MOVE,
    task::TASK_DELETE,
    bulk::TASK_BULK_UPDATE,
    task::TASK_REORDER_SUBTASK,
    description::TASK_RESOLVE_CONFLICT,
    description::CHECKLIST_UPDATE,
    task::TASK_DUPLICATE,
    task::TASK_CLAIM,
    task::TASK_RELEASE,
    task::RELATION_ADD,
    task::RELATION_REMOVE,
    description::DESCRIPTION_HISTORY,
    description::DESCRIPTION_REVISION,
    description::MARKDOWN_RENDER,
    comment::COMMENT_LIST,
    comment::COMMENT_ADD,
    comment::COMMENT_UPDATE,
    comment::COMMENT_DELETE,
    time::TIME_LOG,
    time::TIME_UPDATE,
    time::TIME_DELETE,
    time::TIME_REPORT,
    time::TIMER_GET,
    time::TIMER_START,
    time::TIMER_PAUSE,
    time::TIMER_STOP,
    time::REMINDER_SET,
    time::REMINDER_CLEAR,
    time::REMINDER_DUE,
    time::REMINDER_ACKNOWLEDGE,
    today::TODAY_GET,
    board::BOARD_GET,
    board::LAYOUT_GET,
    board::LAYOUT_UPDATE,
    view::VIEW_LIST,
    view::VIEW_CREATE,
    view::VIEW_UPDATE,
    view::VIEW_DELETE,
    view::LIST_LAYOUT_GET,
    view::LIST_LAYOUT_UPDATE,
    query::QUERY_PARSE,
    wiki::WIKI_LIST,
    wiki::WIKI_SEARCH,
    wiki::WIKI_GET,
    wiki::WIKI_CREATE,
    wiki::WIKI_UPDATE,
    wiki::WIKI_DELETE,
    wiki::WIKI_HISTORY,
    file::FILE_GET,
    file::FILE_READ,
    file::FILE_THUMBNAIL,
    file::FILE_ATTACH,
    file::FILE_DELETE,
    agent::AGENT_SESSIONS,
    agent::AGENT_POLICY_UPDATE,
    history::HISTORY_FEED,
    history::HISTORY_REVERT,
    history::HISTORY_UNDO,
    history::HISTORY_REDO,
    trash::TRASH_LIST,
    trash::TRASH_RESTORE,
    trash::TRASH_PURGE,
    backup::BACKUP_CREATE,
    backup::BACKUP_LIST,
    backup::BACKUP_VERIFY,
    backup::BACKUP_RESTORE,
    backup::BACKUP_DELETE,
    backup::BACKUP_FOLDER_UPDATE,
    transfer::TRANSFER_EXPORT,
    transfer::TRANSFER_IMPORT,
];

/// The descriptor of the operation named `id`.
#[must_use]
pub fn descriptor(id: &str) -> Option<&'static OpDescriptor> {
    CATALOG.iter().find(|d| d.id == id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn names_are_unique_and_shaped_area_verb() {
        let mut seen = HashSet::new();
        for d in CATALOG {
            assert!(seen.insert(d.id), "{} is listed twice", d.id);
            let (area, verb) = d.id.split_once('.').expect("area.verb");
            for part in [area, verb] {
                assert!(
                    !part.is_empty()
                        && part
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c == '_' || c == '.'),
                    "{} is not area.verb in lower case",
                    d.id
                );
            }
        }
    }

    #[test]
    fn app_operations_give_a_reason() {
        for d in CATALOG {
            let reason = match d.exposure {
                Exposure::App { reason } => Some(reason),
                Exposure::Data => None,
            };
            if d.safety == Safety::App {
                assert!(
                    reason.is_some(),
                    "{} is app class but exposed as data",
                    d.id
                );
            }
            if let Some(reason) = reason {
                assert!(
                    reason.split_whitespace().count() >= 5,
                    "{} needs a written reason for being app-only",
                    d.id
                );
            }
        }
    }

    #[test]
    fn queries_are_read_class_and_commands_are_not() {
        for d in CATALOG {
            match d.kind {
                OpKind::Query => assert_eq!(d.safety, Safety::Read, "{}", d.id),
                OpKind::Command => assert_ne!(d.safety, Safety::Read, "{}", d.id),
            }
        }
    }

    #[test]
    fn data_operations_without_a_command_are_listed() {
        let missing: Vec<&str> = CATALOG
            .iter()
            .filter(|d| matches!(d.exposure, Exposure::Data) && d.cli.is_none())
            .map(|d| d.id)
            .collect();
        if !missing.is_empty() {
            // A warning until every data operation has a command.
            eprintln!(
                "warning: data operations without a command: {}",
                missing.join(", ")
            );
        }
    }
}
