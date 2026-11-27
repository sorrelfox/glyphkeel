// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Encrypting a workspace, changing its passphrase and decrypting it.
//!
//! These work on a closed workspace: the program closes its engine first
//! and opens it again afterwards, and no other program may have the
//! workspace open. Each change makes a backup first, so the workspace can
//! be put back as it was. After encrypting, the plain backups in the
//! workspace's backup folder are encrypted too, with the same passphrase;
//! after decrypting, the backups stay encrypted and restore with the
//! passphrase they were made with.
//!
//! The stored files (attachments) follow the database: encrypting seals
//! them with a new random file key kept inside the encrypted database, a
//! new passphrase brings a new file key (the old ones stay to open older
//! backups' files), and decrypting makes them plain again. Each file is
//! written anew, checked against its hash and renamed over the old one, so
//! an interruption leaves every file either as it was or converted; both
//! open. The thumbnail cache is cleared and made again as needed.

use std::path::Path;

use gk_api::OpError;
use gk_api::ops::backup::{Backup, BackupKind};
use gk_store::{Encryption, Passphrase, Store, WorkspaceLayout};
use serde::Serialize;

use crate::backup::{BackupFolder, default_folder};
use crate::failure::store_error;
use crate::messages;

/// What the program can tell about a workspace's encryption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Status {
    /// This build can encrypt.
    pub available: bool,
    /// The workspace is encrypted.
    pub encrypted: bool,
}

/// The encryption of the workspace at `layout`.
pub fn status(layout: &WorkspaceLayout) -> Result<Status, OpError> {
    let database = layout.database();
    let encrypted =
        gk_store::encryption_of(&database).map_err(|e| store_error(&e))? == Encryption::Encrypted;
    Ok(Status {
        available: gk_store::encryption_available(),
        encrypted,
    })
}

/// A change to a workspace's encryption.
#[derive(Debug, Clone)]
pub enum Change {
    /// Encrypt it with this passphrase, or give it this new one.
    Encrypt(Passphrase),
    /// Make it plain again.
    Decrypt,
}

/// What a change did.
#[derive(Debug, Clone, Serialize)]
pub struct Changed {
    /// The backup made just before.
    pub backup: Backup,
    /// Backups that stayed plain after encrypting, because encrypting them
    /// failed.
    pub backups_left_plain: u64,
    /// Stored files converted (sealed or made plain), in the workspace and
    /// its backup folder.
    pub files_converted: u64,
    /// Stored files left as they were, because no known key opens them or
    /// their content doesn't match their name.
    pub files_failed: u64,
}

/// Applies `change` to the closed workspace at `layout`. `backups` is the
/// backup folder the configuration fixed, if any; otherwise the
/// workspace's own setting or the default folder is used. `now` (ms) names
/// the backup made first.
pub fn change(
    layout: &WorkspaceLayout,
    backups: Option<&Path>,
    change: &Change,
    now: i64,
    program_version: &str,
) -> Result<Changed, OpError> {
    let database = layout.database();
    if !database.is_file() {
        return Err(store_error(&gk_store::StoreError::NotAWorkspace(database)));
    }
    if matches!(change, Change::Encrypt(_)) && !gk_store::encryption_available() {
        return Err(store_error(&gk_store::StoreError::EncryptionUnavailable));
    }
    if matches!(change, Change::Decrypt)
        && gk_store::encryption_of(&database).map_err(|e| store_error(&e))? == Encryption::Plain
    {
        return Err(messages::error(
            gk_api::ErrorCode::Invalid,
            "encryption-not-encrypted",
            &[],
        ));
    }
    gk_store::unlock_without_asking(&database).map_err(|e| store_error(&e))?;
    let folder = backup_folder(layout, backups);
    let backup = folder.create(layout, BackupKind::Safety, now, program_version)?;
    tracing::info!(name = %backup.name, "workspace backed up before changing its encryption");

    // Every file key the workspace keeps, so each stored file opens
    // whatever state an earlier, interrupted change left it in.
    let old_keys = gk_store::load_file_keys(layout).map_err(|e| store_error(&e))?;
    match change {
        Change::Encrypt(passphrase) => gk_store::encrypt_workspace(layout, passphrase),
        Change::Decrypt => gk_store::decrypt_workspace(layout),
    }
    .map_err(|e| store_error(&e))?;

    let mut files = gk_store::Converted::default();
    let mut add = |more: gk_store::Converted| {
        files.converted += more.converted;
        files.unchanged += more.unchanged;
        files.failed += more.failed;
    };
    let mut backups_left_plain = 0;
    let convert_failed = |e: std::io::Error| {
        messages::error(
            gk_api::ErrorCode::Storage,
            "encryption-files-failed",
            &[("error", e.to_string())],
        )
    };
    match change {
        Change::Encrypt(passphrase) => {
            let key = gk_store::FileKey::generate().map_err(convert_failed)?;
            let mut keys = vec![key.clone()];
            keys.extend(old_keys.iter().filter(|k| **k != key).cloned());
            gk_store::save_file_keys(layout, &keys).map_err(|e| store_error(&e))?;
            add(
                gk_store::convert_blobs(&layout.files_dir(), Some(&key), None)
                    .map_err(convert_failed)?,
            );
            // Plain backups are encrypted with the same passphrase and get
            // the file key, and the backup folder's stored files are sealed
            // with it (as links to the workspace's converted files where
            // they can be). Backups encrypted already keep their files and
            // keys: a new passphrase doesn't reach into them.
            let mut sealed_any = false;
            for item in folder.list() {
                let Ok((path, _)) = folder.database_of(&item.name) else {
                    continue;
                };
                let was_plain =
                    gk_store::encryption_of(&path).is_ok_and(|e| e == Encryption::Plain);
                if !was_plain {
                    continue;
                }
                if let Err(e) = gk_store::encrypt_file(&path, passphrase) {
                    tracing::warn!(name = %item.name, error = %e, "a backup could not be encrypted");
                    backups_left_plain += 1;
                    continue;
                }
                match gk_store::add_file_keys_to(&path, &keys) {
                    Ok(()) => sealed_any = true,
                    Err(e) => {
                        tracing::warn!(name = %item.name, error = %e, "a backup did not take the file key");
                    }
                }
            }
            if sealed_any {
                add(gk_store::convert_blobs(
                    &folder.blobs_dir(),
                    Some(&key),
                    Some(&layout.files_dir()),
                )
                .map_err(convert_failed)?);
            }
        }
        Change::Decrypt => {
            let made =
                gk_store::convert_blobs(&layout.files_dir(), None, None).map_err(convert_failed)?;
            // No key stays in a plain database unless a file still needs it.
            if made.failed == 0 {
                gk_store::save_file_keys(layout, &[]).map_err(|e| store_error(&e))?;
            }
            add(made);
        }
    }
    // Plain copies opened in other apps before the workspace was encrypted.
    if matches!(change, Change::Encrypt(_)) {
        let _ = std::fs::remove_dir_all(layout.cache_dir().join("open"));
    }
    if let Err(e) = gk_store::clear_thumbs(layout) {
        tracing::warn!(error = %e, "the thumbnail cache could not be cleared");
    }
    if files.failed > 0 {
        tracing::warn!(failed = files.failed, "stored files were left as they were");
    }
    Ok(Changed {
        backup,
        backups_left_plain,
        files_converted: files.converted,
        files_failed: files.failed,
    })
}

/// The message for backups that stayed plain, for the program to show, in
/// its language.
#[must_use]
pub fn backups_left_plain_text(count: u64) -> String {
    messages::ENGINE.format(
        "encryption-backups-left-plain",
        &[("count", messages::Arg::number(count))],
    )
}

/// The message for stored files a change left as they were, for the
/// program to show.
#[must_use]
pub fn files_left_text(count: u64) -> String {
    messages::text("encryption-files-left", &[("count", count.to_string())])
}

/// The backup folder of the closed workspace at `layout`.
fn backup_folder(layout: &WorkspaceLayout, fixed: Option<&Path>) -> BackupFolder {
    if let Some(fixed) = fixed {
        return BackupFolder::new(fixed);
    }
    let chosen = Store::probe_setting(layout, crate::backup::SETTING)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<Option<String>>(&text).ok())
        .flatten();
    BackupFolder::new(chosen.map_or_else(|| default_folder(layout), Into::into))
}
