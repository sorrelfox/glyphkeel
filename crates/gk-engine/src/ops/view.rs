// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Saved views. A view is kept whole in history (one `record` field), so
//! creating, changing and deleting one can all be undone and reverted.
//!
//! Views belong to the workspace: everyone using it sees the same ones.
//! The table can take an owner later, should the workspace ever have
//! several people; until then there is no per-person view.

use gk_api::OpError;
use gk_api::ops::view::{
    SavedView, ViewChange, ViewCreate, ViewCreateInput, ViewDelete, ViewDeleteInput, ViewField,
    ViewKind, ViewList, ViewListInput, ViewListOp, ViewUpdate, ViewUpdateInput,
};
use gk_core::{Id, Rank, fold};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::desc;
use super::lookup::{self, b, id_at, opt_id_at};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's name for saved views.
pub(crate) const ENTITY: &str = "view";
/// The one recorded field: the whole view.
pub(crate) const RECORD: &str = "record";
/// The longest view name, in characters.
const NAME_MAX: usize = 100;
/// The longest description, in characters.
const DESCRIPTION_MAX: usize = 1000;

pub(super) fn register(r: &mut Registry) {
    r.query::<ViewListOp, _>(desc::<ViewListOp>(), |q, input: ViewListInput| {
        list(q.tx, &input)
    });
    r.command::<ViewCreate, _>(desc::<ViewCreate>(), create);
    r.command::<ViewUpdate, _>(desc::<ViewUpdate>(), update);
    r.command::<ViewDelete, _>(desc::<ViewDelete>(), delete);
}

/// A view as history records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    kind: ViewKind,
    name: String,
    #[serde(default)]
    description: String,
    query: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    columns: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group_by: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    totals: Vec<String>,
    is_default: bool,
    rank: String,
    created_at: i64,
    updated_at: i64,
}

impl Record {
    fn project_id(&self) -> Option<Id> {
        self.project.as_deref().and_then(|p| p.parse().ok())
    }
}

const COLUMNS: &str = "id, project_id, view_kind, name, description, query, columns, sort, \
                       group_by, totals, is_default, rank, created_at, updated_at";

fn json_list(text: Option<String>) -> Vec<String> {
    text.and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn json_text(text: Option<String>) -> Option<String> {
    text.and_then(|t| serde_json::from_str::<String>(&t).ok())
}

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<(Id, Record)> {
    let kind: String = r.get(2)?;
    Ok((
        id_at(r, 0)?,
        Record {
            project: opt_id_at(r, 1)?.map(|p| p.to_string()),
            kind: match kind.as_str() {
                "board" => ViewKind::Board,
                "gantt" => ViewKind::Gantt,
                "calendar" => ViewKind::Calendar,
                _ => ViewKind::List,
            },
            name: r.get(3)?,
            description: r.get(4)?,
            query: r.get(5)?,
            columns: json_list(r.get(6)?),
            sort: json_text(r.get(7)?),
            group_by: json_text(r.get(8)?),
            totals: json_list(r.get(9)?),
            is_default: r.get(10)?,
            rank: r.get(11)?,
            created_at: r.get(12)?,
            updated_at: r.get(13)?,
        },
    ))
}

fn load(tx: &Transaction<'_>, id: Id) -> Result<Option<Record>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM saved_views WHERE id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?
        .map(|(_, r)| r))
}

/// The stored view `id` as history records it, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    load(tx, id)?
        .map(|r| serde_json::to_value(r).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

/// Writes view `id` as history recorded it (`None` deletes it), without
/// recording.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    w.tx()
        .execute("DELETE FROM saved_views WHERE id = ?1", [b(id)])?;
    let Some(value) = value else {
        return Ok(());
    };
    let r: Record = serde_json::from_value(value.clone())
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    let list = |v: &[String]| {
        (!v.is_empty()).then(|| serde_json::to_string(v).unwrap_or_else(|_| "[]".to_owned()))
    };
    let text = |v: &Option<String>| v.as_ref().map(|s| Value::from(s.as_str()).to_string());
    w.tx().execute(
        &format!(
            "INSERT INTO saved_views ({COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
        ),
        params![
            b(id),
            r.project_id().map(b),
            r.kind.as_str(),
            r.name,
            r.description,
            r.query,
            list(&r.columns),
            text(&r.sort),
            text(&r.group_by),
            list(&r.totals),
            r.is_default,
            r.rank,
            r.created_at,
            r.updated_at,
        ],
    )?;
    Ok(())
}

/// Writes view `id` and records the change.
fn set(w: &mut WriteCtx<'_>, id: Id, record: Option<&Record>) -> Result<(), Failure> {
    let before = raw(w.tx(), id)?;
    let after = record
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?;
    if before == after {
        return Ok(());
    }
    let project = record.and_then(Record::project_id).or_else(|| {
        before
            .as_ref()
            .and_then(|v| v.get("project"))
            .and_then(Value::as_str)
            .and_then(|p| p.parse().ok())
    });
    write_raw(w, id, after.as_ref())?;
    w.record_field(ENTITY, id, project, RECORD, before, after);
    Ok(())
}

fn project_key(tx: &Transaction<'_>, project: Option<Id>) -> Result<Option<String>, Failure> {
    Ok(match project {
        Some(p) => tx
            .query_row("SELECT key FROM projects WHERE id = ?1", [b(p)], |r| {
                r.get(0)
            })
            .optional()?,
        None => None,
    })
}

fn output(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<SavedView, Failure> {
    Ok(SavedView {
        name: r.name.clone(),
        kind: r.kind,
        project: project_key(tx, r.project_id())?,
        description: r.description.clone(),
        query: r.query.clone(),
        columns: r.columns.clone(),
        sort: r.sort.clone(),
        group_by: r.group_by.clone(),
        totals: r.totals.clone(),
        is_default: r.is_default,
        rank: r.rank.clone(),
        created_at: rfc3339(r.created_at),
        updated_at: rfc3339(r.updated_at),
        id: id.to_string(),
    })
}

/// Every view of `project` (`None`: the workspace's own), or of all.
fn rows(tx: &Transaction<'_>, project: Option<Option<Id>>) -> Result<Vec<(Id, Record)>, Failure> {
    let mut stmt = tx.prepare(&format!(
        "SELECT {COLUMNS} FROM saved_views ORDER BY view_kind, rank, id"
    ))?;
    let all = stmt
        .query_map([], read_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(match project {
        Some(p) => all
            .into_iter()
            .filter(|(_, r)| r.project_id() == p)
            .collect(),
        None => all,
    })
}

fn list(tx: &Transaction<'_>, input: &ViewListInput) -> Result<ViewList, Failure> {
    let scope = if input.all {
        None
    } else {
        Some(match &input.project {
            Some(p) => Some(lookup::project(tx, "project", p)?.id),
            None => None,
        })
    };
    let items = rows(tx, scope)?
        .into_iter()
        .filter(|(_, r)| input.kind.is_none_or(|k| k == r.kind))
        .map(|(id, r)| output(tx, id, &r))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ViewList {
        total: items.len() as u64,
        items,
    })
}

fn invalid(field: &str, e: impl gk_core::Message) -> Failure {
    super::task::invalid(field, e)
}

fn check_query(w: &WriteCtx<'_>, query: &str) -> Result<(), Failure> {
    super::query::compile(w.tx(), w.zone(), w.now(), query, "query").map(drop)
}

fn check_sort(w: &WriteCtx<'_>, sort: &str) -> Result<String, Failure> {
    super::query::sort_keys(w.tx(), w.zone(), w.now(), sort, "sort")?;
    Ok(sort.trim().to_owned())
}

fn clean_list(field: &str, items: &[String]) -> Result<Vec<String>, Failure> {
    items
        .iter()
        .map(|c| {
            let c = c.trim();
            if c.is_empty() || c.len() > 64 {
                Err(messages::invalid(
                    field,
                    "view-field-name",
                    &[("max", "64".to_owned())],
                ))
            } else {
                Ok(c.to_owned())
            }
        })
        .collect()
}

/// Refuses a name that another view of the same project and screen has.
fn check_unique(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), Failure> {
    let wanted = fold(&r.name);
    let taken = rows(tx, Some(r.project_id()))?
        .into_iter()
        .any(|(other, o)| other != id && o.kind == r.kind && fold(&o.name) == wanted);
    if taken {
        let error = messages::conflict("view-name-taken", &[("view", r.name.clone())])
            .with_detail("field", "name");
        return Err(messages::hint(error, "view-name-taken-hint", &[]).into());
    }
    Ok(())
}

/// Makes `id` the only default view of its project and screen.
fn single_default(w: &mut WriteCtx<'_>, id: Id, r: &Record) -> Result<(), Failure> {
    if !r.is_default {
        return Ok(());
    }
    for (other, mut o) in rows(w.tx(), Some(r.project_id()))? {
        if other != id && o.kind == r.kind && o.is_default {
            o.is_default = false;
            o.updated_at = w.now();
            set(w, other, Some(&o))?;
        }
    }
    Ok(())
}

fn create(w: &mut WriteCtx<'_>, input: ViewCreateInput) -> Result<ViewChange, Failure> {
    let name =
        gk_core::check_line("name", &input.name, NAME_MAX).map_err(|e| invalid("name", e))?;
    check_query(w, &input.query)?;
    let project = match &input.project {
        Some(p) => {
            let p = lookup::project(w.tx(), "project", p)?;
            lookup::ensure_active(&p)?;
            Some(p.id)
        }
        None => None,
    };
    let sort = input
        .sort
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| check_sort(w, s))
        .transpose()?;
    let description = match &input.description {
        Some(d) if !d.trim().is_empty() => gk_core::check_line("description", d, DESCRIPTION_MAX)
            .map_err(|e| invalid("description", e))?,
        _ => String::new(),
    };
    let kind = input.kind.unwrap_or_default();
    let last = rows(w.tx(), Some(project))?
        .into_iter()
        .filter(|(_, r)| r.kind == kind)
        .filter_map(|(_, r)| Rank::parse(&r.rank).ok())
        .max_by(|a, b| a.as_str().cmp(b.as_str()));
    let rank = last.map_or_else(Rank::first, |r| r.after());
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let record = Record {
        project: project.map(|p| p.to_string()),
        kind,
        name,
        description,
        query: input.query.trim().to_owned(),
        columns: clean_list("columns", &input.columns)?,
        sort,
        group_by: input
            .group_by
            .as_deref()
            .map(str::trim)
            .filter(|g| !g.is_empty())
            .map(str::to_owned),
        totals: clean_list("totals", &input.totals)?,
        is_default: input.is_default,
        rank: rank.as_str().to_owned(),
        created_at: w.now(),
        updated_at: w.now(),
    };
    check_unique(w.tx(), id, &record)?;
    single_default(w, id, &record)?;
    set(w, id, Some(&record))?;
    Ok(ViewChange {
        view: output(w.tx(), id, &record)?,
        deleted: false,
        group: w.group_if_changed(),
    })
}

/// Rewrites the query of every saved view of `project` (every view of the
/// workspace for `None`) that `rewrite` changes, in the running command,
/// so each change shows in the view's history and undoes with the rest.
/// Returns how many views changed.
pub(crate) fn rewrite_queries(
    w: &mut WriteCtx<'_>,
    project: Option<Id>,
    rewrite: impl Fn(&str) -> Option<String>,
) -> Result<usize, Failure> {
    let views: Vec<(Id, Record)> = match project {
        Some(p) => {
            let mut stmt = w.tx().prepare_cached(&format!(
                "SELECT {COLUMNS} FROM saved_views WHERE project_id = ?1 ORDER BY rank, id"
            ))?;
            stmt.query_map([b(p)], read_row)?
                .collect::<Result<Vec<_>, _>>()?
        }
        None => {
            let mut stmt = w.tx().prepare_cached(&format!(
                "SELECT {COLUMNS} FROM saved_views ORDER BY rank, id"
            ))?;
            stmt.query_map([], read_row)?
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    let mut changed = 0;
    for (id, mut record) in views {
        if let Some(query) = rewrite(&record.query) {
            record.query = query;
            record.updated_at = w.now();
            set(w, id, Some(&record))?;
            changed += 1;
        }
    }
    // Automation rules' conditions are queries too.
    super::automation::rewrite_conditions(w, project, &rewrite)?;
    // Boards' colour rules are saved queries too.
    super::board::rewrite_colour_rules(w, project, &rewrite)?;
    Ok(changed)
}

/// Like [`rewrite_queries`], for a change that also touches the views'
/// sort keys and columns: `query` rewrites the query and the sort keys
/// (given as `sort:` text); `column` renames a column (`Some(Some(new))`)
/// or takes it out (`Some(None)`). Returns how many views changed.
pub(crate) fn rewrite_views(
    w: &mut WriteCtx<'_>,
    project: Option<Id>,
    query: &dyn Fn(&str) -> Option<String>,
    column: &dyn Fn(&str) -> Option<Option<String>>,
) -> Result<usize, Failure> {
    let views: Vec<(Id, Record)> = {
        let (sql, arg) = match project {
            Some(p) => (
                format!(
                    "SELECT {COLUMNS} FROM saved_views WHERE project_id = ?1 ORDER BY rank, id"
                ),
                Some(b(p)),
            ),
            None => (
                format!("SELECT {COLUMNS} FROM saved_views WHERE ?1 IS NULL ORDER BY rank, id"),
                None,
            ),
        };
        let mut stmt = w.tx().prepare_cached(&sql)?;
        stmt.query_map([arg], read_row)?
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut changed = 0;
    for (id, mut record) in views {
        let mut touched = false;
        if let Some(q) = query(&record.query) {
            record.query = q;
            touched = true;
        }
        if let Some(sort) = record.sort.clone()
            && let Some(new) = query(&format!("sort:{sort}"))
        {
            let new = new.trim();
            let keys = new.strip_prefix("sort:").unwrap_or(new).trim();
            record.sort = (!keys.is_empty()).then(|| keys.to_owned());
            touched = true;
        }
        let mut columns = Vec::with_capacity(record.columns.len());
        for c in &record.columns {
            match column(c) {
                Some(Some(new)) => {
                    columns.push(new);
                    touched = true;
                }
                Some(None) => touched = true,
                None => columns.push(c.clone()),
            }
        }
        record.columns = columns;
        // A custom field the rows are grouped by is named like a column.
        if let Some(group) = record.group_by.clone()
            && let Some(new) = column(&group)
        {
            record.group_by = new;
            touched = true;
        }
        if touched {
            record.updated_at = w.now();
            set(w, id, Some(&record))?;
            changed += 1;
        }
    }
    super::board::rewrite_colour_rules(w, project, query)?;
    Ok(changed)
}

/// The names of the saved views `ids` that still exist, in that order.
pub(crate) fn names(tx: &Transaction<'_>, ids: &[Id]) -> Result<Vec<String>, Failure> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for id in ids {
        if seen.contains(id) {
            continue;
        }
        seen.push(*id);
        if let Some(r) = load(tx, *id)? {
            out.push(r.name);
        }
    }
    Ok(out)
}

/// The view `text` names: its ID, or its name among the views of
/// `project` (the workspace's own without one).
fn find(
    tx: &Transaction<'_>,
    text: &str,
    project: Option<&String>,
) -> Result<(Id, Record), Failure> {
    if let Ok(id) = text.trim().parse::<Id>()
        && let Some(r) = load(tx, id)?
        && r.project_id().is_none_or(crate::scope::sees)
    {
        return Ok((id, r));
    }
    let scope = match project {
        Some(p) => Some(lookup::project(tx, "project", p)?.id),
        None => None,
    };
    let wanted = fold(text.trim());
    let all = rows(tx, Some(scope))?;
    let mut found: Vec<(Id, Record)> = all
        .iter()
        .filter(|(_, r)| fold(&r.name) == wanted)
        .cloned()
        .collect();
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => {
            let names: Vec<&str> = all.iter().map(|(_, r)| r.name.as_str()).collect();
            let args = [("view", text.trim().to_owned())];
            let error = messages::with("view-not-found", &args, |m| OpError::not_found(m, &names))
                .with_detail("field", "view");
            Err(messages::hint(error, "view-not-found-hint", &[]).into())
        }
        _ => Err(messages::hint(
            messages::invalid_at(
                "view",
                "view-name-ambiguous",
                &[("view", text.trim().to_owned())],
            ),
            "view-name-ambiguous-hint",
            &[],
        )
        .into()),
    }
}

fn update(w: &mut WriteCtx<'_>, input: ViewUpdateInput) -> Result<ViewChange, Failure> {
    let (id, old) = find(w.tx(), &input.view, input.project.as_ref())?;
    let clears = |f: ViewField| input.clear.contains(&f);
    for (given, field, name) in [
        (
            input.description.is_some(),
            ViewField::Description,
            "description",
        ),
        (input.columns.is_some(), ViewField::Columns, "columns"),
        (input.sort.is_some(), ViewField::Sort, "sort"),
        (input.group_by.is_some(), ViewField::GroupBy, "group_by"),
        (input.totals.is_some(), ViewField::Totals, "totals"),
    ] {
        if given && clears(field) {
            return Err(messages::invalid(
                "clear",
                "field-given-and-cleared",
                &[("field", name.to_owned())],
            ));
        }
    }
    let mut r = old.clone();
    if let Some(name) = &input.name {
        r.name = gk_core::check_line("name", name, NAME_MAX).map_err(|e| invalid("name", e))?;
    }
    if let Some(query) = &input.query {
        check_query(w, query)?;
        r.query = query.trim().to_owned();
    }
    if let Some(d) = &input.description {
        r.description = if d.trim().is_empty() {
            String::new()
        } else {
            gk_core::check_line("description", d, DESCRIPTION_MAX)
                .map_err(|e| invalid("description", e))?
        };
    }
    if let Some(c) = &input.columns {
        r.columns = clean_list("columns", c)?;
    }
    if let Some(s) = &input.sort {
        r.sort = if s.trim().is_empty() {
            None
        } else {
            Some(check_sort(w, s)?)
        };
    }
    if let Some(g) = &input.group_by {
        r.group_by = Some(g.trim().to_owned()).filter(|g| !g.is_empty());
    }
    if let Some(t) = &input.totals {
        r.totals = clean_list("totals", t)?;
    }
    if let Some(d) = input.is_default {
        r.is_default = d;
    }
    if clears(ViewField::Description) {
        r.description.clear();
    }
    if clears(ViewField::Columns) {
        r.columns.clear();
    }
    if clears(ViewField::Sort) {
        r.sort = None;
    }
    if clears(ViewField::GroupBy) {
        r.group_by = None;
    }
    if clears(ViewField::Totals) {
        r.totals.clear();
    }
    if r != old {
        r.updated_at = w.now();
        check_unique(w.tx(), id, &r)?;
        single_default(w, id, &r)?;
        set(w, id, Some(&r))?;
    }
    Ok(ViewChange {
        view: output(w.tx(), id, &r)?,
        deleted: false,
        group: w.group_if_changed(),
    })
}

fn delete(w: &mut WriteCtx<'_>, input: ViewDeleteInput) -> Result<ViewChange, Failure> {
    let (id, old) = find(w.tx(), &input.view, input.project.as_ref())?;
    let view = output(w.tx(), id, &old)?;
    set(w, id, None)?;
    Ok(ViewChange {
        view,
        deleted: true,
        group: w.group_if_changed(),
    })
}
