// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The export and import operations; the work is in `transfer/`. The app
//! calls [`crate::Engine::export_archive`] and
//! [`crate::Engine::import_archive`] directly to follow the progress.

use gk_api::ops::transfer::{TransferExport, TransferImport};

use super::desc;
use crate::registry::Registry;
use crate::transfer::no_progress;

pub(super) fn register(r: &mut Registry) {
    r.engine_op::<TransferExport, _>(desc::<TransferExport>(), |engine, ctx, input| {
        engine.export_archive(ctx, &input, &no_progress())
    });
    r.engine_op::<TransferImport, _>(desc::<TransferImport>(), |engine, ctx, input| {
        engine.import_archive(ctx, &input, &no_progress())
    });
}
