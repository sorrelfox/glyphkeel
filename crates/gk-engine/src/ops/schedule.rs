// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The schedule behind the Gantt chart (`schedule.get`), and "also move
//! dependents" for `task.update`.
//!
//! `schedule.get` reads everything it needs in a few queries: the tasks of
//! the projects involved (for summary ranges and progress), the `blocks`
//! links of the workspace (for dependencies, "blocked" and how many tasks
//! depend on each), and the tasks at the far end of links into other
//! projects. The rules themselves are `gk_core::schedule`'s.
//!
//! A caller limited to a project sees no links to tasks outside it, and
//! "also move dependents" leaves such tasks alone.

use std::collections::{BTreeMap, HashMap, HashSet};

use gk_api::ops::common::Category;
use gk_api::ops::common::Hue;
use gk_api::ops::query::TaskSearchInput;
use gk_api::ops::schedule::{
    DayRange, MovedTask, ReadOnlyReason, Schedule, ScheduleDependency, ScheduleGet, ScheduleInput,
    SchedulePreview, SchedulePreviewInput, SchedulePreviewMove, ScheduleProblem,
    ScheduleProblemKind, ScheduleShape, ScheduledTask, UnscheduledTask,
};
use gk_api::ops::task::{LabelRef, StatusRef};
use gk_core::schedule::{self as rules, Dates, Dependency, Graph, ShiftError};
use gk_core::{Day, Id, SubtaskProgress};
use gk_store::rusqlite::types::Value as Sql;
use gk_store::rusqlite::{Row, Transaction, params_from_iter};

use super::desc;
use super::fields::{self, TASK, text};
use super::lookup::{b, id_at, opt_id_at};
use super::project::parse_enum;
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::write::WriteCtx;

/// The most tasks one schedule shows.
const MAX_TASKS: usize = 5_000;
/// Values per `IN (…)` list.
const CHUNK: usize = 500;
/// Levels of subtasks followed for progress; subtasks go 5 levels deep,
/// so this only guards against damaged data.
const MAX_DEPTH: usize = 16;

pub(super) fn register(r: &mut Registry) {
    r.query::<ScheduleGet, _>(desc::<ScheduleGet>(), get);
    r.query::<SchedulePreviewMove, _>(desc::<SchedulePreviewMove>(), preview);
}

// ---------------------------------------------------------------------
// Reading

/// What the schedule needs of one task.
#[derive(Debug, Clone)]
struct Entry {
    id: Id,
    project: Id,
    project_key: String,
    project_active: bool,
    number: i64,
    title: String,
    status: StatusRef,
    milestone: bool,
    parent: Option<Id>,
    /// Subtask order, then board order: how siblings are listed.
    order: (String, String),
    dates: Dates,
    estimate: Option<i64>,
    progress_override: Option<i64>,
    closed: bool,
    checklist: (u32, u32),
    version: i64,
}

impl Entry {
    fn key(&self) -> String {
        format!("{}-{}", self.project_key, self.number)
    }

    /// Why its dates can't be changed, if they can't.
    fn read_only(&self) -> Option<ReadOnlyReason> {
        (!self.project_active).then_some(ReadOnlyReason::ArchivedProject)
    }
}

const ROW_SQL: &str = "SELECT t.id, t.project_id, p.key, p.state = 'active', t.number, t.title,
        s.id, s.name, s.category, y.glyph = 'milestone', t.parent_id,
        coalesce(t.subtask_rank, ''), t.rank, t.start_date, t.due_date, t.estimate_min,
        t.progress_override, t.closed_at IS NOT NULL, t.checklist_done, t.checklist_total,
        t.version
   FROM tasks t JOIN projects p ON p.id = t.project_id
   JOIN statuses s ON s.id = t.status_id JOIN task_types y ON y.id = t.type_id
  WHERE t.deleted_at IS NULL AND p.deleted_at IS NULL";

fn day(text: Option<String>) -> Option<Day> {
    text.and_then(|t| t.parse().ok())
}

fn row(r: &Row<'_>) -> gk_store::rusqlite::Result<Entry> {
    let count = |n: i64| u32::try_from(n.max(0)).unwrap_or(u32::MAX);
    Ok(Entry {
        id: id_at(r, 0)?,
        project: id_at(r, 1)?,
        project_key: r.get(2)?,
        project_active: r.get(3)?,
        number: r.get(4)?,
        title: r.get(5)?,
        status: StatusRef {
            id: id_at(r, 6)?.to_string(),
            name: r.get(7)?,
            category: parse_enum(&r.get::<_, String>(8)?).unwrap_or(Category::Todo),
        },
        milestone: r.get(9)?,
        parent: opt_id_at(r, 10)?,
        order: (r.get(11)?, r.get(12)?),
        dates: Dates::new(day(r.get(13)?), day(r.get(14)?)),
        estimate: r.get(15)?,
        progress_override: r.get(16)?,
        closed: r.get(17)?,
        checklist: (count(r.get(18)?), count(r.get(19)?)),
        version: r.get(20)?,
    })
}

/// The live tasks whose `column` is one of `ids`, into `out`.
fn rows_where(
    tx: &Transaction<'_>,
    column: &str,
    ids: &[Id],
    out: &mut HashMap<Id, Entry>,
) -> Result<(), Failure> {
    for chunk in ids.chunks(CHUNK) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let sql = format!("{ROW_SQL} AND {column} IN ({marks})");
        let mut stmt = tx.prepare(&sql)?;
        let found = stmt
            .query_map(
                params_from_iter(chunk.iter().map(|id| Sql::Blob(b(*id)))),
                row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        for r in found {
            out.insert(r.id, r);
        }
    }
    Ok(())
}

/// The labels of the tasks `ids`, each in its project's label order.
fn labels_of(tx: &Transaction<'_>, ids: &[Id]) -> Result<HashMap<Id, Vec<LabelRef>>, Failure> {
    let mut out: HashMap<Id, Vec<LabelRef>> = HashMap::new();
    for chunk in ids.chunks(CHUNK) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT tl.task_id, l.id, l.name, l.hue FROM task_labels tl
               JOIN labels l ON l.id = tl.label_id
              WHERE tl.task_id IN ({marks}) ORDER BY l.rank, l.id"
        );
        let mut stmt = tx.prepare(&sql)?;
        let found = stmt
            .query_map(
                params_from_iter(chunk.iter().map(|id| Sql::Blob(b(*id)))),
                |r| {
                    Ok((
                        id_at(r, 0)?,
                        LabelRef {
                            id: id_at(r, 1)?.to_string(),
                            name: r.get(2)?,
                            hue: parse_enum(&r.get::<_, String>(3)?).unwrap_or(Hue::Grey),
                        },
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        for (task, label) in found {
            out.entry(task).or_default().push(label);
        }
    }
    Ok(out)
}

/// One `blocks` link between two live tasks the caller may see.
#[derive(Debug, Clone, Copy)]
struct Link {
    id: Id,
    from: Id,
    to: Id,
    lag_days: Option<i64>,
    from_open: bool,
}

/// Every `blocks` link of the workspace between live tasks, leaving out
/// links to tasks a caller limited to a project can't see.
fn links(tx: &Transaction<'_>) -> Result<Vec<Link>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT r.id, r.from_task, r.to_task, r.lag_days, f.closed_at IS NULL,
                f.project_id, t.project_id
           FROM task_relations r
           JOIN tasks f ON f.id = r.from_task JOIN projects fp ON fp.id = f.project_id
           JOIN tasks t ON t.id = r.to_task JOIN projects tp ON tp.id = t.project_id
          WHERE r.kind = 'blocks' AND f.deleted_at IS NULL AND t.deleted_at IS NULL
            AND fp.deleted_at IS NULL AND tp.deleted_at IS NULL
          ORDER BY r.created_at, r.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                Link {
                    id: id_at(r, 0)?,
                    from: id_at(r, 1)?,
                    to: id_at(r, 2)?,
                    lag_days: r.get(3)?,
                    from_open: r.get(4)?,
                },
                id_at(r, 5)?,
                id_at(r, 6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter(|(_, from_project, to_project)| {
            crate::scope::sees(*from_project) && crate::scope::sees(*to_project)
        })
        .map(|(l, _, _)| l)
        .collect())
}

/// The links as a graph over numbered tasks, with the numbering.
struct Numbered {
    graph: Graph,
    index: HashMap<Id, usize>,
    ids: Vec<Id>,
}

fn numbered(links: &[Link]) -> Numbered {
    let mut index = HashMap::new();
    let mut ids = Vec::new();
    let mut number = |id: Id| {
        *index.entry(id).or_insert_with(|| {
            ids.push(id);
            ids.len() - 1
        })
    };
    let deps: Vec<Dependency> = links
        .iter()
        .map(|l| Dependency {
            from: number(l.from),
            to: number(l.to),
            lag_days: l.lag_days.unwrap_or(0),
        })
        .collect();
    Numbered {
        graph: Graph::new(ids.len(), deps),
        index,
        ids,
    }
}

/// A problem with its message.
fn problem(
    kind: ScheduleProblemKind,
    task: &str,
    other: Option<String>,
    id: &str,
    args: &[(&str, String)],
) -> ScheduleProblem {
    ScheduleProblem {
        kind,
        task: task.to_owned(),
        other,
        others: Vec::new(),
        message: messages::text(id, args),
        message_id: id.to_owned(),
        args: args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// Progress of every task, as `gk_core::progress` defines it, from rows
/// that hold each task's subtasks.
struct Progress<'a> {
    rows: &'a HashMap<Id, Entry>,
    children: &'a HashMap<Id, Vec<Id>>,
    known: HashMap<Id, u8>,
}

impl Progress<'_> {
    fn of(&mut self, id: Id, depth: usize) -> u8 {
        if let Some(p) = self.known.get(&id) {
            return *p;
        }
        let Some(r) = self.rows.get(&id) else {
            return 0;
        };
        let by_hand = r
            .progress_override
            .and_then(|p| u8::try_from(p.clamp(0, 100)).ok());
        let p = if by_hand.is_some() || r.closed || r.checklist.1 > 0 {
            gk_core::progress(by_hand, r.closed, r.checklist, &[])
        } else {
            let kids = self.children.get(&id).cloned().unwrap_or_default();
            let mut subtasks = Vec::with_capacity(kids.len());
            for k in kids {
                let Some(c) = self.rows.get(&k) else { continue };
                let (estimate, closed) = (c.estimate, c.closed);
                let percent = if closed {
                    100
                } else if depth < MAX_DEPTH {
                    self.of(k, depth + 1)
                } else {
                    0
                };
                subtasks.push(SubtaskProgress {
                    percent,
                    estimate_minutes: estimate,
                    closed,
                });
            }
            gk_core::progress(None, false, (0, 0), &subtasks)
        };
        self.known.insert(id, p);
        p
    }
}

fn range(r: (Day, Day)) -> DayRange {
    DayRange {
        start: r.0.to_string(),
        end: r.1.to_string(),
    }
}

/// The task's own dates and its descendants', for its summary range.
fn descendants(children: &HashMap<Id, Vec<Id>>, id: Id) -> Vec<Id> {
    let mut out = vec![id];
    let mut i = 0;
    while i < out.len() && out.len() <= MAX_TASKS * 4 {
        if let Some(kids) = children.get(&out[i]) {
            out.extend(kids.iter().copied());
        }
        i += 1;
    }
    out
}

#[allow(clippy::too_many_lines)]
fn get(q: &QueryCtx<'_>, input: ScheduleInput) -> Result<Schedule, Failure> {
    let tx = q.tx;
    if input.project.is_none() && input.query.as_deref().is_none_or(|t| t.trim().is_empty()) {
        return Err(messages::invalid("project", "schedule-scope-needed", &[]));
    }
    let parse = |field: &str, text: &Option<String>| -> Result<Option<Day>, Failure> {
        text.as_deref()
            .filter(|t| !t.trim().is_empty())
            .map(|t| super::task::parse_date(tx, &q.env.zone, q.now, field, t))
            .transpose()
    };
    let from = parse("from", &input.from)?;
    let to = parse("to", &input.to)?;
    if let (Some(f), Some(t)) = (from, to)
        && f > t
    {
        return Err(messages::invalid("to", "schedule-window-reversed", &[]));
    }
    let today = q.env.zone.day_of(q.now);

    // The tasks asked for, in the query's order.
    let search = TaskSearchInput {
        query: input.query.clone().unwrap_or_default(),
        project: input.project.clone(),
        include_subprojects: input.include_subprojects,
        sort: Some("start,due,key".to_owned()),
        ..TaskSearchInput::default()
    };
    let (_, compiled) = super::query::compile_search(tx, &q.env.zone, q.now, &search)?;
    let shown = super::query::matching_ids(tx, &compiled)?;
    if shown.len() > MAX_TASKS {
        return Err(messages::invalid(
            "query",
            "schedule-too-many",
            &[
                ("count", shown.len().to_string()),
                ("max", MAX_TASKS.to_string()),
            ],
        ));
    }
    let position: HashMap<Id, usize> = shown.iter().enumerate().map(|(i, id)| (*id, i)).collect();

    // Every task of the projects involved: subtasks, parents, progress.
    let mut rows: HashMap<Id, Entry> = HashMap::new();
    rows_where(tx, "t.id", &shown, &mut rows)?;
    let projects: Vec<Id> = {
        let set: HashSet<Id> = rows.values().map(|r| r.project).collect();
        let mut v: Vec<Id> = set.into_iter().collect();
        v.sort();
        v
    };
    rows_where(tx, "t.project_id", &projects, &mut rows)?;
    let links = links(tx)?;
    let missing: Vec<Id> = {
        let mut set = HashSet::new();
        for l in &links {
            if position.contains_key(&l.to) && !rows.contains_key(&l.from) {
                set.insert(l.from);
            }
            if position.contains_key(&l.from) && !rows.contains_key(&l.to) {
                set.insert(l.to);
            }
        }
        let mut v: Vec<Id> = set.into_iter().collect();
        v.sort();
        v
    };
    rows_where(tx, "t.id", &missing, &mut rows)?;
    let shown: Vec<Id> = shown
        .into_iter()
        .filter(|id| rows.contains_key(id))
        .collect();

    let mut children: HashMap<Id, Vec<Id>> = HashMap::new();
    for r in rows.values() {
        if let Some(p) = r.parent {
            children.entry(p).or_default().push(r.id);
        }
    }
    for kids in children.values_mut() {
        kids.sort_by(|a, b| {
            let (ra, rb) = (&rows[a], &rows[b]);
            ra.order.cmp(&rb.order).then(a.cmp(b))
        });
    }
    let summary_of = |id: Id| -> Option<(Day, Day)> {
        if !children.contains_key(&id) {
            return None;
        }
        let all = descendants(&children, id);
        rules::summary(all.iter().filter_map(|i| rows.get(i)).map(|r| &r.dates))
    };

    let net = numbered(&links);
    let mut blocked_by: HashMap<Id, Vec<&Link>> = HashMap::new();
    for l in &links {
        blocked_by.entry(l.to).or_default().push(l);
    }

    // Which tasks get a bar, and which go to the Unscheduled lane.
    let mut dated: Vec<(Id, Option<(Day, Day)>)> = Vec::new();
    let mut undated: Vec<Id> = Vec::new();
    for id in &shown {
        let r = &rows[id];
        let summary = summary_of(*id);
        let span = summary.or_else(|| Some((r.dates.begin()?, r.dates.end()?)));
        match span {
            Some((start, end)) => {
                let before = from.is_some_and(|f| end < f);
                let after = to.is_some_and(|t| start > t);
                if !before && !after {
                    dated.push((*id, summary));
                }
            }
            None => undated.push(*id),
        }
    }

    // The tree of rows: a task under its parent when the parent is shown.
    let kept: HashSet<Id> = dated.iter().map(|(id, _)| *id).collect();
    let summaries: HashMap<Id, Option<(Day, Day)>> = dated.iter().copied().collect();
    let mut roots: Vec<Id> = dated
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| rows[id].parent.is_none_or(|p| !kept.contains(&p)))
        .collect();
    roots.sort_by_key(|id| position[id]);
    let mut ordered: Vec<(Id, u8)> = Vec::with_capacity(dated.len());
    let mut stack: Vec<(Id, u8)> = roots.into_iter().rev().map(|id| (id, 0)).collect();
    while let Some((id, depth)) = stack.pop() {
        ordered.push((id, depth));
        if let Some(kids) = children.get(&id) {
            for k in kids.iter().rev().filter(|k| kept.contains(k)) {
                stack.push((*k, depth.saturating_add(1)));
            }
        }
    }

    let mut progress = Progress {
        rows: &rows,
        children: &children,
        known: HashMap::new(),
    };
    let key_of = |id: &Id| rows.get(id).map(Entry::key);
    let labelled: Vec<Id> = ordered
        .iter()
        .map(|(id, _)| *id)
        .chain(undated.iter().copied())
        .collect();
    let mut labels = labels_of(tx, &labelled)?;
    let mut tasks = Vec::with_capacity(ordered.len());
    let mut span: Option<(Day, Day)> = None;
    for (id, depth) in &ordered {
        let r = &rows[id];
        let summary = summaries[id];
        let shape = if children.contains_key(id) {
            ScheduleShape::Summary
        } else if r.milestone {
            ScheduleShape::Milestone
        } else {
            match (r.dates.start, r.dates.due) {
                (Some(_), None) => ScheduleShape::NoDeadline,
                (None, Some(_)) => ScheduleShape::Deadline,
                _ => ScheduleShape::Bar,
            }
        };
        let own = summary.or_else(|| Some((r.dates.begin()?, r.dates.end()?)));
        if let Some((lo, hi)) = own {
            span = Some(span.map_or((lo, hi), |(a, z)| (a.min(lo), z.max(hi))));
        }
        let dependents = net
            .index
            .get(id)
            .map_or(0, |&i| net.graph.dependents(i).len());
        tasks.push(ScheduledTask {
            key: r.key(),
            title: r.title.clone(),
            project: r.project_key.clone(),
            parent: r.parent.as_ref().and_then(key_of),
            depth: *depth,
            status: r.status.clone(),
            closed: r.closed,
            labels: labels.remove(id).unwrap_or_default(),
            read_only: r.read_only(),
            shape,
            start_date: r.dates.start.map(|d| d.to_string()),
            due_date: r.dates.due.map(|d| d.to_string()),
            summary: summary.map(range),
            progress_percent: progress.of(*id, 0),
            overdue: !r.closed && r.dates.due.is_some_and(|d| d < today),
            blocked: blocked_by
                .get(id)
                .is_some_and(|ls| ls.iter().any(|l| l.from_open)),
            dependents: u32::try_from(dependents).unwrap_or(u32::MAX),
            version: r.version,
            id: id.to_string(),
        });
    }
    let unscheduled: Vec<UnscheduledTask> = undated
        .iter()
        .map(|id| {
            let r = &rows[id];
            UnscheduledTask {
                key: r.key(),
                title: r.title.clone(),
                project: r.project_key.clone(),
                parent: r.parent.as_ref().and_then(key_of),
                status: r.status.clone(),
                closed: r.closed,
                labels: labels.remove(id).unwrap_or_default(),
                read_only: r.read_only(),
                milestone: r.milestone,
                version: r.version,
                id: id.to_string(),
            }
        })
        .collect();

    // Links between the tasks shown.
    let out: HashSet<Id> = ordered
        .iter()
        .map(|(id, _)| *id)
        .chain(undated.iter().copied())
        .collect();
    let too_early = |l: &Link| -> Option<(Day, Day)> {
        let (f, t) = (rows.get(&l.from)?, rows.get(&l.to)?);
        if t.closed {
            return None;
        }
        rules::starts_too_early(&f.dates, &t.dates, l.lag_days.unwrap_or(0))
    };
    let dependencies: Vec<ScheduleDependency> = links
        .iter()
        .filter(|l| out.contains(&l.from) && out.contains(&l.to))
        .map(|l| ScheduleDependency {
            from: key_of(&l.from).unwrap_or_default(),
            to: key_of(&l.to).unwrap_or_default(),
            lag_days: l.lag_days.filter(|d| *d != 0),
            violated: too_early(l).is_some(),
            id: l.id.to_string(),
        })
        .collect();

    // Problems, task by task in row order.
    let mut loops: HashMap<Id, Vec<Id>> = HashMap::new();
    for group in net.graph.loops() {
        let mut members: Vec<Id> = group.iter().map(|&i| net.ids[i]).collect();
        members.sort_by_key(|id| (position.get(id).copied().unwrap_or(usize::MAX), *id));
        if let Some(first) = members.first().filter(|id| out.contains(id)) {
            loops.insert(*first, members);
        }
    }
    let mut problems = Vec::new();
    for id in ordered.iter().map(|(id, _)| id).chain(undated.iter()) {
        let r = &rows[id];
        let key = r.key();
        for l in blocked_by.get(id).into_iter().flatten() {
            if let Some((begin, end)) = too_early(l) {
                let blocker = key_of(&l.from).unwrap_or_default();
                problems.push(problem(
                    ScheduleProblemKind::StartsBeforeBlockerEnds,
                    &key,
                    Some(blocker.clone()),
                    "schedule-starts-before-blocker",
                    &[
                        ("task", key.clone()),
                        ("start", begin.to_string()),
                        ("blocker", blocker),
                        ("end", end.to_string()),
                    ],
                ));
            }
        }
        if let Some(members) = loops.get(id) {
            let mut p = problem(
                ScheduleProblemKind::DependencyLoop,
                &key,
                None,
                "schedule-dependency-loop",
                &[("task", key.clone()), ("count", members.len().to_string())],
            );
            p.others = members[1..].iter().filter_map(key_of).collect();
            problems.push(p);
        }
        if let Some(parent) = r.parent.and_then(|p| rows.get(&p)) {
            let pk = parent.key();
            if let (Some(begin), Some(ps)) = (r.dates.begin(), parent.dates.start)
                && begin < ps
            {
                problems.push(problem(
                    ScheduleProblemKind::OutsideParent,
                    &key,
                    Some(pk.clone()),
                    "schedule-starts-before-parent",
                    &[
                        ("task", key.clone()),
                        ("start", begin.to_string()),
                        ("parent", pk.clone()),
                        ("parent_start", ps.to_string()),
                    ],
                ));
            }
            if let (Some(end), Some(pd)) = (r.dates.end(), parent.dates.due)
                && end > pd
            {
                problems.push(problem(
                    ScheduleProblemKind::OutsideParent,
                    &key,
                    Some(pk.clone()),
                    "schedule-ends-after-parent",
                    &[
                        ("task", key.clone()),
                        ("end", end.to_string()),
                        ("parent", pk),
                        ("parent_due", pd.to_string()),
                    ],
                ));
            }
        }
        if !r.closed {
            if let Some(due) = r.dates.due.filter(|d| *d < today) {
                problems.push(problem(
                    ScheduleProblemKind::Overdue,
                    &key,
                    None,
                    "schedule-overdue",
                    &[("task", key.clone()), ("due", due.to_string())],
                ));
            }
            if let (Some(start), None) = (r.dates.start, r.dates.due) {
                problems.push(problem(
                    ScheduleProblemKind::NoDueDate,
                    &key,
                    None,
                    "schedule-no-due-date",
                    &[("task", key.clone()), ("start", start.to_string())],
                ));
            }
        }
    }

    Ok(Schedule {
        today: today.to_string(),
        range: span.map(range),
        tasks,
        unscheduled,
        dependencies,
        problems,
    })
}

// ---------------------------------------------------------------------
// Trying out a move

/// A day given to the preview, `YYYY-MM-DD` only.
fn preview_day(field: &str, text: Option<&str>) -> Result<Option<Day>, Failure> {
    text.filter(|t| !t.trim().is_empty())
        .map(|t| {
            t.trim().parse::<Day>().map_err(|_| {
                messages::invalid(
                    field,
                    "schedule-preview-date",
                    &[("field", field.to_owned())],
                )
            })
        })
        .transpose()
}

/// What "also move dependents" would move if task `key` got the given
/// dates: the same walk as `move_dependents`, on dates held in memory.
fn preview(q: &QueryCtx<'_>, input: SchedulePreviewInput) -> Result<SchedulePreview, Failure> {
    let start = preview_day("start_date", input.start_date.as_deref())?;
    let due = preview_day("due_date", input.due_date.as_deref())?;
    if let (Some(s), Some(d)) = (start, due)
        && s > d
    {
        return Err(messages::invalid(
            "start_date",
            "schedule-preview-reversed",
            &[],
        ));
    }
    let task = super::lookup::task(q.tx, "key", &input.key)?;
    let links = links(q.tx)?;
    let net = numbered(&links);
    let Some(&root) = net.index.get(&task.id) else {
        return Ok(SchedulePreview::default());
    };
    let reach = net.graph.dependents(root);
    if reach.is_empty() {
        return Ok(SchedulePreview::default());
    }
    let ids: Vec<Id> = reach.iter().map(|&i| net.ids[i]).collect();
    let mut rows = HashMap::new();
    rows_where(q.tx, "t.id", &ids, &mut rows)?;
    let n = net.ids.len();
    let mut dates = vec![Dates::default(); n];
    let mut movable = vec![false; n];
    for (i, id) in net.ids.iter().enumerate() {
        if let Some(r) = rows.get(id) {
            dates[i] = r.dates;
            movable[i] = !r.closed && r.project_active;
        }
    }
    dates[root] = Dates::new(start, due);
    // A loop or a date out of range: the change itself will say what is
    // wrong; nothing moves.
    let shifts = rules::shift_dependents(&net.graph, &dates, root, &movable).unwrap_or_default();
    let moved = shifts
        .iter()
        .filter_map(|s| {
            let id = net.ids[s.task];
            let r = rows.get(&id)?;
            Some(MovedTask {
                key: r.key(),
                days: s.days,
                start_date: s.dates.start.map(|d| d.to_string()),
                due_date: s.dates.due.map(|d| d.to_string()),
                id: id.to_string(),
            })
        })
        .collect();
    Ok(SchedulePreview { moved })
}

// ---------------------------------------------------------------------
// Also move dependents

/// Whether a task's due date must be written before its start date when
/// both change: when the new start comes after the current due date, the
/// start written first would pass it, which the store refuses. Otherwise
/// the start goes first, for the same reason the other way round.
pub(crate) fn due_first(
    tx: &Transaction<'_>,
    task: Id,
    new_start: Option<&serde_json::Value>,
) -> Result<bool, Failure> {
    let Some(start) = new_start.and_then(serde_json::Value::as_str) else {
        return Ok(false);
    };
    let due = fields::read(tx, TASK, task, "due_date")?;
    // Days are `YYYY-MM-DD`, so text order is date order.
    Ok(due
        .as_ref()
        .and_then(serde_json::Value::as_str)
        .is_some_and(|d| start > d))
}

/// Moves the tasks that depend on task `root` (which has its new dates
/// already) later where they must, to begin after their blockers end; see
/// `gk_core::schedule::shift_dependents`. Closed tasks and tasks the
/// caller can't see stay; tasks of archived projects stay too, with a
/// warning when they now begin too early. Every move is part of the
/// running command, so it is one change in history.
pub(crate) fn move_dependents(
    w: &mut WriteCtx<'_>,
    root: Id,
    override_claim: bool,
    warnings: &mut Vec<String>,
) -> Result<Vec<MovedTask>, Failure> {
    let links = links(w.tx())?;
    let net = numbered(&links);
    let Some(&start) = net.index.get(&root) else {
        return Ok(Vec::new());
    };
    let reach = net.graph.dependents(start);
    if reach.is_empty() {
        return Ok(Vec::new());
    }
    let mut ids: Vec<Id> = reach.iter().map(|&i| net.ids[i]).collect();
    ids.push(root);
    let mut rows = HashMap::new();
    rows_where(w.tx(), "t.id", &ids, &mut rows)?;
    let n = net.ids.len();
    let mut dates = vec![Dates::default(); n];
    let mut movable = vec![false; n];
    for (i, id) in net.ids.iter().enumerate() {
        if let Some(r) = rows.get(id) {
            dates[i] = r.dates;
            movable[i] = !r.closed && r.project_active;
        }
    }
    let shifts =
        rules::shift_dependents(&net.graph, &dates, start, &movable).map_err(|e| match e {
            ShiftError::Loop { tasks } => {
                let first = tasks
                    .first()
                    .and_then(|&i| rows.get(&net.ids[i]))
                    .map(Entry::key)
                    .unwrap_or_default();
                Failure::Op(messages::hint(
                    messages::conflict(
                        "schedule-move-loop",
                        &[("task", first), ("count", tasks.len().to_string())],
                    ),
                    "schedule-move-loop-hint",
                    &[],
                ))
            }
            ShiftError::Day(e) => super::task::invalid("move_dependents", e),
        })?;

    let mut now = dates;
    let mut changed = vec![false; n];
    changed[start] = true;
    let mut moved = Vec::with_capacity(shifts.len());
    for s in &shifts {
        let id = net.ids[s.task];
        let Some(r) = rows.get(&id) else { continue };
        let key = r.key();
        super::claim::guard(w, id, &key, override_claim)?;
        let project = Some(r.project);
        // Later dates: the due date first, so the start never passes it.
        if let Some(d) = s.dates.due {
            fields::set(w, TASK, id, project, "due_date", text(d.to_string()))?;
        }
        if let Some(d) = s.dates.start {
            fields::set(w, TASK, id, project, "start_date", text(d.to_string()))?;
        }
        now[s.task] = s.dates;
        changed[s.task] = true;
        moved.push(MovedTask {
            key,
            days: s.days,
            start_date: s.dates.start.map(|d| d.to_string()),
            due_date: s.dates.due.map(|d| d.to_string()),
            id: id.to_string(),
        });
    }
    // Open tasks of archived projects that stayed and now begin too early.
    for &i in &reach {
        let Some(r) = rows.get(&net.ids[i]) else {
            continue;
        };
        if r.closed || r.project_active {
            continue;
        }
        let late = net.graph.blocked_by(i).any(|d| {
            changed[d.from] && rules::starts_too_early(&now[d.from], &now[i], d.lag_days).is_some()
        });
        if late {
            warnings.push(messages::text(
                "schedule-dependent-archived",
                &[("task", r.key()), ("project", r.project_key.clone())],
            ));
        }
    }
    Ok(moved)
}
