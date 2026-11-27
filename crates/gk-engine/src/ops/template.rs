// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Task templates and project templates, and making tasks from a task
//! blueprint (templates and recurring tasks share it).
//!
//! A template is kept whole in history (one `record` field), so creating,
//! changing and deleting one can be undone and reverted. Its content names
//! statuses, types, labels, custom fields and versions by name; what a
//! project lacks is left out and reported, and so is a version that is no
//! longer open. Using a template is an ordinary creation: Undo removes
//! what it made, and it is held to the same checks as creating the tasks
//! by hand (required custom fields, status rules). Occurrences of a
//! recurring task are made from the same blueprint by the engine itself,
//! and are made anyway: see `recurrence`.
//!
//! Templates belong to the workspace's data, not to a person; the table
//! records who made each one, so it can take an owner later.

use std::collections::{BTreeMap, HashMap};

use gk_api::OpError;
use gk_api::ops::common::{Category, Glyph, Hue, Priority};
use gk_api::ops::project::{Modules, ProjectCreateInput};
use gk_api::ops::template::{
    LabelBlueprint, NamedBlueprint, PageBlueprint, ProjectBlueprint, StatusBlueprint,
    SubtaskBlueprint, TaskBlueprint, Template, TemplateApplied, TemplateApply, TemplateApplyInput,
    TemplateChange, TemplateCreate, TemplateCreateInput, TemplateDelete, TemplateGet, TemplateKind,
    TemplateList, TemplateListInput, TemplateListOp, TemplateRef, TemplateSummary, TemplateUpdate,
    TemplateUpdateInput, TypeBlueprint,
};
use gk_api::ops::wiki::WikiCreateInput;
use gk_core::{Day, Id, Rank, fold};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::builtin_templates;
use super::comment::actor;
use super::desc;
use super::lookup::{self, List, ProjectRow, Target, b, id_at, opt_id_at};
use super::project::{enum_text, parse_enum};
use super::task::{NewTask, first_status, invalid};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's name for templates.
pub(crate) const ENTITY: &str = "template";
/// The one recorded field: the whole template.
pub(crate) const RECORD: &str = "record";
/// The longest template name, in characters.
const NAME_MAX: usize = 100;
/// The longest description, in characters.
const DESCRIPTION_MAX: usize = 1000;
/// Most tasks a template makes, subtasks included.
const MAX_TASKS: usize = 500;
/// Most wiki pages a project template makes.
const MAX_PAGES: usize = 200;

pub(super) fn register(r: &mut Registry) {
    r.query::<TemplateListOp, _>(desc::<TemplateListOp>(), |q, input: TemplateListInput| {
        list(q.tx, &input)
    });
    r.query::<TemplateGet, _>(desc::<TemplateGet>(), |q, input: TemplateRef| {
        let (id, record) = find(q.tx, &input.template, input.project.as_ref())?;
        output(q.tx, id, &record)
    });
    r.command::<TemplateCreate, _>(desc::<TemplateCreate>(), create);
    r.command::<TemplateUpdate, _>(desc::<TemplateUpdate>(), update);
    r.command::<TemplateDelete, _>(desc::<TemplateDelete>(), |w, input: TemplateRef| {
        let (id, old) = find(w.tx(), &input.template, input.project.as_ref())?;
        refuse_built_in(id, &old)?;
        let template = output(w.tx(), id, &old)?;
        set(w, id, None)?;
        Ok(TemplateChange {
            template,
            deleted: true,
            group: w.group_if_changed(),
        })
    });
    r.command::<TemplateApply, _>(desc::<TemplateApply>(), apply);
}

// ---------------------------------------------------------------------
// Storage

/// A template as history records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) project: Option<String>,
    pub(crate) kind: TemplateKind,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: String,
    pub(crate) content: Value,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) updated_at: i64,
}

impl Record {
    fn project_id(&self) -> Option<Id> {
        self.project.as_deref().and_then(|p| p.parse().ok())
    }
}

const COLUMNS: &str = "id, project_id, kind, name, description, content, rank, created_at, \
                       created_by_kind, created_by_name, updated_at";

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<(Id, Record)> {
    let kind: String = r.get(2)?;
    let content: String = r.get(5)?;
    Ok((
        id_at(r, 0)?,
        Record {
            project: opt_id_at(r, 1)?.map(|p| p.to_string()),
            kind: if kind == "project" {
                TemplateKind::Project
            } else {
                TemplateKind::Task
            },
            name: r.get(3)?,
            description: r.get(4)?,
            content: serde_json::from_str(&content).unwrap_or(Value::Null),
            rank: r.get(6)?,
            created_at: r.get(7)?,
            created_by_kind: r.get(8)?,
            created_by_name: r.get(9)?,
            updated_at: r.get(10)?,
        },
    ))
}

/// The built-in templates as records, for listing, reading and using.
fn built_ins() -> Vec<(Id, Record)> {
    builtin_templates::all()
        .into_iter()
        .map(|t| {
            let content = serde_json::to_value(&t.setup).unwrap_or(Value::Null);
            (
                t.id,
                Record {
                    project: None,
                    kind: t.kind,
                    name: t.name.to_owned(),
                    description: t.description.to_owned(),
                    content,
                    rank: String::new(),
                    created_at: 0,
                    created_by_kind: "user".to_owned(),
                    created_by_name: builtin_templates::CREATOR.to_owned(),
                    updated_at: 0,
                },
            )
        })
        .collect()
}

/// Refuses to change or delete a template that comes with the program.
fn refuse_built_in(id: Id, r: &Record) -> Result<(), Failure> {
    if builtin_templates::get(id).is_some() {
        return Err(messages::hint(
            messages::invalid_at(
                "template",
                "template-built-in",
                &[("template", r.name.clone())],
            ),
            "template-built-in-hint",
            &[],
        )
        .into());
    }
    Ok(())
}

fn load(tx: &Transaction<'_>, id: Id) -> Result<Option<Record>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM templates WHERE id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?
        .map(|(_, r)| r))
}

/// Every template, project templates first, then by project and rank.
pub(crate) fn all(tx: &Transaction<'_>) -> Result<Vec<(Id, Record)>, StoreError> {
    let mut stmt = tx.prepare(&format!(
        "SELECT {COLUMNS} FROM templates ORDER BY kind = 'task', project_id, rank, id"
    ))?;
    let rows = stmt
        .query_map([], read_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The stored template `id` as history records it, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    load(tx, id)?
        .map(|r| serde_json::to_value(r).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

/// Writes template `id` as history recorded it (`None` deletes it),
/// without recording.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    w.tx()
        .execute("DELETE FROM templates WHERE id = ?1", [b(id)])?;
    let Some(value) = value else {
        return Ok(());
    };
    let r: Record = serde_json::from_value(value.clone())
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    insert_row(w.tx(), id, &r)?;
    Ok(())
}

/// Inserts a template row as it is; for history and imports.
pub(crate) fn insert_row(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), StoreError> {
    tx.execute(
        &format!(
            "INSERT INTO templates ({COLUMNS}, name_fold)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
        ),
        params![
            b(id),
            r.project_id().map(b),
            match r.kind {
                TemplateKind::Task => "task",
                TemplateKind::Project => "project",
            },
            r.name,
            r.description,
            r.content.to_string(),
            r.rank,
            r.created_at,
            r.created_by_kind,
            r.created_by_name,
            r.updated_at,
            fold(&r.name),
        ],
    )?;
    Ok(())
}

/// Writes template `id` and records the change.
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

// ---------------------------------------------------------------------
// Reading

fn task_of(r: &Record) -> Option<TaskBlueprint> {
    (r.kind == TemplateKind::Task)
        .then(|| serde_json::from_value(r.content.clone()).ok())
        .flatten()
}

fn setup_of_record(r: &Record) -> Option<ProjectBlueprint> {
    (r.kind == TemplateKind::Project)
        .then(|| serde_json::from_value(r.content.clone()).ok())
        .flatten()
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

fn output(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<Template, Failure> {
    Ok(Template {
        id: id.to_string(),
        name: r.name.clone(),
        kind: r.kind,
        project: project_key(tx, r.project_id())?,
        description: r.description.clone(),
        task: (r.kind == TemplateKind::Task).then(|| r.content.clone()),
        setup: (r.kind == TemplateKind::Project).then(|| r.content.clone()),
        rank: r.rank.clone(),
        created_by: actor(&r.created_by_kind, r.created_by_name.clone()),
        created_at: rfc3339(r.created_at),
        updated_at: rfc3339(r.updated_at),
        built_in: builtin_templates::get(id).map(|t| t.key.to_owned()),
    })
}

fn summary(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<TemplateSummary, Failure> {
    let (title, items) = match r.kind {
        TemplateKind::Task => {
            let t = task_of(r);
            let n = t.as_ref().map_or(0, |t| 1 + t.subtasks.len());
            (t.map(|t| t.title), n)
        }
        TemplateKind::Project => {
            let s = setup_of_record(r).unwrap_or_default();
            let tasks: usize = s.tasks.iter().map(|t| 1 + t.subtasks.len()).sum();
            (None, tasks + s.wiki.len())
        }
    };
    Ok(TemplateSummary {
        id: id.to_string(),
        name: r.name.clone(),
        kind: r.kind,
        project: project_key(tx, r.project_id())?,
        description: r.description.clone(),
        title,
        items: u32::try_from(items).unwrap_or(u32::MAX),
        built_in: builtin_templates::get(id).map(|t| t.key.to_owned()),
    })
}

fn visible(r: &Record) -> bool {
    r.project_id().is_none_or(crate::scope::sees)
}

fn list(tx: &Transaction<'_>, input: &TemplateListInput) -> Result<TemplateList, Failure> {
    let project = match &input.project {
        Some(p) => Some(lookup::project(tx, "project", p)?.id),
        None => None,
    };
    let items = built_ins()
        .into_iter()
        .chain(all(tx)?)
        .filter(|(_, r)| visible(r))
        .filter(|(_, r)| input.kind.is_none_or(|k| k == r.kind))
        .filter(|(_, r)| {
            r.kind == TemplateKind::Project || project.is_none_or(|p| r.project_id() == Some(p))
        })
        .map(|(id, r)| summary(tx, id, &r))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TemplateList {
        total: items.len() as u64,
        items,
    })
}

/// The template `text` names: its ID, or its name among the task templates
/// of `project` and the project templates.
fn find(
    tx: &Transaction<'_>,
    text: &str,
    project: Option<&String>,
) -> Result<(Id, Record), Failure> {
    if let Ok(id) = text.trim().parse::<Id>() {
        if let Some(r) = load(tx, id)?
            && visible(&r)
        {
            return Ok((id, r));
        }
        if let Some(found) = built_ins().into_iter().find(|(b, _)| *b == id) {
            return Ok(found);
        }
    }
    let scope = match project {
        Some(p) => Some(lookup::project(tx, "project", p)?.id),
        None => None,
    };
    let wanted = fold(text.trim());
    let candidates: Vec<(Id, Record)> = all(tx)?
        .into_iter()
        .chain(built_ins())
        .filter(|(_, r)| visible(r))
        .filter(|(_, r)| {
            r.kind == TemplateKind::Project || scope.is_none_or(|p| r.project_id() == Some(p))
        })
        .collect();
    let mut found: Vec<(Id, Record)> = candidates
        .iter()
        .filter(|(_, r)| fold(&r.name) == wanted)
        .cloned()
        .collect();
    // A task template of the named project wins over a project template of
    // the same name.
    if found.len() > 1 && scope.is_some() {
        found.retain(|(_, r)| r.kind == TemplateKind::Task);
    }
    // The workspace's own template wins over a built-in one of its name.
    if found.len() > 1 {
        found.retain(|(id, _)| builtin_templates::get(*id).is_none());
    }
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => {
            let names: Vec<&str> = candidates.iter().map(|(_, r)| r.name.as_str()).collect();
            let args = [("template", text.trim().to_owned())];
            let error = messages::with("template-not-found", &args, |m| {
                OpError::not_found(m, &names)
            })
            .with_detail("field", "template");
            Err(messages::hint(error, "template-not-found-hint", &[]).into())
        }
        _ => Err(messages::hint(
            messages::invalid_at(
                "template",
                "template-name-ambiguous",
                &[("template", text.trim().to_owned())],
            ),
            "template-name-ambiguous-hint",
            &[],
        )
        .into()),
    }
}

// ---------------------------------------------------------------------
// Blueprints from tasks and projects

/// `markdown` with every ticked checklist item cleared.
pub(crate) fn untick(markdown: &str) -> String {
    let mut bytes = markdown.as_bytes().to_vec();
    for item in gk_markdown::checklist(markdown) {
        if item.checked
            && let Some(b) = bytes.get_mut(item.symbol_offset)
            && (*b == b'x' || *b == b'X')
        {
            *b = b' ';
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| markdown.to_owned())
}

struct Row7 {
    title: String,
    description: String,
    type_name: String,
    priority: i64,
    start: Option<String>,
    due: Option<String>,
    estimate: Option<i64>,
}

fn task_fields(tx: &Transaction<'_>, id: Id) -> Result<Row7, Failure> {
    Ok(tx.query_row(
        "SELECT t.title, t.description, y.name, t.priority, t.start_date, t.due_date,
                t.estimate_min
           FROM tasks t JOIN task_types y ON y.id = t.type_id WHERE t.id = ?1",
        [b(id)],
        |r| {
            Ok(Row7 {
                title: r.get(0)?,
                description: r.get(1)?,
                type_name: r.get(2)?,
                priority: r.get(3)?,
                start: r.get(4)?,
                due: r.get(5)?,
                estimate: r.get(6)?,
            })
        },
    )?)
}

fn label_names(tx: &Transaction<'_>, id: Id) -> Result<Vec<String>, Failure> {
    Ok(super::task::task_labels(tx, id)?
        .into_iter()
        .map(|l| l.name)
        .collect())
}

fn offset(day: Option<&String>, anchor: Day) -> Option<i32> {
    let d: Day = day?.parse().ok()?;
    i32::try_from(d.days_since_epoch() - anchor.days_since_epoch()).ok()
}

fn priority_of(n: i64) -> Priority {
    parse_enum(super::fields::priority_name(n)).unwrap_or(Priority::None)
}

/// Task `id` with its live subtasks as a blueprint, dates as days after
/// `anchor`, checklists unticked.
pub(crate) fn blueprint_of(
    tx: &Transaction<'_>,
    id: Id,
    anchor: Day,
) -> Result<TaskBlueprint, Failure> {
    let ids = super::task::descendants(tx, id)?;
    let mut index: HashMap<Id, u32> = HashMap::new();
    let root = task_fields(tx, id)?;
    let mut bp = TaskBlueprint {
        fields: super::custom_field::blueprint_values(tx, id)?,
        fix_version: open_version(tx, id)?,
        sprint: super::sprint::of_task(tx, id)?
            .filter(|s| s.state == gk_api::ops::sprint::SprintState::Active)
            .map(|_| ACTIVE_SPRINT.to_owned()),
        title: root.title,
        description: untick(&root.description),
        task_type: Some(root.type_name),
        priority: priority_of(root.priority),
        labels: label_names(tx, id)?,
        estimate_minutes: root.estimate,
        start_in_days: offset(root.start.as_ref(), anchor),
        due_in_days: offset(root.due.as_ref(), anchor),
        subtasks: Vec::new(),
    };
    for sub in ids.into_iter().skip(1).take(MAX_TASKS - 1) {
        let parent: Option<Id> =
            tx.query_row("SELECT parent_id FROM tasks WHERE id = ?1", [b(sub)], |r| {
                opt_id_at(r, 0)
            })?;
        let f = task_fields(tx, sub)?;
        let under = parent.and_then(|p| index.get(&p).copied());
        index.insert(sub, u32::try_from(bp.subtasks.len()).unwrap_or(u32::MAX));
        bp.subtasks.push(SubtaskBlueprint {
            fields: super::custom_field::blueprint_values(tx, sub)?,
            fix_version: open_version(tx, sub)?,
            title: f.title,
            description: untick(&f.description),
            task_type: Some(f.type_name),
            priority: priority_of(f.priority),
            labels: label_names(tx, sub)?,
            estimate_minutes: f.estimate,
            start_in_days: offset(f.start.as_ref(), anchor),
            due_in_days: offset(f.due.as_ref(), anchor),
            under,
        });
    }
    Ok(bp)
}

/// The name of the version task `id` is planned for, while it is open.
/// The one sprint a blueprint may name.
const ACTIVE_SPRINT: &str = "active";

fn open_version(tx: &Transaction<'_>, id: Id) -> Result<Option<String>, Failure> {
    Ok(super::version::of_task(tx, id)?
        .filter(|v| v.status == gk_api::ops::version::VersionStatus::Open)
        .map(|v| v.name))
}

fn created_day(
    tx: &Transaction<'_>,
    table: &str,
    id: Id,
    zone: &crate::time::Zone,
) -> Result<Day, Failure> {
    let at: i64 = tx.query_row(
        &format!("SELECT created_at FROM {table} WHERE id = ?1"),
        [b(id)],
        |r| r.get(0),
    )?;
    Ok(zone.day_of(at))
}

/// Project `p`'s set-up as a blueprint; with `tasks`, its open top-level
/// tasks too, dates counted from the day the project was made.
fn setup_of(w: &WriteCtx<'_>, p: &ProjectRow, tasks: bool) -> Result<ProjectBlueprint, Failure> {
    let tx = w.tx();
    let (description, modules): (String, String) = tx.query_row(
        "SELECT description, modules FROM projects WHERE id = ?1",
        [b(p.id)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut stmt = tx.prepare(
        "SELECT name, category, hue, wip_limit, rules, wip_min FROM statuses
          WHERE project_id = ?1 ORDER BY rank, id",
    )?;
    let statuses = stmt
        .query_map([b(p.id)], |r| {
            Ok(StatusBlueprint {
                name: r.get(0)?,
                category: parse_enum(&r.get::<_, String>(1)?).unwrap_or(Category::Todo),
                hue: parse_enum(&r.get::<_, String>(2)?).unwrap_or(Hue::Grey),
                wip_limit: r
                    .get::<_, Option<i64>>(3)?
                    .and_then(|n| u32::try_from(n).ok()),
                wip_min: r
                    .get::<_, Option<i64>>(5)?
                    .and_then(|n| u32::try_from(n).ok()),
                rules: super::rules::parse(r.get::<_, Option<String>>(4)?.as_deref()),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut stmt = tx.prepare(
        "SELECT name, glyph, hue, is_default, due_offset_days FROM task_types
          WHERE project_id = ?1 ORDER BY rank, id",
    )?;
    let types = stmt
        .query_map([b(p.id)], |r| {
            Ok(TypeBlueprint {
                name: r.get(0)?,
                glyph: parse_enum(&r.get::<_, String>(1)?).unwrap_or(Glyph::Task),
                hue: parse_enum(&r.get::<_, String>(2)?).unwrap_or(Hue::Grey),
                is_default: r.get(3)?,
                due_offset_days: r
                    .get::<_, Option<i64>>(4)?
                    .and_then(|n| u32::try_from(n).ok()),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let labels = lookup::all(tx, List::Labels, p.id)?
        .into_iter()
        .map(|l| LabelBlueprint {
            name: l.name,
            hue: parse_enum(&l.kind).unwrap_or(Hue::Grey),
        })
        .collect();
    // Parents before their children, siblings in order.
    let mut wiki = Vec::new();
    let mut ordered: Vec<(Id, String, Option<String>)> = super::wiki::children(tx, p.id, None)?
        .into_iter()
        .map(|page| (page.id, page.title, None))
        .collect();
    let mut i = 0;
    while i < ordered.len() && ordered.len() < MAX_PAGES {
        let (parent, title, _) = ordered[i].clone();
        for c in super::wiki::children(tx, p.id, Some(parent))? {
            ordered.push((c.id, c.title, Some(title.clone())));
        }
        i += 1;
    }
    for (id, title, parent) in ordered.into_iter().take(MAX_PAGES) {
        let (body, _) = super::description::current_text(tx, super::description::Owner::Wiki(id))?;
        wiki.push(PageBlueprint {
            title,
            body,
            parent,
        });
    }
    let task_templates = all(tx)?
        .into_iter()
        .filter(|(_, r)| r.project_id() == Some(p.id))
        .filter_map(|(_, r)| {
            task_of(&r).map(|task| NamedBlueprint {
                name: r.name.clone(),
                description: r.description.clone(),
                task,
            })
        })
        .collect();
    let mut blueprints = Vec::new();
    if tasks {
        let anchor = created_day(tx, "projects", p.id, w.zone())?;
        let mut stmt = tx.prepare(
            "SELECT t.id FROM tasks t JOIN statuses s ON s.id = t.status_id
              WHERE t.project_id = ?1 AND t.parent_id IS NULL AND t.deleted_at IS NULL
                AND t.closed_at IS NULL
              ORDER BY s.rank, t.rank, t.id",
        )?;
        let ids = stmt
            .query_map([b(p.id)], |r| id_at(r, 0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut made = 0;
        for id in ids {
            let bp = blueprint_of(tx, id, anchor)?;
            made += 1 + bp.subtasks.len();
            if made > MAX_TASKS {
                break;
            }
            blueprints.push(bp);
        }
    }
    Ok(ProjectBlueprint {
        description,
        modules: Some(super::project::modules_of(&modules)),
        statuses,
        types,
        labels,
        wiki,
        tasks: blueprints,
        task_templates,
        fields: super::custom_field::own_blueprints(tx, p.id)?,
        agent_rules: super::rules::agent_rules(tx, p.id)?,
    })
}

// ---------------------------------------------------------------------
// Checking content

fn check_name(name: &str) -> Result<String, Failure> {
    gk_core::check_line("name", name, NAME_MAX).map_err(|e| invalid("name", e))
}

fn check_description(text: Option<&String>) -> Result<String, Failure> {
    match text {
        Some(d) if !d.trim().is_empty() => gk_core::check_line("description", d, DESCRIPTION_MAX)
            .map_err(|e| invalid("description", e)),
        _ => Ok(String::new()),
    }
}

/// Checks a blueprint's texts and the places of its subtasks; `path` is
/// where it sits in the input.
pub(crate) fn check_blueprint(bp: &TaskBlueprint, path: &str) -> Result<(), Failure> {
    let at = |field: &str| format!("{path}.{field}");
    gk_core::check_title(&bp.title).map_err(|e| invalid(&at("title"), e))?;
    gk_core::check_markdown("description", &bp.description)
        .map_err(|e| invalid(&at("description"), e))?;
    if let Some(s) = &bp.sprint
        && !s.trim().eq_ignore_ascii_case(ACTIVE_SPRINT)
    {
        return Err(messages::invalid(
            &at("sprint"),
            "template-sprint-active-only",
            &[("sprint", s.clone())],
        ));
    }
    if bp.subtasks.len() + 1 > MAX_TASKS {
        return Err(messages::invalid(
            &at("subtasks"),
            "template-too-many-tasks",
            &[("max", MAX_TASKS.to_string())],
        ));
    }
    let mut depth: Vec<usize> = Vec::with_capacity(bp.subtasks.len());
    for (i, s) in bp.subtasks.iter().enumerate() {
        let sub = format!("{path}.subtasks[{i}]");
        gk_core::check_title(&s.title).map_err(|e| invalid(&format!("{sub}.title"), e))?;
        gk_core::check_markdown("description", &s.description)
            .map_err(|e| invalid(&format!("{sub}.description"), e))?;
        let d = match s.under {
            None => 1,
            Some(u) => {
                let u = usize::try_from(u).unwrap_or(usize::MAX);
                if u >= i {
                    return Err(messages::invalid(
                        &format!("{sub}.under"),
                        "template-under-later",
                        &[],
                    ));
                }
                depth[u] + 1
            }
        };
        depth.push(d);
        if d + 1 > super::task::MAX_LEVELS {
            return Err(messages::invalid(
                &format!("{sub}.under"),
                "task-too-deep",
                &[
                    ("max", super::task::MAX_LEVELS.to_string()),
                    ("levels", (d + 1).to_string()),
                ],
            ));
        }
    }
    Ok(())
}

fn check_setup(s: &ProjectBlueprint) -> Result<(), Failure> {
    gk_core::check_markdown("description", &s.description)
        .map_err(|e| invalid("setup.description", e))?;
    let unique = |names: Vec<&str>, field: &str| -> Result<(), Failure> {
        let mut seen = std::collections::HashSet::new();
        for (i, n) in names.iter().enumerate() {
            let clean = gk_core::check_line("name", n, 50)
                .map_err(|e| invalid(&format!("setup.{field}[{i}].name"), e))?;
            if !seen.insert(fold(&clean)) {
                return Err(messages::invalid(
                    &format!("setup.{field}[{i}].name"),
                    "template-name-twice",
                    &[("name", clean)],
                ));
            }
        }
        Ok(())
    };
    unique(
        s.statuses.iter().map(|x| x.name.as_str()).collect(),
        "statuses",
    )?;
    unique(s.types.iter().map(|x| x.name.as_str()).collect(), "types")?;
    unique(s.labels.iter().map(|x| x.name.as_str()).collect(), "labels")?;
    super::custom_field::check_blueprints(&s.fields, "setup.fields")?;
    if !s.statuses.is_empty() && !s.statuses.iter().any(|x| x.category == Category::Todo) {
        return Err(messages::invalid(
            "setup.statuses",
            "template-needs-todo-status",
            &[],
        ));
    }
    if s.types.iter().filter(|t| t.is_default).count() > 1 {
        return Err(messages::invalid(
            "setup.types",
            "template-one-default-type",
            &[],
        ));
    }
    if let Some(t) = s
        .types
        .iter()
        .find(|t| t.due_offset_days.is_some_and(|d| d > 3650))
    {
        return Err(messages::invalid(
            "setup.types",
            "type-due-offset-range",
            &[("type", t.name.clone())],
        ));
    }
    if s.wiki.len() > MAX_PAGES {
        return Err(messages::invalid(
            "setup.wiki",
            "template-too-many-pages",
            &[("max", MAX_PAGES.to_string())],
        ));
    }
    let tasks: usize = s.tasks.iter().map(|t| 1 + t.subtasks.len()).sum();
    if tasks > MAX_TASKS {
        return Err(messages::invalid(
            "setup.tasks",
            "template-too-many-tasks",
            &[("max", MAX_TASKS.to_string())],
        ));
    }
    for (i, t) in s.tasks.iter().enumerate() {
        check_blueprint(t, &format!("setup.tasks[{i}]"))?;
    }
    for (i, t) in s.task_templates.iter().enumerate() {
        check_name(&t.name)?;
        check_blueprint(&t.task, &format!("setup.task_templates[{i}].task"))?;
    }
    for (i, t) in s.tasks.iter().enumerate() {
        check_required(s, t, &format!("setup.tasks[{i}]"))?;
    }
    for (i, t) in s.task_templates.iter().enumerate() {
        check_required(s, &t.task, &format!("setup.task_templates[{i}].task"))?;
    }
    Ok(())
}

/// Refuses a task of a project template, or one of its subtasks, that
/// lacks a value for one of the template's required fields without a
/// default: the template couldn't be used, since its tasks are held to the
/// same checks as tasks made by hand.
fn check_required(s: &ProjectBlueprint, bp: &TaskBlueprint, path: &str) -> Result<(), Failure> {
    if !s.fields.iter().any(|f| f.required && f.default.is_none()) {
        return Ok(());
    }
    // The type a task gets: its own by name, else the template's default.
    let type_of = |name: Option<&String>| -> Option<String> {
        if s.types.is_empty() {
            return None;
        }
        name.and_then(|n| s.types.iter().find(|y| fold(&y.name) == fold(n)))
            .or_else(|| s.types.iter().find(|y| y.is_default))
            .or_else(|| s.types.first())
            .map(|y| y.name.clone())
    };
    let refuse = |at: String, field: &str| {
        messages::invalid(
            &format!("{at}.fields.{field}"),
            "field-required",
            &[("field", field.to_owned())],
        )
    };
    let own = type_of(bp.task_type.as_ref());
    if let Some(field) =
        super::custom_field::missing_required(&s.fields, own.as_deref(), &bp.fields)
    {
        return Err(refuse(path.to_owned(), field));
    }
    for (i, sub) in bp.subtasks.iter().enumerate() {
        let own = type_of(sub.task_type.as_ref());
        if let Some(field) =
            super::custom_field::missing_required(&s.fields, own.as_deref(), &sub.fields)
        {
            return Err(refuse(format!("{path}.subtasks[{i}]"), field));
        }
    }
    Ok(())
}

/// Refuses a name another template of the same kind and project has.
fn check_unique(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), Failure> {
    let wanted = fold(&r.name);
    let taken = all(tx)?.into_iter().any(|(other, o)| {
        other != id && o.kind == r.kind && o.project == r.project && fold(&o.name) == wanted
    });
    if taken {
        let error = messages::conflict("template-name-taken", &[("template", r.name.clone())])
            .with_detail("field", "name");
        return Err(messages::hint(error, "template-name-taken-hint", &[]).into());
    }
    Ok(())
}

fn next_rank(
    tx: &Transaction<'_>,
    kind: TemplateKind,
    project: Option<Id>,
) -> Result<Rank, Failure> {
    let last = all(tx)?
        .into_iter()
        .filter(|(_, r)| r.kind == kind && r.project_id() == project)
        .filter_map(|(_, r)| Rank::parse(&r.rank).ok())
        .max_by(|a, b| a.as_str().cmp(b.as_str()));
    Ok(last.map_or_else(Rank::first, |r| r.after()))
}

fn content_value<T: Serialize>(v: &T) -> Result<Value, Failure> {
    serde_json::to_value(v).map_err(|e| Failure::Op(OpError::storage(e.to_string())))
}

/// Saves a new template record and returns its ID.
pub(crate) fn add(
    w: &mut WriteCtx<'_>,
    kind: TemplateKind,
    project: Option<Id>,
    name: String,
    description: String,
    content: Value,
) -> Result<(Id, Record), Failure> {
    let caller = w.caller().clone();
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let record = Record {
        project: project.map(|p| p.to_string()),
        kind,
        name,
        description,
        content,
        rank: next_rank(w.tx(), kind, project)?.as_str().to_owned(),
        created_at: w.now(),
        created_by_kind: caller.actor.kind.as_str().to_owned(),
        created_by_name: caller.actor.name,
        updated_at: w.now(),
    };
    check_unique(w.tx(), id, &record)?;
    set(w, id, Some(&record))?;
    Ok((id, record))
}

fn create(w: &mut WriteCtx<'_>, input: TemplateCreateInput) -> Result<TemplateChange, Failure> {
    let name = check_name(&input.name)?;
    let description = check_description(input.description.as_ref())?;
    let kind = input
        .kind
        .unwrap_or(if input.from_project.is_some() || input.setup.is_some() {
            TemplateKind::Project
        } else {
            TemplateKind::Task
        });
    let given = [
        input.from_task.is_some(),
        input.from_project.is_some(),
        input.task.is_some(),
        input.setup.is_some(),
    ]
    .iter()
    .filter(|g| **g)
    .count();
    if given != 1 {
        return Err(messages::invalid("from_task", "template-one-source", &[]));
    }
    let (project, content) = match kind {
        TemplateKind::Task => {
            if input.from_project.is_some() || input.setup.is_some() {
                return Err(messages::invalid("kind", "template-kind-mismatch", &[]));
            }
            let (project, bp) = if let Some(key) = &input.from_task {
                let task = lookup::task(w.tx(), "from_task", key)?;
                let anchor = created_day(w.tx(), "tasks", task.id, w.zone())?;
                let project = match &input.project {
                    Some(p) => lookup::project(w.tx(), "project", p)?.id,
                    None => task.project,
                };
                (project, blueprint_of(w.tx(), task.id, anchor)?)
            } else {
                let Some(p) = &input.project else {
                    return Err(messages::invalid("project", "template-needs-project", &[]));
                };
                let bp = input.task.clone().unwrap_or_default();
                check_blueprint(&bp, "task")?;
                (lookup::project(w.tx(), "project", p)?.id, bp)
            };
            if let Some(p) = lookup::project_by_id(w.tx(), project)? {
                lookup::ensure_active(&p)?;
            }
            (Some(project), content_value(&bp)?)
        }
        TemplateKind::Project => {
            if input.from_task.is_some() || input.task.is_some() {
                return Err(messages::invalid("kind", "template-kind-mismatch", &[]));
            }
            let setup = if let Some(key) = &input.from_project {
                let p = lookup::project(w.tx(), "from_project", key)?;
                setup_of(w, &p, input.include_tasks)?
            } else {
                let s = input.setup.clone().unwrap_or_default();
                check_setup(&s)?;
                s
            };
            (None, content_value(&setup)?)
        }
    };
    let (id, record) = add(w, kind, project, name, description, content)?;
    Ok(TemplateChange {
        template: output(w.tx(), id, &record)?,
        deleted: false,
        group: w.group_if_changed(),
    })
}

fn update(w: &mut WriteCtx<'_>, input: TemplateUpdateInput) -> Result<TemplateChange, Failure> {
    let (id, old) = find(w.tx(), &input.template, input.project.as_ref())?;
    refuse_built_in(id, &old)?;
    let mut r = old.clone();
    if let Some(name) = &input.name {
        r.name = check_name(name)?;
    }
    if let Some(d) = &input.description {
        r.description = check_description(Some(d))?;
    }
    match (old.kind, &input.task, &input.setup) {
        (TemplateKind::Task, Some(t), None) => {
            check_blueprint(t, "task")?;
            r.content = content_value(t)?;
        }
        (TemplateKind::Project, None, Some(s)) => {
            check_setup(s)?;
            r.content = content_value(s)?;
        }
        (_, None, None) => {}
        _ => return Err(messages::invalid("task", "template-kind-mismatch", &[])),
    }
    if r != old {
        r.updated_at = w.now();
        check_unique(w.tx(), id, &r)?;
        set(w, id, Some(&r))?;
    }
    Ok(TemplateChange {
        template: output(w.tx(), id, &r)?,
        deleted: false,
        group: w.group_if_changed(),
    })
}

// ---------------------------------------------------------------------
// Making tasks

/// The days after `anchor` as a date.
fn day_after(anchor: Day, days: Option<i32>) -> Result<Option<String>, Failure> {
    days.map(|n| {
        anchor
            .add_days(i64::from(n))
            .map(|d| d.to_string())
            .map_err(|e| invalid("date", e))
    })
    .transpose()
}

/// What one blueprint task needs to be written.
struct Plain<'a> {
    title: &'a str,
    description: &'a str,
    task_type: Option<&'a String>,
    priority: Priority,
    labels: &'a [String],
    estimate: Option<i64>,
    start: Option<i32>,
    due: Option<i32>,
    fields: &'a BTreeMap<String, Value>,
    fix_version: Option<&'a String>,
}

/// How a blueprint's tasks are made: as when someone creates them
/// (`Checked`: a required custom field without a value is refused), or by
/// the engine on its own for a recurring task (`Anyway`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Making {
    Checked,
    Anyway,
}

#[allow(clippy::too_many_arguments)]
fn write_one(
    w: &mut WriteCtx<'_>,
    project: &ProjectRow,
    t: &Plain<'_>,
    anchor: Day,
    parent: Option<Id>,
    status: Id,
    making: Making,
    left_out: &mut Vec<String>,
) -> Result<Id, Failure> {
    let types = lookup::all(w.tx(), List::Types, project.id)?;
    let task_type = match t
        .task_type
        .and_then(|name| types.iter().find(|y| fold(&y.name) == fold(name)))
    {
        Some(y) => y.id,
        None => {
            if let Some(name) = t.task_type
                && !left_out.contains(name)
            {
                left_out.push(name.clone());
            }
            super::task::default_type(w.tx(), project)?
        }
    };
    let known = lookup::all(w.tx(), List::Labels, project.id)?;
    let mut labels = Vec::new();
    for name in t.labels {
        match known.iter().find(|l| fold(&l.name) == fold(name)) {
            Some(l) => labels.push(l.id),
            None => {
                if !left_out.contains(name) {
                    left_out.push(name.clone());
                }
            }
        }
    }
    let start = day_after(anchor, t.start)?;
    let mut due = day_after(anchor, t.due)?;
    if due.is_none() && t.due.is_none() {
        due = super::task::type_due_date(w, task_type, start.as_deref())?;
    }
    let (start, due) = match (start, due) {
        (Some(s), Some(d)) if s > d => (None, Some(d)),
        other => other,
    };
    let id = super::task::insert(
        w,
        NewTask {
            project: project.id,
            project_key: project.key.clone(),
            title: gk_core::check_title(t.title).map_err(|e| invalid("title", e))?,
            description: t.description.to_owned(),
            status,
            task_type,
            priority: super::task::priority_index(t.priority),
            parent,
            start_date: start,
            due_date: due,
            estimate: t.estimate.filter(|m| *m >= 0),
            progress: None,
            labels,
            target: Target::Bottom,
        },
    )?;
    if let Some(name) = t.fix_version {
        match super::version::resolve_for_task(w.tx(), project, "fix_version", name) {
            Ok(version) => {
                super::version::set_task(w, id, project.id, Some(version))?;
            }
            Err(Failure::Op(_)) => {
                if !left_out.contains(name) {
                    left_out.push(name.clone());
                }
            }
            Err(other) => return Err(other),
        }
    }
    super::custom_field::apply_carried(
        w,
        id,
        project,
        t.fields,
        making == Making::Checked,
        left_out,
    )?;
    Ok(id)
}

/// Makes the task `bp` describes, with its subtasks, in `project`; dates
/// count from `anchor`. Names the project lacks go to `left_out`. Returns
/// the new task and how many tasks were made.
#[allow(clippy::too_many_arguments)]
pub(crate) fn make_task(
    w: &mut WriteCtx<'_>,
    project: &ProjectRow,
    bp: &TaskBlueprint,
    title: Option<&str>,
    anchor: Day,
    parent: Option<Id>,
    status: Option<Id>,
    making: Making,
    left_out: &mut Vec<String>,
) -> Result<(Id, u32), Failure> {
    let fresh = first_status(w.tx(), project)?;
    let root = write_one(
        w,
        project,
        &Plain {
            title: title.unwrap_or(&bp.title),
            description: &bp.description,
            task_type: bp.task_type.as_ref(),
            priority: bp.priority,
            labels: &bp.labels,
            estimate: bp.estimate_minutes,
            start: bp.start_in_days,
            due: bp.due_in_days,
            fields: &bp.fields,
            fix_version: bp.fix_version.as_ref(),
        },
        anchor,
        parent,
        status.unwrap_or(fresh),
        making,
        left_out,
    )?;
    if bp
        .sprint
        .as_deref()
        .is_some_and(|s| s.trim().eq_ignore_ascii_case(ACTIVE_SPRINT))
        && let Some(sprint) = super::sprint::active_in(w.tx(), project.id)?
    {
        super::sprint::set_task(w, root, project.id, Some(sprint))?;
    }
    let mut made: Vec<Id> = Vec::with_capacity(bp.subtasks.len());
    for s in &bp.subtasks {
        let under = match s.under {
            Some(u) => made.get(usize::try_from(u).unwrap_or(usize::MAX)).copied(),
            None => None,
        }
        .unwrap_or(root);
        let id = write_one(
            w,
            project,
            &Plain {
                title: &s.title,
                description: &s.description,
                task_type: s.task_type.as_ref(),
                priority: s.priority,
                labels: &s.labels,
                estimate: s.estimate_minutes,
                start: s.start_in_days,
                due: s.due_in_days,
                fields: &s.fields,
                fix_version: s.fix_version.as_ref(),
            },
            anchor,
            Some(under),
            fresh,
            making,
            left_out,
        )?;
        made.push(id);
    }
    Ok((root, u32::try_from(made.len() + 1).unwrap_or(u32::MAX)))
}

// ---------------------------------------------------------------------
// Using a template

fn anchor_day(w: &WriteCtx<'_>, date: Option<&String>) -> Result<Day, Failure> {
    match date {
        Some(d) => super::task::date_input(w, "date", d),
        None => Ok(w.zone().day_of(w.now())),
    }
}

fn apply(w: &mut WriteCtx<'_>, input: TemplateApplyInput) -> Result<TemplateApplied, Failure> {
    let (_, record) = find(w.tx(), &input.template, input.project.as_ref())?;
    let anchor = anchor_day(w, input.date.as_ref())?;
    let mut left_out = Vec::new();
    match record.kind {
        TemplateKind::Task => {
            let bp = task_of(&record)
                .ok_or_else(|| Failure::Op(messages::storage("template-unreadable", &[])))?;
            let project = match (&input.project, record.project_id()) {
                (Some(p), _) => lookup::project(w.tx(), "project", p)?,
                (None, Some(p)) => lookup::project_by_id(w.tx(), p)?.ok_or_else(|| {
                    Failure::Op(messages::not_found(
                        "template-project-gone",
                        &[("template", record.name.clone())],
                    ))
                })?,
                (None, None) => {
                    return Err(messages::invalid("project", "template-needs-project", &[]));
                }
            };
            lookup::ensure_active(&project)?;
            let title = input
                .title
                .as_deref()
                .map(gk_core::check_title)
                .transpose()
                .map_err(|e| invalid("title", e))?;
            let status = match &input.status {
                Some(s) => Some(lookup::named(w.tx(), List::Statuses, &project, "status", s)?.id),
                None => None,
            };
            let parent = match &input.parent {
                Some(p) => {
                    let parent = lookup::task(w.tx(), "parent", p)?;
                    super::task::check_parent(w.tx(), None, project.id, &parent, None)?;
                    let levels = super::task::level_of(w.tx(), parent.id)?
                        + 1
                        + bp.subtasks
                            .iter()
                            .scan(Vec::<usize>::new(), |depths, s| {
                                let d = s
                                    .under
                                    .and_then(|u| depths.get(u as usize).copied())
                                    .map_or(1, |d| d + 1);
                                depths.push(d);
                                Some(d)
                            })
                            .max()
                            .unwrap_or(0);
                    if levels > super::task::MAX_LEVELS {
                        return Err(messages::invalid(
                            "parent",
                            "task-too-deep",
                            &[
                                ("max", super::task::MAX_LEVELS.to_string()),
                                ("levels", levels.to_string()),
                            ],
                        ));
                    }
                    Some(parent.id)
                }
                None => None,
            };
            let (id, tasks) = make_task(
                w,
                &project,
                &bp,
                title.as_deref(),
                anchor,
                parent,
                status,
                Making::Checked,
                &mut left_out,
            )?;
            // A due date or priority given with the template wins over its
            // own, in the same change.
            if input.due_date.is_some() || input.priority.is_some() {
                super::task::update(
                    w,
                    gk_api::ops::task::TaskUpdateInput {
                        key: id.to_string(),
                        due_date: input.due_date.clone(),
                        priority: input.priority,
                        ..gk_api::ops::task::TaskUpdateInput::default()
                    },
                )?;
            }
            let task = super::task::changed(w, id, Vec::new())?.task;
            Ok(TemplateApplied {
                task: Some(task),
                project: None,
                tasks,
                left_out,
                group: w.group_if_changed(),
            })
        }
        TemplateKind::Project => {
            let setup = setup_of_record(&record)
                .ok_or_else(|| Failure::Op(messages::storage("template-unreadable", &[])))?;
            let (Some(name), Some(key)) = (&input.name, &input.key) else {
                return Err(messages::invalid(
                    "name",
                    "template-project-needs-name",
                    &[],
                ));
            };
            let created = super::project::create(
                w,
                ProjectCreateInput {
                    name: name.clone(),
                    key: key.clone(),
                    parent: input.parent_project.clone(),
                    description: (!setup.description.is_empty()).then(|| setup.description.clone()),
                    hue: input.hue,
                    ..ProjectCreateInput::default()
                },
            )?;
            let pid: Id = created
                .project
                .id
                .parse()
                .map_err(|_| Failure::Op(messages::storage("template-unreadable", &[])))?;
            let project = lookup::project_by_id(w.tx(), pid)?
                .ok_or_else(|| Failure::Op(messages::storage("template-unreadable", &[])))?;
            set_up(w, &project, &setup)?;
            let mut tasks = 0;
            if setup.modules.is_none_or(|m| m.wiki) {
                for page in &setup.wiki {
                    super::wiki::create_page(
                        w,
                        WikiCreateInput {
                            project: pid.to_string(),
                            title: page.title.clone(),
                            body: (!page.body.is_empty()).then(|| page.body.clone()),
                            parent: page.parent.clone(),
                            ..WikiCreateInput::default()
                        },
                    )?;
                }
            }
            for t in &setup.task_templates {
                add(
                    w,
                    TemplateKind::Task,
                    Some(pid),
                    t.name.clone(),
                    t.description.clone(),
                    content_value(&t.task)?,
                )?;
            }
            for bp in &setup.tasks {
                let (_, n) = make_task(
                    w,
                    &project,
                    bp,
                    None,
                    anchor,
                    None,
                    None,
                    Making::Checked,
                    &mut left_out,
                )?;
                tasks += n;
            }
            Ok(TemplateApplied {
                task: None,
                project: Some(super::project::load(w.tx(), pid)?),
                tasks,
                left_out,
                group: w.group_if_changed(),
            })
        }
    }
}

/// Gives the new project `p` the statuses, types, labels, modules, custom
/// fields and rules of `setup`, in place of those it was made with. The
/// project has no tasks yet; its creation in history covers these rows.
fn set_up(w: &WriteCtx<'_>, p: &ProjectRow, setup: &ProjectBlueprint) -> Result<(), Failure> {
    let tx = w.tx();
    let id = p.id;
    let mut status_ids = Vec::with_capacity(setup.statuses.len());
    let new_id = || Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    if !setup.statuses.is_empty() {
        tx.execute("DELETE FROM statuses WHERE project_id = ?1", [b(id)])?;
        let ranks = Rank::spread(setup.statuses.len());
        for (s, rank) in setup.statuses.iter().zip(&ranks) {
            let name = gk_core::clean_line(&s.name);
            let status = new_id();
            status_ids.push(status);
            tx.execute(
                "INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank,
                                       wip_limit, wip_min)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    b(status),
                    b(id),
                    name,
                    fold(&name),
                    enum_text(s.category),
                    enum_text(s.hue),
                    rank.as_str(),
                    s.wip_limit.filter(|n| *n > 0),
                    s.wip_min
                        .filter(|n| *n > 0 && s.wip_limit.is_none_or(|l| *n <= l))
                ],
            )?;
        }
    }
    if !setup.types.is_empty() {
        tx.execute("DELETE FROM task_types WHERE project_id = ?1", [b(id)])?;
        let ranks = Rank::spread(setup.types.len());
        let default = setup.types.iter().position(|t| t.is_default).unwrap_or(0);
        for (i, (t, rank)) in setup.types.iter().zip(&ranks).enumerate() {
            let name = gk_core::clean_line(&t.name);
            tx.execute(
                "INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank,
                                         is_default, due_offset_days)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    b(new_id()),
                    b(id),
                    name,
                    fold(&name),
                    enum_text(t.glyph),
                    enum_text(t.hue),
                    rank.as_str(),
                    i == default,
                    t.due_offset_days
                ],
            )?;
        }
    }
    if !setup.labels.is_empty() {
        let existing = lookup::all(tx, List::Labels, id)?;
        let last: Option<String> = tx.query_row(
            "SELECT max(rank) FROM labels WHERE project_id = ?1",
            [b(id)],
            |r| r.get(0),
        )?;
        let mut rank = last
            .and_then(|r| Rank::parse(&r).ok())
            .map_or_else(Rank::first, |r| r.after());
        for l in &setup.labels {
            let name = gk_core::clean_line(&l.name);
            if existing.iter().any(|e| fold(&e.name) == fold(&name)) {
                continue;
            }
            tx.execute(
                "INSERT INTO labels (id, project_id, name, name_fold, hue, rank)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    b(new_id()),
                    b(id),
                    name,
                    fold(&name),
                    enum_text(l.hue),
                    rank.as_str()
                ],
            )?;
            rank = rank.after();
        }
    }
    if let Some(m) = setup.modules {
        tx.execute(
            "UPDATE projects SET modules = ?2 WHERE id = ?1",
            params![b(id), modules_text(m)],
        )?;
    }
    // Fields after the types they show for; rules after the fields they name.
    let copies = super::custom_field::insert_blueprints(w, p, &setup.fields)?;
    for (i, (s, status)) in setup.statuses.iter().zip(&status_ids).enumerate() {
        let path = format!("setup.statuses[{i}]");
        if let Some(text) = super::rules::from_template(tx, p, &s.rules, &copies, &path)? {
            tx.execute(
                "UPDATE statuses SET rules = ?2 WHERE id = ?1",
                params![b(*status), text],
            )?;
        }
    }
    if let Some(text) = super::rules::agents_from_template(tx, p, &setup.agent_rules, &copies)? {
        tx.execute(
            "UPDATE projects SET agent_rules = ?2 WHERE id = ?1",
            params![b(id), text],
        )?;
    }
    Ok(())
}

fn modules_text(m: Modules) -> String {
    serde_json::json!({
        "board": m.board, "list": m.list, "gantt": m.gantt, "wiki": m.wiki, "time": m.time,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticked_items_are_cleared_and_nothing_else_changes() {
        let text = "Steps\n\n- [x] one\n- [ ] two\n  - [X] three\n\n`- [x] code`\n";
        assert_eq!(
            untick(text),
            "Steps\n\n- [ ] one\n- [ ] two\n  - [ ] three\n\n`- [x] code`\n"
        );
    }
}
