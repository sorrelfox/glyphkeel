// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Work outside single commands: backups and restoring them, purging the
//! trash, removing stored files nothing uses, and the daily jobs that run
//! them in the process holding the jobs lock.

use std::collections::HashSet;
use std::fs;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use gk_api::ops::backup::{Backup, BackupKind, BackupRestored};
use gk_api::ops::common::Via;
use gk_api::ops::trash::{TrashChange, TrashPurgeInput};
use gk_api::{ErrorCode, OpError, descriptor};
use gk_store::LockFile;
use serde_json::{Value, json};

use crate::backup::BackupFolder;
use crate::context::Context;
use crate::engine::{Engine, Role};
use crate::failure::{Failure, store_error};
use crate::messages;
use crate::ops::{file, trash};
use crate::registry::CommandFn;

/// How often a long-running process looks whether a daily job is due.
const CHECK_EVERY_MS: i64 = 10 * 60_000;
/// The age of the newest daily backup after which the next one is made.
const BACKUP_AFTER_MS: i64 = 24 * 3_600_000;
/// The same for a one-shot command, which backs up only when long-running
/// programs evidently haven't.
const CLI_BACKUP_AFTER_MS: i64 = 26 * 3_600_000;
/// Stored files younger than this are never removed as unused: a file is
/// stored before the row that uses it is committed.
const UNUSED_FILE_AGE: Duration = Duration::from_secs(3600);
/// Leftover temporary files of the file store older than this are removed.
const TEMP_FILE_AGE: Duration = Duration::from_secs(86_400);
/// How long a restore waits for other programs' write transactions.
const RESTORE_BUSY_BUDGET: Duration = Duration::from_secs(30);

/// What one run of the daily jobs did.
#[derive(Debug, Clone, Default)]
pub struct JobsReport {
    /// The daily backup, when one was due.
    pub backup: Option<Backup>,
    /// Items purged from the trash because their time was up.
    pub purged: u64,
    /// Stored files removed because nothing used them.
    pub unused_files: u64,
    /// Occurrences of recurring tasks made because their day came.
    pub occurrences: u64,
}

impl Engine {
    /// This workspace's backup folder: the one the configuration fixed,
    /// else the workspace's `backup_folder` setting, else the default. The
    /// setting is read each time, so a change by another program applies
    /// at once.
    #[must_use]
    pub fn backups(&self) -> BackupFolder {
        if self.backups_fixed {
            return self.backups.clone();
        }
        let chosen = self
            .store
            .read(|tx| Ok::<_, gk_store::StoreError>(crate::ops::settings::load(tx)?.backup_folder))
            .ok()
            .flatten();
        chosen.map_or_else(|| self.backups.clone(), BackupFolder::new)
    }

    fn now(&self) -> i64 {
        (self.clock)()
    }

    fn housekeeping(&self) -> Context {
        Context::user(match self.role {
            Role::App => Via::App,
            Role::Cli => Via::Cli,
            Role::Mcp => Via::Mcp,
        })
    }

    /// Makes a backup of kind `kind` now.
    pub fn create_backup(&self, kind: BackupKind) -> Result<Backup, OpError> {
        self.backups().create(
            self.store.layout(),
            kind,
            self.now(),
            &self.env.program_version,
        )
    }

    /// Whether the daily jobs should be looked at again.
    pub(crate) fn daily_jobs_due(&self) -> bool {
        let last = *self
            .last_daily
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        last.is_none_or(|t| self.now() - t >= CHECK_EVERY_MS)
    }

    /// Runs the daily jobs that are due: the daily backup when the newest
    /// one is a day old, the purge of items whose time in the trash is up,
    /// and the removal of stored files nothing uses. Long-running programs
    /// call it through [`Engine::jobs_tick`] while they hold the jobs lock.
    pub fn run_daily_jobs(&self) -> Result<JobsReport, OpError> {
        let now = self.now();
        *self
            .last_daily
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(now);
        let mut report = JobsReport::default();
        let newest = self
            .backups()
            .newest(&[BackupKind::Daily, BackupKind::Weekly]);
        if newest.is_none_or(|t| now - t >= BACKUP_AFTER_MS) {
            report.backup = Some(self.create_backup(BackupKind::Daily)?);
        }
        let ctx = self.housekeeping();
        let (change, unused) = self.purge_command(&ctx, None)?;
        report.purged = change.total;
        report.unused_files =
            file::remove_unused_files(&self.store, &unused).map_err(OpError::from)?;
        report.unused_files += self.sweep_unused_files()?;
        report.occurrences = self.make_occurrences(&ctx)?;
        Ok(report)
    }

    /// Runs `recurrence.run`: makes the occurrences of recurring tasks made
    /// ahead whose day has come. Returns how many were made.
    pub fn make_occurrences(&self, ctx: &Context) -> Result<u64, OpError> {
        let f: CommandFn = Arc::new(|w, _| {
            let run = crate::ops::recurrence::run(w)?;
            serde_json::to_value(run).map_err(|e| {
                Failure::Op(messages::storage(
                    "storage-json",
                    &[("error", e.to_string())],
                ))
            })
        });
        let desc =
            descriptor("recurrence.run").ok_or_else(|| OpError::storage("no recurrence.run"))?;
        let output = self.execute_command(ctx, desc, f, Value::Null)?;
        Ok(output
            .get("created")
            .and_then(Value::as_array)
            .map_or(0, |a| a.len() as u64))
    }

    /// For a one-shot command: makes the daily backup when the newest one
    /// is more than 26 hours old and no other program holds the jobs lock,
    /// so a person who only uses agents and the command line still gets
    /// daily backups.
    pub fn backup_if_overdue(&self) -> Result<Option<Backup>, OpError> {
        let newest = self
            .backups()
            .newest(&[BackupKind::Daily, BackupKind::Weekly]);
        if newest.is_some_and(|t| self.now() - t < CLI_BACKUP_AFTER_MS) {
            return Ok(None);
        }
        let Some(_lock) =
            LockFile::try_acquire(&self.store.layout().jobs_lock()).map_err(|e| store_error(&e))?
        else {
            return Ok(None);
        };
        self.create_backup(BackupKind::Daily).map(Some)
    }

    /// Purges trash entries for `trash.purge`: the command, then the stored
    /// files nobody uses any more.
    pub(crate) fn purge_trash(
        &self,
        ctx: &Context,
        input: TrashPurgeInput,
    ) -> Result<TrashChange, OpError> {
        let (change, unused) = self.purge_command(ctx, Some(input))?;
        file::remove_unused_files(&self.store, &unused).map_err(OpError::from)?;
        Ok(change)
    }

    /// Runs a purge as the command `trash.purge`: of the entries `input`
    /// names, or (`None`) of those whose time in the trash is up. Returns
    /// the result and the stored files to remove.
    fn purge_command(
        &self,
        ctx: &Context,
        input: Option<TrashPurgeInput>,
    ) -> Result<(TrashChange, Vec<String>), OpError> {
        let unused = Arc::new(Mutex::new(Vec::new()));
        let out = Arc::clone(&unused);
        let f: CommandFn = Arc::new(move |w, _| {
            let mut purged = trash::Purged::default();
            let change = match &input {
                Some(input) => trash::purge(w, input, &mut purged)?,
                None => trash::purge_expired(w, &mut purged)?,
            };
            *out.lock().unwrap_or_else(PoisonError::into_inner) = purged.unused_files;
            serde_json::to_value(change).map_err(|e| {
                Failure::Op(messages::storage(
                    "storage-json",
                    &[("error", e.to_string())],
                ))
            })
        });
        let desc = descriptor("trash.purge").ok_or_else(|| OpError::storage("no trash.purge"))?;
        let output = self.execute_command(ctx, desc, f, Value::Null)?;
        let change: TrashChange = serde_json::from_value(output)
            .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))?;
        let unused = std::mem::take(&mut *unused.lock().unwrap_or_else(PoisonError::into_inner));
        Ok((change, unused))
    }

    /// Removes stored files that no blob row knows and that are older than
    /// an hour, and leftover temporary files older than a day. Returns how
    /// many went.
    pub fn sweep_unused_files(&self) -> Result<u64, OpError> {
        let known: HashSet<String> = self
            .store
            .read(|tx| {
                let mut stmt = tx.prepare("SELECT sha256 FROM blobs")?;
                let rows = stmt
                    .query_map([], |r| r.get::<_, String>(0))?
                    .collect::<Result<HashSet<_>, _>>()?;
                Ok::<_, Failure>(rows)
            })
            .map_err(OpError::from)?;
        let layout = self.store.layout();
        let old = |path: &std::path::Path, age: Duration| {
            fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|d| d >= age)
        };
        let mut removed = 0;
        let Ok(dirs) = fs::read_dir(layout.files_dir()) else {
            return Ok(0);
        };
        for dir in dirs.flatten() {
            let is_temp = dir.file_name() == "tmp";
            let Ok(files) = fs::read_dir(dir.path()) else {
                continue;
            };
            for f in files.flatten() {
                let path = f.path();
                let name = f.file_name().to_string_lossy().into_owned();
                if is_temp {
                    if old(&path, TEMP_FILE_AGE) && fs::remove_file(&path).is_ok() {
                        removed += 1;
                    }
                } else if layout.blob_path(&name).is_some()
                    && !known.contains(&name)
                    && old(&path, UNUSED_FILE_AGE)
                    && gk_store::remove_file(layout, &name).is_ok()
                {
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    /// Replaces the workspace with the backup `name`, for the person only:
    /// after a confirmation, checks the backup, backs up the current state,
    /// puts the stored files back, brings the copy to this program's schema
    /// and copies it into the live database. Every program that had the
    /// workspace open, this one included, must restart afterwards.
    ///
    /// The command line refuses while the app has the workspace open; the
    /// app restores itself. The migrate lock keeps two restores (or a
    /// restore and an upgrade) apart.
    pub fn restore_backup(&self, ctx: &Context, name: &str) -> Result<BackupRestored, OpError> {
        let desc =
            descriptor("backup.restore").ok_or_else(|| OpError::storage("no backup.restore"))?;
        if ctx.is_agent() {
            return Err(messages::error(
                ErrorCode::Forbidden,
                "backup-restore-app-only",
                &[("op", desc.id.to_owned())],
            ));
        }
        let folder = self.backups();
        let (path, restored) = folder.database_of(name)?;
        self.ask_confirmation(ctx, desc, desc, &json!({ "name": name }))?;
        let layout = self.store.layout().clone();
        let _app = if self.role == Role::App {
            None
        } else {
            match LockFile::try_acquire(&layout.app_lock()).map_err(|e| store_error(&e))? {
                Some(lock) => Some(lock),
                None => {
                    return Err(messages::hint(
                        messages::tag(
                            OpError::new(
                                ErrorCode::Busy,
                                messages::text("backup-restore-app-open", &[]),
                            ),
                            "backup-restore-app-open",
                            &[],
                        ),
                        "backup-restore-app-open-hint",
                        &[],
                    ));
                }
            }
        };
        let _migrate = LockFile::acquire(&layout.migrate_lock()).map_err(|e| store_error(&e))?;
        let problems = gk_store::check_integrity(&path, true).map_err(|e| store_error(&e))?;
        if !problems.is_empty() {
            let args = [("name", name.to_owned())];
            return Err(messages::tag(
                OpError::conflict(messages::text("backup-restore-damaged", &args)),
                "backup-restore-damaged",
                &args,
            )
            .with_detail("problems", problems));
        }
        let missing = folder.missing_files(name)?;
        if !missing.is_empty() {
            let args = [
                ("name", name.to_owned()),
                ("count", missing.len().to_string()),
            ];
            return Err(messages::tag(
                OpError::conflict(messages::text("backup-restore-missing-files", &args)),
                "backup-restore-missing-files",
                &args,
            )
            .with_detail("missing", missing));
        }
        let previous = self.create_backup(BackupKind::PreRestore)?;
        folder.put_files_back(name, &layout)?;
        let copy = layout.root().join(format!(".restore-{}.db", self.now()));
        let result = (|| {
            fs::copy(&path, &copy).map_err(|e| {
                messages::error(
                    ErrorCode::Storage,
                    "backup-copy-failed",
                    &[
                        ("path", path.display().to_string()),
                        ("error", e.to_string()),
                    ],
                )
            })?;
            gk_store::upgrade_copy(&copy).map_err(|e| store_error(&e))?;
            gk_store::replace_database(&layout, &copy, RESTORE_BUSY_BUDGET)
                .map_err(|e| store_error(&e))
        })();
        let _ = fs::remove_file(&copy);
        let _ = fs::remove_file(copy.with_extension("db-journal"));
        result?;
        self.upgraded.store(true, Ordering::Release);
        tracing::info!(name, previous = %previous.name, "workspace restored from a backup");
        Ok(BackupRestored { restored, previous })
    }
}
