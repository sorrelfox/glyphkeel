// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The flow reports: created against closed, the cumulative flow and cycle
//! time, read from the tasks and their history in one snapshot.
//!
//! Every report loads the same things: the statuses (also those removed
//! since, by the names history kept), the tasks in scope as they are now,
//! and each task's status and trash events from the activity log. A status
//! is found by its ID; failing that (an archive written by another tool) by
//! its name in the task's project, then in any project.
//!
//! History may be incomplete: tasks imported without their history, or a
//! status change history never saw. The replay then ends where the task is
//! now: when the events don't end in the task's current status, a move into
//! it is added at the time it entered that status. A task with no status
//! event at all starts in its project's first open status when it entered
//! its current one later than its creation.

use std::collections::{HashMap, HashSet};

use gk_api::ops::common::{Category, Hue};
use gk_api::ops::report::{
    CreatedClosedReport, CreatedClosedRow, CumulativeFlowReport, CumulativeFlowRow,
    CycleTimeReport, CycleTimeRow, CycleTimeTask, FlowBand, FlowBucket, FlowGroupBy,
    FlowReportInput, ReportCreatedClosed, ReportCumulativeFlow, ReportCycleTime,
};
use gk_core::{DEFAULT_STATUSES, Day, Id, StatusCategory, fold, worklog};
use gk_query::{Compiled, SqlEnv};
use gk_store::rusqlite::{Transaction, params};
use serde_json::Value;

use super::desc;
use super::lookup::{self, id_at};
use super::task::invalid;
use super::time::{csv_line, first_weekday};
use crate::failure::Failure;
use crate::flow::{self, Change, Event, Kind, Period, Replay};
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::time::{Zone, rfc3339};

/// The most periods one report has.
const MAX_PERIODS: usize = 1000;

pub(super) fn register(r: &mut Registry) {
    r.query::<ReportCreatedClosed, _>(desc::<ReportCreatedClosed>(), |q, input| {
        created_closed(&Flow::load(q, &input)?, &input)
    });
    r.query::<ReportCumulativeFlow, _>(desc::<ReportCumulativeFlow>(), |q, input| {
        cumulative_flow(&Flow::load(q, &input)?, &input)
    });
    r.query::<ReportCycleTime, _>(desc::<ReportCycleTime>(), |q, input| {
        cycle_time(&Flow::load(q, &input)?, &input)
    });
}

/// Tasks created against closed, as `report.created_closed` gives it.
pub(crate) fn created_closed_report(
    q: &QueryCtx<'_>,
    input: &FlowReportInput,
) -> Result<CreatedClosedReport, Failure> {
    created_closed(&Flow::load(q, input)?, input)
}

/// The cumulative flow, as `report.cumulative_flow` gives it.
pub(crate) fn cumulative_flow_report(
    q: &QueryCtx<'_>,
    input: &FlowReportInput,
) -> Result<CumulativeFlowReport, Failure> {
    cumulative_flow(&Flow::load(q, input)?, input)
}

/// Cycle and lead time, as `report.cycle_time` gives it.
pub(crate) fn cycle_time_report(
    q: &QueryCtx<'_>,
    input: &FlowReportInput,
) -> Result<CycleTimeReport, Failure> {
    cycle_time(&Flow::load(q, input)?, input)
}

// ---------------------------------------------------------------------
// Loading

/// One status, live or known only from history.
struct StatusRow {
    project: Option<Id>,
    name: String,
    fold: String,
    category: StatusCategory,
    rank: String,
    hue: Option<Hue>,
}

/// What history knows of a removed status: its project, name and category.
type Gone = (Option<Id>, Option<String>, Option<String>);

/// The statuses and how to find them.
#[derive(Default)]
struct Statuses {
    rows: Vec<StatusRow>,
    by_id: HashMap<Id, usize>,
    by_name: HashMap<(Id, String), usize>,
    any_name: HashMap<String, usize>,
}

impl Statuses {
    fn load(tx: &Transaction<'_>) -> Result<Self, Failure> {
        let mut out = Self::default();
        let mut stmt = tx.prepare_cached(
            "SELECT id, project_id, name, category, rank, hue FROM statuses ORDER BY rank, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                id_at(r, 0)?,
                id_at(r, 1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        for row in rows {
            let (id, project, name, category, rank, hue) = row?;
            let Ok(category) = category.parse::<StatusCategory>() else {
                continue;
            };
            out.push(
                id,
                StatusRow {
                    project: Some(project),
                    fold: fold(&name),
                    name,
                    category,
                    rank,
                    hue: serde_json::from_value(Value::String(hue)).ok(),
                },
            );
        }
        // Statuses removed since: their last name and category in history.
        // A removal keeps the name only; a status the project started with
        // has no history. The category then comes from a live status of
        // that name, a default status of that name, or is taken as in
        // progress.
        let mut gone: HashMap<Id, Gone> = HashMap::new();
        let mut order = Vec::new();
        let mut stmt = tx.prepare_cached(
            "SELECT entity_id, project_id,
                    coalesce(json_extract(changes, '$.fields.name.after'),
                             json_extract(changes, '$.fields.name.before')),
                    json_extract(changes, '$.fields.category.after')
               FROM activity WHERE entity_kind = 'status' ORDER BY seq",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                id_at(r, 0)?,
                r.get::<_, Option<Vec<u8>>>(1)?,
                text_at(r, 2)?,
                text_at(r, 3)?,
            ))
        })?;
        for row in rows {
            let (id, project, name, category) = row?;
            if out.by_id.contains_key(&id) {
                continue;
            }
            let entry = gone.entry(id).or_insert_with(|| {
                order.push(id);
                (None, None, None)
            });
            if let Some(p) = project.and_then(|p| Id::from_slice(&p).ok()) {
                entry.0 = Some(p);
            }
            if name.is_some() {
                entry.1 = name;
            }
            if category.is_some() {
                entry.2 = category;
            }
        }
        for id in order {
            let (project, name, category) = gone.remove(&id).unwrap_or_default();
            let Some(name) = name else {
                continue;
            };
            let f = fold(&name);
            let category = category
                .and_then(|c| c.parse::<StatusCategory>().ok())
                .or_else(|| out.any_name.get(&f).map(|i| out.rows[*i].category))
                .or_else(|| {
                    DEFAULT_STATUSES
                        .iter()
                        .find(|d| fold(d.name) == f)
                        .map(|d| d.category)
                })
                .unwrap_or(StatusCategory::InProgress);
            out.push(
                id,
                StatusRow {
                    project,
                    fold: fold(&name),
                    name,
                    category,
                    rank: "~".into(),
                    hue: None,
                },
            );
        }
        Ok(out)
    }

    fn push(&mut self, id: Id, row: StatusRow) {
        let i = self.rows.len();
        self.by_id.insert(id, i);
        if let Some(p) = row.project {
            self.by_name.entry((p, row.fold.clone())).or_insert(i);
        }
        self.any_name.entry(row.fold.clone()).or_insert(i);
        self.rows.push(row);
    }

    /// The status a history value names, for a task of `project`.
    fn resolve(&self, value: &str, project: Id) -> Option<usize> {
        if let Ok(id) = value.trim().parse::<Id>() {
            return self.by_id.get(&id).copied();
        }
        let f = fold(value);
        self.by_name
            .get(&(project, f.clone()))
            .or_else(|| self.any_name.get(&f))
            .copied()
    }

    fn kind(&self, i: usize) -> Kind {
        match self.rows[i].category {
            StatusCategory::Todo => Kind::Todo,
            StatusCategory::InProgress => Kind::InProgress,
            StatusCategory::Done | StatusCategory::Cancelled => Kind::Closed,
        }
    }

    /// The first open status of `project` in board order.
    fn first_open(&self, project: Id) -> Option<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, s)| s.project == Some(project) && !s.category.is_closed())
            .min_by(|a, b| (a.1.category, &a.1.rank).cmp(&(b.1.category, &b.1.rank)))
            .map(|(i, _)| i)
    }
}

fn text_at(
    r: &gk_store::rusqlite::Row<'_>,
    i: usize,
) -> gk_store::rusqlite::Result<Option<String>> {
    use gk_store::rusqlite::types::ValueRef;
    Ok(match r.get_ref(i)? {
        ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
        _ => None,
    })
}

/// Who did something: kind (`user`, `agent`) and name.
type Who = (String, String);

/// One task in scope, replayed.
struct TaskFlow {
    key: String,
    title: String,
    created: i64,
    created_by: Who,
    replay: Replay<Who>,
}

/// Everything a report reads, for one input.
struct Flow {
    zone: Zone,
    from: Day,
    to: Day,
    bucket: FlowBucket,
    periods: Vec<Period>,
    /// Each period's start and end (exclusive), in milliseconds.
    bounds: Vec<(i64, i64)>,
    statuses: Statuses,
    /// The statuses of the projects in scope, which the cumulative flow
    /// shows even when empty.
    scoped_statuses: Vec<usize>,
    tasks: Vec<TaskFlow>,
}

impl Flow {
    fn load(q: &QueryCtx<'_>, input: &FlowReportInput) -> Result<Self, Failure> {
        let tx = q.tx;
        let zone = q.env.zone.clone();
        let settings = super::settings::load(tx)?;
        let week_start = first_weekday(settings.week_start);
        let today = zone.day_of(q.now);
        let bucket = input.bucket.unwrap_or_default();
        let day = |field: &str, text: &str| {
            worklog::parse_work_day(text, today).map_err(|e| invalid(field, e))
        };
        let to = match &input.to {
            Some(t) => day("to", t)?,
            None => today,
        };
        let from = match &input.from {
            Some(t) => day("from", t)?,
            None => flow::default_from(to, bucket, week_start),
        };
        if to < from {
            return Err(messages::invalid("to", "activity-to-before-from", &[]));
        }
        let periods =
            flow::periods(from, to, bucket, week_start, MAX_PERIODS).ok_or_else(|| {
                messages::invalid(
                    "from",
                    "flow-report-too-many-periods",
                    &[("max", MAX_PERIODS.to_string())],
                )
            })?;
        let bounds = periods
            .iter()
            .map(|p| {
                let end = p.last.add_days(1).unwrap_or(p.last);
                (zone.moment(p.first, 0), zone.moment(end, 0))
            })
            .collect();
        if input.actor.as_deref().is_some_and(|a| a.trim().is_empty()) {
            return Err(messages::invalid("actor", "flow-report-no-actor", &[]));
        }

        let projects: Option<HashSet<Id>> = match &input.project {
            Some(p) => {
                let p = lookup::project(tx, "project", p)?;
                Some(if input.include_subprojects.unwrap_or(true) {
                    super::project::subtree(tx, p.id)?.into_iter().collect()
                } else {
                    HashSet::from([p.id])
                })
            }
            None => None,
        };
        let types = match input.task_type.as_deref().map(str::trim) {
            Some(name) => Some(named_ids(tx, "task_types", "type", name)?),
            None => None,
        };
        let labelled: Option<HashSet<Id>> = match input.label.as_deref().map(str::trim) {
            Some(name) => {
                let labels = named_ids(tx, "labels", "label", name)?;
                let mut stmt = tx.prepare_cached("SELECT task_id, label_id FROM task_labels")?;
                let rows = stmt.query_map([], |r| Ok((id_at(r, 0)?, id_at(r, 1)?)))?;
                let mut set = HashSet::new();
                for row in rows {
                    let (task, label) = row?;
                    if labels.contains(&label) {
                        set.insert(task);
                    }
                }
                Some(set)
            }
            None => None,
        };
        let versions = match input.fix_version.as_deref().map(str::trim) {
            Some(text) => Some(version_ids(tx, text)?),
            None => None,
        };
        let matching: Option<HashSet<Id>> = match input.query.as_deref().map(str::trim) {
            Some(src) if !src.is_empty() => {
                let (_, checked) = super::query::compile(tx, &zone, q.now, src, "query")?;
                let compiled = Compiled::new(
                    &checked,
                    &SqlEnv {
                        now: q.now,
                        scope: None,
                        default_sort: Vec::new(),
                    },
                );
                let compiled = super::query::with_schedule(tx, &checked, compiled)?;
                Some(
                    super::query::matching_ids(tx, &compiled)?
                        .into_iter()
                        .collect(),
                )
            }
            _ => None,
        };

        let statuses = Statuses::load(tx)?;
        let mut allowed: HashMap<Id, bool> = HashMap::new();
        let mut rows = Vec::new();
        {
            let mut stmt = tx.prepare_cached(
                "SELECT t.id, t.project_id, t.type_id, p.key || '-' || t.number, t.title,
                        t.status_id, t.status_entered_at, t.created_at, t.created_by_kind,
                        t.created_by_name, t.fix_version_id
                   FROM tasks t JOIN projects p ON p.id = t.project_id
                  WHERE t.deleted_at IS NULL AND p.deleted_at IS NULL",
            )?;
            let found = stmt.query_map([], |r| {
                Ok(TaskRow {
                    id: id_at(r, 0)?,
                    project: id_at(r, 1)?,
                    kind: id_at(r, 2)?,
                    key: r.get(3)?,
                    title: r.get(4)?,
                    status: id_at(r, 5)?,
                    entered: r.get(6)?,
                    created: r.get(7)?,
                    by: (r.get(8)?, r.get(9)?),
                    version: lookup::opt_id_at(r, 10)?,
                })
            })?;
            for t in found {
                let t = t?;
                if projects.as_ref().is_some_and(|p| !p.contains(&t.project))
                    || types.as_ref().is_some_and(|s| !s.contains(&t.kind))
                    || labelled.as_ref().is_some_and(|s| !s.contains(&t.id))
                    || versions
                        .as_ref()
                        .is_some_and(|s| !t.version.is_some_and(|v| s.contains(&v)))
                    || matching.as_ref().is_some_and(|s| !s.contains(&t.id))
                {
                    continue;
                }
                if crate::scope::active() {
                    let ok = match allowed.get(&t.project) {
                        Some(ok) => *ok,
                        None => {
                            let ok = crate::scope::allows(tx, t.project)?;
                            allowed.insert(t.project, ok);
                            ok
                        }
                    };
                    if !ok {
                        continue;
                    }
                }
                rows.push(t);
            }
        }
        let index: HashMap<Id, usize> = rows.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
        let mut events: Vec<Vec<Event<Who>>> = vec![Vec::new(); rows.len()];
        {
            let mut stmt = tx.prepare_cached(
                "SELECT entity_id, at, actor_kind, actor_name,
                        json_extract(changes, '$.fields.status.after'),
                        json_type(changes, '$.fields.trashed'),
                        json_extract(changes, '$.fields.trashed.after'),
                        json_type(changes, '$.status'),
                        json_extract(changes, '$.status')
                   FROM activity
                  WHERE entity_kind = 'task'
                    AND (json_type(changes, '$.fields.status') IS NOT NULL
                         OR json_type(changes, '$.fields.trashed') IS NOT NULL
                         OR json_type(changes, '$.status') IS NOT NULL)
                  ORDER BY seq",
            )?;
            let found = stmt.query_map([], |r| {
                Ok((
                    id_at(r, 0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    text_at(r, 4)?,
                    text_at(r, 5)?,
                    r.get::<_, Option<i64>>(6).unwrap_or(None),
                    text_at(r, 7)?,
                    text_at(r, 8)?,
                ))
            })?;
            for row in found {
                let (id, at, kind, name, status, trash_type, trashed, legacy_type, legacy) = row?;
                let Some(&i) = index.get(&id) else {
                    continue;
                };
                let project = rows[i].project;
                let actor = Some((kind, name));
                if trash_type.is_some() {
                    events[i].push(Event {
                        at,
                        change: Change::Trashed(trashed == Some(1)),
                        actor: actor.clone(),
                    });
                }
                let named = status.or_else(|| match legacy_type.as_deref() {
                    Some("text") => legacy,
                    Some("array") => legacy
                        .and_then(|l| serde_json::from_str::<Vec<Value>>(&l).ok())
                        .and_then(|v| v.last().and_then(|s| s.as_str().map(str::to_owned))),
                    _ => None,
                });
                if let Some(s) = named.and_then(|n| statuses.resolve(&n, project)) {
                    events[i].push(Event {
                        at,
                        change: Change::Status(s),
                        actor,
                    });
                }
            }
        }

        let mut tasks = Vec::with_capacity(rows.len());
        for (t, mut ev) in rows.into_iter().zip(events) {
            let current = statuses.by_id.get(&t.status).copied();
            let last_status = ev.iter().rev().find_map(|e| match e.change {
                Change::Status(s) => Some(s),
                Change::Trashed(_) => None,
            });
            if last_status.is_none()
                && t.entered > t.created + 1000
                && let Some(first) = statuses.first_open(t.project)
                && Some(first) != current
            {
                ev.insert(
                    0,
                    Event {
                        at: t.created,
                        change: Change::Status(first),
                        actor: None,
                    },
                );
            }
            if let Some(c) = current
                && last_status != Some(c)
            {
                let after = ev.last().map_or(t.created, |e| e.at);
                ev.push(Event {
                    at: t.entered.max(after),
                    change: Change::Status(c),
                    actor: None,
                });
            }
            let replay = flow::replay(t.created, &ev, |s| statuses.kind(s));
            tasks.push(TaskFlow {
                key: t.key,
                title: t.title,
                created: t.created,
                created_by: t.by,
                replay,
            });
        }

        let scoped_statuses = statuses
            .rows
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.rank != "~"
                    && projects
                        .as_ref()
                        .is_none_or(|p| s.project.is_some_and(|id| p.contains(&id)))
            })
            .map(|(i, _)| i)
            .collect();

        Ok(Self {
            zone,
            from,
            to,
            bucket,
            periods,
            bounds,
            statuses,
            scoped_statuses,
            tasks,
        })
    }

    /// The range's start and end (exclusive), in milliseconds.
    fn range(&self) -> (i64, i64) {
        (
            self.bounds.first().map_or(0, |b| b.0),
            self.bounds.last().map_or(0, |b| b.1),
        )
    }

    /// The period moment `at` falls in.
    fn period_of(&self, at: i64) -> Option<usize> {
        let (start, end) = self.range();
        if at < start || at >= end {
            return None;
        }
        Some(self.bounds.partition_point(|b| b.1 <= at))
    }

    /// Whether the task is open (live and in an open status) at `at`.
    fn open_at(&self, t: &TaskFlow, at: i64) -> bool {
        t.replay.at(at).is_some_and(|s| {
            s.live
                && s.status
                    .is_some_and(|i| self.statuses.kind(i) != Kind::Closed)
        })
    }
}

struct TaskRow {
    id: Id,
    project: Id,
    kind: Id,
    key: String,
    title: String,
    status: Id,
    entered: i64,
    created: i64,
    by: Who,
    version: Option<Id>,
}

/// The IDs of the rows of `table` named `name` (any project); an unknown
/// name is a mistake in `field`.
fn named_ids(
    tx: &Transaction<'_>,
    table: &str,
    field: &str,
    name: &str,
) -> Result<HashSet<Id>, Failure> {
    let mut stmt = tx.prepare_cached(&format!("SELECT id FROM {table} WHERE name_fold = ?1"))?;
    let ids = stmt
        .query_map(params![fold(name)], |r| id_at(r, 0))?
        .collect::<Result<HashSet<_>, _>>()?;
    if ids.is_empty() {
        return Err(messages::invalid(
            field,
            if field == "type" {
                "flow-report-unknown-type"
            } else {
                "flow-report-unknown-label"
            },
            &[("name", name.to_owned())],
        ));
    }
    Ok(ids)
}

/// The versions `text` names: the one of that ID, or every version of
/// that name.
fn version_ids(tx: &Transaction<'_>, text: &str) -> Result<HashSet<Id>, Failure> {
    if let Ok(id) = text.parse::<Id>() {
        let found: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM versions WHERE id = ?1)",
            params![lookup::b(id)],
            |r| r.get(0),
        )?;
        if found {
            return Ok(HashSet::from([id]));
        }
    }
    let mut stmt = tx.prepare_cached("SELECT id FROM versions WHERE name_fold = ?1")?;
    let ids = stmt
        .query_map(params![fold(text)], |r| id_at(r, 0))?
        .collect::<Result<HashSet<_>, _>>()?;
    if ids.is_empty() {
        return Err(messages::invalid(
            "fix_version",
            "flow-report-unknown-version",
            &[("name", text.to_owned())],
        ));
    }
    Ok(ids)
}

/// Whether `who` (`me`, `agent` or a name) did it.
fn by(who: Option<&str>, actor: Option<&Who>) -> bool {
    let Some(who) = who.map(str::trim) else {
        return true;
    };
    let Some((kind, name)) = actor else {
        return false;
    };
    match who.to_lowercase().as_str() {
        "me" => kind == "user",
        "agent" => kind == "agent",
        n => name.to_lowercase() == n,
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn category(c: StatusCategory) -> Category {
    match c {
        StatusCategory::Todo => Category::Todo,
        StatusCategory::InProgress => Category::InProgress,
        StatusCategory::Done => Category::Done,
        StatusCategory::Cancelled => Category::Cancelled,
    }
}

// ---------------------------------------------------------------------
// The reports

fn created_closed(f: &Flow, input: &FlowReportInput) -> Result<CreatedClosedReport, Failure> {
    let who = input.actor.as_deref();
    let n = f.periods.len();
    let (mut created, mut closed, mut reopened, mut open) = (
        vec![0usize; n],
        vec![0usize; n],
        vec![0usize; n],
        vec![0usize; n],
    );
    let (start, _) = f.range();
    let mut open_before = 0usize;
    for t in &f.tasks {
        if by(who, Some(&t.created_by))
            && let Some(p) = f.period_of(t.created)
        {
            created[p] += 1;
        }
        for (at, actor) in &t.replay.closes {
            if by(who, actor.as_ref())
                && let Some(p) = f.period_of(*at)
            {
                closed[p] += 1;
            }
        }
        for (at, actor) in &t.replay.reopens {
            if by(who, actor.as_ref())
                && let Some(p) = f.period_of(*at)
            {
                reopened[p] += 1;
            }
        }
        if f.open_at(t, start - 1) {
            open_before += 1;
        }
        for (p, (_, end)) in f.bounds.iter().enumerate() {
            if f.open_at(t, end - 1) {
                open[p] += 1;
            }
        }
    }
    let items: Vec<CreatedClosedRow> = f
        .periods
        .iter()
        .enumerate()
        .map(|(i, p)| CreatedClosedRow {
            start: p.first.to_string(),
            end: p.last.to_string(),
            created: count(created[i]),
            closed: count(closed[i]),
            reopened: count(reopened[i]),
            open: count(open[i]),
        })
        .collect();
    let csv = input.csv.then(|| {
        let mut out = String::new();
        csv_line(
            &mut out,
            &["start", "end", "created", "closed", "reopened", "open"],
        );
        for r in &items {
            csv_line(
                &mut out,
                &[
                    &r.start,
                    &r.end,
                    &r.created.to_string(),
                    &r.closed.to_string(),
                    &r.reopened.to_string(),
                    &r.open.to_string(),
                ],
            );
        }
        out
    });
    Ok(CreatedClosedReport {
        from: f.from.to_string(),
        to: f.to.to_string(),
        bucket: f.bucket,
        created: count(created.iter().sum()),
        closed: count(closed.iter().sum()),
        reopened: count(reopened.iter().sum()),
        open_before: count(open_before),
        items,
        csv,
    })
}

fn cumulative_flow(f: &Flow, input: &FlowReportInput) -> Result<CumulativeFlowReport, Failure> {
    let group_by = input.group_by.unwrap_or_default();
    // Each status's band, and the bands in workflow order.
    let mut band_of: Vec<Option<usize>> = vec![None; f.statuses.rows.len()];
    let mut bands: Vec<FlowBand> = Vec::new();
    let mut keys: HashMap<String, usize> = HashMap::new();
    if group_by == FlowGroupBy::Category {
        for c in StatusCategory::ALL {
            keys.insert(c.as_str().to_owned(), bands.len());
            bands.push(FlowBand {
                key: c.as_str().to_owned(),
                name: c.as_str().to_owned(),
                category: category(c),
                hue: None,
            });
        }
    }
    let mut sorted: Vec<usize> = (0..f.statuses.rows.len()).collect();
    sorted.sort_by(|a, b| {
        let (a, b) = (&f.statuses.rows[*a], &f.statuses.rows[*b]);
        (a.category, &a.rank, &a.fold).cmp(&(b.category, &b.rank, &b.fold))
    });
    for i in sorted {
        let s = &f.statuses.rows[i];
        let key = match group_by {
            FlowGroupBy::Status => s.fold.clone(),
            FlowGroupBy::Category => s.category.as_str().to_owned(),
        };
        let b = *keys.entry(key.clone()).or_insert_with(|| {
            let band = match group_by {
                FlowGroupBy::Status => FlowBand {
                    key,
                    name: s.name.clone(),
                    category: category(s.category),
                    hue: s.hue,
                },
                FlowGroupBy::Category => FlowBand {
                    name: key.clone(),
                    key,
                    category: category(s.category),
                    hue: None,
                },
            };
            bands.push(band);
            bands.len() - 1
        });
        band_of[i] = Some(b);
    }
    let mut counts = vec![vec![0u32; bands.len()]; f.periods.len()];
    for t in &f.tasks {
        for (p, (_, end)) in f.bounds.iter().enumerate() {
            if let Some(s) = t.replay.at(end - 1)
                && s.live
                && let Some(b) = s.status.and_then(|i| band_of[i])
            {
                counts[p][b] += 1;
            }
        }
    }
    // Keep the bands of the statuses in scope and every band with tasks.
    let mut keep = vec![false; bands.len()];
    for &i in &f.scoped_statuses {
        if let Some(b) = band_of[i] {
            keep[b] = true;
        }
    }
    for row in &counts {
        for (b, n) in row.iter().enumerate() {
            if *n > 0 {
                keep[b] = true;
            }
        }
    }
    if group_by == FlowGroupBy::Category {
        keep.fill(true);
    }
    let kept: Vec<usize> = (0..bands.len()).filter(|b| keep[*b]).collect();
    let items: Vec<CumulativeFlowRow> = f
        .periods
        .iter()
        .zip(&counts)
        .map(|(p, row)| CumulativeFlowRow {
            start: p.first.to_string(),
            end: p.last.to_string(),
            counts: kept.iter().map(|b| row[*b]).collect(),
        })
        .collect();
    let bands: Vec<FlowBand> = kept.iter().map(|b| bands[*b].clone()).collect();
    let csv = input.csv.then(|| {
        let mut out = String::new();
        let mut head: Vec<&str> = vec!["start", "end"];
        head.extend(bands.iter().map(|b| b.name.as_str()));
        csv_line(&mut out, &head);
        for r in &items {
            let numbers: Vec<String> = r.counts.iter().map(u32::to_string).collect();
            let mut cells: Vec<&str> = vec![&r.start, &r.end];
            cells.extend(numbers.iter().map(String::as_str));
            csv_line(&mut out, &cells);
        }
        out
    });
    Ok(CumulativeFlowReport {
        from: f.from.to_string(),
        to: f.to.to_string(),
        bucket: f.bucket,
        group_by,
        bands,
        items,
        csv,
    })
}

fn cycle_time(f: &Flow, input: &FlowReportInput) -> Result<CycleTimeReport, Failure> {
    let who = input.actor.as_deref();
    let n = f.periods.len();
    let mut cycle: Vec<Vec<f64>> = vec![Vec::new(); n];
    let mut lead: Vec<Vec<f64>> = vec![Vec::new(); n];
    let mut tasks: Vec<(i64, CycleTimeTask)> = Vec::new();
    for t in &f.tasks {
        let Some(last) = t.replay.states.last() else {
            continue;
        };
        let closed_now = last.live
            && last
                .status
                .is_some_and(|s| f.statuses.kind(s) == Kind::Closed);
        let Some((at, actor)) = t.replay.closes.last() else {
            continue;
        };
        if !closed_now || !by(who, actor.as_ref()) {
            continue;
        }
        let Some(p) = f.period_of(*at) else {
            continue;
        };
        let lead_days = flow::days(at - t.created);
        let cycle_days = t
            .replay
            .first_progress
            .filter(|fp| fp <= at)
            .map(|fp| flow::days(at - fp));
        lead[p].push(lead_days);
        if let Some(c) = cycle_days {
            cycle[p].push(c);
        }
        let reopened = t.replay.reopens.iter().filter(|r| r.0 <= *at).count();
        tasks.push((
            *at,
            CycleTimeTask {
                key: t.key.clone(),
                title: t.title.clone(),
                closed_at: rfc3339(*at),
                closed_day: f.zone.day_of(*at).to_string(),
                cycle_days: cycle_days.map(flow::round2),
                lead_days: flow::round2(lead_days),
                reopened: count(reopened),
            },
        ));
    }
    tasks.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.key.cmp(&b.1.key)));
    let tasks: Vec<CycleTimeTask> = tasks.into_iter().map(|(_, t)| t).collect();
    let items = f
        .periods
        .iter()
        .enumerate()
        .map(|(i, p)| CycleTimeRow {
            start: p.first.to_string(),
            end: p.last.to_string(),
            cycle: flow::stats(cycle[i].clone()),
            lead: flow::stats(lead[i].clone()),
        })
        .collect();
    let csv = input.csv.then(|| {
        let mut out = String::new();
        csv_line(
            &mut out,
            &[
                "key",
                "title",
                "closed_at",
                "closed_day",
                "cycle_days",
                "lead_days",
                "reopened",
            ],
        );
        for t in &tasks {
            csv_line(
                &mut out,
                &[
                    &t.key,
                    &t.title,
                    &t.closed_at,
                    &t.closed_day,
                    &t.cycle_days.map(|c| c.to_string()).unwrap_or_default(),
                    &t.lead_days.to_string(),
                    &t.reopened.to_string(),
                ],
            );
        }
        out
    });
    Ok(CycleTimeReport {
        from: f.from.to_string(),
        to: f.to.to_string(),
        bucket: f.bucket,
        cycle: flow::stats(cycle.concat()),
        lead: flow::stats(lead.concat()),
        items,
        tasks: input.tasks.then_some(tasks),
        csv,
    })
}
