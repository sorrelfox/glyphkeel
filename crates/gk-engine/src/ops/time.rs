// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Time entries, the timer and the time report.
//!
//! Rules: an entry is 1 minute to a day of work on one day, on a live task
//! in an active project. A task's `spent_min` is the sum of its live
//! entries, recomputed in the transaction that changes them; logging time
//! changes the task's `updated_at`, not its fields' `edited_at` or version.
//! Editing an entry raises its version; deleting moves it to the trash.
//!
//! One timer per workspace (one table row). Its time is the time of its
//! earlier running stretches plus the current one; a paused timer has no
//! current stretch. Starting it on another task stops the running one and
//! logs its time first. Stopping logs the time, rounded to the minute, on
//! the day the timer was first started.
//!
//! An entry and the timer may name a kind of work (`work_kind.rs`).
//!
//! History records an entry's `created` (its life), `trashed`, `task`,
//! `date`, `minutes`, `note` and `work_kind` (its ID), and the timer as one value (`timer`) on an
//! item whose ID is the workspace's, so Undo and revert cover both.

use std::collections::{HashMap, HashSet};

use gk_api::OpError;
use gk_api::ops::settings::WeekStart;
use gk_api::ops::time::{
    TimeDelete, TimeEntry, TimeEntryChange, TimeEntryRef, TimeGroupBy, TimeLog, TimeLogInput,
    TimeReport, TimeReportCsv, TimeReportInput, TimeReportOp, TimeReportRow, TimeUpdate,
    TimeUpdateInput, Timer, TimerChange, TimerGet, TimerPause, TimerStart, TimerStartInput,
    TimerState, TimerStatus, TimerStop, TimerStopInput, WorkKindRef,
};
use gk_core::worklog::{self, WorklogError};
use gk_core::{Day, DurationUnits, Id, Weekday, format_duration};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Row, Transaction, params};
use serde_json::{Value, json};

use super::comment::actor;
use super::desc;
use super::fields::text;
use super::lookup::{self, b, id_at};
use super::task::{invalid, task_ref, units};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::time::{Zone, rfc3339};
use crate::write::WriteCtx;

/// History's name for time entries.
pub(crate) const ENTRY: &str = "time_entry";
/// History's name for the timer; its ID is the workspace's.
pub(crate) const TIMER: &str = "timer";

/// The entry field holding its kind of work.
pub(crate) const WORK_KIND_FIELD: &str = "work_kind";

/// The longest range of the time report, in days.
const REPORT_MAX_DAYS: i64 = 366;

pub(super) fn register(r: &mut Registry) {
    r.command::<TimeLog, _>(desc::<TimeLog>(), log);
    r.command::<TimeUpdate, _>(desc::<TimeUpdate>(), update);
    r.command::<TimeDelete, _>(desc::<TimeDelete>(), |w, input: TimeEntryRef| {
        let e = find(w.tx(), &input.entry)?;
        ensure_active(w.tx(), e.project)?;
        set_live(w, e.id, Some(e.project), "trashed", Some(Value::Bool(true)))?;
        changed(w, e.id)
    });
    r.query::<TimeReportOp, _>(desc::<TimeReportOp>(), |q, input| {
        report(q.tx, &q.env.zone, q.now, input)
    });
    r.query::<TimerGet, _>(desc::<TimerGet>(), |q, _input| {
        Ok(TimerStatus {
            timer: current(q.tx, q.now, units(q.tx)?)?,
        })
    });
    r.command::<TimerStart, _>(desc::<TimerStart>(), start);
    r.command::<TimerPause, _>(desc::<TimerPause>(), |w, _input| pause(w));
    r.command::<TimerStop, _>(desc::<TimerStop>(), stop);
}

fn bad(field: &str, e: &WorklogError) -> Failure {
    invalid(field, e)
}

fn ensure_active(tx: &Transaction<'_>, project: Id) -> Result<(), Failure> {
    if let Some(p) = lookup::project_by_id(tx, project)? {
        lookup::ensure_active(&p)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Entries

const COLUMNS: &str = "e.id, p.key || '-' || t.number, e.work_date, e.minutes, e.note,
    e.actor_kind, e.actor_name, e.created_at, e.edited_at, e.version, k.id, k.name";

const FROM: &str = "time_entries e JOIN tasks t ON t.id = e.task_id
    JOIN projects p ON p.id = t.project_id LEFT JOIN work_kinds k ON k.id = e.work_kind_id";

fn row(r: &Row<'_>, units: DurationUnits) -> gk_store::rusqlite::Result<TimeEntry> {
    let minutes: i64 = r.get(3)?;
    Ok(TimeEntry {
        id: id_at(r, 0)?.to_string(),
        task: r.get(1)?,
        date: r.get(2)?,
        minutes,
        duration: format_duration(minutes, units),
        note: r.get(4)?,
        work_kind: match (lookup::opt_id_at(r, 10)?, r.get::<_, Option<String>>(11)?) {
            (Some(id), Some(name)) => Some(WorkKindRef {
                id: id.to_string(),
                name,
            }),
            _ => None,
        },
        author: actor(&r.get::<_, String>(5)?, r.get(6)?),
        created_at: rfc3339(r.get(7)?),
        edited_at: r.get::<_, Option<i64>>(8)?.map(rfc3339),
        version: r.get(9)?,
    })
}

/// One entry as outputs carry it, trashed or not.
fn load(tx: &Transaction<'_>, id: Id, units: DurationUnits) -> Result<TimeEntry, Failure> {
    Ok(tx.query_row(
        &format!("SELECT {COLUMNS} FROM {FROM} WHERE e.id = ?1"),
        [b(id)],
        |r| row(r, units),
    )?)
}

/// The live entries of task `task`, newest day first.
pub(crate) fn of_task(
    tx: &Transaction<'_>,
    task: Id,
    units: DurationUnits,
) -> Result<Vec<TimeEntry>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM {FROM} WHERE e.task_id = ?1 AND e.deleted_at IS NULL
          ORDER BY e.work_date DESC, e.created_at DESC, e.id DESC"
    ))?;
    let items = stmt
        .query_map([b(task)], |r| row(r, units))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// What commands need of an entry row.
struct EntryRow {
    id: Id,
    project: Id,
    version: i64,
}

/// The live entry `text` names (its ID), on a live task.
fn find(tx: &Transaction<'_>, text: &str) -> Result<EntryRow, Failure> {
    let missing = || {
        Failure::Op(messages::hint(
            messages::not_found("time-entry-not-found", &[("entry", text.to_owned())])
                .with_detail("field", "entry"),
            "time-entry-not-found-hint",
            &[],
        ))
    };
    let id: Id = text.trim().parse().map_err(|_| missing())?;
    tx.query_row(
        "SELECT e.id, t.project_id, e.version FROM time_entries e
           JOIN tasks t ON t.id = e.task_id
          WHERE e.id = ?1 AND e.deleted_at IS NULL AND t.deleted_at IS NULL",
        [b(id)],
        |r| {
            Ok(EntryRow {
                id: id_at(r, 0)?,
                project: id_at(r, 1)?,
                version: r.get(2)?,
            })
        },
    )
    .optional()?
    .filter(|e: &EntryRow| crate::scope::sees(e.project))
    .ok_or_else(missing)
}

/// The task of entry `id`.
fn task_of(tx: &Transaction<'_>, id: Id) -> Result<Id, Failure> {
    Ok(tx.query_row(
        "SELECT task_id FROM time_entries WHERE id = ?1",
        [b(id)],
        |r| id_at(r, 0),
    )?)
}

/// Recomputes the task's logged time from its live entries and marks the
/// task as updated. Returns the new total.
fn refresh_spent(w: &WriteCtx<'_>, task: Id) -> Result<i64, Failure> {
    w.tx().execute(
        "UPDATE tasks SET spent_min = (SELECT coalesce(sum(minutes), 0) FROM time_entries
                                        WHERE task_id = ?1 AND deleted_at IS NULL),
                          updated_at = ?2
          WHERE id = ?1",
        params![b(task), w.now()],
    )?;
    Ok(w.tx().query_row(
        "SELECT spent_min FROM tasks WHERE id = ?1",
        [b(task)],
        |r| r.get(0),
    )?)
}

/// Logs `minutes` on `task` on `date` and records it. Returns the entry.
fn insert(
    w: &mut WriteCtx<'_>,
    task: &lookup::TaskRow,
    date: Day,
    minutes: i64,
    note: &str,
    work_kind: Option<Id>,
) -> Result<Id, Failure> {
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let caller = w.caller().clone();
    w.tx().execute(
        "INSERT INTO time_entries (id, task_id, work_date, minutes, note, actor_kind, actor_name,
                                   session_id, created_at, work_kind_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            b(id),
            b(task.id),
            date.to_string(),
            minutes,
            note,
            caller.actor.kind.as_str(),
            caller.actor.name,
            caller.session.map(b),
            w.now(),
            work_kind.map(b)
        ],
    )?;
    refresh_spent(w, task.id)?;
    let key = lookup::task_key(w.tx(), task.project, task.number)?;
    let pid = Some(task.project);
    w.record_field(ENTRY, id, pid, "created", None, Some(Value::Bool(true)));
    w.record_field(ENTRY, id, pid, "task", None, text(key));
    w.record_field(ENTRY, id, pid, "date", None, text(date.to_string()));
    w.record_field(ENTRY, id, pid, "minutes", None, Some(minutes.into()));
    if !note.is_empty() {
        w.record_field(ENTRY, id, pid, "note", None, text(note));
    }
    if let Some(kind) = work_kind {
        w.record_field(
            ENTRY,
            id,
            pid,
            WORK_KIND_FIELD,
            None,
            text(kind.to_string()),
        );
    }
    Ok(id)
}

fn work_day(w: &WriteCtx<'_>, field: &str, text: Option<&str>) -> Result<Day, Failure> {
    let today = w.zone().day_of(w.now());
    match text {
        Some(t) => worklog::parse_work_day(t, today).map_err(|e| bad(field, &e)),
        None => Ok(today),
    }
}

pub(crate) fn log(w: &mut WriteCtx<'_>, input: TimeLogInput) -> Result<TimeEntryChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    ensure_active(w.tx(), task.project)?;
    let units = units(w.tx())?;
    let minutes =
        worklog::entry_minutes(&input.duration, units).map_err(|e| bad("duration", &e))?;
    let date = work_day(w, "date", input.date.as_deref())?;
    let note = worklog::entry_note(input.note.as_deref().unwrap_or_default())
        .map_err(|e| bad("note", &e))?;
    let kind = match &input.work_kind {
        Some(k) => super::work_kind::for_entry(w.tx(), "work_kind", k, None)?,
        None => None,
    };
    let id = insert(w, &task, date, minutes, &note, kind)?;
    changed(w, id)
}

fn update(w: &mut WriteCtx<'_>, input: TimeUpdateInput) -> Result<TimeEntryChange, Failure> {
    let e = find(w.tx(), &input.entry)?;
    ensure_active(w.tx(), e.project)?;
    if let Some(expected) = input.expected_version
        && expected != e.version
    {
        return Err(messages::hint(
            messages::conflict(
                "time-entry-version",
                &[
                    ("version", e.version.to_string()),
                    ("expected", expected.to_string()),
                ],
            )
            .with_detail("version", e.version),
            "time-entry-version-hint",
            &[],
        )
        .into());
    }
    let units = units(w.tx())?;
    let mut wanted: Vec<(&str, Option<Value>)> = Vec::new();
    if let Some(d) = &input.duration {
        let minutes = worklog::entry_minutes(d, units).map_err(|e| bad("duration", &e))?;
        wanted.push(("minutes", Some(minutes.into())));
    }
    if let Some(d) = &input.date {
        let day = work_day(w, "date", Some(d))?;
        wanted.push(("date", text(day.to_string())));
    }
    if let Some(n) = &input.note {
        let note = worklog::entry_note(n).map_err(|e| bad("note", &e))?;
        wanted.push(("note", (!note.is_empty()).then_some(Value::String(note))));
    }
    if let Some(k) = &input.work_kind {
        let current = entry_kind(w.tx(), e.id)?;
        let kind = super::work_kind::for_entry(w.tx(), "work_kind", k, current)?;
        wanted.push((WORK_KIND_FIELD, kind.and_then(|k| text(k.to_string()))));
    }
    let mut any = false;
    for (field, value) in wanted {
        any |= set_field(w, e.id, Some(e.project), field, value)?;
    }
    if any {
        touch(w, e.id)?;
    }
    changed(w, e.id)
}

fn changed(w: &WriteCtx<'_>, id: Id) -> Result<TimeEntryChange, Failure> {
    let (deleted, spent): (bool, i64) = w.tx().query_row(
        "SELECT e.deleted_at IS NOT NULL, t.spent_min FROM time_entries e
           JOIN tasks t ON t.id = e.task_id WHERE e.id = ?1",
        [b(id)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(TimeEntryChange {
        entry: load(w.tx(), id, units(w.tx())?)?,
        deleted,
        spent_minutes: spent,
        group: w.group_if_changed(),
    })
}

/// Raises the entry's version and edit time.
fn touch(w: &WriteCtx<'_>, id: Id) -> Result<(), Failure> {
    w.tx().execute(
        "UPDATE time_entries SET version = version + 1, edited_at = ?2 WHERE id = ?1",
        params![b(id), w.now()],
    )?;
    Ok(())
}

/// The kind of work of entry `id`.
fn entry_kind(tx: &Transaction<'_>, id: Id) -> Result<Option<Id>, StoreError> {
    Ok(tx
        .query_row(
            "SELECT work_kind_id FROM time_entries WHERE id = ?1",
            [b(id)],
            |r| lookup::opt_id_at(r, 0),
        )
        .optional()?
        .flatten())
}

/// The entry's `date`, `minutes`, `note` or `work_kind`, as history
/// records it.
pub(crate) fn raw_field(
    tx: &Transaction<'_>,
    id: Id,
    field: &str,
) -> Result<Option<Value>, StoreError> {
    let row: Option<(String, i64, String, Option<Id>)> = tx
        .query_row(
            "SELECT work_date, minutes, note, work_kind_id FROM time_entries WHERE id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, lookup::opt_id_at(r, 3)?)),
        )
        .optional()?;
    Ok(row.and_then(|(date, minutes, note, kind)| match field {
        "date" => Some(Value::String(date)),
        "minutes" => Some(minutes.into()),
        "note" => (!note.is_empty()).then_some(Value::String(note)),
        WORK_KIND_FIELD => kind.map(|k| Value::String(k.to_string())),
        _ => None,
    }))
}

/// Sets the entry's `date`, `minutes` or `note` and records the change.
/// Returns whether it changed.
fn set_field(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: Option<Id>,
    field: &str,
    value: Option<Value>,
) -> Result<bool, Failure> {
    let before = raw_field(w.tx(), id, field)?;
    if before == value {
        return Ok(false);
    }
    let corrupt = || {
        Failure::Op(messages::storage(
            "storage-field-unreadable",
            &[("field", field.to_owned())],
        ))
    };
    match field {
        "date" => {
            let day: Day = value
                .as_ref()
                .and_then(Value::as_str)
                .and_then(|t| t.parse().ok())
                .ok_or_else(corrupt)?;
            w.tx().execute(
                "UPDATE time_entries SET work_date = ?2 WHERE id = ?1",
                params![b(id), day.to_string()],
            )?;
        }
        "minutes" => {
            let minutes = value
                .as_ref()
                .and_then(Value::as_i64)
                .and_then(|m| worklog::check_entry_minutes(m).ok())
                .ok_or_else(corrupt)?;
            w.tx().execute(
                "UPDATE time_entries SET minutes = ?2 WHERE id = ?1",
                params![b(id), minutes],
            )?;
        }
        "note" => {
            let note = value.as_ref().and_then(Value::as_str).unwrap_or_default();
            w.tx().execute(
                "UPDATE time_entries SET note = ?2 WHERE id = ?1",
                params![b(id), note],
            )?;
        }
        WORK_KIND_FIELD => {
            let kind = match value.as_ref() {
                None | Some(Value::Null) => None,
                Some(v) => Some(
                    v.as_str()
                        .and_then(|t| t.parse::<Id>().ok())
                        .ok_or_else(corrupt)?,
                ),
            };
            if let Some(k) = kind
                && super::work_kind::raw(w.tx(), k)?.is_none()
            {
                return Err(messages::conflict("work-kind-gone", &[]).into());
            }
            w.tx().execute(
                "UPDATE time_entries SET work_kind_id = ?2 WHERE id = ?1",
                params![b(id), kind.map(b)],
            )?;
        }
        _ => return Err(corrupt()),
    }
    refresh_spent(w, task_of(w.tx(), id)?)?;
    w.record_field(ENTRY, id, project, field, before, value);
    Ok(true)
}

/// Writes a recorded field back, for revert and Undo.
pub(crate) fn apply_field(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: Option<Id>,
    field: &str,
    value: Option<Value>,
) -> Result<(), Failure> {
    if set_field(w, id, project, field, value)? {
        touch(w, id)?;
    }
    Ok(())
}

/// Whether entry `id` is live: as `created` records it (`true` while
/// live) or as `trashed` does (`true` while in the trash).
pub(crate) fn raw_life(
    tx: &Transaction<'_>,
    id: Id,
    field: &str,
) -> Result<Option<Value>, StoreError> {
    let live: Option<bool> = tx
        .query_row(
            "SELECT deleted_at IS NULL FROM time_entries WHERE id = ?1",
            [b(id)],
            |r| r.get(0),
        )
        .optional()?;
    Ok(live
        .filter(|l| *l == (field == "created"))
        .map(|_| Value::Bool(true)))
}

/// Moves entry `id` to the trash or back, as `field` (`created` or
/// `trashed`) set to `value` asks, and records the change.
pub(crate) fn set_live(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: Option<Id>,
    field: &str,
    value: Option<Value>,
) -> Result<(), Failure> {
    let live = (field == "created") == value.is_some();
    let before = raw_life(w.tx(), id, field)?;
    if before == value {
        return Ok(());
    }
    if live {
        w.tx().execute(
            "UPDATE time_entries SET deleted_at = NULL, deleted_group = NULL WHERE id = ?1",
            [b(id)],
        )?;
    } else {
        w.tx().execute(
            "UPDATE time_entries SET deleted_at = ?2, deleted_group = ?3 WHERE id = ?1",
            params![b(id), w.now(), b(w.group())],
        )?;
    }
    refresh_spent(w, task_of(w.tx(), id)?)?;
    w.record_field(ENTRY, id, project, field, before, value);
    Ok(())
}

// ---------------------------------------------------------------------
// The report

pub(crate) fn first_weekday(start: WeekStart) -> Weekday {
    match start {
        WeekStart::Monday => Weekday::Monday,
        WeekStart::Saturday => Weekday::Saturday,
        WeekStart::Sunday => Weekday::Sunday,
    }
}

pub(crate) fn report(
    tx: &Transaction<'_>,
    zone: &Zone,
    now: i64,
    input: TimeReportInput,
) -> Result<TimeReport, Failure> {
    let settings = super::settings::load(tx)?;
    let units =
        DurationUnits::new(settings.hours_per_day, settings.days_per_week).unwrap_or_default();
    let week_start = first_weekday(settings.week_start);
    let today = zone.day_of(now);
    let day =
        |field: &str, text: &str| worklog::parse_work_day(text, today).map_err(|e| bad(field, &e));
    let from = match &input.from {
        Some(t) => day("from", t)?,
        None => worklog::week_start(today, week_start),
    };
    let to = match &input.to {
        Some(t) => day("to", t)?,
        None => from.add_days(6).map_err(|e| invalid("from", e))?,
    };
    let span = to.days_since_epoch() - from.days_since_epoch();
    if span < 0 {
        return Err(messages::invalid("to", "activity-to-before-from", &[]));
    }
    if span >= REPORT_MAX_DAYS {
        return Err(messages::invalid(
            "to",
            "time-report-too-long",
            &[("max", REPORT_MAX_DAYS.to_string())],
        ));
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
    let tasks: Option<HashSet<Id>> = match &input.task {
        Some(t) => {
            let t = lookup::task(tx, "task", t)?;
            Some(super::task::descendants(tx, t.id)?.into_iter().collect())
        }
        None => None,
    };
    let person = input.person.as_deref().map(str::trim);
    if person.is_some_and(str::is_empty) {
        return Err(messages::invalid("person", "time-report-no-person", &[]));
    }
    // `Some(None)`: only time without a kind.
    let kind: Option<Option<Id>> = match input.work_kind.as_deref().map(str::trim) {
        None => None,
        Some(k) if k.eq_ignore_ascii_case("none") => Some(None),
        Some(k) => Some(Some(super::work_kind::find(tx, "work_kind", k)?.0)),
    };

    let mut stmt = tx.prepare_cached(&format!(
        "SELECT e.id, e.task_id, t.project_id, e.work_date, e.minutes, e.actor_kind,
                e.actor_name, e.note, k.id, k.name FROM {FROM}
          WHERE e.deleted_at IS NULL AND t.deleted_at IS NULL AND p.deleted_at IS NULL
            AND e.work_date BETWEEN ?1 AND ?2
          ORDER BY e.work_date, e.created_at, e.id"
    ))?;
    let lines: Vec<Line> = stmt
        .query_map(params![from.to_string(), to.to_string()], |r| {
            Ok(Line {
                id: id_at(r, 0)?,
                task: id_at(r, 1)?,
                project: id_at(r, 2)?,
                date: r.get(3)?,
                minutes: r.get(4)?,
                actor_kind: r.get(5)?,
                actor_name: r.get(6)?,
                note: r.get(7)?,
                kind: lookup::opt_id_at(r, 8)?,
                kind_name: r.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let lines: Vec<Line> = lines
        .into_iter()
        .filter(|l| projects.as_ref().is_none_or(|p| p.contains(&l.project)))
        .filter(|l| tasks.as_ref().is_none_or(|t| t.contains(&l.task)))
        .filter(|l| person.is_none_or(|p| l.logged_by(p)))
        .filter(|l| kind.is_none_or(|k| l.kind == k))
        .collect();

    let group_by = input.group_by.unwrap_or_default();
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, (usize, i64, u32)> = HashMap::new();
    for (i, l) in lines.iter().enumerate() {
        let key = match group_by {
            TimeGroupBy::Day => l.date.clone(),
            TimeGroupBy::Week => l.day().map_or_else(
                || l.date.clone(),
                |d| worklog::week_start(d, week_start).to_string(),
            ),
            TimeGroupBy::Month => l.date.get(..7).unwrap_or(&l.date).to_owned(),
            TimeGroupBy::Task => l.task.to_string(),
            TimeGroupBy::Project => l.project.to_string(),
            TimeGroupBy::Person => format!("{}:{}", l.actor_kind, l.actor_name),
            TimeGroupBy::WorkKind => l.kind.map_or_else(|| "none".to_owned(), |k| k.to_string()),
        };
        let g = groups.entry(key.clone()).or_insert_with(|| {
            order.push(key);
            (i, 0, 0)
        });
        g.1 += l.minutes;
        g.2 += 1;
    }
    let mut names = Names::default();
    let mut items = Vec::with_capacity(order.len());
    for key in order {
        let (first, minutes, count) = groups[&key];
        let l = &lines[first];
        let mut person = None;
        let (key, title) = match group_by {
            TimeGroupBy::Day => {
                let weekday = l
                    .day()
                    .map(|d| format!("{:?}", d.weekday()))
                    .unwrap_or_default();
                (key, weekday)
            }
            TimeGroupBy::Week => {
                let last = key
                    .parse::<Day>()
                    .ok()
                    .and_then(|d| d.add_days(6).ok())
                    .map(|d| d.to_string())
                    .unwrap_or_default();
                let title = format!("{key}..{last}");
                (key, title)
            }
            TimeGroupBy::Month => (key.clone(), key),
            TimeGroupBy::Task => names.task(tx, l.task)?,
            TimeGroupBy::Project => names.project(tx, l.project)?,
            TimeGroupBy::Person => {
                person = Some(actor(&l.actor_kind, l.actor_name.clone()));
                (key, l.actor_name.clone())
            }
            TimeGroupBy::WorkKind => (key, l.kind_name.clone().unwrap_or_default()),
        };
        items.push(TimeReportRow {
            key,
            title,
            person,
            minutes,
            duration: format_duration(minutes, units),
            entry_count: count,
        });
    }
    if matches!(
        group_by,
        TimeGroupBy::Task | TimeGroupBy::Project | TimeGroupBy::Person | TimeGroupBy::WorkKind
    ) {
        items.sort_by(|a, b| b.minutes.cmp(&a.minutes).then_with(|| a.key.cmp(&b.key)));
    } else {
        items.sort_by(|a, b| a.key.cmp(&b.key));
    }
    let total: i64 = lines.iter().map(|l| l.minutes).sum();
    let entries = if input.entries {
        Some(
            lines
                .iter()
                .map(|l| load(tx, l.id, units))
                .collect::<Result<Vec<_>, _>>()?,
        )
    } else {
        None
    };
    let csv = match input.csv {
        None => None,
        Some(TimeReportCsv::Groups) => Some(groups_csv(group_by, &items)),
        Some(TimeReportCsv::Entries) => Some(entries_csv(tx, &mut names, &lines, units)?),
    };
    Ok(TimeReport {
        from: from.to_string(),
        to: to.to_string(),
        group_by,
        items,
        total_minutes: total,
        total_duration: format_duration(total, units),
        hours_per_day: settings.hours_per_day,
        days_per_week: settings.days_per_week,
        entries,
        csv,
    })
}

/// One live entry the report counts.
struct Line {
    id: Id,
    task: Id,
    project: Id,
    date: String,
    minutes: i64,
    actor_kind: String,
    actor_name: String,
    note: String,
    kind: Option<Id>,
    kind_name: Option<String>,
}

impl Line {
    fn day(&self) -> Option<Day> {
        self.date.parse().ok()
    }

    /// Whether `who` logged it: `me` is the local user, `agent` any agent,
    /// anything else a name, compared without regard to case.
    fn logged_by(&self, who: &str) -> bool {
        match who.to_lowercase().as_str() {
            "me" => self.actor_kind == "user",
            "agent" => self.actor_kind == "agent",
            name => self.actor_name.to_lowercase() == name,
        }
    }
}

/// Task and project keys and titles, each read once.
#[derive(Default)]
struct Names {
    tasks: HashMap<Id, (String, String)>,
    projects: HashMap<Id, (String, String)>,
}

impl Names {
    fn task(&mut self, tx: &Transaction<'_>, id: Id) -> Result<(String, String), Failure> {
        if let Some(t) = self.tasks.get(&id) {
            return Ok(t.clone());
        }
        let t = task_ref(tx, id)?;
        let t = (t.key, t.title);
        self.tasks.insert(id, t.clone());
        Ok(t)
    }

    fn project(&mut self, tx: &Transaction<'_>, id: Id) -> Result<(String, String), Failure> {
        if let Some(p) = self.projects.get(&id) {
            return Ok(p.clone());
        }
        let p: (String, String) = tx.query_row(
            "SELECT key, name FROM projects WHERE id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        self.projects.insert(id, p.clone());
        Ok(p)
    }
}

/// The report's groups as CSV: the grouping's name heads the key column.
/// The column names are the JSON field names, for scripts and spreadsheets
/// alike.
fn groups_csv(group_by: TimeGroupBy, items: &[TimeReportRow]) -> String {
    let group = serde_json::to_value(group_by)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    let mut out = String::new();
    csv_line(
        &mut out,
        &[&group, "title", "minutes", "duration", "entry_count"],
    );
    for row in items {
        csv_line(
            &mut out,
            &[
                &row.key,
                &row.title,
                &row.minutes.to_string(),
                &row.duration,
                &row.entry_count.to_string(),
            ],
        );
    }
    out
}

/// The report's entries as CSV, oldest first.
fn entries_csv(
    tx: &Transaction<'_>,
    names: &mut Names,
    lines: &[Line],
    units: DurationUnits,
) -> Result<String, Failure> {
    let mut out = String::new();
    csv_line(
        &mut out,
        &[
            "date",
            "person_kind",
            "person",
            "project",
            "task",
            "title",
            "minutes",
            "duration",
            "note",
            "work_kind",
        ],
    );
    for l in lines {
        let (project, _) = names.project(tx, l.project)?;
        let (task, title) = names.task(tx, l.task)?;
        csv_line(
            &mut out,
            &[
                &l.date,
                &l.actor_kind,
                &l.actor_name,
                &project,
                &task,
                &title,
                &l.minutes.to_string(),
                &format_duration(l.minutes, units),
                &l.note,
                l.kind_name.as_deref().unwrap_or_default(),
            ],
        );
    }
    Ok(out)
}

/// One CSV line (RFC 4180, CRLF). A text cell that a spreadsheet would
/// run as a formula (it starts with `=`, `+`, `-`, `@`, a tab or a carriage
/// return) gets a leading apostrophe; numbers stay as they are.
pub(crate) fn csv_line(out: &mut String, cells: &[&str]) {
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let numeric = !cell.is_empty() && cell.bytes().all(|c| c.is_ascii_digit());
        let guarded = if !numeric && cell.starts_with(['=', '+', '-', '@', '\t', '\r']) {
            format!("'{cell}")
        } else {
            (*cell).to_owned()
        };
        if guarded.contains([',', '"', '\n', '\r']) {
            out.push('"');
            out.push_str(&guarded.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(&guarded);
        }
    }
    out.push_str("\r\n");
}

// ---------------------------------------------------------------------
// The timer

/// The timer's row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TimerRow {
    pub(crate) task: Id,
    pub(crate) started_at: i64,
    pub(crate) running_since: Option<i64>,
    pub(crate) elapsed_ms: i64,
    note: String,
    work_kind: Option<Id>,
    actor_kind: String,
    actor_name: String,
    pub(crate) reminded_at: Option<i64>,
}

impl TimerRow {
    /// Its time at `now`, in milliseconds.
    pub(crate) fn elapsed_at(&self, now: i64) -> i64 {
        self.elapsed_ms + self.running_since.map_or(0, |s| (now - s).max(0))
    }
}

/// The timer's row, if there is one.
pub(crate) fn timer_row(tx: &Transaction<'_>) -> Result<Option<TimerRow>, StoreError> {
    Ok(tx
        .query_row(
            "SELECT task_id, started_at, running_since, elapsed_ms, note, actor_kind, actor_name,
                    reminded_at, work_kind_id
               FROM timer WHERE id = 1",
            [],
            |r| {
                Ok(TimerRow {
                    task: id_at(r, 0)?,
                    started_at: r.get(1)?,
                    running_since: r.get(2)?,
                    elapsed_ms: r.get(3)?,
                    note: r.get(4)?,
                    actor_kind: r.get(5)?,
                    actor_name: r.get(6)?,
                    reminded_at: r.get(7)?,
                    work_kind: lookup::opt_id_at(r, 8)?,
                })
            },
        )
        .optional()?)
}

fn timer_out(
    tx: &Transaction<'_>,
    row: &TimerRow,
    now: i64,
    units: DurationUnits,
) -> Result<Timer, Failure> {
    let elapsed = row.elapsed_at(now);
    let trashed: bool = tx.query_row(
        "SELECT t.deleted_at IS NOT NULL OR p.deleted_at IS NOT NULL
           FROM tasks t JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
        [b(row.task)],
        |r| r.get(0),
    )?;
    Ok(Timer {
        task: task_ref(tx, row.task)?,
        state: if row.running_since.is_some() {
            TimerState::Running
        } else {
            TimerState::Paused
        },
        started_at: rfc3339(row.started_at),
        running_since: row.running_since.map(rfc3339),
        elapsed_seconds: elapsed / 1000,
        elapsed: format_duration(worklog::timer_minutes(elapsed), units),
        note: row.note.clone(),
        work_kind: super::work_kind::reference(tx, row.work_kind)?,
        started_by: actor(&row.actor_kind, row.actor_name.clone()),
        task_trashed: trashed,
    })
}

/// The timer as outputs carry it at `now`, if there is one.
pub(crate) fn current(
    tx: &Transaction<'_>,
    now: i64,
    units: DurationUnits,
) -> Result<Option<Timer>, Failure> {
    timer_row(tx)?
        .map(|row| timer_out(tx, &row, now, units))
        .transpose()
}

/// The timer as history records it.
pub(crate) fn raw_timer(tx: &Transaction<'_>) -> Result<Option<Value>, StoreError> {
    Ok(timer_row(tx)?.map(|r| {
        let mut v = json!({
            "task": r.task.to_string(),
            "started_at": r.started_at,
            "running_since": r.running_since,
            "elapsed_ms": r.elapsed_ms,
            "note": r.note,
            "actor_kind": r.actor_kind,
            "actor_name": r.actor_name,
        });
        if let Some(k) = r.work_kind {
            v["work_kind"] = Value::String(k.to_string());
        }
        v
    }))
}

fn row_from_value(value: &Value) -> Option<TimerRow> {
    Some(TimerRow {
        task: value.get("task")?.as_str()?.parse().ok()?,
        started_at: value.get("started_at")?.as_i64()?,
        running_since: value.get("running_since").and_then(Value::as_i64),
        elapsed_ms: value.get("elapsed_ms")?.as_i64()?,
        note: value.get("note")?.as_str()?.to_owned(),
        actor_kind: value.get("actor_kind")?.as_str()?.to_owned(),
        actor_name: value.get("actor_name")?.as_str()?.to_owned(),
        reminded_at: None,
        work_kind: value
            .get("work_kind")
            .and_then(Value::as_str)
            .and_then(|k| k.parse().ok()),
    })
}

/// The project of the timer's task, for history rows.
fn project_of(tx: &Transaction<'_>, task: Id) -> Result<Option<Id>, Failure> {
    Ok(tx
        .query_row(
            "SELECT project_id FROM tasks WHERE id = ?1",
            [b(task)],
            |r| id_at(r, 0),
        )
        .optional()?)
}

/// Replaces the timer (`None`: removes it) and records the change.
fn write_timer(w: &mut WriteCtx<'_>, row: Option<&TimerRow>) -> Result<(), Failure> {
    let before = raw_timer(w.tx())?;
    let old_task = timer_row(w.tx())?.map(|r| r.task);
    w.tx().execute("DELETE FROM timer WHERE id = 1", [])?;
    if let Some(r) = row {
        w.tx().execute(
            "INSERT INTO timer (id, task_id, started_at, running_since, elapsed_ms, note,
                                actor_kind, actor_name, reminded_at, work_kind_id)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                b(r.task),
                r.started_at,
                r.running_since,
                r.elapsed_ms,
                r.note,
                r.actor_kind,
                r.actor_name,
                r.reminded_at,
                r.work_kind.map(b)
            ],
        )?;
    }
    let after = raw_timer(w.tx())?;
    let project = match row.map(|r| r.task).or(old_task) {
        Some(task) => project_of(w.tx(), task)?,
        None => None,
    };
    let id = w.workspace_id();
    w.record_field(TIMER, id, project, "timer", before, after);
    Ok(())
}

/// Writes a recorded timer back, for revert and Undo.
pub(crate) fn apply_timer(w: &mut WriteCtx<'_>, value: Option<&Value>) -> Result<(), Failure> {
    let row = match value {
        Some(v) => Some(
            row_from_value(v)
                .ok_or_else(|| Failure::Op(messages::storage("timer-unreadable", &[])))?,
        ),
        None => None,
    };
    write_timer(w, row.as_ref())
}

/// Logs the time of timer `row` on its task, or the `duration` given
/// instead. Returns the entry, if any time was logged, and warnings.
fn log_timer(
    w: &mut WriteCtx<'_>,
    row: &TimerRow,
    note: Option<&str>,
    duration: Option<&str>,
    kind: Option<Option<Id>>,
) -> Result<(Option<Id>, Vec<String>), Failure> {
    let units = units(w.tx())?;
    let Some(task) = lookup::task_by_id(w.tx(), row.task)? else {
        return Err(messages::hint(
            messages::conflict("timer-task-trashed", &[]),
            "timer-task-trashed-hint",
            &[],
        )
        .into());
    };
    ensure_active(w.tx(), task.project)?;
    let minutes = match duration {
        Some(d) => worklog::entry_minutes(d, units).map_err(|e| bad("duration", &e))?,
        None => {
            let elapsed = row.elapsed_at(w.now());
            let minutes = worklog::timer_minutes(elapsed);
            if minutes == 0 {
                return Ok((None, vec![messages::text("timer-too-short", &[])]));
            }
            worklog::check_entry_minutes(minutes).map_err(|_| {
                Failure::Op(messages::hint(
                    messages::with(
                        "timer-too-long",
                        &[("duration", format_duration(minutes, units))],
                        OpError::invalid,
                    ),
                    "timer-too-long-hint",
                    &[],
                ))
            })?
        }
    };
    let note = match note {
        Some(n) => worklog::entry_note(n).map_err(|e| bad("note", &e))?,
        None => row.note.clone(),
    };
    let date = w.zone().day_of(row.started_at);
    let kind = kind.unwrap_or(row.work_kind);
    let id = insert(w, &task, date, minutes, &note, kind)?;
    Ok((Some(id), Vec::new()))
}

/// Stops the timer before its task goes to the trash, when `going` says
/// the timer's task is among those going: its time is logged on the task
/// (and goes to the trash with it) and the timer is removed, in the
/// deleting command, so undoing the deletion brings the timer back. A timer
/// whose time can't be logged (it ran for more than a day) is paused
/// instead, for the person to stop it.
pub(crate) fn stop_before_trash(
    w: &mut WriteCtx<'_>,
    going: impl Fn(&Transaction<'_>, Id) -> Result<bool, Failure>,
) -> Result<(), Failure> {
    let Some(row) = timer_row(w.tx())? else {
        return Ok(());
    };
    if !going(w.tx(), row.task)? {
        return Ok(());
    }
    match log_timer(w, &row, None, None, None) {
        Ok(_) => write_timer(w, None),
        Err(Failure::Op(_)) if row.running_since.is_some() => {
            let paused = TimerRow {
                elapsed_ms: row.elapsed_at(w.now()),
                running_since: None,
                ..row
            };
            write_timer(w, Some(&paused))
        }
        Err(Failure::Op(_)) => Ok(()),
        Err(e) => Err(e),
    }
}

fn timer_change(
    w: &WriteCtx<'_>,
    entry: Option<Id>,
    warnings: Vec<String>,
) -> Result<TimerChange, Failure> {
    let units = units(w.tx())?;
    Ok(TimerChange {
        timer: current(w.tx(), w.now(), units)?,
        entry: entry.map(|id| load(w.tx(), id, units)).transpose()?,
        group: w.group_if_changed(),
        warnings,
    })
}

fn no_timer() -> Failure {
    messages::hint(
        messages::not_found("timer-none", &[]),
        "timer-none-hint",
        &[],
    )
    .into()
}

fn start(w: &mut WriteCtx<'_>, input: TimerStartInput) -> Result<TimerChange, Failure> {
    let note = input
        .note
        .as_deref()
        .map(worklog::entry_note)
        .transpose()
        .map_err(|e| bad("note", &e))?;
    let now = w.now();
    let current = timer_row(w.tx())?;
    let kind = input
        .work_kind
        .as_deref()
        .map(|k| {
            super::work_kind::for_entry(
                w.tx(),
                "work_kind",
                k,
                current.as_ref().and_then(|r| r.work_kind),
            )
        })
        .transpose()?;
    let task = match &input.key {
        Some(key) => {
            let t = lookup::task(w.tx(), "key", key)?;
            ensure_active(w.tx(), t.project)?;
            t.id
        }
        None => current.as_ref().map(|r| r.task).ok_or_else(no_timer)?,
    };
    let mut entry = None;
    let mut warnings = Vec::new();
    let next = match current {
        Some(mut row) if row.task == task => {
            if row.running_since.is_none() {
                row.running_since = Some(now);
            }
            if let Some(n) = note {
                row.note = n;
            }
            if let Some(k) = kind {
                row.work_kind = k;
            }
            row
        }
        other => {
            if let Some(old) = other {
                (entry, warnings) = log_timer(w, &old, None, None, None)?;
            }
            let caller = w.caller().clone();
            TimerRow {
                task,
                started_at: now,
                running_since: Some(now),
                elapsed_ms: 0,
                note: note.unwrap_or_default(),
                work_kind: kind.flatten(),
                actor_kind: caller.actor.kind.as_str().to_owned(),
                actor_name: caller.actor.name,
                reminded_at: None,
            }
        }
    };
    if timer_row(w.tx())?.as_ref() != Some(&next) {
        write_timer(w, Some(&next))?;
    }
    timer_change(w, entry, warnings)
}

fn pause(w: &mut WriteCtx<'_>) -> Result<TimerChange, Failure> {
    let mut row = timer_row(w.tx())?.ok_or_else(no_timer)?;
    if let Some(since) = row.running_since.take() {
        row.elapsed_ms += (w.now() - since).max(0);
        write_timer(w, Some(&row))?;
    }
    timer_change(w, None, Vec::new())
}

fn stop(w: &mut WriteCtx<'_>, input: TimerStopInput) -> Result<TimerChange, Failure> {
    let row = timer_row(w.tx())?.ok_or_else(no_timer)?;
    let (entry, warnings) = if input.discard {
        (None, Vec::new())
    } else {
        let kind = input
            .work_kind
            .as_deref()
            .map(|k| super::work_kind::for_entry(w.tx(), "work_kind", k, row.work_kind))
            .transpose()?;
        log_timer(
            w,
            &row,
            input.note.as_deref(),
            input.duration.as_deref(),
            kind,
        )?
    };
    write_timer(w, None)?;
    timer_change(w, entry, warnings)
}
