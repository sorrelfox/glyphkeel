// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Reminders: what is due, when the next one comes, and acknowledging.
//!
//! Three kinds come due, all computed here so every program agrees:
//!
//! - a task's personal reminder: a moment, or a number of days before the
//!   due date at a time of day (the workspace's reminder time unless set).
//!   One per task. A reminder that follows the due date fires again when
//!   the due date moves, because the acknowledged moment is kept;
//! - a deadline alert for every open task with a due date: the day before
//!   the due date, at the reminder time, but not before the task was
//!   created. `reminder_log` keeps the due dates already acknowledged, so a
//!   new due date alerts again. Due dates more than a week past are no
//!   longer alerted;
//! - the timer, once it has run for the hours the workspace sets;
//! - a notification an automation rule left.
//!
//! Tasks in the trash or in archived projects don't remind; closed tasks
//! keep only reminders set for a moment. Nothing here fires anything: the
//! app asks for what is due, shows it, acknowledges it, and sleeps until
//! `next_at`. Reminders that came due before the caller's `since` are
//! `missed` (due while no program was showing them).
//!
//! History records a task's reminder as the task's field `reminder`, so
//! Undo and revert cover it; acknowledging is not history.

use gk_api::OpError;
use gk_api::ops::time::{
    DueReminder, DueReminders, Reminder, ReminderAckInput, ReminderAcknowledge,
    ReminderAcknowledged, ReminderChange, ReminderClear, ReminderClearInput, ReminderDue,
    ReminderDueInput, ReminderKind, ReminderSet, ReminderSetInput,
};
use gk_core::worklog::{self, REMINDER_MAX_DAYS_BEFORE};
use gk_core::{Day, Id};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Row, Transaction, params};
use serde_json::{Map, Value};

use super::comment::actor;
use super::desc;
use super::fields;
use super::lookup::{self, b, id_at};
use super::task::{invalid, task_ref};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::time::{Zone, clock_minutes, clock_text, rfc3339};
use crate::write::WriteCtx;

/// Reminders due this long ago count as missed when the caller gives no
/// `since`.
const MISSED_AFTER_MS: i64 = 5 * 60_000;
/// Due dates this many days past are no longer alerted.
const DEADLINE_GRACE_DAYS: i64 = 7;
/// How the ID of a rule's notification starts.
const AUTOMATION_PREFIX: &str = "automation:";
/// The default and largest number of due reminders listed.
const DEFAULT_LIMIT: u32 = 100;
const MAX_LIMIT: u32 = 500;

pub(super) fn register(r: &mut Registry) {
    r.command::<ReminderSet, _>(desc::<ReminderSet>(), set);
    r.command::<ReminderClear, _>(desc::<ReminderClear>(), |w, input: ReminderClearInput| {
        let task = lookup::task(w.tx(), "key", &input.key)?;
        ensure_active(w.tx(), task.project)?;
        write_value(w, task.id, Some(task.project), None)?;
        changed(w, task.id)
    });
    r.query::<ReminderDue, _>(desc::<ReminderDue>(), |q, input| {
        due(q.tx, &q.env.zone, q.now, &input)
    });
    r.command::<ReminderAcknowledge, _>(desc::<ReminderAcknowledge>(), acknowledge);
}

fn ensure_active(tx: &Transaction<'_>, project: Id) -> Result<(), Failure> {
    if let Some(p) = lookup::project_by_id(tx, project)? {
        lookup::ensure_active(&p)?;
    }
    Ok(())
}

/// A reminder row.
struct Row4 {
    id: Id,
    task: Id,
    remind_at: Option<i64>,
    days_before_due: Option<u32>,
    time_of_day: Option<u32>,
    note: String,
    actor_kind: String,
    actor_name: String,
    created_at: i64,
    acked_moment: Option<i64>,
}

const COLUMNS: &str = "r.id, r.task_id, r.remind_at, r.days_before_due, r.time_of_day, r.note,
    r.actor_kind, r.actor_name, r.created_at, r.acked_moment";

fn row(r: &Row<'_>) -> gk_store::rusqlite::Result<Row4> {
    Ok(Row4 {
        id: id_at(r, 0)?,
        task: id_at(r, 1)?,
        remind_at: r.get(2)?,
        days_before_due: r.get(3)?,
        time_of_day: r.get(4)?,
        note: r.get(5)?,
        actor_kind: r.get(6)?,
        actor_name: r.get(7)?,
        created_at: r.get(8)?,
        acked_moment: r.get(9)?,
    })
}

fn of(tx: &Transaction<'_>, task: Id) -> Result<Option<Row4>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM reminders r WHERE r.task_id = ?1"),
            [b(task)],
            row,
        )
        .optional()?)
}

/// The workspace's reminder time, in minutes after midnight.
fn reminder_time(tx: &Transaction<'_>) -> Result<u32, Failure> {
    let s = super::settings::load(tx)?;
    Ok(clock_minutes(&s.reminder_time).unwrap_or(9 * 60))
}

impl Row4 {
    /// When it fires, given the task's due date; `None` while it follows a
    /// due date the task doesn't have.
    fn moment(&self, due: Option<Day>, default_time: u32, zone: &Zone) -> Option<i64> {
        if let Some(at) = self.remind_at {
            return Some(at);
        }
        let day = worklog::reminder_day(due?, self.days_before_due?).ok()?;
        Some(zone.moment(day, self.time_of_day.unwrap_or(default_time)))
    }
}

fn due_date(tx: &Transaction<'_>, task: Id) -> Result<Option<Day>, StoreError> {
    let text: Option<String> =
        tx.query_row("SELECT due_date FROM tasks WHERE id = ?1", [b(task)], |r| {
            r.get(0)
        })?;
    Ok(text.and_then(|t| t.parse().ok()))
}

/// Task `task`'s reminder as outputs carry it.
pub(crate) fn of_task(
    tx: &Transaction<'_>,
    task: Id,
    zone: &Zone,
) -> Result<Option<Reminder>, Failure> {
    let Some(r) = of(tx, task)? else {
        return Ok(None);
    };
    let at = r.moment(due_date(tx, task)?, reminder_time(tx)?, zone);
    let key: String = tx.query_row(
        "SELECT p.key || '-' || t.number FROM tasks t JOIN projects p ON p.id = t.project_id
          WHERE t.id = ?1",
        [b(task)],
        |row| row.get(0),
    )?;
    Ok(Some(Reminder {
        id: r.id.to_string(),
        task: key,
        at: at.map(rfc3339),
        days_before_due: r.days_before_due,
        time: r.time_of_day.map(clock_text),
        note: r.note.clone(),
        acknowledged: at.is_some() && r.acked_moment == at,
        set_by: actor(&r.actor_kind, r.actor_name.clone()),
        created_at: rfc3339(r.created_at),
    }))
}

/// Task `task`'s reminder as history records it: `at` (RFC 3339) or
/// `days_before_due` with `time`, and `note`.
pub(crate) fn raw(tx: &Transaction<'_>, task: Id) -> Result<Option<Value>, StoreError> {
    Ok(of(tx, task)?.map(|r| {
        let mut m = Map::new();
        if let Some(at) = r.remind_at {
            m.insert("at".into(), Value::String(rfc3339(at)));
        }
        if let Some(days) = r.days_before_due {
            m.insert("days_before_due".into(), days.into());
        }
        if let Some(t) = r.time_of_day {
            m.insert("time".into(), Value::String(clock_text(t)));
        }
        if !r.note.is_empty() {
            m.insert("note".into(), Value::String(r.note));
        }
        Value::Object(m)
    }))
}

/// Replaces task `task`'s reminder with `value` (as [`raw`] gives it;
/// `None` removes it) and records the change on the task.
pub(crate) fn write_value(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: Option<Id>,
    value: Option<Value>,
) -> Result<(), Failure> {
    let before = raw(w.tx(), task)?;
    if before == value {
        return Ok(());
    }
    w.tx()
        .execute("DELETE FROM reminders WHERE task_id = ?1", [b(task)])?;
    if let Some(v) = &value {
        let corrupt = || Failure::Op(messages::storage("reminder-unreadable", &[]));
        let at = match v.get("at").and_then(Value::as_str) {
            Some(text) => Some(w.zone().parse_moment(text).ok_or_else(corrupt)?),
            None => None,
        };
        let days = v
            .get("days_before_due")
            .and_then(Value::as_u64)
            .and_then(|d| u32::try_from(d).ok());
        let time = match v.get("time").and_then(Value::as_str) {
            Some(text) => Some(clock_minutes(text).ok_or_else(corrupt)?),
            None => None,
        };
        if at.is_some() == days.is_some() {
            return Err(corrupt());
        }
        let note = v.get("note").and_then(Value::as_str).unwrap_or_default();
        let caller = w.caller().clone();
        w.tx().execute(
            "INSERT INTO reminders (id, task_id, remind_at, days_before_due, time_of_day, note,
                                    actor_kind, actor_name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                b(Id::generate_at(u64::try_from(w.now()).unwrap_or(0))),
                b(task),
                at,
                days,
                time,
                note,
                caller.actor.kind.as_str(),
                caller.actor.name,
                w.now()
            ],
        )?;
    }
    w.record_field(fields::TASK, task, project, "reminder", before, value);
    Ok(())
}

fn changed(w: &WriteCtx<'_>, task: Id) -> Result<ReminderChange, Failure> {
    let t = task_ref(w.tx(), task)?;
    Ok(ReminderChange {
        task: t.key,
        reminder: of_task(w.tx(), task, w.zone())?,
        group: w.group_if_changed(),
    })
}

fn set(w: &mut WriteCtx<'_>, input: ReminderSetInput) -> Result<ReminderChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    ensure_active(w.tx(), task.project)?;
    let mut value = Map::new();
    match (&input.at, input.days_before_due) {
        (Some(at), None) => {
            if input.time.is_some() {
                return Err(messages::invalid("time", "reminder-time-with-at", &[]));
            }
            let ms = w.zone().parse_moment(at).ok_or_else(|| {
                Failure::Op(messages::hint(
                    messages::invalid_at("at", "reminder-not-a-moment", &[("text", at.clone())]),
                    "reminder-moment-hint",
                    &[],
                ))
            })?;
            value.insert("at".into(), Value::String(rfc3339(ms)));
        }
        (None, Some(days)) => {
            if days > REMINDER_MAX_DAYS_BEFORE {
                return Err(invalid("days_before_due", worklog::WorklogError::TooEarly));
            }
            value.insert("days_before_due".into(), days.into());
            if let Some(t) = &input.time {
                let minutes = clock_minutes(t).ok_or_else(|| {
                    messages::invalid("time", "reminder-not-a-time", &[("time", t.clone())])
                })?;
                value.insert("time".into(), Value::String(clock_text(minutes)));
            }
        }
        _ => {
            return Err(messages::hint(
                messages::with("reminder-at-or-days", &[], OpError::invalid),
                "reminder-at-or-days-hint",
                &[],
            )
            .into());
        }
    }
    let note = worklog::reminder_note(input.note.as_deref().unwrap_or_default())
        .map_err(|e| invalid("note", e))?;
    if !note.is_empty() {
        value.insert("note".into(), Value::String(note));
    }
    write_value(w, task.id, Some(task.project), Some(Value::Object(value)))?;
    changed(w, task.id)
}

// ---------------------------------------------------------------------
// What is due

fn moment_input(zone: &Zone, field: &str, text: &str) -> Result<i64, Failure> {
    zone.parse_moment(text).ok_or_else(|| {
        Failure::Op(messages::hint(
            messages::invalid_at(field, "reminder-not-a-moment", &[("text", text.to_owned())]),
            "reminder-moment-hint",
            &[],
        ))
    })
}

struct Candidate {
    at: i64,
    id: String,
    kind: ReminderKind,
    task: Id,
    due: Option<String>,
    note: String,
}

/// Every reminder not acknowledged, with its moment.
fn candidates(tx: &Transaction<'_>, zone: &Zone, until: i64) -> Result<Vec<Candidate>, Failure> {
    let settings = super::settings::load(tx)?;
    let default_time = clock_minutes(&settings.reminder_time).unwrap_or(9 * 60);
    let mut out = Vec::new();

    // Personal reminders.
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {COLUMNS}, t.due_date, t.closed_at IS NOT NULL FROM reminders r
           JOIN tasks t ON t.id = r.task_id JOIN projects p ON p.id = t.project_id
          WHERE t.deleted_at IS NULL AND p.deleted_at IS NULL AND p.state = 'active'"
    ))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                row(r)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, bool>(11)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (r, due, closed) in rows {
        if closed && r.remind_at.is_none() {
            continue;
        }
        let Some(at) = r.moment(
            due.as_deref().and_then(|d| d.parse().ok()),
            default_time,
            zone,
        ) else {
            continue;
        };
        if r.acked_moment == Some(at) {
            continue;
        }
        out.push(Candidate {
            at,
            id: format!("reminder:{}", r.id),
            kind: ReminderKind::Reminder,
            task: r.task,
            due,
            note: r.note,
        });
    }

    // Deadline alerts.
    let cutoff = zone
        .day_of(until)
        .add_days(-DEADLINE_GRACE_DAYS)
        .map_or_else(|_| "0000-00-00".to_owned(), |d| d.to_string());
    let mut stmt = tx.prepare_cached(
        "SELECT t.id, t.due_date, t.created_at FROM tasks t
           JOIN projects p ON p.id = t.project_id
          WHERE t.deleted_at IS NULL AND t.closed_at IS NULL AND t.due_date >= ?1
            AND p.deleted_at IS NULL AND p.state = 'active'
            AND NOT EXISTS (SELECT 1 FROM reminder_log l
                             WHERE l.task_id = t.id AND l.due_date = t.due_date)",
    )?;
    let rows = stmt
        .query_map([cutoff], |r| {
            Ok((id_at(r, 0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (task, due, created) in rows {
        let Ok(day) = due.parse::<Day>() else {
            continue;
        };
        let at = zone
            .moment(worklog::deadline_alert_day(day), default_time)
            .max(created);
        out.push(Candidate {
            at,
            id: format!("deadline:{task}:{due}"),
            kind: ReminderKind::Deadline,
            task,
            due: Some(due),
            note: String::new(),
        });
    }

    // Notifications automation rules left.
    for (id, task, at, text) in super::automation::open_notices(tx)? {
        let due: Option<String> =
            tx.query_row("SELECT due_date FROM tasks WHERE id = ?1", [b(task)], |r| {
                r.get(0)
            })?;
        out.push(Candidate {
            at,
            id: format!("{AUTOMATION_PREFIX}{id}"),
            kind: ReminderKind::Automation,
            task,
            due,
            note: text,
        });
    }

    // The timer.
    if settings.timer_reminder_hours > 0
        && let Some(timer) = super::time::timer_row(tx)?
        && let (Some(since), None) = (timer.running_since, timer.reminded_at)
    {
        let limit = i64::from(settings.timer_reminder_hours) * 3_600_000;
        let at = since + (limit - timer.elapsed_ms).max(0);
        let due: Option<String> = tx.query_row(
            "SELECT due_date FROM tasks WHERE id = ?1",
            [b(timer.task)],
            |r| r.get(0),
        )?;
        out.push(Candidate {
            at,
            id: format!("timer:{}", timer.started_at),
            kind: ReminderKind::Timer,
            task: timer.task,
            due,
            note: String::new(),
        });
    }
    Ok(out)
}

fn due(
    tx: &Transaction<'_>,
    zone: &Zone,
    now: i64,
    input: &ReminderDueInput,
) -> Result<DueReminders, Failure> {
    let until = match &input.until {
        Some(t) => moment_input(zone, "until", t)?,
        None => now,
    };
    let since = match &input.since {
        Some(t) => moment_input(zone, "since", t)?,
        None => now - MISSED_AFTER_MS,
    };
    let limit = input.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(messages::invalid(
            "limit",
            "input-limit",
            &[("max", MAX_LIMIT.to_string())],
        ));
    }
    let mut all = candidates(tx, zone, until)?;
    if crate::scope::active() {
        let mut kept = Vec::with_capacity(all.len());
        for c in all {
            if super::lookup::task_by_id(tx, c.task)?.is_some() {
                kept.push(c);
            }
        }
        all = kept;
    }
    let next_at = all.iter().map(|c| c.at).filter(|at| *at > until).min();
    let mut due: Vec<Candidate> = all.into_iter().filter(|c| c.at <= until).collect();
    due.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
    let total = due.len() as u64;
    let items = due
        .into_iter()
        .take(limit as usize)
        .map(|c| {
            Ok(DueReminder {
                id: c.id,
                kind: c.kind,
                at: rfc3339(c.at),
                missed: c.at < since,
                task: task_ref(tx, c.task)?,
                due_date: c.due,
                note: c.note,
            })
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    Ok(DueReminders {
        items,
        total,
        next_at: next_at.map(rfc3339),
        until: rfc3339(until),
    })
}

/// The personal reminders not acknowledged that come due up to `until`,
/// the earliest first; those due before `since` are marked missed.
pub(crate) fn personal_due(
    tx: &Transaction<'_>,
    zone: &Zone,
    since: i64,
    until: i64,
) -> Result<Vec<DueReminder>, Failure> {
    let mut due: Vec<Candidate> = candidates(tx, zone, until)?
        .into_iter()
        .filter(|c| c.kind == ReminderKind::Reminder && c.at <= until)
        .collect();
    due.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
    due.into_iter()
        .map(|c| {
            Ok(DueReminder {
                id: c.id,
                kind: c.kind,
                at: rfc3339(c.at),
                missed: c.at < since,
                task: task_ref(tx, c.task)?,
                due_date: c.due,
                note: c.note,
            })
        })
        .collect()
}

/// The personal reminders and the notifications of automation rules that
/// came due from `since` up to `until` and weren't acknowledged, the
/// earliest first, each with its task's project. Deadline alerts and the
/// timer are left out: the digest lists due dates on their own.
pub(crate) fn came_due(
    tx: &Transaction<'_>,
    zone: &Zone,
    since: i64,
    until: i64,
) -> Result<Vec<(Id, DueReminder)>, Failure> {
    let mut due: Vec<Candidate> = candidates(tx, zone, until)?
        .into_iter()
        .filter(|c| {
            matches!(c.kind, ReminderKind::Reminder | ReminderKind::Automation)
                && c.at >= since
                && c.at <= until
        })
        .collect();
    due.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
    let mut out = Vec::with_capacity(due.len());
    for c in due {
        let Some(task) = super::lookup::task_by_id(tx, c.task)? else {
            continue;
        };
        out.push((
            task.project,
            DueReminder {
                id: c.id,
                kind: c.kind,
                at: rfc3339(c.at),
                missed: false,
                task: task_ref(tx, c.task)?,
                due_date: c.due,
                note: c.note,
            },
        ));
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// Acknowledging

fn acknowledge(
    w: &mut WriteCtx<'_>,
    input: ReminderAckInput,
) -> Result<ReminderAcknowledged, Failure> {
    if input.ids.is_empty() || input.ids.len() > MAX_LIMIT as usize {
        return Err(messages::invalid(
            "ids",
            "reminder-ack-count",
            &[("max", MAX_LIMIT.to_string())],
        ));
    }
    let pending = candidates(w.tx(), w.zone(), w.now())?;
    let mut acknowledged = Vec::new();
    let mut unknown = Vec::new();
    for id in input.ids {
        let Some(c) = pending.iter().find(|c| c.id == id.trim()) else {
            unknown.push(id);
            continue;
        };
        match c.kind {
            ReminderKind::Reminder => {
                w.tx().execute(
                    "UPDATE reminders SET acked_at = ?2, acked_moment = ?3 WHERE task_id = ?1",
                    params![b(c.task), w.now(), c.at],
                )?;
            }
            ReminderKind::Deadline => {
                w.tx().execute(
                    "INSERT OR IGNORE INTO reminder_log (task_id, due_date, fired_at)
                     VALUES (?1, ?2, ?3)",
                    params![b(c.task), c.due, w.now()],
                )?;
            }
            ReminderKind::Timer => {
                w.tx()
                    .execute("UPDATE timer SET reminded_at = ?1 WHERE id = 1", [w.now()])?;
            }
            ReminderKind::Automation => {
                if let Some(notice) =
                    c.id.strip_prefix(AUTOMATION_PREFIX)
                        .and_then(|n| n.parse::<Id>().ok())
                {
                    super::automation::acknowledge_notice(w.tx(), notice, w.now())?;
                }
            }
        }
        acknowledged.push(c.id.clone());
    }
    Ok(ReminderAcknowledged {
        acknowledged,
        unknown,
    })
}
