// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The handlers of the catalog's operations.

use gk_api::{OpDescriptor, Operation, descriptor};

use crate::registry::Registry;

pub(crate) mod agent;
pub(crate) mod history;
pub(crate) mod settings;
pub(crate) mod workspace;

/// The catalog's descriptor of `O`.
fn desc<O: Operation>() -> &'static OpDescriptor {
    descriptor(O::ID).unwrap_or_else(|| panic!("{} is not in the catalog", O::ID))
}

pub(crate) fn register(r: &mut Registry) {
    workspace::register(r);
    settings::register(r);
    agent::register(r);
    history::register(r);
}
