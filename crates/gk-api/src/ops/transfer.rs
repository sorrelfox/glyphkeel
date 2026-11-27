// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Export and import archives: projects, or the whole workspace, as plain
//! JSON and Markdown files with their attachments.
//!
//! An archive is a `.zip` file or a folder with the same layout. Import
//! creates new projects and never changes existing ones; a project key
//! that is taken needs a new one, and IDs that exist already are replaced.
//! An archive is checked completely before anything is written: its
//! format version, every path, size and hash, and every record.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};

/// How an archive is stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveFormat {
    /// One `.zip` file.
    #[default]
    Zip,
    /// A folder.
    Folder,
}

/// What an archive holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArchiveCounts {
    /// Projects, sub-projects included.
    pub projects: u64,
    /// Tasks, trashed ones included.
    pub tasks: u64,
    /// Comments.
    pub comments: u64,
    /// Time entries.
    pub time_entries: u64,
    /// Revisions of descriptions and wiki pages.
    pub revisions: u64,
    /// Wiki pages.
    pub wiki_pages: u64,
    /// Attachments.
    pub attachments: u64,
    /// Distinct stored files.
    pub files: u64,
    /// History rows.
    pub activity: u64,
    /// Agent sessions the history refers to.
    pub agent_sessions: u64,
}

/// What to export.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportInput {
    /// The projects (keys or IDs), each with its sub-projects; the whole
    /// workspace when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<String>,
    /// Where to write it: a new file or folder. For agents, a plain name
    /// goes into the workspace's `exports` folder; other paths must be
    /// inside a folder agents may use. Left out: a name with the date in
    /// the `exports` folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// A `.zip` file (the default) or a folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<ArchiveFormat>,
    /// Encrypt the archive (a `.zip` file) with the workspace's passphrase,
    /// so that it opens only in Glyphkeel with that passphrase. Left out:
    /// on when the workspace is encrypted and the archive is a `.zip` file
    /// or goes into the `exports` folder. Only an encrypted workspace can
    /// encrypt an export.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypt: Option<bool>,
}

/// A finished export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportResult {
    /// The archive written.
    pub path: String,
    /// How it is stored.
    pub format: ArchiveFormat,
    /// The keys of the projects in it.
    pub projects: Vec<String>,
    /// What it holds.
    pub counts: ArchiveCounts,
    /// Its size in bytes.
    pub size: u64,
    /// Whether it is encrypted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub encrypted: bool,
}

/// A new key for a project whose key is taken.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeyChange {
    /// The key in the archive.
    pub from: String,
    /// The key to use instead.
    pub to: String,
}

/// What to import.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportInput {
    /// The archive: a `.zip` file or a folder. For agents, a plain name is
    /// one in the workspace's `exports` folder; other paths must be inside
    /// a folder agents may use.
    pub path: String,
    /// New keys for projects whose key is taken in this workspace.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<KeyChange>,
    /// Check the archive and report what would be imported, without
    /// importing it.
    #[serde(default)]
    pub dry_run: bool,
}

/// One project of an import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportedProject {
    /// Its key in this workspace.
    pub key: String,
    /// Its key in the archive.
    pub from_key: String,
    /// Its name.
    pub name: String,
    /// Its ID in this workspace.
    pub id: String,
}

/// The result of an import, or of checking one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportResult {
    /// Whether this only checked the archive.
    pub dry_run: bool,
    /// The projects, as they are (or would be) in this workspace.
    pub projects: Vec<ImportedProject>,
    /// What the archive holds.
    pub counts: ArchiveCounts,
    /// Keys in the archive that are taken here and have no new key yet.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub taken_keys: Vec<String>,
    /// How many IDs existed here already and were replaced.
    pub new_ids: u64,
    /// The archive's format version.
    pub format_version: u32,
    /// The program version that wrote the archive.
    pub app_version: String,
    /// The history group, for revert and Undo (which moves the imported
    /// projects to the trash); left out for a check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// Write an archive.
    TransferExport,
    "transfer.export",
    ExportInput,
    ExportResult
);
op!(
    /// Read an archive into new projects.
    TransferImport,
    "transfer.import",
    ImportInput,
    ImportResult
);

pub(crate) const TRANSFER_EXPORT: OpDescriptor = OpDescriptor::of::<TransferExport>(Meta {
    summary: "Export projects, or the whole workspace, to an archive of JSON and Markdown files with the attachments.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("export"),
    input_name: "ExportInput",
    output_name: "ExportResult",
});

pub(crate) const TRANSFER_IMPORT: OpDescriptor = OpDescriptor::of::<TransferImport>(Meta {
    summary: "Import an archive as new projects; checks it completely first and never changes existing data.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("import <path>"),
    input_name: "ImportInput",
    output_name: "ImportResult",
});

/// The step an export or import is at, for a progress bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    /// Reading the workspace (export) or the archive (import).
    Reading,
    /// Checking the archive's paths, hashes and records.
    Checking,
    /// Copying attached files.
    Files,
    /// Writing the records.
    Writing,
    /// Building the search index of the imported items.
    Indexing,
    /// Finished.
    Done,
}

/// How far an export or import is: `done` of `total` units of `phase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransferProgress {
    /// The step.
    pub phase: TransferPhase,
    /// Units done in this step.
    pub done: u64,
    /// Units in this step.
    pub total: u64,
}
