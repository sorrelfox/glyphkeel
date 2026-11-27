// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Project folders: one level of grouping for the top-level projects in
//! the sidebar. A folder is kept whole in history (one `record` field), and
//! a project's folder is a project field (`folder`), so making, renaming,
//! reordering and deleting folders and moving projects in and out of them
//! can all be undone.
//!
//! Folders only group. A project keeps its own place among the top-level
//! projects (`rank`), inside a folder or not; its sub-projects follow it
//! and never name a folder themselves.

use gk_api::OpError;
use gk_api::ops::common::{Actor, ActorKind, Edge, Hue};
use gk_api::ops::folder::{
    FolderChange, FolderCreate, FolderCreateInput, FolderDelete, FolderField, FolderList,
    FolderListInput, FolderListOp, FolderMove, FolderMoveInput, FolderRef, FolderRename,
    FolderRenameInput, FolderUpdate, FolderUpdateInput, ProjectFolder,
};
use gk_core::{Id, Rank, fold};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::desc;
use super::fields::{self, PROJECT, id_value, text};
use super::lookup::{self, Target, b, id_at, opt_id_at};
use super::project::{enum_text, parse_enum};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's name for project folders.
pub(crate) const ENTITY: &str = "project_folder";
/// The one recorded field: the whole folder.
pub(crate) const RECORD: &str = "record";
/// A project's field naming its folder.
pub(crate) const FIELD: &str = "folder";
/// The longest folder name, in characters.
const NAME_MAX: usize = 100;

pub(super) fn register(r: &mut Registry) {
    r.query::<FolderListOp, _>(desc::<FolderListOp>(), |q, _: FolderListInput| list(q.tx));
    r.command::<FolderCreate, _>(desc::<FolderCreate>(), |w, input| {
        whole_workspace()?;
        create(w, input)
    });
    r.command::<FolderUpdate, _>(desc::<FolderUpdate>(), |w, input| {
        whole_workspace()?;
        update(w, input)
    });
    r.command::<FolderRename, _>(desc::<FolderRename>(), |w, input: FolderRenameInput| {
        whole_workspace()?;
        update(
            w,
            FolderUpdateInput {
                folder: input.folder,
                name: Some(input.name),
                ..FolderUpdateInput::default()
            },
        )
    });
    r.command::<FolderDelete, _>(desc::<FolderDelete>(), |w, input| {
        whole_workspace()?;
        delete(w, input)
    });
    r.command::<FolderMove, _>(desc::<FolderMove>(), |w, input| {
        whole_workspace()?;
        move_project(w, input)
    });
}

/// Folders arrange the whole workspace's sidebar: a caller limited to one
/// project reads them but doesn't change them.
pub(crate) fn whole_workspace() -> Result<(), Failure> {
    if crate::scope::root().is_some() {
        return Err(crate::scope::outside());
    }
    Ok(())
}

// ---------------------------------------------------------------------
// The stored folder

/// A folder as history records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) hue: Option<String>,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) updated_at: i64,
}

const COLUMNS: &str =
    "id, name, hue, rank, created_at, created_by_kind, created_by_name, updated_at";

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<(Id, Record)> {
    Ok((
        id_at(r, 0)?,
        Record {
            name: r.get(1)?,
            hue: r.get(2)?,
            rank: r.get(3)?,
            created_at: r.get(4)?,
            created_by_kind: r.get(5)?,
            created_by_name: r.get(6)?,
            updated_at: r.get(7)?,
        },
    ))
}

fn load(tx: &Transaction<'_>, id: Id) -> Result<Option<Record>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM project_folders WHERE id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?
        .map(|(_, r)| r))
}

/// The stored folder `id` as history records it, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    load(tx, id)?
        .map(|r| serde_json::to_value(r).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

fn insert(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), Failure> {
    tx.execute(
        "INSERT INTO project_folders (id, name, name_fold, hue, rank, created_at,
                                      created_by_kind, created_by_name, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, name_fold = excluded.name_fold,
                hue = excluded.hue, rank = excluded.rank, created_at = excluded.created_at,
                created_by_kind = excluded.created_by_kind,
                created_by_name = excluded.created_by_name, updated_at = excluded.updated_at",
        params![
            b(id),
            r.name,
            fold(&r.name),
            r.hue,
            r.rank,
            r.created_at,
            r.created_by_kind,
            r.created_by_name,
            r.updated_at,
        ],
    )?;
    Ok(())
}

/// Writes folder `id` as history recorded it (`None` deletes it), without
/// recording. A folder still named by projects is not deleted here: see
/// [`apply`].
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    let Some(value) = value else {
        w.tx()
            .execute("DELETE FROM project_folders WHERE id = ?1", [b(id)])?;
        return Ok(());
    };
    let r: Record = serde_json::from_value(value.clone())
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    insert(w.tx(), id, &r)
}

/// Writes back a folder history recorded and records the change. A folder
/// going away first lets go of the projects still in it (each a recorded
/// change of its own), so they go back to the top level.
pub(crate) fn apply(w: &mut WriteCtx<'_>, id: Id, value: Option<Value>) -> Result<(), Failure> {
    if value.is_none() {
        release(w, id)?;
    }
    let before = raw(w.tx(), id)?;
    write_raw(w, id, value.as_ref())?;
    w.record_field(ENTITY, id, None, RECORD, before, value);
    Ok(())
}

/// Writes folder `id` and records the change.
fn set(w: &mut WriteCtx<'_>, id: Id, record: Option<&Record>) -> Result<(), Failure> {
    let before = raw(w.tx(), id)?;
    let after = record
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?;
    if before == after {
        return Ok(());
    }
    write_raw(w, id, after.as_ref())?;
    w.record_field(ENTITY, id, None, RECORD, before, after);
    Ok(())
}

/// Takes every project out of folder `id`, the trashed ones too; returns
/// the keys of the live ones, in order.
fn release(w: &mut WriteCtx<'_>, id: Id) -> Result<Vec<String>, Failure> {
    let members: Vec<(Id, String, bool)> = {
        let mut stmt = w.tx().prepare_cached(
            "SELECT id, key, deleted_at IS NULL FROM projects WHERE folder_id = ?1
              ORDER BY rank, id",
        )?;
        stmt.query_map([b(id)], |r| Ok((id_at(r, 0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut keys = Vec::new();
    for (project, key, live) in members {
        fields::set(w, PROJECT, project, Some(project), FIELD, None)?;
        if live {
            keys.push(key);
        }
    }
    Ok(keys)
}

/// Every folder, in order.
pub(crate) fn rows(tx: &Transaction<'_>) -> Result<Vec<(Id, Record)>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM project_folders ORDER BY rank, id"
    ))?;
    Ok(stmt
        .query_map([], read_row)?
        .collect::<Result<Vec<_>, _>>()?)
}

/// Stores folder `id` from an archive, outside history (an import is one
/// change of its own).
pub(crate) fn insert_imported(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), Failure> {
    insert(tx, id, r)
}

/// The folder named `name` (ignoring case), if any.
pub(crate) fn by_name(tx: &Transaction<'_>, name: &str) -> Result<Option<Id>, Failure> {
    Ok(tx
        .query_row(
            "SELECT id FROM project_folders WHERE name_fold = ?1",
            [fold(name.trim())],
            |r| id_at(r, 0),
        )
        .optional()?)
}

/// The folder top-level project `project` is in, if any.
pub(crate) fn of_project(tx: &Transaction<'_>, project: Id) -> Result<Option<Id>, Failure> {
    Ok(tx
        .query_row(
            "SELECT folder_id FROM projects WHERE id = ?1",
            [b(project)],
            |r| opt_id_at(r, 0),
        )
        .optional()?
        .flatten())
}

/// Puts project `project` in folder `folder` (or none), recorded. A
/// project that becomes a sub-project leaves its folder this way.
pub(crate) fn set_project_folder(
    w: &mut WriteCtx<'_>,
    project: Id,
    folder: Option<Id>,
) -> Result<bool, Failure> {
    fields::set(
        w,
        PROJECT,
        project,
        Some(project),
        FIELD,
        folder.and_then(id_value),
    )
}

// ---------------------------------------------------------------------
// Reading

/// One project as the folders see it.
struct Member {
    id: Id,
    parent: Option<Id>,
    folder: Option<Id>,
    key: String,
    archived: bool,
    open: u64,
}

fn members(tx: &Transaction<'_>) -> Result<Vec<Member>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT p.id, p.parent_id, p.folder_id, p.key, p.state,
                (SELECT count(*) FROM tasks t JOIN statuses s ON s.id = t.status_id
                  WHERE t.project_id = p.id AND t.deleted_at IS NULL
                    AND s.category IN ('todo', 'in_progress'))
           FROM projects p WHERE p.deleted_at IS NULL ORDER BY p.rank, p.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Member {
                id: id_at(r, 0)?,
                parent: opt_id_at(r, 1)?,
                folder: opt_id_at(r, 2)?,
                key: r.get(3)?,
                archived: r.get::<_, String>(4)? == "archived",
                open: r.get::<_, i64>(5)?.unsigned_abs(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter(|m| crate::scope::sees(m.id))
        .collect())
}

/// Open tasks of active project `id` and its active sub-projects.
fn open_below(all: &[Member], id: Id) -> u64 {
    let Some(me) = all.iter().find(|m| m.id == id) else {
        return 0;
    };
    if me.archived {
        return 0;
    }
    me.open
        + all
            .iter()
            .filter(|m| m.parent == Some(id))
            .map(|m| open_below(all, m.id))
            .sum::<u64>()
}

fn output(all: &[Member], id: Id, r: &Record) -> ProjectFolder {
    let inside: Vec<&Member> = all
        .iter()
        .filter(|m| m.parent.is_none() && m.folder == Some(id))
        .collect();
    ProjectFolder {
        id: id.to_string(),
        name: r.name.clone(),
        hue: r.hue.as_deref().and_then(parse_enum::<Hue>),
        rank: r.rank.clone(),
        projects: inside.iter().map(|m| m.key.clone()).collect(),
        open_tasks: inside.iter().map(|m| open_below(all, m.id)).sum(),
        created_by: Actor {
            kind: ActorKind::from_stored(&r.created_by_kind),
            name: r.created_by_name.clone(),
        },
        created_at: rfc3339(r.created_at),
        updated_at: rfc3339(r.updated_at),
    }
}

fn list(tx: &Transaction<'_>) -> Result<FolderList, Failure> {
    let all = members(tx)?;
    let items: Vec<ProjectFolder> = rows(tx)?
        .iter()
        .map(|(id, r)| output(&all, *id, r))
        .collect();
    Ok(FolderList {
        total: items.len() as u64,
        items,
    })
}

/// The folder `text` names: its ID, or its name (ignoring case).
pub(crate) fn find(tx: &Transaction<'_>, field: &str, text: &str) -> Result<(Id, Record), Failure> {
    let text = text.trim();
    if let Ok(id) = text.parse::<Id>()
        && let Some(r) = load(tx, id)?
    {
        return Ok((id, r));
    }
    let all = rows(tx)?;
    let wanted = fold(text);
    if let Some(found) = all.iter().find(|(_, r)| fold(&r.name) == wanted) {
        return Ok(found.clone());
    }
    let names: Vec<&str> = all.iter().map(|(_, r)| r.name.as_str()).collect();
    let args = [("folder", text.to_owned())];
    let error = messages::with("folder-not-found", &args, |m| OpError::not_found(m, &names))
        .with_detail("field", field);
    Err(messages::hint(error, "folder-not-found-hint", &[]).into())
}

fn change(
    w: &WriteCtx<'_>,
    folder: Option<(Id, &Record)>,
    project: Option<String>,
    released: Vec<String>,
    deleted: bool,
) -> Result<FolderChange, Failure> {
    let all = members(w.tx())?;
    Ok(FolderChange {
        folder: folder.map(|(id, r)| output(&all, id, r)),
        project,
        released,
        deleted,
        group: w.group_if_changed(),
    })
}

// ---------------------------------------------------------------------
// Changing

fn check_name(name: &str) -> Result<String, Failure> {
    gk_core::check_line("name", name, NAME_MAX).map_err(|e| super::task::invalid("name", e))
}

fn check_unique(tx: &Transaction<'_>, id: Id, name: &str) -> Result<(), Failure> {
    if let Some(other) = by_name(tx, name)?
        && other != id
    {
        let error = messages::conflict("folder-name-taken", &[("folder", name.to_owned())])
            .with_detail("field", "name");
        return Err(messages::hint(error, "folder-name-taken-hint", &[]).into());
    }
    Ok(())
}

/// The ranks that put folder `id` at the place `before`/`after`/`at` ask
/// for; empty when none is asked for. The folder's own new rank comes
/// first when it is among them.
fn placed(
    w: &WriteCtx<'_>,
    id: Id,
    before: Option<&str>,
    after: Option<&str>,
    at: Option<Edge>,
) -> Result<Vec<(Id, Rank)>, Failure> {
    let other = |field: &str, text: &str| -> Result<Id, Failure> {
        let (other, _) = find(w.tx(), field, text)?;
        if other == id {
            return Err(messages::invalid(field, "folder-not-itself", &[]));
        }
        Ok(other)
    };
    let before = before.map(|t| other("before", t)).transpose()?;
    let after = after.map(|t| other("after", t)).transpose()?;
    let Some(target) = Target::from_parts(before, after, at)? else {
        return Ok(Vec::new());
    };
    let siblings: Vec<(Id, String)> = rows(w.tx())?
        .into_iter()
        .map(|(i, r)| (i, r.rank))
        .collect();
    lookup::place(&siblings, id, target)
}

/// Gives the other folders their new ranks (recorded); returns the moving
/// folder's own new rank, if it has one.
fn rerank(w: &mut WriteCtx<'_>, id: Id, ranks: Vec<(Id, Rank)>) -> Result<Option<String>, Failure> {
    let mut mine = None;
    for (other, rank) in ranks {
        if other == id {
            mine = Some(rank.as_str().to_owned());
        } else if let Some(mut r) = load(w.tx(), other)? {
            r.rank = rank.as_str().to_owned();
            set(w, other, Some(&r))?;
        }
    }
    Ok(mine)
}

fn create(w: &mut WriteCtx<'_>, input: FolderCreateInput) -> Result<FolderChange, Failure> {
    let name = check_name(&input.name)?;
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    check_unique(w.tx(), id, &name)?;
    let ranks = placed(
        w,
        id,
        input.before.as_deref(),
        input.after.as_deref(),
        input.at,
    )?;
    let rank = match rerank(w, id, ranks)? {
        Some(rank) => rank,
        None => {
            let last = rows(w.tx())?
                .into_iter()
                .filter_map(|(_, r)| Rank::parse(&r.rank).ok())
                .max_by(|a, b| a.as_str().cmp(b.as_str()));
            last.map_or_else(Rank::first, |r| r.after())
                .as_str()
                .to_owned()
        }
    };
    let caller = w.caller().actor.clone();
    let record = Record {
        name,
        hue: input.hue.map(enum_text),
        rank,
        created_at: w.now(),
        created_by_kind: caller.kind.as_str().to_owned(),
        created_by_name: caller.name,
        updated_at: w.now(),
    };
    set(w, id, Some(&record))?;
    change(w, Some((id, &record)), None, Vec::new(), false)
}

fn update(w: &mut WriteCtx<'_>, input: FolderUpdateInput) -> Result<FolderChange, Failure> {
    let (id, old) = find(w.tx(), "folder", &input.folder)?;
    let clears_hue = input.clear.contains(&FolderField::Hue);
    if input.hue.is_some() && clears_hue {
        return Err(messages::invalid(
            "clear",
            "field-given-and-cleared",
            &[("field", "hue".to_owned())],
        ));
    }
    let mut r = old.clone();
    if let Some(name) = &input.name {
        r.name = check_name(name)?;
        check_unique(w.tx(), id, &r.name)?;
    }
    if let Some(hue) = input.hue {
        r.hue = Some(enum_text(hue));
    }
    if clears_hue {
        r.hue = None;
    }
    let ranks = placed(
        w,
        id,
        input.before.as_deref(),
        input.after.as_deref(),
        input.at,
    )?;
    if let Some(rank) = rerank(w, id, ranks)? {
        r.rank = rank;
    }
    if r != old {
        r.updated_at = w.now();
        set(w, id, Some(&r))?;
    }
    change(w, Some((id, &r)), None, Vec::new(), false)
}

fn delete(w: &mut WriteCtx<'_>, input: FolderRef) -> Result<FolderChange, Failure> {
    let (id, old) = find(w.tx(), "folder", &input.folder)?;
    let released = release(w, id)?;
    set(w, id, None)?;
    let all = members(w.tx())?;
    Ok(FolderChange {
        folder: Some(output(&all, id, &old)),
        project: None,
        released,
        deleted: true,
        group: w.group_if_changed(),
    })
}

/// The top-level projects, in order, with their folders and ranks.
fn top_level(tx: &Transaction<'_>) -> Result<Vec<(Id, Option<Id>, String)>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT id, folder_id, rank FROM projects
          WHERE parent_id IS NULL AND deleted_at IS NULL ORDER BY rank, id",
    )?;
    Ok(stmt
        .query_map([], |r| Ok((id_at(r, 0)?, opt_id_at(r, 1)?, r.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn move_project(w: &mut WriteCtx<'_>, input: FolderMoveInput) -> Result<FolderChange, Failure> {
    let p = lookup::project(w.tx(), "project", &input.project)?;
    if p.parent.is_some() {
        return Err(messages::invalid(
            "project",
            "folder-subproject",
            &[("project", p.key.clone())],
        ));
    }
    if input.none && input.folder.is_some() {
        return Err(messages::invalid(
            "none",
            "field-given-and-cleared",
            &[("field", "folder".to_owned())],
        ));
    }
    let current = of_project(w.tx(), p.id)?;
    let target = if input.none {
        None
    } else if let Some(text) = &input.folder {
        Some(find(w.tx(), "folder", text)?.0)
    } else {
        current
    };
    let projects = top_level(w.tx())?;
    let neighbour = |field: &str, text: &str| -> Result<Id, Failure> {
        let s = lookup::project(w.tx(), field, text)?;
        let fits = projects
            .iter()
            .any(|(id, folder, _)| *id == s.id && *folder == target);
        if !fits || s.id == p.id {
            return Err(messages::invalid(
                field,
                "folder-not-a-neighbour",
                &[("neighbour", s.key.clone()), ("project", p.key.clone())],
            ));
        }
        Ok(s.id)
    };
    let before = input
        .before
        .as_deref()
        .map(|t| neighbour("before", t))
        .transpose()?;
    let after = input
        .after
        .as_deref()
        .map(|t| neighbour("after", t))
        .transpose()?;
    let moves = target != current;
    if moves {
        set_project_folder(w, p.id, target)?;
    }
    // Places in the folder are places among all top-level projects: the
    // folder's first and last projects stand in for its two ends.
    let others: Vec<Id> = projects
        .iter()
        .filter(|(id, folder, _)| *folder == target && *id != p.id)
        .map(|(id, _, _)| *id)
        .collect();
    let edge = match (input.at, moves) {
        (Some(at), _) => Some(at),
        (None, true) if before.is_none() && after.is_none() => Some(Edge::Bottom),
        _ => None,
    };
    let goal = match Target::from_parts(before, after, edge)? {
        Some(Target::Top) => others.first().map(|&o| Target::Before(o)),
        Some(Target::Bottom) => others.last().map(|&o| Target::After(o)),
        other => other,
    };
    if let Some(goal) = goal {
        let siblings: Vec<(Id, String)> = projects
            .iter()
            .map(|(id, _, rank)| (*id, rank.clone()))
            .collect();
        for (id, rank) in lookup::place(&siblings, p.id, goal)? {
            fields::set(w, PROJECT, id, Some(id), "rank", text(rank.as_str()))?;
        }
    }
    let folder = match target {
        Some(id) => load(w.tx(), id)?.map(|r| (id, r)),
        None => None,
    };
    change(
        w,
        folder.as_ref().map(|(id, r)| (*id, r)),
        Some(p.key),
        Vec::new(),
        false,
    )
}
