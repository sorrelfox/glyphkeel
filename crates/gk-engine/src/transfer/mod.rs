// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Export and import archives (format version 1).
//!
//! ```text
//! manifest.json                      format, version, projects, counts, SHA-256 of every other file
//! agent_sessions.jsonl               agent sessions the history, comments and claims refer to
//! projects/<KEY>/project.json        project, statuses, types, labels, views, layouts
//! projects/<KEY>/tasks.jsonl         one task per line, with labels, relations, claim
//! projects/<KEY>/tasks/<KEY>-<n>.md  the description (no file when it is empty)
//! projects/<KEY>/comments.jsonl      comments, each with its earlier texts
//! projects/<KEY>/time.jsonl          time entries
//! projects/<KEY>/revisions.jsonl     description revisions of tasks and the project; wiki revisions
//! projects/<KEY>/wiki.jsonl          wiki pages: tree, ranks, slugs, current revision
//! projects/<KEY>/wiki/<path>.md      each page's current text, the tree as folders
//! projects/<KEY>/files.jsonl         attachments
//! projects/<KEY>/activity.jsonl      history rows
//! files/<sha256>                     attached files, once per distinct content
//! ```
//!
//! An import reads and checks the whole archive first ([`import`]): the
//! paths, sizes and hashes, then every record and the references between
//! them. Only then are the stored files copied and the records written, in
//! one transaction under one history group: undoing the import moves the
//! imported projects to the trash. The imported history rows are kept for
//! reading but can't be reverted, since their values may name items of the
//! workspace they came from.

mod csv;
mod custom_fields;
mod export;
mod import;
mod model;
mod sink;
mod source;
mod wiki_export;
mod wiki_html;
mod wiki_import;

pub(crate) use csv::register as register_csv;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use gk_api::OpError;
use gk_api::ops::transfer::{
    ArchiveFormat, ExportInput, ExportResult, ImportInput, ImportResult, TransferPhase,
    TransferProgress,
};

use crate::context::Context;
use crate::engine::Engine;
use crate::messages;

/// The archive layout's name in `manifest.json`.
pub const FORMAT_NAME: &str = "glyphkeel-archive";
/// The newest archive version this program reads and the one it writes.
pub const FORMAT_VERSION: u32 = 1;
/// The archive's project templates, which belong to the workspace rather
/// than to one project; left out when there are none.
pub(crate) const TEMPLATES: &str = "templates.jsonl";

/// A function that hears how far an export or import is.
pub type Progress = Arc<dyn Fn(TransferProgress) + Send + Sync>;

/// A progress function that ignores what it hears.
#[must_use]
pub fn no_progress() -> Progress {
    Arc::new(|_| {})
}

pub(crate) fn report(progress: &Progress, phase: TransferPhase, done: u64, total: u64) {
    progress(TransferProgress { phase, done, total });
}

/// How much an archive may hold before it is refused unread.
#[derive(Debug, Clone)]
pub(crate) struct Limits {
    /// Entries in the archive.
    pub(crate) max_entries: usize,
    /// Bytes of all entries, uncompressed.
    pub(crate) max_total: u64,
    /// The most an entry above `ratio_floor` bytes may expand.
    pub(crate) max_ratio: u64,
    pub(crate) ratio_floor: u64,
    /// `manifest.json`.
    pub(crate) max_manifest: u64,
    /// One `.json` or `.jsonl` file.
    pub(crate) max_json: u64,
    /// One line of a `.jsonl` file.
    pub(crate) max_line: usize,
    /// One Markdown file.
    pub(crate) max_markdown: u64,
    /// One attached file.
    pub(crate) max_file: u64,
    /// Records of all kinds.
    pub(crate) max_records: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 2_000_000,
            max_total: 64 << 30,
            max_ratio: 200,
            ratio_floor: 1 << 20,
            max_manifest: 256 << 20,
            max_json: 2 << 30,
            max_line: 8 << 20,
            max_markdown: 2 << 20,
            max_file: 1 << 30,
            max_records: 20_000_000,
        }
    }
}

/// Why an archive was refused. Each becomes an `invalid` error with its
/// own message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rejected {
    NotAnArchive(String),
    Unreadable(String),
    BadPath(String),
    Link(String),
    Duplicate(String),
    DuplicateEntries,
    TooManyEntries(usize),
    TooLarge(u64),
    EntryTooLarge(String, u64),
    Bomb(String),
    SizeMismatch(String),
    Missing(String),
    Unexpected(String),
    Format(String),
    Version(u32),
    Hash(String),
    Unlisted(String),
    Json(String, usize, String),
    LineTooLong(String, usize),
    /// Record `line` of the file at the path is invalid, as the message
    /// with this ID says.
    Invalid(String, usize, &'static str, Vec<(&'static str, String)>),
    Counts(String, u64, u64),
    TooManyRecords(u64),
    Encrypted(String),
}

impl Rejected {
    fn message(&self) -> (&'static str, Vec<(&'static str, String)>) {
        match self {
            Self::NotAnArchive(d) => ("transfer-not-an-archive", vec![("detail", d.clone())]),
            Self::Unreadable(d) => ("transfer-unreadable", vec![("detail", d.clone())]),
            Self::BadPath(p) => ("transfer-bad-path", vec![("path", p.clone())]),
            Self::Link(p) => ("transfer-link", vec![("path", p.clone())]),
            Self::Duplicate(p) => ("transfer-duplicate", vec![("path", p.clone())]),
            Self::DuplicateEntries => ("transfer-duplicate-entries", vec![]),
            Self::TooManyEntries(n) => ("transfer-too-many-entries", vec![("max", n.to_string())]),
            Self::TooLarge(n) => ("transfer-too-large", vec![("max", n.to_string())]),
            Self::EntryTooLarge(p, n) => (
                "transfer-entry-too-large",
                vec![("path", p.clone()), ("max", n.to_string())],
            ),
            Self::Bomb(p) => ("transfer-bomb", vec![("path", p.clone())]),
            Self::SizeMismatch(p) => ("transfer-size-mismatch", vec![("path", p.clone())]),
            Self::Missing(p) => ("transfer-missing", vec![("path", p.clone())]),
            Self::Unexpected(p) => ("transfer-unexpected", vec![("path", p.clone())]),
            Self::Format(f) => ("transfer-format", vec![("format", f.clone())]),
            Self::Version(v) => (
                if *v > FORMAT_VERSION {
                    "transfer-version-newer"
                } else {
                    "transfer-version-unknown"
                },
                vec![
                    ("version", v.to_string()),
                    ("supported", FORMAT_VERSION.to_string()),
                ],
            ),
            Self::Hash(p) => ("transfer-hash", vec![("path", p.clone())]),
            Self::Unlisted(p) => ("transfer-unlisted", vec![("path", p.clone())]),
            Self::Json(p, l, d) => (
                "transfer-json",
                vec![
                    ("path", p.clone()),
                    ("line", l.to_string()),
                    ("detail", d.clone()),
                ],
            ),
            Self::LineTooLong(p, l) => (
                "transfer-line-too-long",
                vec![("path", p.clone()), ("line", l.to_string())],
            ),
            Self::Invalid(p, l, id, extra) => {
                let mut args = vec![("path", p.clone()), ("line", l.to_string())];
                args.extend(extra.iter().cloned());
                (id, args)
            }
            Self::TooManyRecords(n) => ("transfer-too-many-records", vec![("max", n.to_string())]),
            Self::Encrypted(p) => ("transfer-encrypted", vec![("path", p.clone())]),
            Self::Counts(what, said, found) => (
                "transfer-counts",
                vec![
                    ("what", what.clone()),
                    ("said", said.to_string()),
                    ("found", found.to_string()),
                ],
            ),
        }
    }

    /// The error callers see.
    pub(crate) fn error(&self) -> OpError {
        let (id, args) = self.message();
        let mut e = messages::hint(
            messages::tag(OpError::invalid(messages::text(id, &args)), id, &args),
            "transfer-rejected-hint",
            &[],
        );
        for (k, v) in &args {
            if matches!(*k, "path" | "line") {
                e = e.with_detail(k, v.clone());
            }
        }
        e
    }
}

impl From<Rejected> for OpError {
    fn from(r: Rejected) -> Self {
        r.error()
    }
}

/// Whether `name` is a plain file name: what agents may give as a path.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':'])
        && !name.chars().any(char::is_control)
}

/// Where an agent's `path` is: a plain name in the workspace's `exports`
/// folder. The person's paths are taken as given.
fn resolve(engine: &Engine, ctx: &Context, path: &str, field: &str) -> Result<PathBuf, OpError> {
    if ctx.is_agent() {
        if !plain_name(path) {
            return Err(messages::invalid(field, "transfer-agent-path", &[]).into());
        }
        Ok(engine.store().layout().exports_dir().join(path))
    } else {
        Ok(PathBuf::from(path))
    }
}

impl Engine {
    /// Exports projects, or the whole workspace, to an archive, telling
    /// `progress` how far it is.
    pub fn export_archive(
        &self,
        ctx: &Context,
        input: &ExportInput,
        progress: &Progress,
    ) -> Result<ExportResult, OpError> {
        let format = input.format.unwrap_or_else(|| match &input.path {
            Some(p) if !p.ends_with(".zip") && !ctx.is_agent() => ArchiveFormat::Folder,
            _ => ArchiveFormat::Zip,
        });
        let target = match &input.path {
            Some(p) => resolve(self, ctx, p, "path")?,
            None => {
                let stamp = crate::time::rfc3339(self.now_ms())
                    .replace([':', '.'], "")
                    .replace('-', "");
                let name = match format {
                    ArchiveFormat::Zip => format!("export-{}.zip", &stamp[..15]),
                    ArchiveFormat::Folder => format!("export-{}", &stamp[..15]),
                };
                self.store().layout().exports_dir().join(name)
            }
        };
        if ctx.project_scope.is_some() {
            // Every project named must be within the caller's limit.
            self.store()
                .read(|tx| {
                    let _scope = crate::scope::enter(tx, ctx)?;
                    for p in &input.projects {
                        crate::ops::lookup::project(tx, "projects", p)?;
                    }
                    Ok::<_, crate::failure::Failure>(())
                })
                .map_err(OpError::from)?;
        }
        export::export(self, input, &target, format, progress)
    }

    /// Imports the archive `input.path` names as new projects (or, with
    /// `dry_run`, only checks it), telling `progress` how far it is.
    pub fn import_archive(
        &self,
        ctx: &Context,
        input: &ImportInput,
        progress: &Progress,
    ) -> Result<ImportResult, OpError> {
        if ctx.project_scope.is_some() {
            // An import makes new top-level projects, which are outside any
            // project a caller is limited to. Refused before the archive is
            // read, so that a dry run doesn't list the keys taken elsewhere.
            let refusal = self
                .store()
                .read(|tx| {
                    let _scope = crate::scope::enter(tx, ctx)?;
                    Ok::<_, crate::failure::Failure>(crate::scope::outside())
                })
                .map_err(OpError::from)?;
            return Err(OpError::from(refusal));
        }
        let path = resolve(self, ctx, &input.path, "path")?;
        import::import(self, ctx, input, &path, &Limits::default(), progress)
    }

    pub(crate) fn now_ms(&self) -> i64 {
        (self.clock)()
    }
}

/// The archive path of project `key`'s folder: the key, with a `_` in
/// front when Windows reserves it as a device name (`CON`, `NUL`, `COM1`…)
/// so that the archive can be unpacked there.
fn project_dir(key: &str) -> String {
    format!("projects/{}", gk_core::portable_file_name(key))
}

/// The folder archives written before device names were avoided used for
/// project `key`; still read.
fn plain_project_dir(key: &str) -> String {
    format!("projects/{key}")
}
