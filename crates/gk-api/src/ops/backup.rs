// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Backups: snapshots of the workspace database and its stored files.
//!
//! A backup is a checked copy of the database plus every stored file it
//! refers to; files are copied once and shared by all backups. One is made
//! every day; the last 7 daily and 4 weekly ones are kept, and the last 3
//! of each automatic kind. Backups made by hand stay until deleted.
//! Restoring one replaces the whole workspace, after a backup of the
//! current state; that is for the person only.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::settings::SettingsChange;

/// Why a backup was made, which decides how long it is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    /// The daily backup; the last 7 are kept.
    Daily,
    /// A daily backup kept for its week; the last 4 are kept.
    Weekly,
    /// Made by hand; kept until deleted.
    Manual,
    /// Made before a schema upgrade; the last 3 are kept.
    PreMigration,
    /// Made before a large change by an agent; the last 3 are kept.
    Safety,
    /// The state before a backup was restored; the last 3 are kept.
    PreRestore,
}

impl BackupKind {
    /// The name used in the backup list.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Manual => "manual",
            Self::PreMigration => "pre_migration",
            Self::Safety => "safety",
            Self::PreRestore => "pre_restore",
        }
    }
}

/// One backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Backup {
    /// Its name; pass it to verify, restore or delete it.
    pub name: String,
    /// Why it was made.
    pub kind: BackupKind,
    /// When it was made (RFC 3339, UTC).
    pub created_at: String,
    /// The database copy's size in bytes.
    pub size: u64,
    /// How many stored files it refers to.
    pub files: u64,
    /// Their size in bytes (shared with other backups).
    pub files_size: u64,
    /// The schema version of the copy.
    pub schema_version: i64,
    /// The program version that made it.
    pub app_version: String,
}

/// No input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupCreateInput {}

/// No input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupListInput {}

/// The workspace's backups, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupList {
    /// The folder that holds them.
    pub folder: String,
    /// The backups.
    pub items: Vec<Backup>,
    /// When the newest backup was made; left out when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_backup_at: Option<String>,
}

/// Names one backup.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupRef {
    /// The backup's name, from the backup list.
    pub name: String,
}

/// What is wrong with a backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupProblemKind {
    /// The database copy fails SQLite's integrity check, or can't be read.
    Database,
    /// A stored file it refers to is missing.
    MissingFile,
    /// A stored file it refers to has other content than its hash says.
    DamagedFile,
}

/// One problem of a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupProblem {
    /// What is wrong.
    pub kind: BackupProblemKind,
    /// The detail: SQLite's report, or the file's SHA-256.
    pub detail: String,
}

/// The result of checking a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupCheck {
    /// The backup checked.
    pub backup: Backup,
    /// Whether it can be restored as it is.
    pub ok: bool,
    /// What is wrong, at most 100 problems.
    pub problems: Vec<BackupProblem>,
    /// How many stored files were read and their hashes compared.
    pub files_checked: u64,
}

/// A restored backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupRestored {
    /// The backup now in place.
    pub restored: Backup,
    /// The backup of the state before, to go back to it.
    pub previous: Backup,
}

/// A deleted backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupDeleted {
    /// The backup's name.
    pub name: String,
    /// Stored files no other backup used, removed with it.
    pub files_removed: u64,
}

op!(
    /// A backup now.
    BackupCreate,
    "backup.create",
    BackupCreateInput,
    Backup
);
op!(
    /// The workspace's backups.
    BackupListOp,
    "backup.list",
    BackupListInput,
    BackupList
);
op!(
    /// Check a backup.
    BackupVerify,
    "backup.verify",
    BackupRef,
    BackupCheck
);
op!(
    /// Put a backup in place of the workspace.
    BackupRestore,
    "backup.restore",
    BackupRef,
    BackupRestored
);
op!(
    /// Move the backup folder.
    BackupFolderUpdate,
    "backup.folder.update",
    BackupFolderPatch,
    SettingsChange
);
op!(
    /// Delete a backup.
    BackupDelete,
    "backup.delete",
    BackupRef,
    BackupDeleted
);

/// Where the backups go.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupFolderPatch {
    /// The new folder: an absolute path outside the workspace's own folder,
    /// which is created if it doesn't exist. Leave it out to go back to the
    /// default folder. Backups already made stay where they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

pub(crate) const BACKUP_FOLDER_UPDATE: OpDescriptor = OpDescriptor::of::<BackupFolderUpdate>(
    Meta {
        summary: "Move the backup folder to another place, or back to the default; backups already made stay where they are.",
        kind: OpKind::Command,
        safety: Safety::App,
        exposure: Exposure::App {
            reason: "it decides where copies of all the person's data are written, so only the person may set it, in the app",
        },
        cli: None,
        input_name: "BackupFolderPatch",
        output_name: "SettingsChange",
    },
);

pub(crate) const BACKUP_CREATE: OpDescriptor = OpDescriptor::of::<BackupCreate>(Meta {
    summary: "Make a backup of the workspace now: a checked copy of the database and its stored files.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("backup create"),
    input_name: "BackupCreateInput",
    output_name: "Backup",
});

pub(crate) const BACKUP_LIST: OpDescriptor = OpDescriptor::of::<BackupListOp>(Meta {
    summary: "The workspace's backups, newest first, with their kind and size.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("backup list"),
    input_name: "BackupListInput",
    output_name: "BackupList",
});

pub(crate) const BACKUP_VERIFY: OpDescriptor = OpDescriptor::of::<BackupVerify>(Meta {
    summary: "Check a backup: the database copy's integrity and every stored file it needs.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("backup verify <name>"),
    input_name: "BackupRef",
    output_name: "BackupCheck",
});

pub(crate) const BACKUP_RESTORE: OpDescriptor = OpDescriptor::of::<BackupRestore>(Meta {
    summary: "Replace the workspace with a backup, after backing up the current state. Other programs with the workspace open must restart.",
    kind: OpKind::Command,
    safety: Safety::App,
    exposure: Exposure::App {
        reason: "it replaces the whole workspace, including every agent's own work, so only the person may do it",
    },
    cli: Some("backup restore <name>"),
    input_name: "BackupRef",
    output_name: "BackupRestored",
});

pub(crate) const BACKUP_DELETE: OpDescriptor = OpDescriptor::of::<BackupDelete>(Meta {
    summary: "Delete a backup and the stored files only it used.",
    kind: OpKind::Command,
    safety: Safety::Permanent,
    exposure: Exposure::App {
        reason: "backups are the person's safety net outside the workspace, so agents never remove them",
    },
    cli: Some("backup delete <name>"),
    input_name: "BackupRef",
    output_name: "BackupDeleted",
});
