// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The handlers of the catalog's operations.

use gk_api::{OpDescriptor, Operation, descriptor};

use crate::registry::Registry;

pub(crate) mod agent;
pub(crate) mod backup;
pub(crate) mod board;
pub(crate) mod bulk;
pub(crate) mod claim;
pub(crate) mod comment;
pub(crate) mod custom_field;
pub(crate) mod description;
pub(crate) mod draft;
pub(crate) mod duplicate;
pub(crate) mod fields;
pub(crate) mod file;
pub(crate) mod gantt_layout;
pub(crate) mod history;
pub(crate) mod links;
pub(crate) mod list_layout;
pub(crate) mod lookup;
pub(crate) mod macros;
pub(crate) mod project;
pub(crate) mod project_delete;
pub(crate) mod query;
pub(crate) mod relation;
pub(crate) mod reminder;
pub(crate) mod rollup;
pub(crate) mod schedule;
pub(crate) mod search;
pub(crate) mod settings;
pub(crate) mod task;
pub(crate) mod time;
pub(crate) mod today;
pub(crate) mod transfer;
pub(crate) mod trash;
pub(crate) mod view;
pub(crate) mod wiki;
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
    project::register(r);
    custom_field::register(r);
    project_delete::register(r);
    task::register(r);
    duplicate::register(r);
    claim::register(r);
    relation::register(r);
    comment::register(r);
    description::register(r);
    draft::register(r);
    board::register(r);
    file::register(r);
    time::register(r);
    reminder::register(r);
    query::register(r);
    bulk::register(r);
    view::register(r);
    schedule::register(r);
    list_layout::register(r);
    gantt_layout::register(r);
    search::register(r);
    today::register(r);
    wiki::register(r);
    links::register(r);
    trash::register(r);
    backup::register(r);
    transfer::register(r);
}
