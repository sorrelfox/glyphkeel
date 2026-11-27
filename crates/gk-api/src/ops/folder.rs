// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Project folders: one level of named groups for the top-level projects
//! in the sidebar, for a workspace with many projects. A folder holds
//! projects only, never other folders; a top-level project is in at most
//! one folder and takes its sub-projects along. Folders change nothing
//! about the projects themselves: keys, sub-projects, queries and pickers
//! work as before.
//!
//! Folders belong to the workspace and are kept whole in history, so
//! making, renaming, reordering and deleting one can be undone.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::{Actor, Edge, Hue};

/// A folder of top-level projects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectFolder {
    /// The folder's ID.
    pub id: String,
    /// Its name, unique among the folders (ignoring case).
    pub name: String,
    /// Its colour, if one was chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<Hue>,
    /// Its place among the folders.
    pub rank: String,
    /// The keys of its projects, in sidebar order, archived ones included.
    pub projects: Vec<String>,
    /// Open tasks in its active projects and their sub-projects.
    pub open_tasks: u64,
    /// Who made it.
    pub created_by: Actor,
    /// When it was made (RFC 3339).
    pub created_at: String,
    /// When it last changed (RFC 3339).
    pub updated_at: String,
}

/// Which folders to list: all of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderListInput {}

/// The folders, in sidebar order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderList {
    /// The folders.
    pub items: Vec<ProjectFolder>,
    /// How many there are.
    pub total: u64,
}

/// One folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderRef {
    /// The folder: its ID or its name.
    pub folder: String,
}

/// A new folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderCreateInput {
    /// Its name, unique among the folders.
    pub name: String,
    /// Its colour; left out: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<Hue>,
    /// Place it before this folder (ID or name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Place it after this folder (ID or name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Place it first or last; default: last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
}

/// A folder field that can be cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FolderField {
    /// The colour.
    Hue,
}

/// Changes to a folder; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderUpdateInput {
    /// The folder: its ID or its name.
    pub folder: String,
    /// A new name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A new colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<Hue>,
    /// Move it before this folder (ID or name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Move it after this folder (ID or name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Move it first or last among the folders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
    /// Fields to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<FolderField>,
}

/// A new name for a folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderRenameInput {
    /// The folder: its ID or its name.
    pub folder: String,
    /// Its new name.
    pub name: String,
}

/// Where to put a top-level project: into a folder, or out of its folder,
/// and where among the projects there.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderMoveInput {
    /// The project: its key (current or old) or ID. Only top-level
    /// projects go into folders; their sub-projects come along.
    pub project: String,
    /// The folder to put it in (ID or name); left out with `none`: out of
    /// any folder, back at the top level; left out without `none`: it
    /// stays in its folder and only moves within it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Take it out of its folder.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub none: bool,
    /// Place it before this project of the same folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Place it after this project of the same folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Place it first or last in the folder (or among the projects in no
    /// folder); default when it changes folder: last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
}

/// A folder after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderChange {
    /// The folder after the change (before it, for a deletion); left out
    /// when a project was taken out of its folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<ProjectFolder>,
    /// The project moved, by key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The projects that were in a deleted folder, now at the top level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub released: Vec<String>,
    /// Whether the folder was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// The project folders.
    FolderListOp,
    "folder.list",
    FolderListInput,
    FolderList
);
op!(
    /// Makes a folder.
    FolderCreate,
    "folder.create",
    FolderCreateInput,
    FolderChange
);
op!(
    /// Renames, recolours or reorders a folder.
    FolderUpdate,
    "folder.update",
    FolderUpdateInput,
    FolderChange
);
op!(
    /// Renames a folder.
    FolderRename,
    "folder.rename",
    FolderRenameInput,
    FolderChange
);
op!(
    /// Deletes a folder; its projects go back to the top level.
    FolderDelete,
    "folder.delete",
    FolderRef,
    FolderChange
);
op!(
    /// Puts a project into a folder, out of it, or elsewhere in it.
    FolderMove,
    "folder.move",
    FolderMoveInput,
    FolderChange
);

pub(crate) const FOLDER_LIST: OpDescriptor = OpDescriptor::of::<FolderListOp>(Meta {
    summary: "The project folders in sidebar order, each with its projects and their open tasks.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("folder list"),
    input_name: "FolderListInput",
    output_name: "FolderList",
});

pub(crate) const FOLDER_CREATE: OpDescriptor = OpDescriptor::of::<FolderCreate>(Meta {
    summary: "Make a folder to group top-level projects in the sidebar.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("folder create"),
    input_name: "FolderCreateInput",
    output_name: "FolderChange",
});

pub(crate) const FOLDER_UPDATE: OpDescriptor = OpDescriptor::of::<FolderUpdate>(Meta {
    summary: "Rename a folder, change its colour or move it among the folders.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("folder edit <folder>"),
    input_name: "FolderUpdateInput",
    output_name: "FolderChange",
});

pub(crate) const FOLDER_RENAME: OpDescriptor = OpDescriptor::of::<FolderRename>(Meta {
    summary: "Rename a folder.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("folder rename <folder> <name>"),
    input_name: "FolderRenameInput",
    output_name: "FolderChange",
});

pub(crate) const FOLDER_DELETE: OpDescriptor = OpDescriptor::of::<FolderDelete>(Meta {
    summary: "Delete a folder; its projects stay and go back to the top level. Revert brings it back.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("folder delete <folder>"),
    input_name: "FolderRef",
    output_name: "FolderChange",
});

pub(crate) const FOLDER_MOVE: OpDescriptor = OpDescriptor::of::<FolderMove>(Meta {
    summary: "Put a top-level project into a folder or take it out, or move it within its folder.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("folder move <project>"),
    input_name: "FolderMoveInput",
    output_name: "FolderChange",
});
