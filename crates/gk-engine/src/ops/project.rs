// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Projects and sub-projects, and their statuses, task types and labels.
//!
//! Rules: keys are unique among current and old keys; sub-projects go at
//! most 3 levels deep and never under their own descendants; a new
//! sub-project copies its parent's statuses, types and labels; archiving acts
//! on the whole subtree and makes it read-only; a status is deleted only by
//! naming where its tasks go; the default type can't be deleted.

use gk_api::OpError;
use gk_api::ops::common::{Category, Edge, Glyph, Hue};
use gk_api::ops::project::{
    Label, LabelChange, LabelCreate, LabelCreateInput, LabelDelete, LabelDeleteInput, LabelUpdate,
    LabelUpdateInput, Modules, ModulesPatch, Project, ProjectArchive, ProjectChange,
    ProjectChangeKey, ProjectCreate, ProjectCreateInput, ProjectField, ProjectGet, ProjectList,
    ProjectListInput, ProjectListOp, ProjectMove, ProjectMoveInput, ProjectRef, ProjectRekeyInput,
    ProjectState, ProjectSummary, ProjectUnarchive, ProjectUpdate, ProjectUpdateInput, Status,
    StatusChange, StatusCreate, StatusCreateInput, StatusDelete, StatusDeleteInput, StatusField,
    StatusReorder, StatusReorderInput, StatusUpdate, StatusUpdateInput, TaskType, TypeChange,
    TypeCreate, TypeCreateInput, TypeDelete, TypeDeleteInput, TypeUpdate, TypeUpdateInput,
};
use gk_core::{DEFAULT_STATUSES, DEFAULT_TASK_TYPES, Id, ProjectKey, Rank, StatusCategory, fold};
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::desc;
use super::fields::{self, LABEL, PROJECT, STATUS, TASK, TYPE, id_value, text};
use super::lookup::{self, List, Named, ProjectRow, Target, b, id_at, opt_id_at};
use crate::failure::Failure;
use crate::registry::{QueryCtx, Registry};
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// Longest project name, in characters.
const PROJECT_NAME_MAX: usize = 100;
/// Longest status, type or label name, in characters.
const ITEM_NAME_MAX: usize = 50;
/// Most levels of projects: a top-level project and two below it.
const MAX_LEVELS: usize = 3;

pub(super) fn register(r: &mut Registry) {
    r.query::<ProjectListOp, _>(desc::<ProjectListOp>(), list);
    r.query::<ProjectGet, _>(desc::<ProjectGet>(), |q, input: ProjectRef| {
        let p = lookup::project(q.tx, "project", &input.project)?;
        load(q.tx, p.id)
    });
    r.command::<ProjectCreate, _>(desc::<ProjectCreate>(), create);
    r.combined::<ProjectUpdate, _>(desc::<ProjectUpdate>(), update_parts, update);
    r.command::<ProjectMove, _>(desc::<ProjectMove>(), |w, input: ProjectMoveInput| {
        let p = lookup::project(w.tx(), "project", &input.project)?;
        lookup::ensure_active(&p)?;
        let parent = if input.top_level {
            if input.parent.is_some() {
                return Err(OpError::invalid("Give either `parent` or `top_level`.").into());
            }
            Some(None)
        } else {
            input.parent.as_deref().map(Some)
        };
        move_project(
            w,
            &p,
            parent,
            input.before.as_deref(),
            input.after.as_deref(),
            input.at,
        )?;
        changed(w, p.id)
    });
    r.command::<ProjectChangeKey, _>(desc::<ProjectChangeKey>(), |w, input: ProjectRekeyInput| {
        let p = lookup::project(w.tx(), "project", &input.project)?;
        lookup::ensure_active(&p)?;
        change_key(w, &p, &input.key)?;
        changed(w, p.id)
    });
    r.command::<ProjectArchive, _>(desc::<ProjectArchive>(), |w, input: ProjectRef| {
        let p = lookup::project(w.tx(), "project", &input.project)?;
        set_archived(w, &p, true)?;
        changed(w, p.id)
    });
    r.command::<ProjectUnarchive, _>(desc::<ProjectUnarchive>(), |w, input: ProjectRef| {
        let p = lookup::project(w.tx(), "project", &input.project)?;
        set_archived(w, &p, false)?;
        changed(w, p.id)
    });
    r.command::<StatusCreate, _>(desc::<StatusCreate>(), |w, i: StatusCreateInput| {
        let p = active(w, &i.project)?;
        let change = StatusChange {
            name: Some(i.name),
            category: i.category,
            hue: i.hue,
            wip_limit: i.wip_limit,
            before: i.before,
            after: i.after,
            at: i.at,
            ..StatusChange::default()
        };
        apply_status(w, &p, &change)?;
        changed(w, p.id)
    });
    r.command::<StatusUpdate, _>(desc::<StatusUpdate>(), |w, i: StatusUpdateInput| {
        let p = active(w, &i.project)?;
        let change = StatusChange {
            status: Some(i.status),
            name: i.name,
            category: i.category,
            hue: i.hue,
            wip_limit: i.wip_limit,
            clear: i.clear,
            ..StatusChange::default()
        };
        apply_status(w, &p, &change)?;
        changed(w, p.id)
    });
    r.command::<StatusReorder, _>(desc::<StatusReorder>(), |w, i: StatusReorderInput| {
        let p = active(w, &i.project)?;
        if i.before.is_none() && i.after.is_none() && i.at.is_none() {
            return Err(OpError::invalid("Say where: `before`, `after` or `at`.").into());
        }
        let change = StatusChange {
            status: Some(i.status),
            before: i.before,
            after: i.after,
            at: i.at,
            ..StatusChange::default()
        };
        apply_status(w, &p, &change)?;
        changed(w, p.id)
    });
    r.command::<StatusDelete, _>(desc::<StatusDelete>(), |w, i: StatusDeleteInput| {
        let p = active(w, &i.project)?;
        let change = StatusChange {
            status: Some(i.status),
            remove: true,
            move_tasks_to: i.move_tasks_to,
            ..StatusChange::default()
        };
        apply_status(w, &p, &change)?;
        changed(w, p.id)
    });
    r.command::<TypeCreate, _>(desc::<TypeCreate>(), |w, i: TypeCreateInput| {
        let p = active(w, &i.project)?;
        let change = TypeChange {
            name: Some(i.name),
            glyph: i.glyph,
            hue: i.hue,
            make_default: i.make_default,
            ..TypeChange::default()
        };
        apply_type(w, &p, &change)?;
        changed(w, p.id)
    });
    r.command::<TypeUpdate, _>(desc::<TypeUpdate>(), |w, i: TypeUpdateInput| {
        let p = active(w, &i.project)?;
        let change = TypeChange {
            task_type: Some(i.task_type),
            name: i.name,
            glyph: i.glyph,
            hue: i.hue,
            make_default: i.make_default,
            before: i.before,
            after: i.after,
            at: i.at,
            ..TypeChange::default()
        };
        apply_type(w, &p, &change)?;
        changed(w, p.id)
    });
    r.command::<TypeDelete, _>(desc::<TypeDelete>(), |w, i: TypeDeleteInput| {
        let p = active(w, &i.project)?;
        let change = TypeChange {
            task_type: Some(i.task_type),
            remove: true,
            move_tasks_to: i.move_tasks_to,
            ..TypeChange::default()
        };
        apply_type(w, &p, &change)?;
        changed(w, p.id)
    });
    r.command::<LabelCreate, _>(desc::<LabelCreate>(), |w, i: LabelCreateInput| {
        let p = active(w, &i.project)?;
        let change = LabelChange {
            name: Some(i.name),
            hue: i.hue,
            ..LabelChange::default()
        };
        apply_label(w, &p, &change)?;
        changed(w, p.id)
    });
    r.command::<LabelUpdate, _>(desc::<LabelUpdate>(), |w, i: LabelUpdateInput| {
        let p = active(w, &i.project)?;
        let change = LabelChange {
            label: Some(i.label),
            name: i.name,
            hue: i.hue,
            ..LabelChange::default()
        };
        apply_label(w, &p, &change)?;
        changed(w, p.id)
    });
    r.command::<LabelDelete, _>(desc::<LabelDelete>(), |w, i: LabelDeleteInput| {
        let p = active(w, &i.project)?;
        let change = LabelChange {
            label: Some(i.label),
            remove: true,
            ..LabelChange::default()
        };
        apply_label(w, &p, &change)?;
        changed(w, p.id)
    });
}

/// The name a value of an API enum has in the database (`in_progress`).
pub(crate) fn enum_text<T: Serialize>(v: T) -> String {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => s,
        _ => String::new(),
    }
}

/// The API enum value stored as `text`, if it is one.
pub(crate) fn parse_enum<T: DeserializeOwned>(text: &str) -> Option<T> {
    serde_json::from_value(Value::String(text.to_owned())).ok()
}

fn active(w: &WriteCtx<'_>, text: &str) -> Result<ProjectRow, Failure> {
    let p = lookup::project(w.tx(), "project", text)?;
    lookup::ensure_active(&p)?;
    Ok(p)
}

fn changed(w: &WriteCtx<'_>, id: Id) -> Result<ProjectChange, Failure> {
    Ok(ProjectChange {
        project: load(w.tx(), id)?,
        group: w.group_if_changed(),
    })
}

fn invalid(field: &str, e: impl std::fmt::Display) -> Failure {
    let text = e.to_string();
    let mut chars = text.chars();
    let message = chars.next().map_or_else(String::new, |c| {
        format!("{}{}.", c.to_uppercase(), chars.as_str())
    });
    Failure::Op(OpError::invalid_at(field, message))
}

/// The cleaned name, or `invalid` at `field`.
fn clean_name(field: &str, what: &'static str, name: &str, max: usize) -> Result<String, Failure> {
    gk_core::check_line(what, name, max).map_err(|e| invalid(field, e))
}

// ---------------------------------------------------------------------
// Reading

fn modules_of(text: &str) -> Modules {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let on = |k: &str| match v.get(k) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64() != Some(0),
        _ => true,
    };
    Modules {
        board: on("board"),
        list: on("list"),
        gantt: on("gantt"),
        wiki: on("wiki"),
        time: on("time"),
    }
}

fn modules_value(m: Modules) -> Value {
    json!({ "board": m.board, "list": m.list, "gantt": m.gantt, "wiki": m.wiki, "time": m.time })
}

/// The names from the top-level project down to `id`, and its depth.
pub(crate) fn path_of(tx: &Transaction<'_>, id: Id) -> Result<(String, usize), Failure> {
    let mut names = Vec::new();
    let mut next = Some(id);
    while let Some(current) = next {
        let (name, parent): (String, Option<Id>) = tx.query_row(
            "SELECT name, parent_id FROM projects WHERE id = ?1",
            [b(current)],
            |r| Ok((r.get(0)?, opt_id_at(r, 1)?)),
        )?;
        names.push(name);
        next = parent;
        if names.len() > 16 {
            break;
        }
    }
    let depth = names.len() - 1;
    names.reverse();
    Ok((names.join(" / "), depth))
}

/// A project with its configuration.
pub(crate) fn load(tx: &Transaction<'_>, id: Id) -> Result<Project, Failure> {
    let row = tx.query_row(
        "SELECT key, name, description, hue, icon, state, parent_id, modules, created_at,
                updated_at
           FROM projects WHERE id = ?1",
        [b(id)],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                opt_id_at(r, 6)?,
                r.get::<_, String>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, i64>(9)?,
            ))
        },
    )?;
    let (key, name, description, hue, icon, state, parent, modules, created, updated) = row;
    let parent_key = match parent {
        Some(p) => Some(
            tx.query_row("SELECT key FROM projects WHERE id = ?1", [b(p)], |r| {
                r.get::<_, String>(0)
            })?,
        ),
        None => None,
    };
    let children = {
        let mut stmt = tx.prepare_cached(
            "SELECT key FROM projects WHERE parent_id = ?1 AND deleted_at IS NULL ORDER BY rank, id",
        )?;
        stmt.query_map([b(id)], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    let old_keys = {
        let mut stmt = tx.prepare_cached(
            "SELECT key FROM project_key_aliases WHERE project_id = ?1 ORDER BY key",
        )?;
        stmt.query_map([b(id)], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    let statuses = {
        let mut stmt = tx.prepare_cached(
            "SELECT s.id, s.name, s.category, s.hue, s.wip_limit,
                    (SELECT count(*) FROM tasks t WHERE t.status_id = s.id AND t.deleted_at IS NULL)
               FROM statuses s WHERE s.project_id = ?1 ORDER BY s.rank, s.id",
        )?;
        stmt.query_map([b(id)], |r| {
            Ok(Status {
                id: id_at(r, 0)?.to_string(),
                name: r.get(1)?,
                category: parse_enum(&r.get::<_, String>(2)?).unwrap_or(Category::Todo),
                hue: parse_enum(&r.get::<_, String>(3)?).unwrap_or(Hue::Grey),
                wip_limit: r
                    .get::<_, Option<i64>>(4)?
                    .and_then(|n| u32::try_from(n).ok()),
                tasks: r.get::<_, i64>(5)?.unsigned_abs(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    let types = {
        let mut stmt = tx.prepare_cached(
            "SELECT y.id, y.name, y.glyph, y.hue, y.is_default,
                    (SELECT count(*) FROM tasks t WHERE t.type_id = y.id AND t.deleted_at IS NULL)
               FROM task_types y WHERE y.project_id = ?1 ORDER BY y.rank, y.id",
        )?;
        stmt.query_map([b(id)], |r| {
            Ok(TaskType {
                id: id_at(r, 0)?.to_string(),
                name: r.get(1)?,
                glyph: parse_enum(&r.get::<_, String>(2)?).unwrap_or(Glyph::Task),
                hue: parse_enum(&r.get::<_, String>(3)?).unwrap_or(Hue::Grey),
                is_default: r.get::<_, i64>(4)? != 0,
                tasks: r.get::<_, i64>(5)?.unsigned_abs(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    let labels = {
        let mut stmt = tx.prepare_cached(
            "SELECT l.id, l.name, l.hue,
                    (SELECT count(*) FROM task_labels tl JOIN tasks t ON t.id = tl.task_id
                      WHERE tl.label_id = l.id AND t.deleted_at IS NULL)
               FROM labels l WHERE l.project_id = ?1 ORDER BY l.rank, l.id",
        )?;
        stmt.query_map([b(id)], |r| {
            Ok(Label {
                id: id_at(r, 0)?.to_string(),
                name: r.get(1)?,
                hue: parse_enum(&r.get::<_, String>(2)?).unwrap_or(Hue::Grey),
                tasks: r.get::<_, i64>(3)?.unsigned_abs(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    let (open, total): (i64, i64) = tx.query_row(
        "SELECT count(*) FILTER (WHERE s.category IN ('todo', 'in_progress')), count(*)
           FROM tasks t JOIN statuses s ON s.id = t.status_id
          WHERE t.project_id = ?1 AND t.deleted_at IS NULL",
        [b(id)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (path, _) = path_of(tx, id)?;
    Ok(Project {
        id: id.to_string(),
        key,
        old_keys,
        name,
        path,
        description,
        hue: parse_enum(&hue).unwrap_or(Hue::Blue),
        icon,
        state: if state == "archived" {
            ProjectState::Archived
        } else {
            ProjectState::Active
        },
        parent: parent_key,
        children,
        modules: modules_of(&modules),
        statuses,
        types,
        labels,
        open_tasks: open.unsigned_abs(),
        total_tasks: total.unsigned_abs(),
        created_at: rfc3339(created),
        updated_at: rfc3339(updated),
    })
}

struct TreeRow {
    id: Id,
    parent: Option<Id>,
    key: String,
    name: String,
    hue: String,
    icon: Option<String>,
    archived: bool,
    open: u64,
}

fn tree(tx: &Transaction<'_>) -> Result<Vec<TreeRow>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT p.id, p.parent_id, p.key, p.name, p.hue, p.icon, p.state,
                (SELECT count(*) FROM tasks t JOIN statuses s ON s.id = t.status_id
                  WHERE t.project_id = p.id AND t.deleted_at IS NULL
                    AND s.category IN ('todo', 'in_progress'))
           FROM projects p WHERE p.deleted_at IS NULL ORDER BY p.rank, p.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(TreeRow {
                id: id_at(r, 0)?,
                parent: opt_id_at(r, 1)?,
                key: r.get(2)?,
                name: r.get(3)?,
                hue: r.get(4)?,
                icon: r.get(5)?,
                archived: r.get::<_, String>(6)? == "archived",
                open: r.get::<_, i64>(7)?.unsigned_abs(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn list(q: &QueryCtx<'_>, input: ProjectListInput) -> Result<ProjectList, Failure> {
    let rows = tree(q.tx)?;
    fn subtree_open(rows: &[TreeRow], id: Id) -> u64 {
        let own = rows.iter().find(|r| r.id == id).map_or(0, |r| r.open);
        own + rows
            .iter()
            .filter(|r| r.parent == Some(id))
            .map(|r| subtree_open(rows, r.id))
            .sum::<u64>()
    }
    fn walk(
        rows: &[TreeRow],
        parent: Option<Id>,
        depth: u32,
        path: &str,
        parent_key: Option<&str>,
        include_archived: bool,
        out: &mut Vec<ProjectSummary>,
    ) {
        for r in rows.iter().filter(|r| r.parent == parent) {
            if r.archived && !include_archived {
                continue;
            }
            let path = if path.is_empty() {
                r.name.clone()
            } else {
                format!("{path} / {}", r.name)
            };
            out.push(ProjectSummary {
                key: r.key.clone(),
                name: r.name.clone(),
                path: path.clone(),
                parent: parent_key.map(str::to_owned),
                depth,
                hue: parse_enum(&r.hue).unwrap_or(Hue::Blue),
                icon: r.icon.clone(),
                state: if r.archived {
                    ProjectState::Archived
                } else {
                    ProjectState::Active
                },
                open_tasks: r.open,
                open_tasks_with_subprojects: subtree_open(rows, r.id),
                id: r.id.to_string(),
            });
            walk(
                rows,
                Some(r.id),
                depth + 1,
                &path,
                Some(&r.key),
                include_archived,
                out,
            );
        }
    }
    let mut items = Vec::new();
    walk(&rows, None, 0, "", None, input.include_archived, &mut items);
    Ok(ProjectList {
        total: items.len() as u64,
        items,
    })
}

/// The project `id` and every project below it, parents first.
pub(crate) fn subtree(tx: &Transaction<'_>, id: Id) -> Result<Vec<Id>, Failure> {
    let mut out = vec![id];
    let mut i = 0;
    while i < out.len() {
        let mut stmt = tx.prepare_cached(
            "SELECT id FROM projects WHERE parent_id = ?1 AND deleted_at IS NULL ORDER BY rank, id",
        )?;
        let children = stmt
            .query_map([b(out[i])], |r| id_at(r, 0))?
            .collect::<Result<Vec<_>, _>>()?;
        out.extend(children);
        i += 1;
    }
    Ok(out)
}

/// Levels below `id`: 0 when it has no sub-projects.
fn height(tx: &Transaction<'_>, id: Id) -> Result<usize, Failure> {
    let mut stmt =
        tx.prepare_cached("SELECT id FROM projects WHERE parent_id = ?1 AND deleted_at IS NULL")?;
    let children = stmt
        .query_map([b(id)], |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut most = 0;
    for c in children {
        most = most.max(1 + height(tx, c)?);
    }
    Ok(most)
}

// ---------------------------------------------------------------------
// Creating

fn key_taken(tx: &Transaction<'_>, key: &str, except: Option<Id>) -> Result<bool, Failure> {
    let owner: Option<Vec<u8>> = tx
        .query_row("SELECT id FROM projects WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    let alias: Option<Vec<u8>> = tx
        .query_row(
            "SELECT project_id FROM project_key_aliases WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    let except = except.map(b);
    Ok(owner.is_some_and(|o| Some(&o) != except.as_ref())
        || alias.is_some_and(|a| Some(&a) != except.as_ref()))
}

fn check_key(tx: &Transaction<'_>, key: &str, except: Option<Id>) -> Result<ProjectKey, Failure> {
    let key = ProjectKey::new(key.trim()).map_err(|e| invalid("key", e))?;
    if key_taken(tx, key.as_str(), except)? {
        return Err(OpError::conflict(format!(
            "The key {} is already used by a project, now or before.",
            key.as_str()
        ))
        .with_detail("field", "key")
        .with_hint("Choose another key; old keys stay reserved so links keep working.")
        .into());
    }
    Ok(key)
}

fn create(w: &mut WriteCtx<'_>, input: ProjectCreateInput) -> Result<ProjectChange, Failure> {
    let name = clean_name("name", "name", &input.name, PROJECT_NAME_MAX)?;
    let key = check_key(w.tx(), &input.key, None)?;
    let parent = match &input.parent {
        Some(text) => {
            let p = lookup::project(w.tx(), "parent", text)?;
            lookup::ensure_active(&p)?;
            let (_, depth) = path_of(w.tx(), p.id)?;
            if depth + 1 >= MAX_LEVELS {
                return Err(OpError::invalid_at(
                    "parent",
                    format!("Projects go at most {MAX_LEVELS} levels deep; {} is already on the last level.", p.key),
                )
                .into());
            }
            Some(p)
        }
        None => None,
    };
    let description = input.description.unwrap_or_default();
    gk_core::check_markdown("description", &description).map_err(|e| invalid("description", e))?;
    let parent_id = parent.as_ref().map(|p| p.id);
    let (hue, modules) = match parent_id {
        Some(p) => w.tx().query_row(
            "SELECT hue, modules FROM projects WHERE id = ?1",
            [b(p)],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )?,
        None => (
            "blue".to_owned(),
            modules_value(Modules::default()).to_string(),
        ),
    };
    let hue = input.hue.map_or(hue, enum_text);
    let siblings = match parent_id {
        Some(p) => lookup::ranked(
            w.tx(),
            "projects",
            "parent_id = ?1 AND deleted_at IS NULL",
            &b(p),
        )?,
        None => lookup::ranked(
            w.tx(),
            "projects",
            "parent_id IS NULL AND deleted_at IS NULL AND ?1 = 1",
            &1,
        )?,
    };
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let mut rank = Rank::first();
    for (item, r) in lookup::place(&siblings, id, Target::Bottom)? {
        if item == id {
            rank = r;
        } else {
            fields::set(w, PROJECT, item, Some(item), "rank", text(r.as_str()))?;
        }
    }
    w.tx().execute(
        "INSERT INTO projects (id, parent_id, rank, key, name, name_fold, description, hue, icon,
                               modules, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
        params![
            b(id),
            parent_id.map(b),
            rank.as_str(),
            key.as_str(),
            name,
            fold(&name),
            description,
            hue,
            input
                .icon
                .as_deref()
                .map(str::trim)
                .filter(|i| !i.is_empty()),
            modules,
            w.now()
        ],
    )?;
    copy_configuration(w, id, parent_id)?;
    super::description::initial(w, super::description::Owner::Project(id), &description)?;
    w.record_field(
        PROJECT,
        id,
        Some(id),
        "created",
        None,
        Some(Value::Bool(true)),
    );
    w.record_field(PROJECT, id, Some(id), "key", None, text(key.as_str()));
    w.record_field(PROJECT, id, Some(id), "name", None, text(name.as_str()));
    if let Some(p) = parent_id {
        w.record_field(PROJECT, id, Some(id), "parent", None, id_value(p));
    }
    changed(w, id)
}

/// Gives a new project its statuses, types and labels: copies of its
/// parent's, or the defaults.
fn copy_configuration(w: &WriteCtx<'_>, id: Id, parent: Option<Id>) -> Result<(), Failure> {
    let tx = w.tx();
    let new_id = || Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    if let Some(parent) = parent {
        let mut stmt = tx.prepare(
            "SELECT name, name_fold, category, hue, rank, wip_limit FROM statuses WHERE project_id = ?1",
        )?;
        let rows = stmt
            .query_map([b(parent)], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (name, name_fold, category, hue, rank, wip) in rows {
            tx.execute(
                "INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank, wip_limit)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![b(new_id()), b(id), name, name_fold, category, hue, rank, wip],
            )?;
        }
        let mut stmt = tx.prepare(
            "SELECT name, name_fold, glyph, hue, rank, is_default FROM task_types WHERE project_id = ?1",
        )?;
        let rows = stmt
            .query_map([b(parent)], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (name, name_fold, glyph, hue, rank, is_default) in rows {
            tx.execute(
                "INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank, is_default)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![b(new_id()), b(id), name, name_fold, glyph, hue, rank, is_default],
            )?;
        }
        let mut stmt =
            tx.prepare("SELECT name, name_fold, hue, rank FROM labels WHERE project_id = ?1")?;
        let rows = stmt
            .query_map([b(parent)], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (name, name_fold, hue, rank) in rows {
            tx.execute(
                "INSERT INTO labels (id, project_id, name, name_fold, hue, rank)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![b(new_id()), b(id), name, name_fold, hue, rank],
            )?;
        }
        return Ok(());
    }
    let ranks = Rank::spread(DEFAULT_STATUSES.len());
    for (s, rank) in DEFAULT_STATUSES.iter().zip(&ranks) {
        tx.execute(
            "INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                b(new_id()),
                b(id),
                s.name,
                fold(s.name),
                s.category.as_str(),
                s.hue.as_str(),
                rank.as_str()
            ],
        )?;
    }
    let ranks = Rank::spread(DEFAULT_TASK_TYPES.len());
    for (t, rank) in DEFAULT_TASK_TYPES.iter().zip(&ranks) {
        tx.execute(
            "INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank, is_default)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                b(new_id()),
                b(id),
                t.name,
                fold(t.name),
                t.glyph.as_str(),
                t.hue.as_str(),
                rank.as_str(),
                i64::from(t.is_default)
            ],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Changing

fn update_parts(input: &Value) -> Vec<&'static str> {
    let mut parts = Vec::new();
    let has = |v: &Value, k: &str| v.get(k).is_some_and(|x| !x.is_null());
    let flag = |v: &Value, k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
    if has(input, "key") {
        parts.push("project.change_key");
    }
    let clears_parent = input
        .get("clear")
        .and_then(Value::as_array)
        .is_some_and(|c| c.iter().any(|f| f == "parent"));
    if ["parent", "before", "after", "at"]
        .iter()
        .any(|k| has(input, k))
        || clears_parent
    {
        parts.push("project.move");
    }
    match input.get("archived").and_then(Value::as_bool) {
        Some(true) => parts.push("project.archive"),
        Some(false) => parts.push("project.unarchive"),
        None => {}
    }
    let lists: [(&str, &str, [&'static str; 4]); 3] = [
        (
            "statuses",
            "status",
            [
                "status.create",
                "status.delete",
                "status.update",
                "status.reorder",
            ],
        ),
        (
            "types",
            "type",
            ["type.create", "type.delete", "type.update", "type.update"],
        ),
        (
            "labels",
            "label",
            [
                "label.create",
                "label.delete",
                "label.update",
                "label.update",
            ],
        ),
    ];
    for (list, field, [create, delete, change, reorder]) in lists {
        for item in input
            .get(list)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let op = if !has(item, field) {
                create
            } else if flag(item, "remove") {
                delete
            } else if ["before", "after", "at"].iter().any(|k| has(item, k))
                && !["name", "category", "hue", "wip_limit", "glyph", "clear"]
                    .iter()
                    .any(|k| has(item, k))
                && !flag(item, "make_default")
            {
                reorder
            } else {
                change
            };
            if !parts.contains(&op) {
                parts.push(op);
            }
        }
    }
    parts
}

fn update(w: &mut WriteCtx<'_>, input: ProjectUpdateInput) -> Result<ProjectChange, Failure> {
    let p = lookup::project(w.tx(), "project", &input.project)?;
    let clears = |f: ProjectField| input.clear.contains(&f);
    for (given, field, name) in [
        (input.icon.is_some(), ProjectField::Icon, "icon"),
        (
            input.description.is_some(),
            ProjectField::Description,
            "description",
        ),
        (input.parent.is_some(), ProjectField::Parent, "parent"),
    ] {
        if given && clears(field) {
            return Err(
                OpError::invalid_at("clear", format!("{name} is both given and cleared.")).into(),
            );
        }
    }
    if input.archived == Some(false) {
        set_archived(w, &p, false)?;
    }
    let p = lookup::project_by_id(w.tx(), p.id)?.unwrap_or(p);
    let other_changes = input.name.is_some()
        || input.description.is_some()
        || input.hue.is_some()
        || input.icon.is_some()
        || input.modules.is_some()
        || input.key.is_some()
        || input.parent.is_some()
        || input.before.is_some()
        || input.after.is_some()
        || input.at.is_some()
        || !input.statuses.is_empty()
        || !input.types.is_empty()
        || !input.labels.is_empty()
        || !input.clear.is_empty();
    if other_changes {
        lookup::ensure_active(&p)?;
    }
    let pid = Some(p.id);
    if let Some(name) = &input.name {
        let name = clean_name("name", "name", name, PROJECT_NAME_MAX)?;
        fields::set(w, PROJECT, p.id, pid, "name", text(name))?;
    }
    if let Some(description) = &input.description {
        gk_core::check_markdown("description", description)
            .map_err(|e| invalid("description", e))?;
        super::description::save(
            w,
            super::description::Owner::Project(p.id),
            pid,
            description,
            None,
            "normal",
        )?;
    }
    if clears(ProjectField::Description) {
        super::description::save(
            w,
            super::description::Owner::Project(p.id),
            pid,
            "",
            None,
            "normal",
        )?;
    }
    if let Some(hue) = input.hue {
        fields::set(w, PROJECT, p.id, pid, "hue", text(enum_text(hue)))?;
    }
    if let Some(icon) = &input.icon {
        let icon = icon.trim();
        fields::set(
            w,
            PROJECT,
            p.id,
            pid,
            "icon",
            (!icon.is_empty()).then(|| Value::from(icon)),
        )?;
    }
    if clears(ProjectField::Icon) {
        fields::set(w, PROJECT, p.id, pid, "icon", None)?;
    }
    if let Some(patch) = input.modules {
        set_modules(w, p.id, patch)?;
    }
    if let Some(key) = &input.key {
        change_key(w, &p, key)?;
    }
    let parent = if clears(ProjectField::Parent) {
        Some(None)
    } else {
        input.parent.as_deref().map(Some)
    };
    if parent.is_some() || input.before.is_some() || input.after.is_some() || input.at.is_some() {
        move_project(
            w,
            &p,
            parent,
            input.before.as_deref(),
            input.after.as_deref(),
            input.at,
        )?;
    }
    for (i, change) in input.statuses.iter().enumerate() {
        apply_status(w, &p, change).map_err(|e| at_index(e, "statuses", i))?;
    }
    for (i, change) in input.types.iter().enumerate() {
        apply_type(w, &p, change).map_err(|e| at_index(e, "types", i))?;
    }
    for (i, change) in input.labels.iter().enumerate() {
        apply_label(w, &p, change).map_err(|e| at_index(e, "labels", i))?;
    }
    if input.archived == Some(true) {
        set_archived(w, &p, true)?;
    }
    changed(w, p.id)
}

/// Puts the list and index of a sub-change in front of an error's field.
fn at_index(e: Failure, list: &str, i: usize) -> Failure {
    match e {
        Failure::Op(mut op) => {
            let inner = op
                .details
                .get("path")
                .or_else(|| op.details.get("field"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            let path = match inner {
                Some(f) if !f.is_empty() => format!("{list}[{i}].{f}"),
                _ => format!("{list}[{i}]"),
            };
            op.details.insert("path".to_owned(), Value::String(path));
            Failure::Op(op)
        }
        other => other,
    }
}

fn set_modules(w: &mut WriteCtx<'_>, id: Id, patch: ModulesPatch) -> Result<(), Failure> {
    let current: String =
        w.tx()
            .query_row("SELECT modules FROM projects WHERE id = ?1", [b(id)], |r| {
                r.get(0)
            })?;
    let mut m = modules_of(&current);
    for (wanted, slot) in [
        (patch.board, &mut m.board),
        (patch.list, &mut m.list),
        (patch.gantt, &mut m.gantt),
        (patch.wiki, &mut m.wiki),
        (patch.time, &mut m.time),
    ] {
        if let Some(on) = wanted {
            *slot = on;
        }
    }
    fields::set(w, PROJECT, id, Some(id), "modules", Some(modules_value(m)))?;
    Ok(())
}

/// Gives project `p` the key `new`, keeping the old one as an alias.
fn change_key(w: &mut WriteCtx<'_>, p: &ProjectRow, new: &str) -> Result<(), Failure> {
    let key = check_key(w.tx(), new, Some(p.id))?;
    if key.as_str() == p.key {
        return Ok(());
    }
    rekey(w, p.id, &p.key, key.as_str())?;
    fields::set(w, PROJECT, p.id, Some(p.id), "key", text(key.as_str()))?;
    Ok(())
}

/// Keeps the key aliases in step when project `id` goes from key `old` to
/// `new`: `old` becomes an alias, and `new` stops being one.
pub(crate) fn rekey(w: &WriteCtx<'_>, id: Id, old: &str, new: &str) -> Result<(), Failure> {
    w.tx()
        .execute("DELETE FROM project_key_aliases WHERE key = ?1", [new])?;
    w.tx().execute(
        "INSERT INTO project_key_aliases (key, project_id) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET project_id = excluded.project_id",
        params![old, b(id)],
    )?;
    Ok(())
}

fn move_project(
    w: &mut WriteCtx<'_>,
    p: &ProjectRow,
    parent: Option<Option<&str>>,
    before: Option<&str>,
    after: Option<&str>,
    at: Option<Edge>,
) -> Result<(), Failure> {
    let new_parent = match parent {
        None => p.parent,
        Some(None) => None,
        Some(Some(text)) => {
            let q = lookup::project(w.tx(), "parent", text)?;
            lookup::ensure_active(&q)?;
            let below = subtree(w.tx(), p.id)?;
            if below.contains(&q.id) {
                return Err(OpError::invalid_at(
                    "parent",
                    format!(
                        "{} can't go under itself or one of its own sub-projects.",
                        p.key
                    ),
                )
                .into());
            }
            let (_, depth) = path_of(w.tx(), q.id)?;
            let levels = depth + 2 + height(w.tx(), p.id)?;
            if levels > MAX_LEVELS {
                return Err(OpError::invalid_at(
                    "parent",
                    format!(
                        "Projects go at most {MAX_LEVELS} levels deep; under {} {} would need {levels}.",
                        q.key, p.key
                    ),
                )
                .into());
            }
            Some(q.id)
        }
    };
    let sibling = |field: &str, text: &str| -> Result<Id, Failure> {
        let s = lookup::project(w.tx(), field, text)?;
        if s.parent != new_parent || s.id == p.id {
            return Err(OpError::invalid_at(
                field,
                format!("{} is not a sibling of {} at its new place.", s.key, p.key),
            )
            .into());
        }
        Ok(s.id)
    };
    let before = before.map(|t| sibling("before", t)).transpose()?;
    let after = after.map(|t| sibling("after", t)).transpose()?;
    let target = Target::from_parts(before, after, at)?;
    let parent_changes = new_parent != p.parent;
    if parent_changes {
        fields::set(
            w,
            PROJECT,
            p.id,
            Some(p.id),
            "parent",
            new_parent.and_then(id_value),
        )?;
    }
    let target = match (target, parent_changes) {
        (Some(t), _) => t,
        (None, true) => Target::Bottom,
        (None, false) => return Ok(()),
    };
    let siblings = match new_parent {
        Some(q) => lookup::ranked(
            w.tx(),
            "projects",
            "parent_id = ?1 AND deleted_at IS NULL",
            &b(q),
        )?,
        None => lookup::ranked(
            w.tx(),
            "projects",
            "parent_id IS NULL AND deleted_at IS NULL AND ?1 = 1",
            &1,
        )?,
    };
    for (id, rank) in lookup::place(&siblings, p.id, target)? {
        fields::set(w, PROJECT, id, Some(id), "rank", text(rank.as_str()))?;
    }
    Ok(())
}

fn set_archived(w: &mut WriteCtx<'_>, p: &ProjectRow, archived: bool) -> Result<(), Failure> {
    if !archived
        && let Some(parent) = p.parent
        && lookup::project_by_id(w.tx(), parent)?.is_some_and(|q| q.archived)
    {
        return Err(OpError::forbidden(format!(
            "{} is inside an archived project; unarchive that one instead.",
            p.key
        ))
        .into());
    }
    let state = if archived { "archived" } else { "active" };
    for id in subtree(w.tx(), p.id)? {
        fields::set(w, PROJECT, id, Some(id), "state", text(state))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Statuses, types and labels

/// Checks that no other item of `list` in `project` has `name` (ignoring
/// case).
fn unique_name(
    tx: &Transaction<'_>,
    list: List,
    project: &ProjectRow,
    name: &str,
    except: Option<Id>,
) -> Result<(), Failure> {
    let wanted = fold(name);
    let clash = lookup::all(tx, list, project.id)?
        .into_iter()
        .find(|n| fold(&n.name) == wanted && Some(n.id) != except);
    match clash {
        Some(n) => Err(OpError::conflict(format!(
            "Project {} already has \"{}\".",
            project.key, n.name
        ))
        .with_detail("field", "name")
        .into()),
        None => Ok(()),
    }
}

fn target_in(
    w: &WriteCtx<'_>,
    list: List,
    p: &ProjectRow,
    before: Option<&str>,
    after: Option<&str>,
    at: Option<Edge>,
) -> Result<Option<Target>, Failure> {
    let before = before
        .map(|t| lookup::named(w.tx(), list, p, "before", t).map(|n| n.id))
        .transpose()?;
    let after = after
        .map(|t| lookup::named(w.tx(), list, p, "after", t).map(|n| n.id))
        .transpose()?;
    Target::from_parts(before, after, at)
}

fn new_rank(
    w: &mut WriteCtx<'_>,
    list: List,
    entity: &str,
    p: &ProjectRow,
    id: Id,
    target: Target,
) -> Result<Rank, Failure> {
    let siblings = lookup::ranked(w.tx(), list.table(), "project_id = ?1", &b(p.id))?;
    let mut mine = None;
    for (item, rank) in lookup::place(&siblings, id, target)? {
        if item == id {
            mine = Some(rank);
        } else {
            fields::set(w, entity, item, Some(p.id), "rank", text(rank.as_str()))?;
        }
    }
    match mine {
        Some(r) => Ok(r),
        None => Ok(Rank::parse(
            siblings
                .iter()
                .find(|(i, _)| *i == id)
                .map_or("i", |(_, r)| r.as_str()),
        )
        .unwrap_or_else(|_| Rank::first())),
    }
}

fn wip(limit: Option<u32>) -> Result<Option<Value>, Failure> {
    match limit {
        Some(0) => Err(OpError::invalid_at("wip_limit", "A WIP limit is 1 or more.").into()),
        Some(n) => Ok(Some(Value::from(n))),
        None => Ok(None),
    }
}

fn apply_status(w: &mut WriteCtx<'_>, p: &ProjectRow, c: &StatusChange) -> Result<(), Failure> {
    let pid = Some(p.id);
    let clears_wip = c.clear.contains(&StatusField::WipLimit);
    if clears_wip && c.wip_limit.is_some() {
        return Err(OpError::invalid_at("clear", "wip_limit is both given and cleared.").into());
    }
    let Some(which) = &c.status else {
        let Some(name) = &c.name else {
            return Err(OpError::invalid_at("name", "A new status needs a name.").into());
        };
        let name = clean_name("name", "name", name, ITEM_NAME_MAX)?;
        unique_name(w.tx(), List::Statuses, p, &name, None)?;
        let category = c.category.unwrap_or(Category::Todo);
        let hue = c.hue.unwrap_or(Hue::Grey);
        let limit = wip(c.wip_limit)?;
        let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
        let target = target_in(
            w,
            List::Statuses,
            p,
            c.before.as_deref(),
            c.after.as_deref(),
            c.at,
        )?
        .unwrap_or(Target::Bottom);
        let rank = new_rank(w, List::Statuses, STATUS, p, id, target)?;
        w.tx().execute(
            "INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank, wip_limit)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                b(id),
                b(p.id),
                name,
                fold(&name),
                enum_text(category),
                enum_text(hue),
                rank.as_str(),
                limit.and_then(|v| v.as_i64())
            ],
        )?;
        w.record_field(STATUS, id, pid, "created", None, Some(Value::Bool(true)));
        w.record_field(STATUS, id, pid, "name", None, text(name));
        return Ok(());
    };
    let status = lookup::named(w.tx(), List::Statuses, p, "status", which)?;
    if c.remove {
        return delete_status(w, p, &status, c.move_tasks_to.as_deref());
    }
    if let Some(name) = &c.name {
        let name = clean_name("name", "name", name, ITEM_NAME_MAX)?;
        unique_name(w.tx(), List::Statuses, p, &name, Some(status.id))?;
        fields::set(w, STATUS, status.id, pid, "name", text(name))?;
    }
    if let Some(category) = c.category {
        let before: StatusCategory = status.kind.parse().unwrap_or(StatusCategory::Todo);
        let after: StatusCategory = enum_text(category).parse().unwrap_or(StatusCategory::Todo);
        if fields::set(
            w,
            STATUS,
            status.id,
            pid,
            "category",
            text(enum_text(category)),
        )? && before.is_closed() != after.is_closed()
        {
            reclose_tasks(w, p.id, status.id, before, after)?;
        }
    }
    if let Some(hue) = c.hue {
        fields::set(w, STATUS, status.id, pid, "hue", text(enum_text(hue)))?;
    }
    if c.wip_limit.is_some() {
        fields::set(w, STATUS, status.id, pid, "wip_limit", wip(c.wip_limit)?)?;
    }
    if clears_wip {
        fields::set(w, STATUS, status.id, pid, "wip_limit", None)?;
    }
    if let Some(target) = target_in(
        w,
        List::Statuses,
        p,
        c.before.as_deref(),
        c.after.as_deref(),
        c.at,
    )? {
        let rank = new_rank(w, List::Statuses, STATUS, p, status.id, target)?;
        fields::set(w, STATUS, status.id, pid, "rank", text(rank.as_str()))?;
    }
    Ok(())
}

/// Tasks in a status whose category changed between open and closed close
/// or reopen with it.
fn reclose_tasks(
    w: &mut WriteCtx<'_>,
    project: Id,
    status: Id,
    from: StatusCategory,
    to: StatusCategory,
) -> Result<(), Failure> {
    let tasks: Vec<(Id, Option<i64>)> = {
        let mut stmt = w.tx().prepare(
            "SELECT id, closed_at FROM tasks WHERE status_id = ?1 AND deleted_at IS NULL",
        )?;
        stmt.query_map([b(status)], |r| Ok((id_at(r, 0)?, r.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?
    };
    let now = w.now();
    for (task, closed) in tasks {
        let after = gk_core::closed_at_after_move(from, to, closed, now);
        fields::set(
            w,
            TASK,
            task,
            Some(project),
            "closed_at",
            after.map(Value::from),
        )?;
    }
    Ok(())
}

fn delete_status(
    w: &mut WriteCtx<'_>,
    p: &ProjectRow,
    status: &Named,
    move_to: Option<&str>,
) -> Result<(), Failure> {
    let count: i64 = w.tx().query_row(
        "SELECT count(*) FROM tasks WHERE status_id = ?1",
        [b(status.id)],
        |r| r.get(0),
    )?;
    let all = lookup::all(w.tx(), List::Statuses, p.id)?;
    if all.len() <= 1 {
        return Err(OpError::invalid_at("status", "A project needs at least one status.").into());
    }
    if count > 0 {
        let Some(move_to) = move_to else {
            let others: Vec<String> = all
                .iter()
                .filter(|s| s.id != status.id)
                .map(|s| s.name.clone())
                .collect();
            return Err(OpError::invalid_at(
                "move_tasks_to",
                format!(
                    "\"{}\" has {count} tasks; say which status they move to.",
                    status.name
                ),
            )
            .with_detail("choices", others)
            .into());
        };
        let target = lookup::named(w.tx(), List::Statuses, p, "move_tasks_to", move_to)?;
        if target.id == status.id {
            return Err(OpError::invalid_at(
                "move_tasks_to",
                "The tasks must move to another status.",
            )
            .into());
        }
        let from: StatusCategory = status.kind.parse().unwrap_or(StatusCategory::Todo);
        let to: StatusCategory = target.kind.parse().unwrap_or(StatusCategory::Todo);
        let tasks: Vec<(Id, Id, Option<i64>)> = {
            let mut stmt = w.tx().prepare(
                "SELECT id, project_id, closed_at FROM tasks WHERE status_id = ?1 ORDER BY rank",
            )?;
            stmt.query_map([b(status.id)], |r| {
                Ok((id_at(r, 0)?, id_at(r, 1)?, r.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
        };
        let mut last = lookup::ranked(
            w.tx(),
            "tasks",
            "status_id = ?1 AND deleted_at IS NULL",
            &b(target.id),
        )?
        .last()
        .and_then(|(_, r)| Rank::parse(r).ok());
        let now = w.now();
        for (task, project, closed) in tasks {
            let rank = last.as_ref().map_or_else(Rank::first, Rank::after);
            fields::set(w, TASK, task, Some(project), "status", id_value(target.id))?;
            fields::set(w, TASK, task, Some(project), "rank", text(rank.as_str()))?;
            let after = gk_core::closed_at_after_move(from, to, closed, now);
            fields::set(
                w,
                TASK,
                task,
                Some(project),
                "closed_at",
                after.map(Value::from),
            )?;
            last = Some(rank);
        }
    }
    record_removal(w, STATUS, status, p.id)?;
    w.tx()
        .execute("DELETE FROM statuses WHERE id = ?1", [b(status.id)])?;
    Ok(())
}

/// Records that `item` was deleted, with its last name.
fn record_removal(
    w: &mut WriteCtx<'_>,
    entity: &str,
    item: &Named,
    project: Id,
) -> Result<(), Failure> {
    w.record_field(
        entity,
        item.id,
        Some(project),
        "deleted",
        None,
        Some(Value::Bool(true)),
    );
    w.record_field(
        entity,
        item.id,
        Some(project),
        "name",
        text(item.name.as_str()),
        None,
    );
    Ok(())
}

fn apply_type(w: &mut WriteCtx<'_>, p: &ProjectRow, c: &TypeChange) -> Result<(), Failure> {
    let pid = Some(p.id);
    let Some(which) = &c.task_type else {
        let Some(name) = &c.name else {
            return Err(OpError::invalid_at("name", "A new task type needs a name.").into());
        };
        let name = clean_name("name", "name", name, ITEM_NAME_MAX)?;
        unique_name(w.tx(), List::Types, p, &name, None)?;
        let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
        let target = target_in(
            w,
            List::Types,
            p,
            c.before.as_deref(),
            c.after.as_deref(),
            c.at,
        )?
        .unwrap_or(Target::Bottom);
        let rank = new_rank(w, List::Types, TYPE, p, id, target)?;
        w.tx().execute(
            "INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank, is_default)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
            params![
                b(id),
                b(p.id),
                name,
                fold(&name),
                enum_text(c.glyph.unwrap_or(Glyph::Task)),
                enum_text(c.hue.unwrap_or(Hue::Grey)),
                rank.as_str()
            ],
        )?;
        w.record_field(TYPE, id, pid, "created", None, Some(Value::Bool(true)));
        w.record_field(TYPE, id, pid, "name", None, text(name));
        if c.make_default {
            make_default(w, p, id)?;
        }
        return Ok(());
    };
    let item = lookup::named(w.tx(), List::Types, p, "type", which)?;
    if c.remove {
        return delete_type(w, p, &item, c.move_tasks_to.as_deref());
    }
    if let Some(name) = &c.name {
        let name = clean_name("name", "name", name, ITEM_NAME_MAX)?;
        unique_name(w.tx(), List::Types, p, &name, Some(item.id))?;
        fields::set(w, TYPE, item.id, pid, "name", text(name))?;
    }
    if let Some(glyph) = c.glyph {
        fields::set(w, TYPE, item.id, pid, "glyph", text(enum_text(glyph)))?;
    }
    if let Some(hue) = c.hue {
        fields::set(w, TYPE, item.id, pid, "hue", text(enum_text(hue)))?;
    }
    if c.make_default {
        make_default(w, p, item.id)?;
    }
    if let Some(target) = target_in(
        w,
        List::Types,
        p,
        c.before.as_deref(),
        c.after.as_deref(),
        c.at,
    )? {
        let rank = new_rank(w, List::Types, TYPE, p, item.id, target)?;
        fields::set(w, TYPE, item.id, pid, "rank", text(rank.as_str()))?;
    }
    Ok(())
}

fn default_type(tx: &Transaction<'_>, project: Id) -> Result<Option<Id>, Failure> {
    Ok(tx
        .query_row(
            "SELECT id FROM task_types WHERE project_id = ?1 AND is_default = 1",
            [b(project)],
            |r| id_at(r, 0),
        )
        .optional()?)
}

fn make_default(w: &mut WriteCtx<'_>, p: &ProjectRow, id: Id) -> Result<(), Failure> {
    if let Some(old) = default_type(w.tx(), p.id)?
        && old != id
    {
        fields::set(
            w,
            TYPE,
            old,
            Some(p.id),
            "is_default",
            Some(Value::Bool(false)),
        )?;
    }
    fields::set(
        w,
        TYPE,
        id,
        Some(p.id),
        "is_default",
        Some(Value::Bool(true)),
    )?;
    Ok(())
}

fn delete_type(
    w: &mut WriteCtx<'_>,
    p: &ProjectRow,
    item: &Named,
    move_to: Option<&str>,
) -> Result<(), Failure> {
    let default = default_type(w.tx(), p.id)?;
    if default == Some(item.id) {
        return Err(OpError::invalid_at(
            "type",
            format!(
                "\"{}\" is the default type; make another type the default first.",
                item.name
            ),
        )
        .into());
    }
    let target = match move_to {
        Some(t) => lookup::named(w.tx(), List::Types, p, "move_tasks_to", t)?.id,
        None => default.ok_or_else(|| {
            Failure::Op(OpError::invalid_at(
                "move_tasks_to",
                "Say which type the tasks get.",
            ))
        })?,
    };
    if target == item.id {
        return Err(
            OpError::invalid_at("move_tasks_to", "The tasks must get another type.").into(),
        );
    }
    let tasks: Vec<(Id, Id)> = {
        let mut stmt = w
            .tx()
            .prepare("SELECT id, project_id FROM tasks WHERE type_id = ?1")?;
        stmt.query_map([b(item.id)], |r| Ok((id_at(r, 0)?, id_at(r, 1)?)))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (task, project) in tasks {
        fields::set(w, TASK, task, Some(project), "type", id_value(target))?;
    }
    record_removal(w, TYPE, item, p.id)?;
    w.tx()
        .execute("DELETE FROM task_types WHERE id = ?1", [b(item.id)])?;
    Ok(())
}

fn apply_label(w: &mut WriteCtx<'_>, p: &ProjectRow, c: &LabelChange) -> Result<(), Failure> {
    let pid = Some(p.id);
    let Some(which) = &c.label else {
        let Some(name) = &c.name else {
            return Err(OpError::invalid_at("name", "A new label needs a name.").into());
        };
        let name = clean_name("name", "name", name, ITEM_NAME_MAX)?;
        unique_name(w.tx(), List::Labels, p, &name, None)?;
        let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
        let rank = new_rank(w, List::Labels, LABEL, p, id, Target::Bottom)?;
        w.tx().execute(
            "INSERT INTO labels (id, project_id, name, name_fold, hue, rank)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                b(id),
                b(p.id),
                name,
                fold(&name),
                enum_text(c.hue.unwrap_or(Hue::Grey)),
                rank.as_str()
            ],
        )?;
        w.record_field(LABEL, id, pid, "created", None, Some(Value::Bool(true)));
        w.record_field(LABEL, id, pid, "name", None, text(name));
        return Ok(());
    };
    let item = lookup::named(w.tx(), List::Labels, p, "label", which)?;
    if c.remove {
        let tasks: Vec<(Id, Id)> = {
            let mut stmt = w.tx().prepare(
                "SELECT t.id, t.project_id FROM task_labels tl JOIN tasks t ON t.id = tl.task_id
                  WHERE tl.label_id = ?1",
            )?;
            stmt.query_map([b(item.id)], |r| Ok((id_at(r, 0)?, id_at(r, 1)?)))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for (task, project) in tasks {
            let mut ids = super::task::label_ids(w.tx(), task)?;
            ids.retain(|l| *l != item.id);
            super::task::set_labels(w, task, project, &ids)?;
        }
        record_removal(w, LABEL, &item, p.id)?;
        w.tx()
            .execute("DELETE FROM labels WHERE id = ?1", [b(item.id)])?;
        return Ok(());
    }
    if let Some(name) = &c.name {
        let name = clean_name("name", "name", name, ITEM_NAME_MAX)?;
        unique_name(w.tx(), List::Labels, p, &name, Some(item.id))?;
        fields::set(w, LABEL, item.id, pid, "name", text(name))?;
    }
    if let Some(hue) = c.hue {
        fields::set(w, LABEL, item.id, pid, "hue", text(enum_text(hue)))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_combined_update_names_its_parts() {
        let parts = update_parts(&json!({
            "project": "TM",
            "name": "New",
            "key": "TN",
            "clear": ["parent"],
            "archived": true,
            "statuses": [
                {"name": "Blocked"},
                {"status": "Review", "remove": true, "move_tasks_to": "Done"},
                {"status": "Done", "at": "top"},
                {"status": "To do", "hue": "red", "after": "Done"}
            ],
            "labels": [{"label": "bug", "name": "Bug"}]
        }));
        assert_eq!(
            parts,
            [
                "project.change_key",
                "project.move",
                "project.archive",
                "status.create",
                "status.delete",
                "status.reorder",
                "status.update",
                "label.update"
            ]
        );
        assert!(update_parts(&json!({"project": "TM", "name": "x"})).is_empty());
    }

    #[test]
    fn modules_read_numbers_and_booleans() {
        let m = modules_of(r#"{"board":1,"wiki":0,"time":false}"#);
        assert!(m.board && m.list && m.gantt);
        assert!(!m.wiki && !m.time);
    }
}
