// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Backups of one workspace, in a folder of their own:
//!
//! ```text
//! <backups>/<workspace name>/
//!   2026-11-22T091200Z.db   snapshots (VACUUM INTO, integrity-checked)
//!   blobs/ab/abcdef…        stored files any snapshot refers to, once each
//!   backups.json            the list: name, kind, time, sizes
//!   backups.lock            held while the list or the folder changes
//! ```
//!
//! A snapshot is written to a temporary name, checked with SQLite's
//! integrity check and renamed. Stored files never change, so each is
//! copied once (a hard link on the same file system) and shared by all
//! snapshots; files no snapshot refers to any more are removed when
//! snapshots are rotated out. Rotation keeps the last 7 daily and 4 weekly
//! snapshots (a daily one that ages out becomes its week's weekly one when
//! that week has none), the last 3 of each other automatic kind, and every
//! manual one.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use gk_api::OpError;
use gk_api::ops::backup::{Backup, BackupCheck, BackupKind, BackupProblem, BackupProblemKind};
use gk_store::{LockFile, WorkspaceLayout};
use serde::{Deserialize, Serialize};

use crate::messages;
use crate::time::rfc3339;

/// The list file in a backup folder.
pub const LIST_FILE: &str = "backups.json";
const LOCK_FILE: &str = "backups.lock";
const BLOBS: &str = "blobs";
/// Daily snapshots kept.
pub const KEEP_DAILY: usize = 7;
/// Weekly snapshots kept.
pub const KEEP_WEEKLY: usize = 4;
/// Snapshots kept of each other automatic kind.
pub const KEEP_OTHER: usize = 3;
/// Most problems a check reports.
const MAX_PROBLEMS: usize = 100;
const DAY_MS: i64 = 86_400_000;

/// The backup folder of the workspace at `layout` when nothing else is set:
/// `<data>/backups/<name>` for a workspace in the data folder
/// (`<data>/workspaces/<name>`), else the workspace's own `backups` folder.
#[must_use]
pub fn default_folder(layout: &WorkspaceLayout) -> PathBuf {
    let root = layout.root();
    let parent = root.parent();
    let in_data_dir = parent
        .and_then(Path::file_name)
        .is_some_and(|n| n == "workspaces");
    match (in_data_dir, parent.and_then(Path::parent), root.file_name()) {
        (true, Some(data), Some(name)) => data.join("backups").join(name),
        _ => layout.backups_dir(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct List {
    format: u32,
    items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Item {
    name: String,
    file: String,
    kind: BackupKind,
    created_at: i64,
    size: u64,
    files: u64,
    files_size: u64,
    schema_version: i64,
    app_version: String,
}

impl Item {
    fn public(&self) -> Backup {
        Backup {
            name: self.name.clone(),
            kind: self.kind,
            created_at: rfc3339(self.created_at),
            size: self.size,
            files: self.files,
            files_size: self.files_size,
            schema_version: self.schema_version,
            app_version: self.app_version.clone(),
        }
    }
}

fn storage(e: impl std::fmt::Display) -> OpError {
    let args = [("error", e.to_string())];
    messages::tag(
        OpError::storage(messages::text("backup-storage", &args)),
        "backup-storage",
        &args,
    )
}

/// One workspace's backup folder.
#[derive(Debug, Clone)]
pub struct BackupFolder {
    root: PathBuf,
}

impl BackupFolder {
    /// The backup folder at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The folder.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn lock(&self) -> Result<LockFile, OpError> {
        fs::create_dir_all(&self.root).map_err(storage)?;
        LockFile::acquire(&self.root.join(LOCK_FILE)).map_err(storage)
    }

    fn blob_path(&self, sha256: &str) -> Option<PathBuf> {
        let ok = sha256.len() == 64
            && sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        ok.then(|| self.root.join(BLOBS).join(&sha256[..2]).join(sha256))
    }

    /// The list, newest first. A missing or unreadable list file is
    /// rebuilt from the snapshots in the folder.
    fn load(&self) -> Vec<Item> {
        let mut items = fs::read(self.root.join(LIST_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<List>(&bytes).ok())
            .map(|l| l.items)
            .unwrap_or_else(|| self.scan());
        items.retain(|i| self.root.join(&i.file).is_file());
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.name.cmp(&a.name)));
        items
    }

    /// The snapshots in the folder, for a list file that is gone.
    fn scan(&self) -> Vec<Item> {
        let Ok(dir) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in dir.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            let Some(name) = file.strip_suffix(".db") else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let created_at = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0));
            out.push(Item {
                name: name.to_owned(),
                file: file.clone(),
                kind: BackupKind::Manual,
                created_at,
                size: entry.metadata().map_or(0, |m| m.len()),
                files: snapshot_blobs(&path).map_or(0, |b| b.len() as u64),
                files_size: 0,
                schema_version: gk_store::schema_version_of(&path).unwrap_or(0),
                app_version: String::new(),
            });
        }
        out
    }

    fn save(&self, items: &[Item]) -> Result<(), OpError> {
        let list = List {
            format: 1,
            items: items.to_vec(),
        };
        let text = serde_json::to_vec_pretty(&list).map_err(storage)?;
        let tmp = self.root.join(format!(".{LIST_FILE}.part"));
        fs::write(&tmp, text).map_err(storage)?;
        fs::rename(&tmp, self.root.join(LIST_FILE)).map_err(storage)
    }

    /// The backups, newest first.
    #[must_use]
    pub fn list(&self) -> Vec<Backup> {
        self.load().iter().map(Item::public).collect()
    }

    /// When the newest backup of one of `kinds` was made.
    #[must_use]
    pub fn newest(&self, kinds: &[BackupKind]) -> Option<i64> {
        self.load()
            .iter()
            .filter(|i| kinds.contains(&i.kind))
            .map(|i| i.created_at)
            .max()
    }

    fn find(&self, name: &str) -> Result<Item, OpError> {
        self.load()
            .into_iter()
            .find(|i| i.name == name)
            .ok_or_else(|| {
                messages::hint(
                    messages::not_found("backup-not-found", &[("name", name.to_owned())]),
                    "backup-not-found-hint",
                    &[],
                )
            })
    }

    /// The database file of the backup `name`.
    pub(crate) fn database_of(&self, name: &str) -> Result<(PathBuf, Backup), OpError> {
        let item = self.find(name)?;
        Ok((self.root.join(&item.file), item.public()))
    }

    /// Makes a snapshot of the workspace at `layout` now (`now`, ms) and
    /// rotates the older ones.
    pub fn create(
        &self,
        layout: &WorkspaceLayout,
        kind: BackupKind,
        now: i64,
        app_version: &str,
    ) -> Result<Backup, OpError> {
        let _lock = self.lock()?;
        let mut items = self.load();
        let base = stamp(now);
        let mut name = base.clone();
        let mut n = 2;
        while items.iter().any(|i| i.name == name) || self.root.join(format!("{name}.db")).exists()
        {
            name = format!("{base}-{n}");
            n += 1;
        }
        let file = format!("{name}.db");
        let tmp = self.root.join(format!(".{file}.part"));
        let _ = fs::remove_file(&tmp);
        let made = (|| {
            gk_store::vacuum_into(&layout.database(), &tmp).map_err(storage)?;
            let problems = gk_store::check_integrity(&tmp, false).map_err(storage)?;
            if !problems.is_empty() {
                return Err(messages::tag(
                    OpError::storage(messages::text("backup-check-failed", &[])),
                    "backup-check-failed",
                    &[],
                )
                .with_detail("problems", problems));
            }
            Ok(())
        })();
        if let Err(e) = made {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        let blobs = snapshot_blobs(&tmp).map_err(storage)?;
        let mut files_size = 0;
        for (sha, size) in &blobs {
            files_size += size;
            let (Some(source), Some(target)) = (layout.blob_path(sha), self.blob_path(sha)) else {
                continue;
            };
            if !source.is_file() {
                tracing::warn!(sha256 = %sha, "a stored file is missing and was not backed up");
                continue;
            }
            gk_store::copy_blob(&source, &target).map_err(storage)?;
        }
        let schema_version = gk_store::schema_version_of(&tmp).map_err(storage)?;
        fs::rename(&tmp, self.root.join(&file)).map_err(storage)?;
        let item = Item {
            name,
            file: file.clone(),
            kind,
            created_at: now,
            size: fs::metadata(self.root.join(&file)).map_or(0, |m| m.len()),
            files: blobs.len() as u64,
            files_size,
            schema_version,
            app_version: app_version.to_owned(),
        };
        items.insert(0, item.clone());
        let removed = rotate(&mut items);
        for gone in &removed {
            let _ = fs::remove_file(self.root.join(&gone.file));
        }
        self.save(&items)?;
        if !removed.is_empty() {
            self.collect_blobs(&items);
        }
        tracing::info!(name = %item.name, kind = kind.as_str(), "backup made");
        Ok(item.public())
    }

    /// Removes stored files that no snapshot in `items` refers to. Returns
    /// how many went.
    fn collect_blobs(&self, items: &[Item]) -> u64 {
        let mut used = HashSet::new();
        for item in items {
            match snapshot_blobs(&self.root.join(&item.file)) {
                Ok(blobs) => used.extend(blobs.into_keys()),
                // A snapshot that can't be read keeps every file.
                Err(_) => return 0,
            }
        }
        let mut removed = 0;
        let Ok(dirs) = fs::read_dir(self.root.join(BLOBS)) else {
            return 0;
        };
        for dir in dirs.flatten() {
            let Ok(files) = fs::read_dir(dir.path()) else {
                continue;
            };
            for f in files.flatten() {
                let name = f.file_name().to_string_lossy().into_owned();
                if !used.contains(&name) && fs::remove_file(f.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        removed
    }

    /// Deletes the backup `name`, with the stored files only it used.
    pub fn delete(&self, name: &str) -> Result<u64, OpError> {
        let _lock = self.lock()?;
        let mut items = self.load();
        let Some(i) = items.iter().position(|i| i.name == name) else {
            return Err(self.find(name).err().unwrap_or_else(|| storage(name)));
        };
        let gone = items.remove(i);
        fs::remove_file(self.root.join(&gone.file)).map_err(storage)?;
        self.save(&items)?;
        Ok(self.collect_blobs(&items))
    }

    /// Checks the backup `name`: the database's full integrity check and
    /// every stored file by hash.
    pub fn verify(&self, name: &str) -> Result<BackupCheck, OpError> {
        let item = self.find(name)?;
        let path = self.root.join(&item.file);
        let mut problems = Vec::new();
        match gk_store::check_integrity(&path, false) {
            Ok(found) => problems.extend(found.into_iter().map(|detail| BackupProblem {
                kind: BackupProblemKind::Database,
                detail,
            })),
            Err(e) => problems.push(BackupProblem {
                kind: BackupProblemKind::Database,
                detail: e.to_string(),
            }),
        }
        let mut files_checked = 0;
        if let Ok(blobs) = snapshot_blobs(&path) {
            for (sha, size) in blobs {
                let Some(file) = self.blob_path(&sha) else {
                    continue;
                };
                match gk_store::sha256_file(&file) {
                    Ok(found) if found.sha256 == sha && found.size == size => {}
                    Ok(_) => problems.push(BackupProblem {
                        kind: BackupProblemKind::DamagedFile,
                        detail: sha,
                    }),
                    Err(_) => problems.push(BackupProblem {
                        kind: BackupProblemKind::MissingFile,
                        detail: sha,
                    }),
                }
                files_checked += 1;
            }
        }
        problems.truncate(MAX_PROBLEMS);
        Ok(BackupCheck {
            backup: item.public(),
            ok: problems.is_empty(),
            problems,
            files_checked,
        })
    }

    /// The stored files of backup `name` that are missing from the backup
    /// folder.
    pub(crate) fn missing_files(&self, name: &str) -> Result<Vec<String>, OpError> {
        let (path, _) = self.database_of(name)?;
        let blobs = snapshot_blobs(&path).map_err(storage)?;
        Ok(blobs
            .into_iter()
            .filter(|(sha, size)| {
                !self
                    .blob_path(sha)
                    .and_then(|p| fs::metadata(p).ok())
                    .is_some_and(|m| m.len() == *size)
            })
            .map(|(sha, _)| sha)
            .collect())
    }

    /// Copies the stored files backup `name` refers to into the workspace
    /// at `layout` where they are missing.
    pub(crate) fn put_files_back(
        &self,
        name: &str,
        layout: &WorkspaceLayout,
    ) -> Result<u64, OpError> {
        let (path, _) = self.database_of(name)?;
        let mut copied = 0;
        for sha in snapshot_blobs(&path).map_err(storage)?.into_keys() {
            let (Some(source), Some(target)) = (self.blob_path(&sha), layout.blob_path(&sha))
            else {
                continue;
            };
            if gk_store::copy_blob(&source, &target).map_err(storage)?
                != gk_store::BlobCopy::Present
            {
                copied += 1;
            }
        }
        Ok(copied)
    }
}

/// The stored files the database file at `path` refers to, with their
/// sizes.
fn snapshot_blobs(path: &Path) -> Result<BTreeMap<String, u64>, gk_store::StoreError> {
    let conn = gk_store::rusqlite::Connection::open_with_flags(
        path,
        gk_store::rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | gk_store::rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut stmt = conn.prepare("SELECT sha256, size FROM blobs")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                u64::try_from(r.get::<_, i64>(1)?).unwrap_or(0),
            ))
        })?
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(rows)
}

/// A snapshot's name from its time: `2026-11-22T091200Z`.
fn stamp(ms: i64) -> String {
    let text = rfc3339(ms);
    // 2026-11-22T09:12:00.000Z
    format!("{}{}{}Z", &text[..13], &text[14..16], &text[17..19])
}

/// The week (Monday to Sunday, UTC) that moment `ms` falls in, as a count
/// of weeks.
fn week_of(ms: i64) -> i64 {
    // 1970-01-01 was a Thursday.
    (ms.div_euclid(DAY_MS) + 3).div_euclid(7)
}

/// Applies the keep rules to `items` (newest first) and returns what goes.
fn rotate(items: &mut Vec<Item>) -> Vec<Item> {
    let mut removed = Vec::new();
    let mut daily = 0;
    let mut weeks_with_weekly: HashSet<i64> = items
        .iter()
        .filter(|i| i.kind == BackupKind::Weekly)
        .map(|i| week_of(i.created_at))
        .collect();
    for item in items.iter_mut() {
        if item.kind != BackupKind::Daily {
            continue;
        }
        daily += 1;
        if daily > KEEP_DAILY && weeks_with_weekly.insert(week_of(item.created_at)) {
            item.kind = BackupKind::Weekly;
        }
    }
    let mut seen: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut keep = Vec::with_capacity(items.len());
    for item in items.drain(..) {
        let limit = match item.kind {
            BackupKind::Manual => usize::MAX,
            BackupKind::Daily => KEEP_DAILY,
            BackupKind::Weekly => KEEP_WEEKLY,
            BackupKind::PreMigration | BackupKind::Safety | BackupKind::PreRestore => KEEP_OTHER,
        };
        let n = seen.entry(item.kind.as_str()).or_insert(0);
        *n += 1;
        if *n > limit {
            removed.push(item);
        } else {
            keep.push(item);
        }
    }
    *items = keep;
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: BackupKind, day: i64) -> Item {
        Item {
            name: format!("d{day}"),
            file: format!("d{day}.db"),
            kind,
            created_at: day * DAY_MS + 9 * 3_600_000,
            size: 0,
            files: 0,
            files_size: 0,
            schema_version: 1,
            app_version: String::new(),
        }
    }

    #[test]
    fn rotation_keeps_seven_daily_four_weekly_and_every_manual() {
        // 60 days of daily backups (day 0 is a Thursday), newest first, and
        // a few others.
        let mut items: Vec<Item> = (0..60).rev().map(|d| item(BackupKind::Daily, d)).collect();
        items.push(item(BackupKind::Manual, 1));
        for d in [2, 3, 4, 5] {
            items.push(item(BackupKind::Safety, d));
        }
        items.sort_by_key(|i| std::cmp::Reverse(i.created_at));
        let removed = rotate(&mut items);
        let names = |k: BackupKind| -> Vec<String> {
            items
                .iter()
                .filter(|i| i.kind == k)
                .map(|i| i.name.clone())
                .collect()
        };
        assert_eq!(
            names(BackupKind::Daily),
            ["d59", "d58", "d57", "d56", "d55", "d54", "d53"]
        );
        // The newest aged-out daily of each week becomes its weekly one.
        let weekly = names(BackupKind::Weekly);
        assert_eq!(weekly.len(), KEEP_WEEKLY);
        let weeks: HashSet<i64> = items
            .iter()
            .filter(|i| i.kind == BackupKind::Weekly)
            .map(|i| week_of(i.created_at))
            .collect();
        assert_eq!(weeks.len(), KEEP_WEEKLY);
        assert_eq!(weekly[0], "d52");
        assert_eq!(names(BackupKind::Manual), ["d1"]);
        assert_eq!(names(BackupKind::Safety), ["d5", "d4", "d3"]);
        assert_eq!(items.len() + removed.len(), 65);
        // Rotating again changes nothing.
        assert!(rotate(&mut items).is_empty());
    }

    #[test]
    fn names_carry_the_time() {
        assert_eq!(stamp(1_795_000_000_000), "2026-11-18T110640Z");
        assert_eq!(week_of(0), week_of(3 * DAY_MS));
        assert_ne!(week_of(3 * DAY_MS), week_of(4 * DAY_MS));
    }

    #[test]
    fn the_default_folder_sits_next_to_the_workspaces() {
        let named = WorkspaceLayout::new("/data/glyphkeel/workspaces/Personal");
        assert_eq!(
            default_folder(&named),
            PathBuf::from("/data/glyphkeel/backups/Personal")
        );
        let elsewhere = WorkspaceLayout::new("/tmp/w");
        assert_eq!(default_folder(&elsewhere), PathBuf::from("/tmp/w/backups"));
    }
}
