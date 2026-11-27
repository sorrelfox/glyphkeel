// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Recurring tasks: series, their next occurrences and skipping one.
//!
//! A series is kept whole in history (one `record` field), and each
//! occurrence names its series in the task field `series`, so making a task
//! recurring, changing the rule, skipping and stopping can all be undone.
//! The next occurrence is made in the same command that closes the current
//! one, so undoing the close removes it again. Series made ahead get their
//! occurrences from `recurrence.run`, which the app's jobs call.
//!
//! "Today" and the day a task was closed are days in the person's time
//! zone. Moments are only needed for `next_at`: local midnight of the day
//! an occurrence is due to be made; a midnight that a clock change skips is
//! taken after the change.
//!
//! Series are workspace data: they record who made them, like every other
//! item, and hold no personal settings.

use gk_api::OpError;
use gk_api::ops::recurrence::{
    CreateWhen, Frequency, Recurrence, RecurrenceChange, RecurrenceClear, RecurrencePreview,
    RecurrencePreviewOp, RecurrenceRule, RecurrenceRun, RecurrenceRunInput, RecurrenceRunOp,
    RecurrenceSet, RecurrenceSetInput, RecurrenceSkip, RecurrenceTaskInput, RepeatFrom,
    RulePreviewInput, Weekday,
};
use gk_api::ops::template::TaskBlueprint;
use gk_core::recurrence::{self as rec, NthWeekday, Rule, RuleError};
use gk_core::{Day, Id};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::desc;
use super::fields::{self, TASK, id_value};
use super::lookup::{self, b, id_at, opt_id_at};
use super::task::invalid;
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::time::{Zone, rfc3339};
use crate::write::WriteCtx;

/// History's name for series.
pub(crate) const ENTITY: &str = "series";
/// The one recorded field: the whole series.
pub(crate) const RECORD: &str = "record";
/// Most days a series made ahead makes its occurrence before its date.
const DAYS_AHEAD_MAX: u32 = 365;
/// The default and largest number of dates a preview lists.
const PREVIEW_DEFAULT: u32 = 5;
const PREVIEW_MAX: u32 = 50;
/// Most dates passed over at once when catching up with the calendar.
const CATCH_UP_MAX: usize = 10_000;

/// Whether the task `t` is the open occurrence of a live series, for list
/// queries.
pub(crate) const RECURRING_SQL: &str = "EXISTS (SELECT 1 FROM task_series rs
     WHERE rs.id = t.series_id AND rs.current_task = t.id AND rs.ended_at IS NULL)";

pub(super) fn register(r: &mut Registry) {
    r.command::<RecurrenceSet, _>(desc::<RecurrenceSet>(), set);
    r.command::<RecurrenceClear, _>(desc::<RecurrenceClear>(), clear);
    r.command::<RecurrenceSkip, _>(desc::<RecurrenceSkip>(), skip);
    r.query::<RecurrencePreviewOp, _>(desc::<RecurrencePreviewOp>(), |q, input| {
        preview(q.tx, &q.env.zone, q.now, &input)
    });
    r.command::<RecurrenceRunOp, _>(desc::<RecurrenceRunOp>(), |w, _: RecurrenceRunInput| run(w));
}

// ---------------------------------------------------------------------
// Storage

/// A series as history records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) project: String,
    pub(crate) rule: RecurrenceRule,
    pub(crate) pattern: TaskBlueprint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) current: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) next_date: Option<String>,
    pub(crate) made: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ended_at: Option<i64>,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) updated_at: i64,
}

impl Record {
    fn current_id(&self) -> Option<Id> {
        self.current.as_deref().and_then(|c| c.parse().ok())
    }
    fn project_id(&self) -> Option<Id> {
        self.project.parse().ok()
    }
}

const COLUMNS: &str = "id, project_id, rule, pattern, current_task, next_date, made, ended_at, \
                       created_at, created_by_kind, created_by_name, updated_at";

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<(Id, Option<Record>)> {
    let rule: String = r.get(2)?;
    let pattern: String = r.get(3)?;
    let record = (|| {
        Some(Record {
            project: id_at(r, 1).ok()?.to_string(),
            rule: serde_json::from_str(&rule).ok()?,
            pattern: serde_json::from_str(&pattern).ok()?,
            current: opt_id_at(r, 4).ok()?.map(|c| c.to_string()),
            next_date: r.get(5).ok()?,
            made: u32::try_from(r.get::<_, i64>(6).ok()?).unwrap_or(0),
            ended_at: r.get(7).ok()?,
            created_at: r.get(8).ok()?,
            created_by_kind: r.get(9).ok()?,
            created_by_name: r.get(10).ok()?,
            updated_at: r.get(11).ok()?,
        })
    })();
    Ok((id_at(r, 0)?, record))
}

pub(crate) fn load(tx: &Transaction<'_>, id: Id) -> Result<Option<Record>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM task_series WHERE id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?
        .and_then(|(_, r)| r))
}

/// Every series of project `project`, for exports.
pub(crate) fn of_project(
    tx: &Transaction<'_>,
    project: Id,
) -> Result<Vec<(Id, Record)>, StoreError> {
    let mut stmt = tx.prepare(&format!(
        "SELECT {COLUMNS} FROM task_series WHERE project_id = ?1 ORDER BY created_at, id"
    ))?;
    let rows = stmt
        .query_map([b(project)], read_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, r)| r.map(|r| (id, r)))
        .collect())
}

/// The stored series `id` as history records it, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    load(tx, id)?
        .map(|r| serde_json::to_value(r).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

/// Inserts a series row as it is; for history and imports.
pub(crate) fn insert_row(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), StoreError> {
    let project: Id = r
        .project
        .parse()
        .map_err(|_| StoreError::Corrupt("series without a project".to_owned()))?;
    tx.execute(
        &format!(
            "INSERT INTO task_series ({COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
        ),
        params![
            b(id),
            b(project),
            json(&r.rule),
            json(&r.pattern),
            r.current_id().map(b),
            r.next_date,
            r.made,
            r.ended_at,
            r.created_at,
            r.created_by_kind,
            r.created_by_name,
            r.updated_at,
        ],
    )?;
    Ok(())
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "{}".to_owned())
}

/// Writes series `id` as history recorded it (`None` deletes it), without
/// recording.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    w.tx()
        .execute("DELETE FROM task_series WHERE id = ?1", [b(id)])?;
    let Some(value) = value else {
        return Ok(());
    };
    let r: Record = serde_json::from_value(value.clone())
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    insert_row(w.tx(), id, &r)?;
    Ok(())
}

/// Writes series `id` and records the change.
fn save(w: &mut WriteCtx<'_>, id: Id, record: Option<&Record>) -> Result<(), Failure> {
    let before = raw(w.tx(), id)?;
    let after = record
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?;
    if before == after {
        return Ok(());
    }
    let project = record.and_then(Record::project_id);
    write_raw(w, id, after.as_ref())?;
    w.record_field(ENTITY, id, project, RECORD, before, after);
    Ok(())
}

// ---------------------------------------------------------------------
// Rules

pub(crate) fn to_core_weekday(w: Weekday) -> gk_core::Weekday {
    match w {
        Weekday::Mon => gk_core::Weekday::Monday,
        Weekday::Tue => gk_core::Weekday::Tuesday,
        Weekday::Wed => gk_core::Weekday::Wednesday,
        Weekday::Thu => gk_core::Weekday::Thursday,
        Weekday::Fri => gk_core::Weekday::Friday,
        Weekday::Sat => gk_core::Weekday::Saturday,
        Weekday::Sun => gk_core::Weekday::Sunday,
    }
}

fn from_core_weekday(w: gk_core::Weekday) -> Weekday {
    match w {
        gk_core::Weekday::Monday => Weekday::Mon,
        gk_core::Weekday::Tuesday => Weekday::Tue,
        gk_core::Weekday::Wednesday => Weekday::Wed,
        gk_core::Weekday::Thursday => Weekday::Thu,
        gk_core::Weekday::Friday => Weekday::Fri,
        gk_core::Weekday::Saturday => Weekday::Sat,
        gk_core::Weekday::Sunday => Weekday::Sun,
    }
}

const fn to_core_unit(f: Frequency) -> rec::Frequency {
    match f {
        Frequency::Day => rec::Frequency::Day,
        Frequency::Week => rec::Frequency::Week,
        Frequency::Month => rec::Frequency::Month,
        Frequency::Year => rec::Frequency::Year,
    }
}

const fn from_core_unit(f: rec::Frequency) -> Frequency {
    match f {
        rec::Frequency::Day => Frequency::Day,
        rec::Frequency::Week => Frequency::Week,
        rec::Frequency::Month => Frequency::Month,
        rec::Frequency::Year => Frequency::Year,
    }
}

/// The calendar rule of a stored rule.
pub(crate) fn to_core(r: &RecurrenceRule) -> Option<Rule> {
    Some(Rule {
        every: to_core_unit(r.every),
        interval: u16::try_from(r.interval).ok()?,
        weekdays: r.weekdays.iter().copied().map(to_core_weekday).collect(),
        month_day: r.month_day.and_then(|d| i8::try_from(d).ok()),
        nth: match (r.week_of_month, r.weekday) {
            (Some(n), Some(w)) => Some(NthWeekday {
                nth: i8::try_from(n).ok()?,
                weekday: to_core_weekday(w),
            }),
            _ => None,
        },
        month: r.month.and_then(|m| u8::try_from(m).ok()),
        start: r.starts.parse().ok()?,
        until: r.until.as_deref().and_then(|u| u.parse().ok()),
        count: r.count,
        from: match r.from {
            RepeatFrom::Schedule => rec::RepeatFrom::Schedule,
            RepeatFrom::Completion => rec::RepeatFrom::Completion,
        },
    })
}

fn from_core(r: &Rule, create: CreateWhen, days_ahead: u32) -> RecurrenceRule {
    RecurrenceRule {
        every: from_core_unit(r.every),
        interval: u32::from(r.interval),
        weekdays: if r.every == rec::Frequency::Week {
            r.days_of_week()
                .into_iter()
                .map(from_core_weekday)
                .collect()
        } else {
            Vec::new()
        },
        month_day: r.month_day.map(i32::from),
        week_of_month: r.nth.map(|n| i32::from(n.nth)),
        weekday: r.nth.map(|n| from_core_weekday(n.weekday)),
        month: r.month.map(u32::from),
        starts: r.start.to_string(),
        until: r.until.map(|u| u.to_string()),
        count: r.count,
        from: match r.from {
            rec::RepeatFrom::Schedule => RepeatFrom::Schedule,
            rec::RepeatFrom::Completion => RepeatFrom::Completion,
        },
        create,
        days_ahead: if create == CreateWhen::Ahead {
            days_ahead
        } else {
            0
        },
    }
}

fn rule_error(e: &RuleError, from_text: bool) -> Failure {
    let field = match e {
        RuleError::Interval { .. } => "interval",
        RuleError::MonthDay | RuleError::DayAndWeekday => "month_day",
        RuleError::WeekOfMonth => "week_of_month",
        RuleError::Month => "month",
        RuleError::Parts { .. } => "every",
        RuleError::CompletionPattern => "from",
        RuleError::UntilBeforeStart { .. } => "until",
        RuleError::Count { .. } => "count",
        RuleError::Text(_) => "text",
    };
    invalid(
        if from_text && field != "until" {
            "text"
        } else {
            field
        },
        e.clone(),
    )
}

fn small(field: &str, n: i32, low: i32, high: i32) -> Result<i8, Failure> {
    if n == -1 || (low..=high).contains(&n) {
        i8::try_from(n).map_err(|_| messages::invalid(field, "recurrence-number-range", &[]))
    } else {
        Err(match field {
            "month_day" => rule_error(&RuleError::MonthDay, false),
            _ => rule_error(&RuleError::WeekOfMonth, false),
        })
    }
}

/// Reads a rule from `input`; `start` is the default first day, and
/// `existing` the rule it changes (kept when `input` gives none).
pub(crate) fn build(
    tx: &Transaction<'_>,
    zone: &Zone,
    now: i64,
    input: &RulePreviewInput,
    start: Day,
    existing: Option<&RecurrenceRule>,
) -> Result<RecurrenceRule, Failure> {
    let date = |field: &str, text: &str| super::task::parse_date(tx, zone, now, field, text);
    let start = match &input.starts {
        Some(s) => date("starts", s)?,
        None => existing
            .and_then(|e| e.starts.parse().ok())
            .unwrap_or(start),
    };
    let from_text = input.text.is_some();
    let mut rule = match (&input.text, input.every, existing.and_then(to_core)) {
        (Some(text), _, _) => rec::parse(text, start).map_err(|e| rule_error(&e, true))?,
        (None, Some(every), _) => Rule::every(to_core_unit(every), 1, start),
        (None, None, Some(old)) => old,
        (None, None, None) => {
            return Err(messages::invalid("every", "recurrence-rule-missing", &[]));
        }
    };
    rule.start = start;
    if let (Some(every), true) = (input.every, from_text) {
        rule.every = to_core_unit(every);
    }
    if let Some(n) = input.interval {
        rule.interval = u16::try_from(n).unwrap_or(u16::MAX);
    }
    if !input.weekdays.is_empty() {
        rule.weekdays = input
            .weekdays
            .iter()
            .copied()
            .map(to_core_weekday)
            .collect();
    }
    if let Some(d) = input.month_day {
        rule.month_day = Some(small("month_day", d, 1, 31)?);
        rule.nth = None;
    }
    if let Some(n) = input.week_of_month {
        let Some(weekday) = input.weekday else {
            return Err(messages::invalid(
                "weekday",
                "recurrence-weekday-missing",
                &[],
            ));
        };
        rule.nth = Some(NthWeekday {
            nth: small("week_of_month", n, 1, 5)?,
            weekday: to_core_weekday(weekday),
        });
        if input.month_day.is_none() {
            rule.month_day = None;
        }
    }
    if let Some(m) = input.month {
        rule.month = Some(u8::try_from(m).unwrap_or(u8::MAX));
    }
    if let Some(u) = &input.until {
        rule.until = if u.trim().is_empty() {
            None
        } else {
            Some(date("until", u)?)
        };
    }
    if let Some(c) = input.count {
        rule.count = Some(c);
    }
    if let Some(f) = input.from {
        rule.from = match f {
            RepeatFrom::Schedule => rec::RepeatFrom::Schedule,
            RepeatFrom::Completion => rec::RepeatFrom::Completion,
        };
    }
    if rule.from == rec::RepeatFrom::Completion {
        // From completion only the unit and interval count.
        if !from_text {
            rule.weekdays.clear();
            rule.month_day = None;
            rule.nth = None;
            rule.month = None;
        }
    }
    rule.check().map_err(|e| rule_error(&e, from_text))?;
    let create = input
        .create
        .or_else(|| existing.map(|e| e.create))
        .unwrap_or_default();
    let days_ahead = input
        .days_ahead
        .or_else(|| existing.map(|e| e.days_ahead))
        .unwrap_or(0);
    if create == CreateWhen::Ahead && rule.from == rec::RepeatFrom::Completion {
        return Err(messages::invalid(
            "create",
            "recurrence-ahead-from-completion",
            &[],
        ));
    }
    if days_ahead > DAYS_AHEAD_MAX {
        return Err(messages::invalid(
            "days_ahead",
            "recurrence-days-ahead",
            &[("max", DAYS_AHEAD_MAX.to_string())],
        ));
    }
    Ok(from_core(&rule, create, days_ahead))
}

// ---------------------------------------------------------------------
// Words

fn weekday_name(w: Weekday) -> String {
    messages::text(
        match w {
            Weekday::Mon => "weekday-monday",
            Weekday::Tue => "weekday-tuesday",
            Weekday::Wed => "weekday-wednesday",
            Weekday::Thu => "weekday-thursday",
            Weekday::Fri => "weekday-friday",
            Weekday::Sat => "weekday-saturday",
            Weekday::Sun => "weekday-sunday",
        },
        &[],
    )
}

fn month_name(m: u32) -> String {
    let id = match m {
        1 => "month-january",
        2 => "month-february",
        3 => "month-march",
        4 => "month-april",
        5 => "month-may",
        6 => "month-june",
        7 => "month-july",
        8 => "month-august",
        9 => "month-september",
        10 => "month-october",
        11 => "month-november",
        _ => "month-december",
    };
    messages::text(id, &[])
}

/// `items` as one list in words ("Monday, Wednesday and Friday"), with the
/// language's own patterns for two items, the start, the middle and the end.
fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => messages::text(
            "recurrence-list-two",
            &[("first", a.clone()), ("second", b.clone())],
        ),
        [first, middle @ .., last] => {
            let mut tail = last.clone();
            let mut rest: Vec<&String> = middle.iter().collect();
            if let Some(before_last) = rest.pop() {
                tail = messages::text(
                    "recurrence-list-end",
                    &[("first", before_last.clone()), ("second", tail)],
                );
            }
            while let Some(item) = rest.pop() {
                tail = messages::text(
                    "recurrence-list-middle",
                    &[("first", item.clone()), ("second", tail)],
                );
            }
            messages::text(
                "recurrence-list-start",
                &[("first", first.clone()), ("second", tail)],
            )
        }
    }
}

fn nth_words(nth: i32, weekday: Weekday) -> String {
    messages::text(
        "recurrence-nth-weekday",
        &[("nth", nth.to_string()), ("weekday", weekday_name(weekday))],
    )
}

/// The rule in words, for people.
pub(crate) fn summary(r: &RecurrenceRule) -> String {
    let n = ("interval", r.interval.to_string());
    let base = if r.from == RepeatFrom::Completion {
        let id = match r.every {
            Frequency::Day => "recurrence-after-done-days",
            Frequency::Week => "recurrence-after-done-weeks",
            Frequency::Month => "recurrence-after-done-months",
            Frequency::Year => "recurrence-after-done-years",
        };
        messages::text(id, &[n])
    } else {
        match r.every {
            Frequency::Day => messages::text("recurrence-daily", &[n]),
            Frequency::Week => {
                let days: Vec<String> = r.weekdays.iter().copied().map(weekday_name).collect();
                messages::text("recurrence-weekly", &[n, ("days", list(&days))])
            }
            Frequency::Month => match (r.week_of_month, r.weekday, r.month_day) {
                (Some(nth), Some(w), _) => messages::text(
                    "recurrence-monthly-weekday",
                    &[n, ("weekday", nth_words(nth, w))],
                ),
                (_, _, Some(-1)) => messages::text("recurrence-monthly-last-day", &[n]),
                (_, _, day) => {
                    let day = day.unwrap_or_else(|| {
                        r.starts
                            .parse::<Day>()
                            .map(|d| i32::from(d.day()))
                            .unwrap_or(1)
                    });
                    messages::text("recurrence-monthly-day", &[n, ("day", day.to_string())])
                }
            },
            Frequency::Year => {
                let start = r.starts.parse::<Day>().ok();
                let month = r
                    .month
                    .unwrap_or_else(|| start.map_or(1, |d| u32::from(d.month())));
                let m = ("month", month_name(month));
                match (r.week_of_month, r.weekday, r.month_day) {
                    (Some(nth), Some(w), _) => messages::text(
                        "recurrence-yearly-weekday",
                        &[n, m, ("weekday", nth_words(nth, w))],
                    ),
                    (_, _, Some(-1)) => messages::text("recurrence-yearly-last-day", &[n, m]),
                    (_, _, day) => {
                        let day = day.unwrap_or_else(|| start.map_or(1, |d| i32::from(d.day())));
                        messages::text("recurrence-yearly-day", &[n, m, ("day", day.to_string())])
                    }
                }
            }
        }
    };
    let with_until = match &r.until {
        Some(u) => messages::text(
            "recurrence-with-until",
            &[("rule", base), ("until", u.clone())],
        ),
        None => base,
    };
    match r.count {
        Some(c) => messages::text(
            "recurrence-with-count",
            &[("rule", with_until), ("count", c.to_string())],
        ),
        None => with_until,
    }
}

// ---------------------------------------------------------------------
// Reading

fn series_of(tx: &Transaction<'_>, task: Id) -> Result<Option<Id>, StoreError> {
    Ok(tx
        .query_row(
            "SELECT series_id FROM tasks WHERE id = ?1",
            [b(task)],
            |r| opt_id_at(r, 0),
        )
        .optional()?
        .flatten())
}

/// The day an occurrence falls on: its due date, else its start date.
fn occurrence_day(tx: &Transaction<'_>, task: Id) -> Result<Option<Day>, StoreError> {
    let (start, due): (Option<String>, Option<String>) = tx
        .query_row(
            "SELECT start_date, due_date FROM tasks WHERE id = ?1",
            [b(task)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or((None, None));
    Ok(due.or(start).and_then(|d| d.parse().ok()))
}

fn exhausted(rule: &RecurrenceRule, made: u32) -> bool {
    rule.count.is_some_and(|c| made >= c)
}

/// The date of the next occurrence still to come, as far as it is known.
fn next_date(tx: &Transaction<'_>, r: &Record) -> Result<Option<Day>, StoreError> {
    if r.ended_at.is_some() || exhausted(&r.rule, r.made) {
        return Ok(None);
    }
    if r.rule.create == CreateWhen::Ahead {
        return Ok(r.next_date.as_deref().and_then(|d| d.parse().ok()));
    }
    let Some(rule) = to_core(&r.rule) else {
        return Ok(None);
    };
    if rule.from == rec::RepeatFrom::Completion {
        return Ok(None);
    }
    let last = match r.current_id() {
        Some(c) => occurrence_day(tx, c)?,
        None => None,
    };
    Ok(match last {
        Some(d) => rule.next_after(d),
        None => rule.first(),
    })
}

fn view(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<Recurrence, Failure> {
    let current = match r.current_id() {
        Some(c) => tx
            .query_row(
                "SELECT p.key || '-' || t.number FROM tasks t JOIN projects p ON p.id = t.project_id
                  WHERE t.id = ?1 AND t.deleted_at IS NULL",
                [b(c)],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        None => None,
    };
    Ok(Recurrence {
        series: id.to_string(),
        rule: r.rule.clone(),
        summary: summary(&r.rule),
        current,
        next_date: next_date(tx, r)?.map(|d| d.to_string()),
        made: r.made,
        ended: r.ended_at.is_some(),
        title: r.pattern.title.clone(),
    })
}

/// The live series `task` is the current occurrence of.
fn live(tx: &Transaction<'_>, task: Id) -> Result<Option<(Id, Record)>, StoreError> {
    let Some(series) = series_of(tx, task)? else {
        return Ok(None);
    };
    Ok(load(tx, series)?
        .filter(|r| r.ended_at.is_none() && r.current_id() == Some(task))
        .map(|r| (series, r)))
}

/// Task `task`'s series as tasks carry it: only while it is the series'
/// current occurrence and the series goes on.
pub(crate) fn of_task(tx: &Transaction<'_>, task: Id) -> Result<Option<Recurrence>, Failure> {
    match live(tx, task)? {
        Some((id, r)) => Ok(Some(view(tx, id, &r)?)),
        None => Ok(None),
    }
}

fn preview(
    tx: &Transaction<'_>,
    zone: &Zone,
    now: i64,
    input: &RulePreviewInput,
) -> Result<RecurrencePreview, Failure> {
    let today = zone.day_of(now);
    let rule = build(tx, zone, now, input, today, None)?;
    let limit = input.limit.unwrap_or(PREVIEW_DEFAULT);
    if limit == 0 || limit > PREVIEW_MAX {
        return Err(messages::invalid(
            "limit",
            "list-limit",
            &[("max", PREVIEW_MAX.to_string())],
        ));
    }
    let core =
        to_core(&rule).ok_or_else(|| messages::invalid("every", "recurrence-rule-missing", &[]))?;
    let mut dates: Vec<String> = Vec::new();
    if let Some(first) = core.first() {
        dates.push(first.to_string());
        let rest = core.upcoming(first, limit as usize - 1, 1);
        dates.extend(rest.iter().map(ToString::to_string));
    }
    dates.truncate(limit as usize);
    Ok(RecurrencePreview {
        summary: summary(&rule),
        rule,
        dates,
    })
}

// ---------------------------------------------------------------------
// Commands

fn preview_input(i: &RecurrenceSetInput) -> RulePreviewInput {
    RulePreviewInput {
        text: i.text.clone(),
        every: i.every,
        interval: i.interval,
        weekdays: i.weekdays.clone(),
        month_day: i.month_day,
        week_of_month: i.week_of_month,
        weekday: i.weekday,
        month: i.month,
        starts: i.starts.clone(),
        until: i.until.clone(),
        count: i.count,
        from: i.from,
        create: i.create,
        days_ahead: i.days_ahead,
        limit: None,
    }
}

fn gives_rule(i: &RulePreviewInput) -> bool {
    i.text.is_some()
        || i.every.is_some()
        || i.interval.is_some()
        || !i.weekdays.is_empty()
        || i.month_day.is_some()
        || i.week_of_month.is_some()
        || i.month.is_some()
        || i.starts.is_some()
        || i.until.is_some()
        || i.count.is_some()
        || i.from.is_some()
        || i.create.is_some()
        || i.days_ahead.is_some()
}

fn open_task(w: &WriteCtx<'_>, key: &str) -> Result<lookup::TaskRow, Failure> {
    let task = lookup::task(w.tx(), "key", key)?;
    if let Some(p) = lookup::project_by_id(w.tx(), task.project)? {
        lookup::ensure_active(&p)?;
    }
    let closed: Option<i64> = w.tx().query_row(
        "SELECT closed_at FROM tasks WHERE id = ?1",
        [b(task.id)],
        |r| r.get(0),
    )?;
    if closed.is_some() {
        let k = lookup::task_key(w.tx(), task.project, task.number)?;
        return Err(messages::invalid(
            "key",
            "recurrence-task-closed",
            &[("task", k)],
        ));
    }
    Ok(task)
}

fn change(w: &WriteCtx<'_>, task: Id, series: Option<Id>) -> Result<RecurrenceChange, Failure> {
    let key: String = w.tx().query_row(
        "SELECT p.key || '-' || t.number FROM tasks t JOIN projects p ON p.id = t.project_id
          WHERE t.id = ?1",
        [b(task)],
        |r| r.get(0),
    )?;
    let recurrence = match series.and_then(|s| load(w.tx(), s).ok().flatten().map(|r| (s, r))) {
        Some((id, r)) if r.ended_at.is_none() => Some(view(w.tx(), id, &r)?),
        _ => None,
    };
    let due_date: Option<String> =
        w.tx()
            .query_row("SELECT due_date FROM tasks WHERE id = ?1", [b(task)], |r| {
                r.get(0)
            })?;
    Ok(RecurrenceChange {
        task: key,
        recurrence,
        due_date,
        group: w.group_if_changed(),
    })
}

/// The first date to make ahead after `last`, for a series made ahead.
fn ahead_next(rule: &RecurrenceRule, last: Option<Day>, made: u32) -> Option<String> {
    if rule.create != CreateWhen::Ahead || exhausted(rule, made) {
        return None;
    }
    let core = to_core(rule)?;
    match last {
        Some(d) => core.next_after(d),
        None => core.first(),
    }
    .map(|d| d.to_string())
}

fn set(w: &mut WriteCtx<'_>, input: RecurrenceSetInput) -> Result<RecurrenceChange, Failure> {
    let task = open_task(w, &input.key)?;
    let rule_input = preview_input(&input);
    let today = w.zone().day_of(w.now());
    let day = occurrence_day(w.tx(), task.id)?;
    let pid = Some(task.project);
    match live(w.tx(), task.id)? {
        Some((id, old)) => {
            let mut r = old.clone();
            if gives_rule(&rule_input) {
                r.rule = build(
                    w.tx(),
                    w.zone(),
                    w.now(),
                    &rule_input,
                    day.unwrap_or(today),
                    Some(&old.rule),
                )?;
            }
            if input.update_details {
                let anchor = day.unwrap_or(today);
                r.pattern = pattern_of(w.tx(), task.id, anchor)?;
            }
            r.next_date = ahead_next(&r.rule, day, r.made);
            if r != old {
                r.updated_at = w.now();
                save(w, id, Some(&r))?;
            }
            change(w, task.id, Some(id))
        }
        None => {
            let rule = build(
                w.tx(),
                w.zone(),
                w.now(),
                &rule_input,
                day.unwrap_or(today),
                None,
            )?;
            let core = to_core(&rule)
                .ok_or_else(|| messages::invalid("every", "recurrence-rule-missing", &[]))?;
            // A task without dates gets the rule's first date as its due
            // date: every occurrence needs a day.
            let day = match day {
                Some(d) => d,
                None => {
                    let first = core
                        .first()
                        .ok_or_else(|| messages::invalid("until", "recurrence-no-dates", &[]))?;
                    fields::set(
                        w,
                        TASK,
                        task.id,
                        pid,
                        "due_date",
                        fields::text(first.to_string()),
                    )?;
                    first
                }
            };
            let pattern = pattern_of(w.tx(), task.id, day)?;
            let caller = w.caller().clone();
            let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
            let record = Record {
                project: task.project.to_string(),
                next_date: ahead_next(&rule, Some(day), 1),
                rule,
                pattern,
                current: Some(task.id.to_string()),
                made: 1,
                ended_at: None,
                created_at: w.now(),
                created_by_kind: caller.actor.kind.as_str().to_owned(),
                created_by_name: caller.actor.name,
                updated_at: w.now(),
            };
            save(w, id, Some(&record))?;
            fields::set(w, TASK, task.id, pid, "series", id_value(id))?;
            change(w, task.id, Some(id))
        }
    }
}

/// The details occurrences start from: task `task` as a blueprint, its
/// dates counted from `anchor`; a task with no due date gets one on the
/// occurrence's day.
fn pattern_of(tx: &Transaction<'_>, task: Id, anchor: Day) -> Result<TaskBlueprint, Failure> {
    let mut bp = super::template::blueprint_of(tx, task, anchor)?;
    if bp.due_in_days.is_none() && bp.start_in_days.is_none() {
        bp.due_in_days = Some(0);
    }
    Ok(bp)
}

fn clear(w: &mut WriteCtx<'_>, input: RecurrenceTaskInput) -> Result<RecurrenceChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    if let Some(p) = lookup::project_by_id(w.tx(), task.project)? {
        lookup::ensure_active(&p)?;
    }
    if let Some((id, mut r)) = live(w.tx(), task.id)? {
        r.ended_at = Some(w.now());
        r.next_date = None;
        r.updated_at = w.now();
        save(w, id, Some(&r))?;
    }
    change(w, task.id, None)
}

fn skip(w: &mut WriteCtx<'_>, input: RecurrenceTaskInput) -> Result<RecurrenceChange, Failure> {
    let task = open_task(w, &input.key)?;
    let key = lookup::task_key(w.tx(), task.project, task.number)?;
    let Some((id, mut r)) = live(w.tx(), task.id)? else {
        return Err(messages::invalid(
            "key",
            "recurrence-not-recurring",
            &[("task", key)],
        ));
    };
    let today = w.zone().day_of(w.now());
    let core = to_core(&r.rule)
        .ok_or_else(|| Failure::Op(messages::storage("recurrence-unreadable", &[])))?;
    let day = occurrence_day(w.tx(), task.id)?.unwrap_or(today);
    let next = if r.rule.create == CreateWhen::Ahead {
        r.next_date.as_deref().and_then(|d| d.parse::<Day>().ok())
    } else {
        core.next(day, today)
    };
    let Some(next) = next.filter(|_| !exhausted(&r.rule, r.made)) else {
        return Err(messages::conflict("recurrence-no-more", &[("task", key)]).into());
    };
    let shift = next.days_since_epoch() - day.days_since_epoch();
    shift_dates(w, task.id, task.project, shift)?;
    r.made += 1;
    r.next_date = ahead_next(&r.rule, Some(next), r.made);
    r.updated_at = w.now();
    save(w, id, Some(&r))?;
    change(w, task.id, Some(id))
}

/// Moves task `task`'s start and due dates by `days`.
fn shift_dates(w: &mut WriteCtx<'_>, task: Id, project: Id, days: i64) -> Result<(), Failure> {
    let (start, due): (Option<String>, Option<String>) = w.tx().query_row(
        "SELECT start_date, due_date FROM tasks WHERE id = ?1",
        [b(task)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let moved = |d: Option<String>| -> Result<Option<Value>, Failure> {
        d.map(|d| {
            d.parse::<Day>()
                .and_then(|d| d.add_days(days))
                .map(|d| Value::String(d.to_string()))
                .map_err(|e| invalid("due_date", e))
        })
        .transpose()
    };
    let (start, due) = (moved(start)?, moved(due)?);
    // The later date first, so the start never passes the due date.
    if days >= 0 {
        fields::set(w, TASK, task, Some(project), "due_date", due)?;
        fields::set(w, TASK, task, Some(project), "start_date", start)?;
    } else {
        fields::set(w, TASK, task, Some(project), "start_date", start)?;
        fields::set(w, TASK, task, Some(project), "due_date", due)?;
    }
    Ok(())
}

/// Makes series `id`'s occurrence on `day` and makes it the current one.
fn make_occurrence(
    w: &mut WriteCtx<'_>,
    id: Id,
    r: &mut Record,
    day: Day,
    parent: Option<Id>,
    project: Id,
) -> Result<Option<Id>, Failure> {
    let Some(p) = lookup::project_by_id(w.tx(), project)? else {
        return Ok(None);
    };
    if p.archived {
        return Ok(None);
    }
    let mut left_out = Vec::new();
    let (task, _) = super::template::make_task(
        w,
        &p,
        &r.pattern,
        None,
        day,
        parent,
        None,
        super::template::Making::Anyway,
        &mut left_out,
    )?;
    fields::set(w, TASK, task, Some(p.id), "series", id_value(id))?;
    for made in super::task::descendants(w.tx(), task)? {
        w.made_automatically(made);
    }
    r.project = p.id.to_string();
    r.current = Some(task.to_string());
    r.made += 1;
    r.updated_at = w.now();
    Ok(Some(task))
}

/// After task `task` moved to project `to`: when it is the current
/// occurrence of a series, the series moves with it, and its next
/// occurrences are made there. The details they start from name
/// statuses, types, labels, custom fields and versions by name, so they
/// take what `to` has by those names.
pub(crate) fn follow_move(w: &mut WriteCtx<'_>, task: Id, to: Id) -> Result<(), Failure> {
    let Some((id, mut r)) = live(w.tx(), task)? else {
        return Ok(());
    };
    if r.project_id() == Some(to) {
        return Ok(());
    }
    r.project = to.to_string();
    r.updated_at = w.now();
    save(w, id, Some(&r))
}

/// What closing an occurrence did to its series: the task closed, and the
/// next occurrence made, if one was.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Followed {
    pub(crate) closed: Id,
    pub(crate) next: Option<Id>,
}

/// After a command: for every task it closed that is the current
/// occurrence of a series made on closing, makes the next occurrence in
/// the same change. Undo, revert and imports replay recorded changes and
/// make nothing.
pub(crate) fn after_command(w: &mut WriteCtx<'_>) -> Result<Vec<Followed>, Failure> {
    let mut followed = Vec::new();
    let op = w.op();
    if op.starts_with("history.") || op.starts_with("transfer.") || op == "backup.restore" {
        return Ok(followed);
    }
    for task in w.newly_closed() {
        let Some((id, mut r)) = live(w.tx(), task)? else {
            continue;
        };
        if r.rule.create != CreateWhen::OnClose {
            continue;
        }
        let Some(rule) = to_core(&r.rule) else {
            continue;
        };
        let today = w.zone().day_of(w.now());
        let closed_at: Option<i64> = w.tx().query_row(
            "SELECT closed_at FROM tasks WHERE id = ?1",
            [b(task)],
            |row| row.get(0),
        )?;
        let done = w.zone().day_of(closed_at.unwrap_or(w.now()));
        let last = occurrence_day(w.tx(), task)?.unwrap_or(done);
        let mut next = rule.next(last, done);
        if rule.from == rec::RepeatFrom::Schedule {
            // Dates already past are passed over, and count as made.
            let mut passed = 0;
            while let Some(d) = next {
                if d >= today || passed >= CATCH_UP_MAX || exhausted(&r.rule, r.made) {
                    break;
                }
                r.made += 1;
                passed += 1;
                next = rule.next_after(d);
            }
        }
        let next = next.filter(|_| !exhausted(&r.rule, r.made));
        let Some(next) = next else {
            r.ended_at = Some(w.now());
            r.updated_at = w.now();
            save(w, id, Some(&r))?;
            followed.push(Followed {
                closed: task,
                next: None,
            });
            continue;
        };
        let (project, parent): (Id, Option<Id>) = w.tx().query_row(
            "SELECT project_id, (SELECT p.id FROM tasks p WHERE p.id = t.parent_id
                                  AND p.deleted_at IS NULL)
               FROM tasks t WHERE t.id = ?1",
            [b(task)],
            |row| Ok((id_at(row, 0)?, opt_id_at(row, 1)?)),
        )?;
        if let Some(made) = make_occurrence(w, id, &mut r, next, parent, project)? {
            save(w, id, Some(&r))?;
            followed.push(Followed {
                closed: task,
                next: Some(made),
            });
        }
    }
    Ok(followed)
}

/// Brings a command's `output` up to date with what [`after_command`] did:
/// a task in it (`task`) that closed an occurrence carries the series as a
/// later read shows it, and `next_occurrence` names the task made next.
pub(crate) fn update_output(
    tx: &Transaction<'_>,
    output: &mut Value,
    followed: &[Followed],
) -> Result<(), Failure> {
    let Some(task) = output.get_mut("task").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    let Some(id) = task
        .get("id")
        .and_then(Value::as_str)
        .and_then(|t| t.parse::<Id>().ok())
    else {
        return Ok(());
    };
    let Some(done) = followed.iter().find(|f| f.closed == id) else {
        return Ok(());
    };
    match of_task(tx, id)? {
        Some(now) => {
            let now = serde_json::to_value(now).map_err(|e| {
                Failure::Op(messages::storage(
                    "storage-json",
                    &[("error", e.to_string())],
                ))
            })?;
            task.insert("recurrence".to_owned(), now);
        }
        None => {
            task.remove("recurrence");
        }
    }
    if let Some(next) = done.next
        && let Some(row) = lookup::task_by_id(tx, next)?
        && let Some(out) = output.as_object_mut()
    {
        let key = lookup::task_key(tx, row.project, row.number)?;
        out.insert("next_occurrence".to_owned(), Value::String(key));
    }
    Ok(())
}

/// The series made ahead that wait for a date.
fn waiting(tx: &Transaction<'_>) -> Result<Vec<(Id, Record)>, StoreError> {
    let mut stmt = tx.prepare(&format!(
        "SELECT {COLUMNS} FROM task_series
          WHERE ended_at IS NULL AND next_date IS NOT NULL ORDER BY next_date, id"
    ))?;
    let rows = stmt
        .query_map([], read_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, r)| r.map(|r| (id, r)))
        .filter(|(_, r)| r.rule.create == CreateWhen::Ahead)
        .collect())
}

/// The day series `r`'s occurrence on `day` is made.
fn making_day(r: &Record, day: Day) -> Day {
    day.add_days(-i64::from(r.rule.days_ahead)).unwrap_or(day)
}

/// Makes the occurrences of series made ahead whose day has come: for each
/// series the latest such date (earlier ones it missed are passed over and
/// count as made).
pub(crate) fn run(w: &mut WriteCtx<'_>) -> Result<RecurrenceRun, Failure> {
    let today = w.zone().day_of(w.now());
    let mut created = Vec::new();
    for (id, mut r) in waiting(w.tx())? {
        let Some(rule) = to_core(&r.rule) else {
            continue;
        };
        let Some(mut day) = r.next_date.as_deref().and_then(|d| d.parse::<Day>().ok()) else {
            continue;
        };
        if making_day(&r, day) > today {
            continue;
        }
        // Pass over the dates whose following date is due too.
        let mut passed = 0;
        while let Some(after) = rule.next_after(day) {
            if making_day(&r, after) > today || passed >= CATCH_UP_MAX {
                break;
            }
            if r.rule.count.is_some_and(|c| r.made + 1 >= c) {
                break;
            }
            r.made += 1;
            passed += 1;
            day = after;
        }
        if exhausted(&r.rule, r.made) {
            r.ended_at = Some(w.now());
            r.next_date = None;
            r.updated_at = w.now();
            save(w, id, Some(&r))?;
            continue;
        }
        let Some(home) = r.project_id() else {
            continue;
        };
        let (project, parent): (Id, Option<Id>) = match r.current_id() {
            Some(c) => w
                .tx()
                .query_row(
                    "SELECT project_id, (SELECT p.id FROM tasks p WHERE p.id = t.parent_id
                                          AND p.deleted_at IS NULL)
                       FROM tasks t WHERE t.id = ?1",
                    [b(c)],
                    |row| Ok((id_at(row, 0)?, opt_id_at(row, 1)?)),
                )
                .optional()?
                .unwrap_or((home, None)),
            None => (home, None),
        };
        if let Some(task) = make_occurrence(w, id, &mut r, day, parent, project)? {
            r.next_date = ahead_next(&r.rule, Some(day), r.made);
            save(w, id, Some(&r))?;
            created.push(super::task::task_ref(w.tx(), task)?);
        }
    }
    let next_at = waiting(w.tx())?
        .iter()
        .filter_map(|(_, r)| {
            let day = r.next_date.as_deref()?.parse::<Day>().ok()?;
            Some(w.zone().moment(making_day(r, day), 0))
        })
        .min()
        .map(rfc3339);
    Ok(RecurrenceRun {
        created,
        next_at,
        group: w.group_if_changed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(every: Frequency) -> RecurrenceRule {
        RecurrenceRule {
            every,
            interval: 1,
            weekdays: Vec::new(),
            month_day: None,
            week_of_month: None,
            weekday: None,
            month: None,
            starts: "2026-11-26".into(),
            until: None,
            count: None,
            from: RepeatFrom::Schedule,
            create: CreateWhen::OnClose,
            days_ahead: 0,
        }
    }

    #[test]
    fn summaries_are_built_from_messages() {
        let mut weekly = rule(Frequency::Week);
        weekly.interval = 2;
        weekly.weekdays = vec![Weekday::Mon, Weekday::Thu];
        assert_eq!(summary(&weekly), "Every 2 weeks on Monday and Thursday");
        weekly.weekdays = vec![Weekday::Mon, Weekday::Wed, Weekday::Fri];
        weekly.interval = 1;
        assert_eq!(
            summary(&weekly),
            "Every week on Monday, Wednesday and Friday"
        );
        let mut monthly = rule(Frequency::Month);
        monthly.week_of_month = Some(-1);
        monthly.weekday = Some(Weekday::Fri);
        assert_eq!(summary(&monthly), "Every month on the last Friday");
        let mut done = rule(Frequency::Day);
        done.interval = 3;
        done.from = RepeatFrom::Completion;
        done.count = Some(1);
        assert_eq!(summary(&done), "3 days after it is done, once");
        let mut yearly = rule(Frequency::Year);
        yearly.until = Some("2030-01-01".into());
        assert_eq!(
            summary(&yearly),
            "Every year on November 26, until 2030-01-01"
        );
    }
}
