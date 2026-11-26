// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The person's layout of a project's Gantt chart: the zoom level, the
//! table's and its columns' widths, the grouping and the "also move
//! dependents" switch, kept per project in `view_settings` apart from the
//! workspace's settings and its saved views, like the list's layout.
//! Changes are recorded in history, so they can be reverted.

use gk_api::OpError;
use gk_api::ops::schedule::{
    GanttLayout, GanttLayoutChange, GanttLayoutGet, GanttLayoutGetInput, GanttLayoutUpdate,
    GanttLayoutUpdateInput,
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

/// History's name for Gantt layouts; the item is the project.
pub(crate) const ENTITY: &str = "gantt_layout";
/// The recorded field: the whole layout.
pub(crate) const FIELD: &str = "settings";
/// The view kind of Gantt layouts in `view_settings`.
const VIEW: &str = "gantt";
/// The columns of the chart's table whose width can be set.
const COLUMNS: [&str; 5] = ["key", "title", "start", "due", "progress"];
/// The narrowest and widest table, in pixels.
const TABLE_MIN: u32 = 200;
const TABLE_MAX: u32 = 1200;
/// The narrowest and widest column, in pixels.
const WIDTH_MIN: u32 = 24;
const WIDTH_MAX: u32 = 600;

pub(super) fn register(r: &mut Registry) {
    r.query::<GanttLayoutGet, _>(desc::<GanttLayoutGet>(), |q, input: GanttLayoutGetInput| {
        let project = lookup::project(q.tx, "project", &input.project)?;
        Ok(GanttLayoutChange {
            project: project.key,
            layout: load(q.tx, project.id)?,
            group: None,
        })
    });
    r.command::<GanttLayoutUpdate, _>(desc::<GanttLayoutUpdate>(), update);
}

/// The stored layout of project `id`'s chart: the JSON as written, or
/// `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    let text: Option<String> = tx
        .query_row(
            "SELECT settings FROM view_settings WHERE scope = ?1 AND view_kind = ?2",
            params![id.to_string(), VIEW],
            |r| r.get(0),
        )
        .optional()?;
    Ok(text.and_then(|t| serde_json::from_str(&t).ok()))
}

/// Writes the stored layout of project `id`'s chart (`None` removes it)
/// without recording it.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    match value {
        Some(v) => {
            w.tx().execute(
                "INSERT INTO view_settings (scope, view_kind, settings, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (scope, view_kind) DO UPDATE SET settings = excluded.settings,
                   updated_at = excluded.updated_at",
                params![id.to_string(), VIEW, v.to_string(), w.now()],
            )?;
        }
        None => {
            w.tx().execute(
                "DELETE FROM view_settings WHERE scope = ?1 AND view_kind = ?2",
                params![id.to_string(), VIEW],
            )?;
        }
    }
    Ok(())
}

/// The layout of project `id`'s chart; a stored value that doesn't read is
/// the default.
fn load(tx: &Transaction<'_>, id: Id) -> Result<GanttLayout, Failure> {
    Ok(raw(tx, id)?
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

fn update(
    w: &mut WriteCtx<'_>,
    input: GanttLayoutUpdateInput,
) -> Result<GanttLayoutChange, Failure> {
    let project = lookup::project(w.tx(), "project", &input.project)?;
    let id = project.id;
    let mut layout = if input.reset {
        GanttLayout::default()
    } else {
        load(w.tx(), id)?
    };
    if let Some(zoom) = input.zoom {
        layout.zoom = zoom;
    }
    if let Some(table) = input.table {
        if !(TABLE_MIN..=TABLE_MAX).contains(&table) {
            return Err(messages::invalid(
                "table",
                "gantt-layout-table",
                &[
                    ("min", TABLE_MIN.to_string()),
                    ("max", TABLE_MAX.to_string()),
                ],
            ));
        }
        layout.table = Some(table);
    }
    for (column, width) in &input.widths {
        if !COLUMNS.contains(&column.as_str()) {
            return Err(messages::invalid(
                &format!("widths.{column}"),
                "gantt-layout-column",
                &[("columns", COLUMNS.join(", "))],
            ));
        }
        if !(WIDTH_MIN..=WIDTH_MAX).contains(width) {
            return Err(messages::invalid(
                &format!("widths.{column}"),
                "gantt-layout-width",
                &[
                    ("min", WIDTH_MIN.to_string()),
                    ("max", WIDTH_MAX.to_string()),
                ],
            ));
        }
        layout.widths.insert(column.clone(), *width);
    }
    if let Some(group_by) = input.group_by {
        layout.group_by = group_by;
    }
    if let Some(on) = input.move_dependents {
        layout.move_dependents = on;
    }
    let before = raw(w.tx(), id)?;
    let after = if layout == GanttLayout::default() {
        None
    } else {
        Some(
            serde_json::to_value(&layout)
                .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?,
        )
    };
    if before != after {
        write_raw(w, id, after.as_ref())?;
        w.record_field(ENTITY, id, Some(id), FIELD, before, after);
    }
    Ok(GanttLayoutChange {
        project: project.key,
        layout,
        group: w.group_if_changed(),
    })
}
