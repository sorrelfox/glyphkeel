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
//! History records an entry's `created` (its life), `trashed`, `task`,
//! `date`, `minutes` and `note`, and the timer as one value (`timer`) on an
//! item whose ID is the workspace's, so Undo and revert cover both.

use std::collections::{HashMap, HashSet};

use gk_api::OpError;
use gk_api::ops::settings::WeekStart;
use gk_api::ops::time::{
    TimeDelete, TimeEntry, TimeEntryChange, TimeEntryRef, TimeGroupBy, TimeLog, TimeLogInput,
    TimeReport, TimeReportInput, TimeReportOp, TimeReportRow, TimeUpdate, TimeUpdateInput, Timer,
    TimerChange, TimerGet, TimerPause, TimerStart, TimerStartInput, TimerState, TimerStatus,
    TimerStop, TimerStopInput,
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
use crate::registry::Registry;
use crate::time::{Zone, rfc3339};
use crate::write::WriteCtx;

/// History's name for time entries.
pub(crate) const ENTRY: &str = "time_entry";
/// History's name for the timer; its ID is the workspace's.
pub(crate) const TIMER: &str = "timer";

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
    e.actor_kind, e.actor_name, e.created_at, e.edited_at, e.version";

const FROM: &str = "time_entries e JOIN tasks t ON t.id = e.task_id
    JOIN projects p ON p.id = t.project_id";

fn row(r: &Row<'_>, units: DurationUnits) -> gk_store::rusqlite::Result<TimeEntry> {
    let minutes: i64 = r.get(3)?;
    Ok(TimeEntry {
        id: id_at(r, 0)?.to_string(),
        task: r.get(1)?,
        date: r.get(2)?,
        minutes,
        duration: format_duration(minutes, units),
        note: r.get(4)?,
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
        Failure::Op(
            OpError::not_found(format!("There is no time entry \"{text}\"."), &[])
                .with_detail("field", "entry")
                .with_hint("Entry IDs are listed by get_task with include [\"time\"]."),
        )
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
) -> Result<Id, Failure> {
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let caller = w.caller().clone();
    w.tx().execute(
        "INSERT INTO time_entries (id, task_id, work_date, minutes, note, actor_kind, actor_name,
                                   session_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            b(id),
            b(task.id),
            date.to_string(),
            minutes,
            note,
            caller.actor.kind.as_str(),
            caller.actor.name,
            caller.session.map(b),
            w.now()
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
    Ok(id)
}

fn work_day(w: &WriteCtx<'_>, field: &str, text: Option<&str>) -> Result<Day, Failure> {
    let today = w.zone().day_of(w.now());
    match text {
        Some(t) => worklog::parse_work_day(t, today).map_err(|e| bad(field, &e)),
        None => Ok(today),
    }
}

fn log(w: &mut WriteCtx<'_>, input: TimeLogInput) -> Result<TimeEntryChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    ensure_active(w.tx(), task.project)?;
    let units = units(w.tx())?;
    let minutes =
        worklog::entry_minutes(&input.duration, units).map_err(|e| bad("duration", &e))?;
    let date = work_day(w, "date", input.date.as_deref())?;
    let note = worklog::entry_note(input.note.as_deref().unwrap_or_default())
        .map_err(|e| bad("note", &e))?;
    let id = insert(w, &task, date, minutes, &note)?;
    changed(w, id)
}

fn update(w: &mut WriteCtx<'_>, input: TimeUpdateInput) -> Result<TimeEntryChange, Failure> {
    let e = find(w.tx(), &input.entry)?;
    ensure_active(w.tx(), e.project)?;
    if let Some(expected) = input.expected_version
        && expected != e.version
    {
        return Err(OpError::conflict(format!(
            "The time entry changed since you read it: it has version {}, not {expected}.",
            e.version
        ))
        .with_detail("version", e.version)
        .with_hint("Read the entry again and repeat the change.")
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

/// The entry's `date`, `minutes` or `note`, as history records it.
pub(crate) fn raw_field(
    tx: &Transaction<'_>,
    id: Id,
    field: &str,
) -> Result<Option<Value>, StoreError> {
    let row: Option<(String, i64, String)> = tx
        .query_row(
            "SELECT work_date, minutes, note FROM time_entries WHERE id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    Ok(row.and_then(|(date, minutes, note)| match field {
        "date" => Some(Value::String(date)),
        "minutes" => Some(minutes.into()),
        "note" => (!note.is_empty()).then_some(Value::String(note)),
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
        Failure::Op(OpError::storage(format!(
            "A recorded {field} can't be read."
        )))
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

fn first_weekday(start: WeekStart) -> Weekday {
    match start {
        WeekStart::Monday => Weekday::Monday,
        WeekStart::Saturday => Weekday::Saturday,
        WeekStart::Sunday => Weekday::Sunday,
    }
}

fn report(
    tx: &Transaction<'_>,
    zone: &Zone,
    now: i64,
    input: TimeReportInput,
) -> Result<TimeReport, Failure> {
    let settings = super::settings::load(tx)?;
    let units =
        DurationUnits::new(settings.hours_per_day, settings.days_per_week).unwrap_or_default();
    let today = zone.day_of(now);
    let day =
        |field: &str, text: &str| worklog::parse_work_day(text, today).map_err(|e| bad(field, &e));
    let from = match &input.from {
        Some(t) => day("from", t)?,
        None => worklog::week_start(today, first_weekday(settings.week_start)),
    };
    let to = match &input.to {
        Some(t) => day("to", t)?,
        None => from.add_days(6).map_err(|e| invalid("from", e))?,
    };
    let span = to.days_since_epoch() - from.days_since_epoch();
    if span < 0 {
        return Err(OpError::invalid_at("to", "The last day is before the first.").into());
    }
    if span >= REPORT_MAX_DAYS {
        return Err(OpError::invalid_at(
            "to",
            format!("A report covers at most {REPORT_MAX_DAYS} days."),
        )
        .into());
    }
    let projects: Option<HashSet<Id>> = match &input.project {
        Some(p) => {
            let p = lookup::project(tx, "project", p)?;
            Some(super::project::subtree(tx, p.id)?.into_iter().collect())
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

    struct Line {
        id: Id,
        task: Id,
        project: Id,
        date: String,
        minutes: i64,
    }
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT e.id, e.task_id, t.project_id, e.work_date, e.minutes FROM {FROM}
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
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let lines: Vec<Line> = lines
        .into_iter()
        .filter(|l| projects.as_ref().is_none_or(|p| p.contains(&l.project)))
        .filter(|l| tasks.as_ref().is_none_or(|t| t.contains(&l.task)))
        .collect();

    let group_by = input.group_by.unwrap_or_default();
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, (Id, i64, u32)> = HashMap::new();
    for l in &lines {
        let (key, owner) = match group_by {
            TimeGroupBy::Day => (l.date.clone(), l.task),
            TimeGroupBy::Task => (l.task.to_string(), l.task),
            TimeGroupBy::Project => (l.project.to_string(), l.project),
        };
        let g = groups.entry(key.clone()).or_insert_with(|| {
            order.push(key);
            (owner, 0, 0)
        });
        g.1 += l.minutes;
        g.2 += 1;
    }
    let mut items = Vec::with_capacity(order.len());
    for key in order {
        let (owner, minutes, count) = groups[&key];
        let (key, title) = match group_by {
            TimeGroupBy::Day => {
                let weekday = key
                    .parse::<Day>()
                    .map(|d| format!("{:?}", d.weekday()))
                    .unwrap_or_default();
                (key, weekday)
            }
            TimeGroupBy::Task => {
                let t = task_ref(tx, owner)?;
                (t.key, t.title)
            }
            TimeGroupBy::Project => tx.query_row(
                "SELECT key, name FROM projects WHERE id = ?1",
                [b(owner)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?,
        };
        items.push(TimeReportRow {
            key,
            title,
            minutes,
            duration: format_duration(minutes, units),
            entry_count: count,
        });
    }
    if group_by != TimeGroupBy::Day {
        items.sort_by(|a, b| b.minutes.cmp(&a.minutes).then_with(|| a.key.cmp(&b.key)));
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
    })
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
                    reminded_at
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
        json!({
            "task": r.task.to_string(),
            "started_at": r.started_at,
            "running_since": r.running_since,
            "elapsed_ms": r.elapsed_ms,
            "note": r.note,
            "actor_kind": r.actor_kind,
            "actor_name": r.actor_name,
        })
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
                                actor_kind, actor_name, reminded_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                b(r.task),
                r.started_at,
                r.running_since,
                r.elapsed_ms,
                r.note,
                r.actor_kind,
                r.actor_name,
                r.reminded_at
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
                .ok_or_else(|| Failure::Op(OpError::storage("A recorded timer can't be read.")))?,
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
) -> Result<(Option<Id>, Vec<String>), Failure> {
    let units = units(w.tx())?;
    let Some(task) = lookup::task_by_id(w.tx(), row.task)? else {
        return Err(OpError::conflict("The timer's task is in the trash.")
            .with_hint("Stop the timer with `discard` to throw its time away.")
            .into());
    };
    ensure_active(w.tx(), task.project)?;
    let minutes = match duration {
        Some(d) => worklog::entry_minutes(d, units).map_err(|e| bad("duration", &e))?,
        None => {
            let elapsed = row.elapsed_at(w.now());
            let minutes = worklog::timer_minutes(elapsed);
            if minutes == 0 {
                return Ok((
                    None,
                    vec!["The timer ran for under half a minute, so nothing was logged.".into()],
                ));
            }
            worklog::check_entry_minutes(minutes).map_err(|_| {
                Failure::Op(
                    OpError::invalid(format!(
                        "The timer ran for {}, more than a day.",
                        format_duration(minutes, units)
                    ))
                    .with_hint(
                        "Stop it with `duration` set to the time worked, or with `discard`.",
                    ),
                )
            })?
        }
    };
    let note = match note {
        Some(n) => worklog::entry_note(n).map_err(|e| bad("note", &e))?,
        None => row.note.clone(),
    };
    let date = w.zone().day_of(row.started_at);
    let id = insert(w, &task, date, minutes, &note)?;
    Ok((Some(id), Vec::new()))
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
    OpError::not_found("No timer is running or paused.", &[])
        .with_hint("Start one on a task with timer.start (`glyphkeel timer start KEY`).")
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
            row
        }
        other => {
            if let Some(old) = other {
                (entry, warnings) = log_timer(w, &old, None, None)?;
            }
            let caller = w.caller().clone();
            TimerRow {
                task,
                started_at: now,
                running_since: Some(now),
                elapsed_ms: 0,
                note: note.unwrap_or_default(),
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
        log_timer(w, &row, input.note.as_deref(), input.duration.as_deref())?
    };
    write_timer(w, None)?;
    timer_change(w, entry, warnings)
}
