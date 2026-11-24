// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The person's layout of a list: the order of its columns, their widths
//! and the groups folded away, kept per project in `view_settings` (the whole workspace's list
//! under the scope `all`), apart from the workspace's settings and its
//! saved views. Changes are recorded in history like the board's layout,
//! so they can be reverted.

use gk_api::OpError;
use gk_api::ops::query::ListGrouping;
use gk_api::ops::view::{
    ListLayout, ListLayoutChange, ListLayoutGet, ListLayoutGetInput, ListLayoutUpdate,
    ListLayoutUpdateInput,
};
use gk_core::Id;
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use super::desc;
use super::lookup;
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::write::WriteCtx;

/// History's name for list layouts; the item is the project, or the nil ID
/// for the whole workspace's list.
pub(crate) const ENTITY: &str = "list_layout";
/// The recorded field: the whole layout.
pub(crate) const FIELD: &str = "settings";
/// The view kind of list layouts in `view_settings`.
const VIEW: &str = "list";
/// The scope of the whole workspace's list.
const ALL: &str = "all";
/// The longest column name, in bytes.
const COLUMN_MAX: usize = 64;
/// The most columns a layout names.
const COLUMNS_MAX: usize = 64;
/// The most folded groups kept per grouping.
const COLLAPSED_MAX: usize = 500;
/// The longest group value kept, in characters.
const VALUE_MAX: usize = 500;
/// The narrowest and widest column, in pixels.
const WIDTH_MIN: u32 = 40;
const WIDTH_MAX: u32 = 1200;

pub(super) fn register(r: &mut Registry) {
    r.query::<ListLayoutGet, _>(desc::<ListLayoutGet>(), |q, input: ListLayoutGetInput| {
        let (project, id) = target(q.tx, input.project.as_deref())?;
        Ok(ListLayoutChange {
            project,
            layout: load(q.tx, id)?,
            group: None,
        })
    });
    r.command::<ListLayoutUpdate, _>(desc::<ListLayoutUpdate>(), update);
}

/// The project's key and ID, or none and the nil ID for the workspace.
fn target(tx: &Transaction<'_>, project: Option<&str>) -> Result<(Option<String>, Id), Failure> {
    match project {
        Some(p) => {
            let row = lookup::project(tx, "project", p)?;
            Ok((Some(row.key), row.id))
        }
        None => Ok((None, Id::from_bytes([0; 16]))),
    }
}

fn scope(id: Id) -> String {
    if id == Id::from_bytes([0; 16]) {
        ALL.to_owned()
    } else {
        id.to_string()
    }
}

/// The stored layout of the list `id` (a project, or the nil ID): the JSON
/// as written, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    let text: Option<String> = tx
        .query_row(
            "SELECT settings FROM view_settings WHERE scope = ?1 AND view_kind = ?2",
            params![scope(id), VIEW],
            |r| r.get(0),
        )
        .optional()?;
    Ok(text.and_then(|t| serde_json::from_str(&t).ok()))
}

/// Writes the stored layout of the list `id` (`None` removes it) without
/// recording it.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    match value {
        Some(v) => {
            w.tx().execute(
                "INSERT INTO view_settings (scope, view_kind, settings, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (scope, view_kind) DO UPDATE SET settings = excluded.settings,
                   updated_at = excluded.updated_at",
                params![scope(id), VIEW, v.to_string(), w.now()],
            )?;
        }
        None => {
            w.tx().execute(
                "DELETE FROM view_settings WHERE scope = ?1 AND view_kind = ?2",
                params![scope(id), VIEW],
            )?;
        }
    }
    Ok(())
}

/// The layout of the list `id`; a stored value that doesn't read is the
/// default.
fn load(tx: &Transaction<'_>, id: Id) -> Result<ListLayout, Failure> {
    Ok(raw(tx, id)?
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

fn column_name(field: &str, name: &str) -> Result<String, Failure> {
    let name = name.trim();
    if name.is_empty() || name.len() > COLUMN_MAX {
        return Err(messages::invalid(
            field,
            "view-field-name",
            &[("max", COLUMN_MAX.to_string())],
        ));
    }
    Ok(name.to_owned())
}

fn update(w: &mut WriteCtx<'_>, input: ListLayoutUpdateInput) -> Result<ListLayoutChange, Failure> {
    let (project, id) = target(w.tx(), input.project.as_deref())?;
    let mut layout = if input.reset {
        ListLayout::default()
    } else {
        load(w.tx(), id)?
    };
    if let Some(columns) = &input.columns {
        if columns.len() > COLUMNS_MAX {
            return Err(messages::invalid(
                "columns",
                "list-layout-columns",
                &[("max", COLUMNS_MAX.to_string())],
            ));
        }
        let mut chosen: Vec<String> = Vec::new();
        for (i, c) in columns.iter().enumerate() {
            let c = column_name(&format!("columns[{i}]"), c)?;
            if !chosen.contains(&c) {
                chosen.push(c);
            }
        }
        layout.columns = chosen;
    }
    for (column, width) in &input.widths {
        let column = column_name("widths", column)?;
        if !(WIDTH_MIN..=WIDTH_MAX).contains(width) {
            return Err(messages::invalid(
                &format!("widths.{column}"),
                "list-layout-width",
                &[
                    ("min", WIDTH_MIN.to_string()),
                    ("max", WIDTH_MAX.to_string()),
                ],
            ));
        }
        layout.widths.insert(column, *width);
    }
    for (grouping, values) in &input.collapsed {
        let path = format!("collapsed.{grouping}");
        if serde_json::from_value::<ListGrouping>(Value::String(grouping.clone())).is_err() {
            return Err(messages::invalid(&path, "list-layout-grouping", &[]));
        }
        if values.len() > COLLAPSED_MAX || values.iter().any(|v| v.chars().count() > VALUE_MAX) {
            return Err(messages::invalid(
                &path,
                "list-layout-collapsed",
                &[
                    ("max", COLLAPSED_MAX.to_string()),
                    ("length", VALUE_MAX.to_string()),
                ],
            ));
        }
        let mut kept: Vec<String> = Vec::new();
        for value in values {
            if !kept.contains(value) {
                kept.push(value.clone());
            }
        }
        if kept.is_empty() {
            layout.collapsed.remove(grouping);
        } else {
            layout.collapsed.insert(grouping.clone(), kept);
        }
    }
    if layout.widths.len() > COLUMNS_MAX {
        return Err(messages::invalid(
            "widths",
            "list-layout-columns",
            &[("max", COLUMNS_MAX.to_string())],
        ));
    }
    let before = raw(w.tx(), id)?;
    let after = if layout == ListLayout::default() {
        None
    } else {
        Some(
            serde_json::to_value(&layout)
                .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?,
        )
    };
    if before != after {
        write_raw(w, id, after.as_ref())?;
        let owner = (scope(id) != ALL).then_some(id);
        w.record_field(ENTITY, id, owner, FIELD, before, after);
    }
    Ok(ListLayoutChange {
        project,
        layout,
        group: w.group_if_changed(),
    })
}
