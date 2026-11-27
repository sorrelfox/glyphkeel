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
//! The stored files (attachments) are not encrypted.

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

    match change {
        Change::Encrypt(passphrase) => gk_store::encrypt_workspace(layout, passphrase),
        Change::Decrypt => gk_store::decrypt_workspace(layout),
    }
    .map_err(|e| store_error(&e))?;

    let mut backups_left_plain = 0;
    if let Change::Encrypt(passphrase) = change {
        for item in folder.list() {
            let Ok((path, _)) = folder.database_of(&item.name) else {
                continue;
            };
            if let Err(e) = gk_store::encrypt_file(&path, passphrase) {
                tracing::warn!(name = %item.name, error = %e, "a backup could not be encrypted");
                backups_left_plain += 1;
            }
        }
    }
    Ok(Changed {
        backup,
        backups_left_plain,
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
