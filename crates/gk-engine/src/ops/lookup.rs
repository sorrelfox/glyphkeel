// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Finding projects, statuses, types, labels and tasks by what callers type
//! (keys, old keys, names ignoring case, IDs), and placing items in ordered
//! lists by rank.

use gk_api::OpError;
use gk_api::ops::common::Edge;
use gk_core::{Id, ProjectKey, Rank, TaskKey, fold};
use gk_store::rusqlite::{OptionalExtension, Row, Transaction, params};

use crate::failure::Failure;
use crate::messages;

/// The ID in column `i`.
pub(crate) fn id_at(r: &Row<'_>, i: usize) -> gk_store::rusqlite::Result<Id> {
    let bytes: Vec<u8> = r.get(i)?;
    Id::from_slice(&bytes).map_err(|e| {
        gk_store::rusqlite::Error::FromSqlConversionFailure(
            i,
            gk_store::rusqlite::types::Type::Blob,
            Box::new(e),
        )
    })
}

/// The ID in column `i`, if it has one.
pub(crate) fn opt_id_at(r: &Row<'_>, i: usize) -> gk_store::rusqlite::Result<Option<Id>> {
    let bytes: Option<Vec<u8>> = r.get(i)?;
    Ok(bytes.and_then(|b| Id::from_slice(&b).ok()))
}

/// The bytes of `id`, for a query parameter.
pub(crate) fn b(id: Id) -> Vec<u8> {
    id.as_bytes().to_vec()
}

/// What the engine needs of a project row.
#[derive(Debug, Clone)]
pub(crate) struct ProjectRow {
    pub(crate) id: Id,
    pub(crate) parent: Option<Id>,
    pub(crate) key: String,
    pub(crate) archived: bool,
}

const PROJECT_COLUMNS: &str = "id, parent_id, key, state";

fn project_row(r: &Row<'_>) -> gk_store::rusqlite::Result<ProjectRow> {
    Ok(ProjectRow {
        id: id_at(r, 0)?,
        parent: opt_id_at(r, 1)?,
        key: r.get(2)?,
        archived: r.get::<_, String>(3)? == "archived",
    })
}

/// The live project with ID `id`.
/// A caller limited to a project finds only that project and the ones
/// below it.
pub(crate) fn project_by_id(tx: &Transaction<'_>, id: Id) -> Result<Option<ProjectRow>, Failure> {
    if !crate::scope::allows(tx, id)? {
        return Ok(None);
    }
    Ok(tx
        .prepare_cached(&format!(
            "SELECT {PROJECT_COLUMNS} FROM projects WHERE id = ?1 AND deleted_at IS NULL"
        ))?
        .query_row([b(id)], project_row)
        .optional()?)
}

/// The live project `text` names: its key (current or old, any case) or
/// its ID. `field` names the input field in the error.
pub(crate) fn project(
    tx: &Transaction<'_>,
    field: &str,
    text: &str,
) -> Result<ProjectRow, Failure> {
    let text = text.trim();
    if let Ok(id) = text.parse::<Id>()
        && let Some(p) = project_by_id(tx, id)?
    {
        return Ok(p);
    }
    if let Ok(key) = ProjectKey::parse_lenient(text) {
        let found = tx
            .prepare_cached(&format!(
                "SELECT {PROJECT_COLUMNS} FROM projects WHERE key = ?1 AND deleted_at IS NULL"
            ))?
            .query_row([key.as_str()], project_row)
            .optional()?;
        if let Some(p) = found
            && crate::scope::allows(tx, p.id)?
        {
            return Ok(p);
        }
        let alias: Option<Vec<u8>> = tx
            .prepare_cached("SELECT project_id FROM project_key_aliases WHERE key = ?1")?
            .query_row([key.as_str()], |r| r.get(0))
            .optional()?;
        if let Some(id) = alias.and_then(|b| Id::from_slice(&b).ok())
            && let Some(p) = project_by_id(tx, id)?
        {
            return Ok(p);
        }
    }
    let mut stmt = tx.prepare_cached(
        "SELECT id, key FROM projects WHERE deleted_at IS NULL ORDER BY key LIMIT 50",
    )?;
    let keys = stmt
        .query_map([], |r| Ok((id_at(r, 0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(id, _)| crate::scope::sees(*id))
        .map(|(_, key)| key)
        .collect::<Vec<_>>();
    let args = [("project", text.to_owned())];
    let error = messages::with("lookup-project-not-found", &args, |m| {
        OpError::not_found(m, &keys.iter().map(String::as_str).collect::<Vec<_>>())
    })
    .with_detail("field", field);
    Err(messages::hint(error, "lookup-project-not-found-hint", &[]).into())
}

/// Refuses changes to an archived project.
pub(crate) fn ensure_active(p: &ProjectRow) -> Result<(), Failure> {
    if p.archived {
        let error = messages::forbidden("project-archived", &[("project", p.key.clone())]);
        Err(messages::hint(error, "project-archived-hint", &[]).into())
    } else {
        Ok(())
    }
}

/// Refuses changes to the wiki of a project that has its wiki switched off.
pub(crate) fn ensure_wiki_on(tx: &Transaction<'_>, p: &ProjectRow) -> Result<(), Failure> {
    let modules: String = tx.query_row(
        "SELECT modules FROM projects WHERE id = ?1",
        [b(p.id)],
        |r| r.get(0),
    )?;
    if super::project::modules_of(&modules).wiki {
        Ok(())
    } else {
        let error = messages::forbidden("project-wiki-off", &[("project", p.key.clone())]);
        Err(messages::hint(error, "project-wiki-off-hint", &[]).into())
    }
}

/// A status, type or label row: the parts lookups need.
#[derive(Debug, Clone)]
pub(crate) struct Named {
    pub(crate) id: Id,
    pub(crate) name: String,
    /// The status's category, the type's glyph, or the label's hue.
    pub(crate) kind: String,
}

/// Which of a project's named lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum List {
    Statuses,
    Types,
    Labels,
}

impl List {
    pub(crate) const fn table(self) -> &'static str {
        match self {
            Self::Statuses => "statuses",
            Self::Types => "task_types",
            Self::Labels => "labels",
        }
    }

    const fn kind_column(self) -> &'static str {
        match self {
            Self::Statuses => "category",
            Self::Types => "glyph",
            Self::Labels => "hue",
        }
    }

    /// The message that the project has no such item.
    const fn not_found_message(self) -> &'static str {
        match self {
            Self::Statuses => "lookup-status-not-found",
            Self::Types => "lookup-type-not-found",
            Self::Labels => "lookup-label-not-found",
        }
    }
}

/// Every item of `list` in project `project`, in order.
pub(crate) fn all(tx: &Transaction<'_>, list: List, project: Id) -> Result<Vec<Named>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT id, name, {} FROM {} WHERE project_id = ?1 ORDER BY rank, id",
        list.kind_column(),
        list.table()
    ))?;
    let rows = stmt
        .query_map([b(project)], |r| {
            Ok(Named {
                id: id_at(r, 0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The item of `list` in `project` that `text` names: its name ignoring
/// case, or its ID.
pub(crate) fn named(
    tx: &Transaction<'_>,
    list: List,
    project: &ProjectRow,
    field: &str,
    text: &str,
) -> Result<Named, Failure> {
    let items = all(tx, list, project.id)?;
    let wanted = fold(text.trim());
    let id = text.trim().parse::<Id>().ok();
    if let Some(found) = items
        .iter()
        .find(|n| Some(n.id) == id || fold(&n.name) == wanted)
    {
        return Ok(found.clone());
    }
    let names: Vec<String> = items.into_iter().map(|n| n.name).collect();
    let args = [
        ("project", project.key.clone()),
        ("name", text.trim().to_owned()),
    ];
    Err(messages::with(list.not_found_message(), &args, |m| {
        OpError::not_found(m, &names.iter().map(String::as_str).collect::<Vec<_>>())
    })
    .with_detail("field", field)
    .into())
}

/// Whether query `src` limits its tasks to project `project`: every match
/// must meet a `project:KEY` naming it by its key or an earlier one.
pub(crate) fn query_limits_to(
    tx: &Transaction<'_>,
    src: &str,
    project: Id,
) -> Result<bool, Failure> {
    let Some(key) = gk_query::rename::limited_to_project(src) else {
        return Ok(false);
    };
    let found: Option<Vec<u8>> = tx
        .prepare_cached(
            "SELECT id FROM projects WHERE key = ?1
             UNION ALL SELECT project_id FROM project_key_aliases WHERE key = ?1 LIMIT 1",
        )?
        .query_row([key.as_str()], |r| r.get(0))
        .optional()?;
    Ok(found.and_then(|b| Id::from_slice(&b).ok()) == Some(project))
}

/// Whether a live project other than `project` has an item of `list`
/// named `name` (ignoring case).
pub(crate) fn named_elsewhere(
    tx: &Transaction<'_>,
    list: List,
    project: Id,
    name: &str,
) -> Result<bool, Failure> {
    let found: Option<i64> = tx
        .prepare_cached(&format!(
            "SELECT 1 FROM {} i JOIN projects p ON p.id = i.project_id
              WHERE i.name_fold = ?1 AND i.project_id <> ?2 AND p.deleted_at IS NULL LIMIT 1",
            list.table()
        ))?
        .query_row(params![fold(name.trim()), b(project)], |r| r.get(0))
        .optional()?;
    Ok(found.is_some())
}

/// What the engine needs of a task row.
#[derive(Debug, Clone)]
pub(crate) struct TaskRow {
    pub(crate) id: Id,
    pub(crate) project: Id,
    pub(crate) number: i64,
    pub(crate) status: Id,
    pub(crate) parent: Option<Id>,
    pub(crate) version: i64,
}

const TASK_COLUMNS: &str = "id, project_id, number, status_id, parent_id, version";

fn task_row(r: &Row<'_>) -> gk_store::rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: id_at(r, 0)?,
        project: id_at(r, 1)?,
        number: r.get(2)?,
        status: id_at(r, 3)?,
        parent: opt_id_at(r, 4)?,
        version: r.get(5)?,
    })
}

/// The live task with ID `id`, in a live project (within the caller's
/// project, for a caller limited to one).
pub(crate) fn task_by_id(tx: &Transaction<'_>, id: Id) -> Result<Option<TaskRow>, Failure> {
    let found = tx
        .query_row(
            &format!(
                "SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1 AND deleted_at IS NULL
                   AND project_id IN (SELECT id FROM projects WHERE deleted_at IS NULL)"
            ),
            [b(id)],
            task_row,
        )
        .optional()?;
    match found {
        Some(t) if !crate::scope::allows(tx, t.project)? => Ok(None),
        other => Ok(other),
    }
}

/// The live task `text` names: its key (current or old, any case) or its
/// ID.
pub(crate) fn task(tx: &Transaction<'_>, field: &str, text: &str) -> Result<TaskRow, Failure> {
    let text = text.trim();
    if let Ok(id) = text.parse::<Id>()
        && let Some(t) = task_by_id(tx, id)?
    {
        return Ok(t);
    }
    if let Ok(key) = text.parse::<TaskKey>() {
        let number = i64::from(key.number());
        if let Ok(p) = project(tx, field, key.project().as_str()) {
            let found = tx
                .prepare_cached(&format!(
                    "SELECT {TASK_COLUMNS} FROM tasks
                          WHERE project_id = ?1 AND number = ?2 AND deleted_at IS NULL"
                ))?
                .query_row(params![b(p.id), number], task_row)
                .optional()?;
            if let Some(t) = found {
                return Ok(t);
            }
        }
        let alias: Option<Vec<u8>> = tx
            .prepare_cached(
                "SELECT task_id FROM task_key_aliases WHERE project_key = ?1 AND number = ?2",
            )?
            .query_row(params![key.project().as_str(), number], |r| r.get(0))
            .optional()?;
        if let Some(id) = alias.and_then(|b| Id::from_slice(&b).ok())
            && let Some(t) = task_by_id(tx, id)?
        {
            return Ok(t);
        }
    }
    let error = messages::not_found("task-not-found", &[("task", text.to_owned())])
        .with_detail("field", field);
    Err(messages::hint(error, "task-not-found-hint", &[]).into())
}

/// The key of the task with project `project` and `number`.
pub(crate) fn task_key(tx: &Transaction<'_>, project: Id, number: i64) -> Result<String, Failure> {
    let key: String = tx.query_row(
        "SELECT key FROM projects WHERE id = ?1",
        [b(project)],
        |r| r.get(0),
    )?;
    Ok(format!("{key}-{number}"))
}

/// Refuses a change when the caller expected another version.
pub(crate) fn check_version(
    task: &TaskRow,
    expected: Option<i64>,
    key: &str,
) -> Result<(), Failure> {
    match expected {
        Some(v) if v != task.version => {
            let error = messages::conflict(
                "task-version-changed",
                &[
                    ("task", key.to_owned()),
                    ("version", task.version.to_string()),
                    ("expected", v.to_string()),
                ],
            );
            Err(messages::hint(error, "task-version-changed-hint", &[])
                .with_detail("version", task.version)
                .into())
        }
        _ => Ok(()),
    }
}

/// Where an item goes in an ordered list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Before(Id),
    After(Id),
    Top,
    Bottom,
}

impl Target {
    /// The target `before`/`after`/`at` ask for, given resolved IDs; `None`
    /// when none is given. More than one is refused.
    pub(crate) fn from_parts(
        before: Option<Id>,
        after: Option<Id>,
        at: Option<Edge>,
    ) -> Result<Option<Self>, Failure> {
        let given = usize::from(before.is_some())
            + usize::from(after.is_some())
            + usize::from(at.is_some());
        if given > 1 {
            return Err(
                messages::error(gk_api::ErrorCode::Invalid, "place-one-target", &[]).into(),
            );
        }
        Ok(before
            .map(Self::Before)
            .or(after.map(Self::After))
            .or(at.map(|e| match e {
                Edge::Top => Self::Top,
                Edge::Bottom => Self::Bottom,
            })))
    }
}

/// The new ranks that put `moving` at `target` among `siblings` (in order,
/// with their ranks; `moving` may be among them): usually one rank, or all
/// of them, spread out, when there is no room left between the neighbours.
pub(crate) fn place(
    siblings: &[(Id, String)],
    moving: Id,
    target: Target,
) -> Result<Vec<(Id, Rank)>, Failure> {
    let list: Vec<(Id, &str)> = siblings
        .iter()
        .filter(|(id, _)| *id != moving)
        .map(|(id, r)| (*id, r.as_str()))
        .collect();
    let index = match target {
        Target::Top => 0,
        Target::Bottom => list.len(),
        Target::Before(other) | Target::After(other) => {
            let pos = list
                .iter()
                .position(|(id, _)| *id == other)
                .ok_or_else(|| {
                    Failure::Op(messages::error(
                        gk_api::ErrorCode::Invalid,
                        "place-bad-neighbour",
                        &[],
                    ))
                })?;
            if matches!(target, Target::After(_)) {
                pos + 1
            } else {
                pos
            }
        }
    };
    let parse = |i: usize| list.get(i).and_then(|(_, r)| Rank::parse(r).ok());
    let lower = index.checked_sub(1).and_then(parse);
    let upper = parse(index);
    let lower_ok = index == 0 || lower.is_some();
    let upper_ok = index >= list.len() || upper.is_some();
    if lower_ok
        && upper_ok
        && let Ok(rank) = Rank::between(lower.as_ref(), upper.as_ref())
        && !rank.needs_respread()
    {
        return Ok(vec![(moving, rank)]);
    }
    let mut order: Vec<Id> = list.iter().map(|(id, _)| *id).collect();
    order.insert(index, moving);
    let ranks = Rank::spread(order.len());
    Ok(order
        .into_iter()
        .zip(ranks)
        .filter(|(id, rank)| {
            siblings
                .iter()
                .find(|(s, _)| s == id)
                .is_none_or(|(_, old)| old != rank.as_str())
        })
        .collect())
}

/// The siblings of a rank-ordered list, as `(id, rank)` in order: the rows
/// of `table` matching `filter` (an SQL condition on `?1`).
pub(crate) fn ranked(
    tx: &Transaction<'_>,
    table: &str,
    filter: &str,
    arg: &dyn gk_store::rusqlite::ToSql,
) -> Result<Vec<(Id, String)>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT id, rank FROM {table} WHERE {filter} ORDER BY rank, id"
    ))?;
    let rows = stmt
        .query_map([arg], |r| Ok((id_at(r, 0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<Id> {
        (0..n).map(|_| Id::generate()).collect()
    }

    #[test]
    fn placing_needs_one_new_rank_when_there_is_room() {
        let ids = ids(3);
        let siblings: Vec<(Id, String)> = ids
            .iter()
            .zip(Rank::spread(3))
            .map(|(id, r)| (*id, r.as_str().to_owned()))
            .collect();
        let new = Id::generate();
        let top = place(&siblings, new, Target::Top).unwrap();
        assert_eq!(top.len(), 1);
        assert!(top[0].1.as_str() < siblings[0].1.as_str());
        let mid = place(&siblings, ids[2], Target::After(ids[0])).unwrap();
        assert_eq!(mid.len(), 1);
        assert!(mid[0].1.as_str() > siblings[0].1.as_str());
        assert!(mid[0].1.as_str() < siblings[1].1.as_str());
        let end = place(&siblings, new, Target::Bottom).unwrap();
        assert!(end[0].1.as_str() > siblings[2].1.as_str());
        assert!(place(&siblings, ids[0], Target::Before(ids[0])).is_err());
    }

    #[test]
    fn crowded_lists_are_spread_out() {
        let ids = ids(2);
        let siblings = vec![(ids[0], "1".to_owned()), (ids[1], "1000001".to_owned())];
        let new = Id::generate();
        let out = place(&siblings, new, Target::After(ids[0])).unwrap();
        assert!(!out.is_empty());
        let mut all: Vec<(Id, String)> = siblings.clone();
        all.push((new, String::new()));
        for (id, rank) in &out {
            if let Some(s) = all.iter_mut().find(|(i, _)| i == id) {
                s.1 = rank.as_str().to_owned();
            }
        }
        all.sort_by(|a, b| a.1.cmp(&b.1));
        let order: Vec<Id> = all.iter().map(|(id, _)| *id).collect();
        assert_eq!(order, vec![ids[0], new, ids[1]]);
    }

    #[test]
    fn only_one_place_may_be_given() {
        let id = Id::generate();
        assert!(Target::from_parts(Some(id), None, Some(Edge::Top)).is_err());
        assert_eq!(Target::from_parts(None, None, None).unwrap(), None);
        assert_eq!(
            Target::from_parts(None, Some(id), None).unwrap(),
            Some(Target::After(id))
        );
    }
}
