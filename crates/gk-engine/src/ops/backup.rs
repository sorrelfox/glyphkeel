// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The backup operations: make one now, list, check, restore and delete.
//! The work is in `backup.rs` and in the engine (restoring needs its
//! locks).

use gk_api::ops::backup::{
    BackupCreate, BackupDelete, BackupDeleted, BackupKind, BackupList, BackupListOp, BackupRestore,
    BackupVerify,
};

use super::desc;
use crate::registry::Registry;
use crate::time::rfc3339;

pub(super) fn register(r: &mut Registry) {
    r.engine_op::<BackupCreate, _>(desc::<BackupCreate>(), |engine, _, _| {
        engine.create_backup(BackupKind::Manual)
    });
    r.engine_op::<BackupListOp, _>(desc::<BackupListOp>(), |engine, _, _| {
        let folder = engine.backups();
        let items = folder.list();
        Ok(BackupList {
            folder: folder.root().to_string_lossy().into_owned(),
            last_backup_at: folder
                .newest(&[
                    BackupKind::Daily,
                    BackupKind::Weekly,
                    BackupKind::Manual,
                    BackupKind::PreMigration,
                    BackupKind::Safety,
                    BackupKind::PreRestore,
                ])
                .map(rfc3339),
            items,
        })
    });
    r.engine_op::<BackupVerify, _>(desc::<BackupVerify>(), |engine, _, input| {
        engine.backups().verify(&input.name)
    });
    r.engine_op::<BackupRestore, _>(desc::<BackupRestore>(), |engine, ctx, input| {
        engine.restore_backup(ctx, &input.name)
    });
    r.engine_op::<BackupDelete, _>(desc::<BackupDelete>(), |engine, _, input| {
        let files_removed = engine.backups().delete(&input.name)?;
        Ok(BackupDeleted {
            name: input.name,
            files_removed,
        })
    });
}
