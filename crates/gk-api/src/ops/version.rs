// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Versions (releases): named goals of a project with dates and a status,
//! which tasks are planned for. A version can be shared with the project's
//! sub-projects. The roadmap lists the versions with their progress and
//! their tasks.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::task::StatusRef;

/// Where a version stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VersionStatus {
    /// Tasks can be planned for it (the default).
    #[default]
    Open,
    /// No new tasks; the tasks in it stay.
    Locked,
    /// Finished; no new tasks.
    Closed,
}

impl VersionStatus {
    /// The name as stored.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Locked => "locked",
            Self::Closed => "closed",
        }
    }
}

/// Which projects can plan tasks for a version.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VersionSharing {
    /// Only its own project (the default).
    #[default]
    None,
    /// Its project and every sub-project below it.
    Descendants,
}

impl VersionSharing {
    /// The name as stored.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Descendants => "descendants",
        }
    }
}

/// A version as a task shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionRef {
    /// Its name.
    pub name: String,
    /// The key of the project it belongs to.
    pub project: String,
    /// Where it stands.
    pub status: VersionStatus,
    /// Its due date (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// Its ID.
    pub id: String,
}

/// How far a version has come.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionProgress {
    /// The tasks planned for it (not in the trash).
    pub total: u32,
    /// How many of them are closed.
    pub closed: u32,
    /// Done, in percent: each task's progress weighted by its estimate;
    /// closed tasks count as 100 %, unestimated ones get the average weight.
    pub percent: u8,
    /// The tasks' estimates added up, in minutes.
    pub estimate_minutes: i64,
    /// The time logged on them, in minutes.
    pub spent_minutes: i64,
}

/// A version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Version {
    /// Its name, unique in its project (ignoring case).
    pub name: String,
    /// The key of the project it belongs to.
    pub project: String,
    /// What it is for: plain text.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Where it stands.
    pub status: VersionStatus,
    /// When work on it starts (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// When it is due (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// Which projects can plan tasks for it.
    pub sharing: VersionSharing,
    /// Whether it belongs to an ancestor project and is shared with this
    /// one, when listed for a project.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inherited: bool,
    /// Its progress.
    pub progress: VersionProgress,
    /// Its tasks, when asked for: top-level first by board order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<VersionTask>>,
    /// Its place among the project's versions.
    pub rank: String,
    /// When it was created (RFC 3339, UTC).
    pub created_at: String,
    /// When it last changed.
    pub updated_at: String,
    /// Its ID.
    pub id: String,
}

/// A task of a version, as the roadmap lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionTask {
    /// Its key.
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its project's key.
    pub project: String,
    /// Its status.
    pub status: StatusRef,
    /// Whether it is closed (done or cancelled).
    pub closed: bool,
    /// Its type's name.
    #[serde(rename = "type")]
    pub task_type: String,
    /// Its progress in percent.
    pub progress_percent: u8,
    /// Its ID.
    pub id: String,
}

/// Which versions to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionListInput {
    /// The project (key or ID): its own versions and those its ancestors
    /// share with it.
    pub project: String,
    /// Only versions with these statuses; default: all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status: Vec<VersionStatus>,
    /// Leave out the versions ancestors share with the project.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub own_only: bool,
    /// Include each version's tasks, for the roadmap.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_tasks: bool,
}

/// Versions: by due date (those without one last), then by their order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionList {
    /// The versions.
    pub items: Vec<Version>,
    /// How many.
    pub total: u64,
    /// With `include_tasks`: the project's open tasks planned for no
    /// version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unplanned: Option<u32>,
}

/// One version to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionGetInput {
    /// The version: its ID, or its name.
    pub version: String,
    /// With a name: the project (key or ID); its own versions come first,
    /// then those shared with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A new version.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionCreateInput {
    /// The project (key or ID).
    pub project: String,
    /// Its name, up to 100 characters, unique in the project.
    pub name: String,
    /// What it is for: plain text, up to 4,000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Where it stands; default: open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<VersionStatus>,
    /// When work starts: `YYYY-MM-DD`, or `today`, `fri`, `+2w`, `dec 1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// When it is due: `YYYY-MM-DD`, or `today`, `fri`, `+2w`, `dec 1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// Which projects can plan tasks for it; default: only its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharing: Option<VersionSharing>,
}

/// A version field that can be cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VersionField {
    /// The description.
    Description,
    /// The start date.
    StartDate,
    /// The due date.
    DueDate,
}

/// Changes to a version; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionUpdateInput {
    /// The version: its ID, or its name.
    pub version: String,
    /// With a name: the version's project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A new name; saved views that name the old one are rewritten.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A new description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// A new status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<VersionStatus>,
    /// A new start date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// A new due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// New sharing. Tasks of projects that no longer see the version lose
    /// it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharing: Option<VersionSharing>,
    /// When closing it: plan its open tasks for this other open version (its
    /// name or ID) instead, or `none` to plan them for no version. Left
    /// out, open tasks stay in the closed version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_open_tasks_to: Option<String>,
    /// Fields to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<VersionField>,
}

/// Which version to delete.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionDeleteInput {
    /// The version: its ID, or its name.
    pub version: String,
    /// With a name: the version's project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A version after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionChange {
    /// The version after the change (before it, for a deletion).
    pub version: Version,
    /// Whether it was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The keys of the tasks whose version changed with it: moved on when
    /// it closed, or cleared by a deletion or new sharing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks_changed: Vec<String>,
    /// The saved views whose query was rewritten because they named the
    /// version (renamed or deleted), by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views_rewritten: Vec<String>,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// A project's versions.
    VersionListOp,
    "version.list",
    VersionListInput,
    VersionList
);
op!(
    /// One version with its tasks.
    VersionGet,
    "version.get",
    VersionGetInput,
    Version
);
op!(
    /// Creates a version.
    VersionCreate,
    "version.create",
    VersionCreateInput,
    VersionChange
);
op!(
    /// Changes a version.
    VersionUpdate,
    "version.update",
    VersionUpdateInput,
    VersionChange
);
op!(
    /// Deletes a version.
    VersionDelete,
    "version.delete",
    VersionDeleteInput,
    VersionChange
);

pub(crate) const VERSION_LIST: OpDescriptor = OpDescriptor::of::<VersionListOp>(Meta {
    summary: "The versions of a project and those its ancestors share with it, with status, dates and progress; with their tasks for the roadmap.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("version list <project>"),
    input_name: "VersionListInput",
    output_name: "VersionList",
});

pub(crate) const VERSION_GET: OpDescriptor = OpDescriptor::of::<VersionGet>(Meta {
    summary: "One version with its progress and its tasks.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("version show <version>"),
    input_name: "VersionGetInput",
    output_name: "Version",
});

pub(crate) const VERSION_CREATE: OpDescriptor = OpDescriptor::of::<VersionCreate>(Meta {
    summary: "Create a version of a project: name, description, status, start and due dates, and whether its sub-projects can use it.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("version create"),
    input_name: "VersionCreateInput",
    output_name: "VersionChange",
});

pub(crate) const VERSION_UPDATE: OpDescriptor = OpDescriptor::of::<VersionUpdate>(Meta {
    summary: "Rename a version, change its description, dates, sharing or status, or close it and move its open tasks to another version.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("version edit <version>"),
    input_name: "VersionUpdateInput",
    output_name: "VersionChange",
});

pub(crate) const VERSION_DELETE: OpDescriptor = OpDescriptor::of::<VersionDelete>(Meta {
    summary: "Delete a version; its tasks are planned for no version. Revert brings both back.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("version delete <version>"),
    input_name: "VersionDeleteInput",
    output_name: "VersionChange",
});
