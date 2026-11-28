// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The planning reports: sprint velocity, the burn-up of a version or a
//! sprint, and estimate accuracy.
//!
//! Velocity and the burn-up replay the tasks' fields from the history:
//! which sprint and version a task was in, whether it was closed or in the
//! trash, and its estimate or number field, at any moment. A field history
//! never saw change keeps the value it has now.
//!
//! A sprint's committed work is what was in it, open and live, the moment
//! it started. Its completed work is what was in it at its end, closed,
//! having closed while in it and after its first day began; a task carried
//! over from an earlier sprint counts in the sprint it was completed in.

use std::collections::{HashMap, HashSet};

use gk_api::ops::planning::{
    BurnupDay, BurnupForecast, BurnupInput, BurnupReport, EstimateGroup, EstimateGroupBy,
    EstimateTask, EstimatesInput, EstimatesReport, PlanMeasure, ReportBurnup, ReportEstimates,
    ReportVelocity, VelocityInput, VelocityReport, VelocitySprint, VelocityUpcoming,
};
use gk_api::ops::sprint::SprintState;
use gk_api::ops::version::VersionRef;
use gk_core::{Day, FieldKind, Id, fold, worklog};
use gk_query::{Compiled, SqlEnv};
use gk_store::rusqlite::types::ValueRef;
use gk_store::rusqlite::{OptionalExtension, Transaction};
use serde_json::Value;

use super::desc;
use super::lookup::{self, b, id_at, opt_id_at};
use super::task::invalid;
use super::time::csv_line;
use crate::failure::Failure;
use crate::flow::round2;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::time::Zone;

/// The most sprints a velocity report shows.
const LAST_MAX: u32 = 50;
/// The sprints shown unless asked otherwise.
const LAST_DEFAULT: u32 = 10;
/// How many sprints the average covers.
const AVERAGE_OF: usize = 3;
/// The most days a burn-up shows; a longer one shows its latest days.
const BURNUP_DAYS_MAX: usize = 1000;
/// The fewest days a forecast is drawn from.
const FORECAST_MIN_POINTS: usize = 3;
/// The furthest ahead a forecast looks, in days.
const FORECAST_DAYS_MAX: f64 = 3650.0;
/// Within this share of the estimate counts as on target.
const ON_TARGET: f64 = 0.2;
/// How many outliers each group lists.
const OUTLIERS: usize = 10;
/// The days an estimate report covers unless asked otherwise, today
/// included.
const ESTIMATE_DAYS_DEFAULT: i64 = 90;

pub(super) fn register(r: &mut Registry) {
    r.query::<ReportVelocity, _>(desc::<ReportVelocity>(), |q, input| velocity(q, &input));
    r.query::<ReportBurnup, _>(desc::<ReportBurnup>(), |q, input| burnup(q, &input));
    r.query::<ReportEstimates, _>(desc::<ReportEstimates>(), |q, input| estimates(q, &input));
}

// ---------------------------------------------------------------------
// Replaying task fields

/// One field of a task through time.
#[derive(Debug, Default)]
struct Track {
    /// The value now.
    now: Option<Value>,
    /// `(at, before, after)` in the order they happened.
    changes: Vec<(i64, Option<Value>, Option<Value>)>,
}

impl Track {
    /// The value at moment `at`: after the last change by then, or before
    /// the first one; the value now when history never saw it change.
    fn at(&self, at: i64) -> Option<&Value> {
        match self.changes.iter().rposition(|(when, _, _)| *when <= at) {
            Some(i) => self.changes[i].2.as_ref(),
            None => match self.changes.first() {
                Some((_, before, _)) => before.as_ref(),
                None => self.now.as_ref(),
            },
        }
    }

    /// The first moment the field took `value`: its first change to it, or
    /// `created` when it had it from the start.
    fn first_set(&self, value: &Value, created: i64) -> Option<i64> {
        if let Some((_, before, _)) = self.changes.first() {
            if before.as_ref() == Some(value) {
                return Some(created);
            }
        } else if self.now.as_ref() == Some(value) {
            return Some(created);
        }
        self.changes
            .iter()
            .find(|(_, _, after)| after.as_ref() == Some(value))
            .map(|(at, _, _)| *at)
    }
}

/// Whether a recorded value says yes: set, and not `false` or `0`.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(_) => true,
    }
}

/// A task's past, as far as the planning reports need it.
#[derive(Debug, Default)]
struct Past {
    created: i64,
    sprint: Track,
    version: Track,
    closed: Track,
    trashed: Track,
    estimate: Track,
    field: Track,
}

impl Past {
    /// Whether the task existed and was out of the trash at `at`.
    fn live(&self, at: i64) -> bool {
        self.created <= at && !truthy(self.trashed.at(at))
    }

    fn closed(&self, at: i64) -> bool {
        truthy(self.closed.at(at))
    }

    /// When the task last closed, by `at`.
    fn closed_since(&self, at: i64) -> Option<i64> {
        if !self.closed(at) {
            return None;
        }
        let last = self
            .closed
            .changes
            .iter()
            .rev()
            .filter(|(when, _, _)| *when <= at)
            .find(|(_, _, after)| truthy(after.as_ref()))
            .map(|(when, _, _)| *when);
        last.or_else(|| match self.closed.at(at) {
            Some(Value::Number(n)) => n.as_i64(),
            _ => None,
        })
    }

    /// Its work at `at`.
    fn value(&self, measure: PlanMeasure, at: i64) -> f64 {
        match measure {
            PlanMeasure::Tasks => 1.0,
            PlanMeasure::Estimate => {
                self.estimate.at(at).and_then(Value::as_f64).unwrap_or(0.0) / 60.0
            }
            PlanMeasure::Field => self.field.at(at).and_then(Value::as_f64).unwrap_or(0.0),
        }
    }
}

fn json_now(
    r: &gk_store::rusqlite::Row<'_>,
    i: usize,
) -> gk_store::rusqlite::Result<Option<Value>> {
    Ok(match r.get_ref(i)? {
        ValueRef::Null | ValueRef::Blob(_) => None,
        ValueRef::Integer(n) => Some(Value::from(n)),
        ValueRef::Real(f) => Some(Value::from(f)),
        ValueRef::Text(t) => Some(Value::String(String::from_utf8_lossy(t).into_owned())),
    })
}

/// The tasks whose history or present names `id` in the task field
/// `field` (`sprint` or `fix_version`), stored in `column` now.
fn ever_in(tx: &Transaction<'_>, field: &str, column: &str, id: Id) -> Result<Vec<Id>, Failure> {
    let wanted = id.to_string();
    let mut ids: Vec<Id> = {
        let mut stmt = tx.prepare_cached(&format!(
            "SELECT DISTINCT entity_id FROM activity
              WHERE entity_kind = 'task'
                AND (json_extract(changes, '$.fields.{field}.after') = ?1
                     OR json_extract(changes, '$.fields.{field}.before') = ?1)"
        ))?;
        stmt.query_map([&wanted], |r| id_at(r, 0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut seen: HashSet<Id> = ids.iter().copied().collect();
    let mut stmt = tx.prepare_cached(&format!("SELECT id FROM tasks WHERE {column} = ?1"))?;
    for t in stmt.query_map([b(id)], |r| id_at(r, 0))? {
        let t = t?;
        if seen.insert(t) {
            ids.push(t);
        }
    }
    Ok(ids)
}

/// The pasts of `ids`, with the number field `field` when one is measured.
fn pasts(tx: &Transaction<'_>, ids: &[Id], field: Option<Id>) -> Result<Vec<Past>, Failure> {
    let field_key = field.map(|f| format!("{}{f}", super::custom_field::VALUE_PREFIX));
    let mut out = Vec::with_capacity(ids.len());
    for &task in ids {
        let row = tx
            .query_row(
                "SELECT created_at, sprint_id, fix_version_id, closed_at,
                        deleted_at IS NOT NULL, estimate_min
                   FROM tasks WHERE id = ?1",
                [b(task)],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        opt_id_at(r, 1)?,
                        opt_id_at(r, 2)?,
                        r.get::<_, Option<i64>>(3)?,
                        r.get::<_, bool>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((created, sprint, version, closed, trashed, estimate)) = row else {
            continue;
        };
        let field_now = match field {
            Some(f) => tx
                .query_row(
                    "SELECT value FROM custom_values WHERE task_id = ?1 AND field_id = ?2",
                    [b(task), b(f)],
                    |r| json_now(r, 0),
                )
                .optional()?
                .flatten(),
            None => None,
        };
        let track = |now: Option<Value>| Track {
            now,
            changes: Vec::new(),
        };
        let mut past = Past {
            created,
            sprint: track(sprint.map(|s| Value::String(s.to_string()))),
            version: track(version.map(|v| Value::String(v.to_string()))),
            closed: track(closed.map(Value::from)),
            trashed: track(trashed.then(|| Value::from(1))),
            estimate: track(estimate.map(Value::from)),
            field: track(field_now),
        };
        let mut stmt = tx.prepare_cached(
            "SELECT at, changes FROM activity
              WHERE entity_kind = 'task' AND entity_id = ?1 ORDER BY seq",
        )?;
        let mut rows = stmt.query([b(task)])?;
        while let Some(r) = rows.next()? {
            let at: i64 = r.get(0)?;
            let text: String = r.get(1)?;
            let Ok(changes) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            let Some(fields) = changes.get("fields").and_then(Value::as_object) else {
                continue;
            };
            let take = |name: &str, track: &mut Track| {
                if let Some(change) = fields.get(name) {
                    let side = |s: &str| change.get(s).filter(|v| !v.is_null()).cloned();
                    track.changes.push((at, side("before"), side("after")));
                }
            };
            take(super::sprint::TASK_FIELD, &mut past.sprint);
            take(super::version::TASK_FIELD, &mut past.version);
            take("closed_at", &mut past.closed);
            take("trashed", &mut past.trashed);
            take("estimate", &mut past.estimate);
            if let Some(key) = &field_key {
                take(key, &mut past.field);
            }
        }
        out.push(past);
    }
    Ok(out)
}

/// The number field `text` names among the fields `project` sees.
pub(crate) fn number_field(
    tx: &Transaction<'_>,
    project: Option<Id>,
    measure: PlanMeasure,
    text: Option<&str>,
) -> Result<Option<(Id, String)>, Failure> {
    let text = text.map(str::trim).filter(|t| !t.is_empty());
    if measure != PlanMeasure::Field {
        if text.is_some() {
            return Err(messages::invalid("field", "planning-field-unused", &[]));
        }
        return Ok(None);
    }
    let Some(text) = text else {
        return Err(messages::invalid("field", "planning-field-needed", &[]));
    };
    let all = super::custom_field::rows(tx, project)?;
    let row = super::custom_field::named(&all, text)
        .ok_or_else(|| super::custom_field::not_found(&all, "field", text))?;
    if !matches!(row.kind, FieldKind::Integer | FieldKind::Decimal) {
        return Err(messages::invalid(
            "field",
            "planning-field-not-number",
            &[("field", row.name.clone())],
        ));
    }
    Ok(Some((row.id, row.name.clone())))
}

/// The start of `day`'s first moment in `zone`.
fn day_start(zone: &Zone, day: Day) -> i64 {
    zone.moment(day, 0)
}

/// The last moment of `day` in `zone`.
fn day_end(zone: &Zone, day: Day) -> i64 {
    zone.moment(day.add_days(1).unwrap_or(day), 0) - 1
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn number_text(n: f64) -> String {
    round2(n).to_string()
}

// ---------------------------------------------------------------------
// Velocity

fn velocity(q: &QueryCtx<'_>, input: &VelocityInput) -> Result<VelocityReport, Failure> {
    let tx = q.tx;
    let zone = &q.env.zone;
    let project = lookup::project(tx, "project", &input.project)?;
    let measure = input.measure.unwrap_or_default();
    let field = number_field(tx, Some(project.id), measure, input.field.as_deref())?;
    let last = input.last.unwrap_or(LAST_DEFAULT);
    if !(1..=LAST_MAX).contains(&last) {
        return Err(messages::invalid(
            "last",
            "planning-last",
            &[("max", LAST_MAX.to_string())],
        ));
    }
    let sprints = super::sprint::of_project(tx, project.id)?;
    let mut done: Vec<VelocitySprint> = Vec::new();
    let mut upcoming = Vec::new();
    for (id, r) in &sprints {
        let ids = ever_in(tx, super::sprint::TASK_FIELD, "sprint_id", *id)?;
        let pasts = pasts(tx, &ids, field.as_ref().map(|f| f.0))?;
        let wanted = Value::String(id.to_string());
        if r.state != SprintState::Closed {
            let now = q.now;
            let mut planned = 0.0;
            let mut tasks = 0usize;
            for p in &pasts {
                if p.live(now) && p.sprint.at(now) == Some(&wanted) {
                    planned += p.value(measure, now);
                    tasks += 1;
                }
            }
            upcoming.push(VelocityUpcoming {
                sprint: super::sprint::reference(*id, r),
                planned: round2(planned),
                tasks: count(tasks),
            });
            continue;
        }
        let Some(ended) = r.completed_at else {
            continue;
        };
        let first_day: Option<Day> = r.start_date.as_deref().and_then(|d| d.parse().ok());
        let started = match (r.started_at, first_day) {
            (Some(at), _) => at,
            (None, Some(d)) => day_start(zone, d),
            (None, None) => continue,
        };
        // Work closed from the start of its first day counts, even when it
        // was started later that day.
        let window = first_day.map_or(started, |d| day_start(zone, d).min(started));
        // Just before completing it moved the open tasks out.
        let end = ended - 1;
        let (mut committed, mut completed) = (0.0, 0.0);
        let (mut committed_tasks, mut completed_tasks) = (0usize, 0usize);
        for p in &pasts {
            if p.live(started) && p.sprint.at(started) == Some(&wanted) && !p.closed(started) {
                committed += p.value(measure, started);
                committed_tasks += 1;
            }
            if p.live(end)
                && p.sprint.at(end) == Some(&wanted)
                && p.closed_since(end)
                    .is_some_and(|at| at >= window && p.sprint.at(at) == Some(&wanted))
            {
                completed += p.value(measure, end);
                completed_tasks += 1;
            }
        }
        let recent: Vec<f64> = done
            .iter()
            .rev()
            .take(AVERAGE_OF - 1)
            .map(|s| s.completed)
            .chain([completed])
            .collect();
        let average = recent.iter().sum::<f64>() / recent.len() as f64;
        done.push(VelocitySprint {
            sprint: super::sprint::reference(*id, r),
            completed_on: zone.day_of(ended).to_string(),
            committed: round2(committed),
            completed: round2(completed),
            committed_tasks: count(committed_tasks),
            completed_tasks: count(completed_tasks),
            average: round2(average),
        });
    }
    // Sprints listed by when they were completed.
    done.sort_by(|a, b| a.completed_on.cmp(&b.completed_on));
    let average_of = done.len().min(AVERAGE_OF);
    let average = (average_of > 0).then(|| {
        round2(
            done.iter()
                .rev()
                .take(average_of)
                .map(|s| s.completed)
                .sum::<f64>()
                / average_of as f64,
        )
    });
    let skip = done.len().saturating_sub(last as usize);
    let shown: Vec<VelocitySprint> = done.into_iter().skip(skip).collect();
    let csv = input.csv.then(|| {
        let mut out = String::new();
        csv_line(
            &mut out,
            &[
                "sprint",
                "start_date",
                "end_date",
                "completed_on",
                "committed",
                "completed",
                "committed_tasks",
                "completed_tasks",
                "average",
            ],
        );
        for s in &shown {
            csv_line(
                &mut out,
                &[
                    &s.sprint.name,
                    s.sprint.start_date.as_deref().unwrap_or_default(),
                    s.sprint.end_date.as_deref().unwrap_or_default(),
                    &s.completed_on,
                    &number_text(s.committed),
                    &number_text(s.completed),
                    &s.committed_tasks.to_string(),
                    &s.completed_tasks.to_string(),
                    &number_text(s.average),
                ],
            );
        }
        out
    });
    Ok(VelocityReport {
        project: project.key,
        measure,
        field: field.map(|f| f.1),
        sprints: shown,
        average,
        average_of: count(average_of),
        upcoming,
        csv,
    })
}

/// The velocity of `project`'s sprints, as `report.velocity` gives it.
pub(crate) fn velocity_report(
    q: &QueryCtx<'_>,
    input: &VelocityInput,
) -> Result<VelocityReport, Failure> {
    velocity(q, input)
}

// ---------------------------------------------------------------------
// Burn-up

fn burnup(q: &QueryCtx<'_>, input: &BurnupInput) -> Result<BurnupReport, Failure> {
    let tx = q.tx;
    let zone = &q.env.zone;
    let today = zone.day_of(q.now);
    let measure = input.measure.unwrap_or_default();
    let version = input
        .version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let sprint = input
        .sprint
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let project = input.project.clone();
    let (id, owner, field_name, version_ref, sprint_ref, first, last, end, cutoff, ids);
    match (version, sprint) {
        (Some(v), None) => {
            let (vid, r, _) = super::version::find(tx, v, project.as_ref())?;
            let owner_id: Option<Id> = r.project.parse().ok();
            let key = match owner_id {
                Some(p) => lookup::project_by_id(tx, p)?.map(|p| p.key),
                None => None,
            }
            .unwrap_or_default();
            id = vid;
            owner = owner_id;
            field_name = super::version::TASK_FIELD;
            ids = ever_in(tx, field_name, "fix_version_id", vid)?;
            let start: Option<Day> = r.start_date.as_deref().and_then(|d| d.parse().ok());
            let due: Option<Day> = r.due_date.as_deref().and_then(|d| d.parse().ok());
            version_ref = Some(VersionRef {
                name: r.name.clone(),
                project: key,
                status: r.status,
                due_date: r.due_date.clone(),
                id: vid.to_string(),
            });
            sprint_ref = None;
            first = start;
            last = match (r.status, due) {
                (gk_api::ops::version::VersionStatus::Closed, Some(d)) => d.min(today),
                _ => today,
            };
            end = due;
            cutoff = None;
        }
        (None, Some(s)) => {
            let (sid, r) = super::sprint::find(tx, s, project.as_ref())?;
            if r.state == SprintState::Planned {
                let error =
                    messages::conflict("burnup-sprint-not-started", &[("sprint", r.name.clone())])
                        .with_detail("field", "sprint");
                return Err(error.into());
            }
            id = sid;
            owner = r.project.parse().ok();
            field_name = super::sprint::TASK_FIELD;
            ids = ever_in(tx, field_name, "sprint_id", sid)?;
            first = r
                .start_date
                .as_deref()
                .and_then(|d| d.parse().ok())
                .or_else(|| r.started_at.map(|at| zone.day_of(at)));
            (last, cutoff) = match (r.state, r.completed_at) {
                (SprintState::Closed, Some(at)) => (zone.day_of(at), Some(at - 1)),
                _ => (today, None),
            };
            end = r.end_date.as_deref().and_then(|d| d.parse().ok());
            sprint_ref = Some(super::sprint::reference(sid, &r));
            version_ref = None;
        }
        _ => {
            return Err(messages::invalid(
                "version",
                "burnup-version-or-sprint",
                &[],
            ));
        }
    }
    let field = number_field(tx, owner, measure, input.field.as_deref())?;
    let pasts = pasts(tx, &ids, field.as_ref().map(|f| f.0))?;
    let wanted = Value::String(id.to_string());
    let by_sprint = field_name == super::sprint::TASK_FIELD;
    fn pick(p: &Past, by_sprint: bool) -> &Track {
        if by_sprint { &p.sprint } else { &p.version }
    }
    let track = |p| pick(p, by_sprint);
    // Without a start: the day the first task was planned for it.
    let first = first.unwrap_or_else(|| {
        pasts
            .iter()
            .filter_map(|p| track(p).first_set(&wanted, p.created))
            .min()
            .map_or(today, |at| zone.day_of(at))
    });
    let mut from = first;
    if last.days_since_epoch() - from.days_since_epoch() >= BURNUP_DAYS_MAX as i64 {
        from = last
            .add_days(-(BURNUP_DAYS_MAX as i64 - 1))
            .map_err(|e| invalid("version", e))?;
    }
    let mut days = Vec::new();
    let mut day = from;
    while day <= last {
        let at = cutoff.map_or(day_end(zone, day), |c| c.min(day_end(zone, day)));
        let mut entry = BurnupDay {
            date: day.to_string(),
            scope: 0.0,
            completed: 0.0,
            scope_tasks: 0,
            completed_tasks: 0,
        };
        for p in &pasts {
            if !p.live(at) || track(p).at(at) != Some(&wanted) {
                continue;
            }
            let v = p.value(measure, at);
            entry.scope += v;
            entry.scope_tasks += 1;
            if p.closed(at) {
                entry.completed += v;
                entry.completed_tasks += 1;
            }
        }
        entry.scope = round2(entry.scope);
        entry.completed = round2(entry.completed);
        days.push(entry);
        match day.add_days(1) {
            Ok(next) => day = next,
            Err(_) => break,
        }
    }
    let forecast = if input.forecast.unwrap_or(true) {
        forecast(from, &days)
    } else {
        None
    };
    let csv = input.csv.then(|| {
        let mut out = String::new();
        csv_line(
            &mut out,
            &[
                "date",
                "scope",
                "completed",
                "scope_tasks",
                "completed_tasks",
            ],
        );
        for d in &days {
            csv_line(
                &mut out,
                &[
                    &d.date,
                    &number_text(d.scope),
                    &number_text(d.completed),
                    &d.scope_tasks.to_string(),
                    &d.completed_tasks.to_string(),
                ],
            );
        }
        out
    });
    Ok(BurnupReport {
        version: version_ref,
        sprint: sprint_ref,
        measure,
        field: field.map(|f| f.1),
        from: from.to_string(),
        to: if last < from { from } else { last }.to_string(),
        end: end.map(|d| d.to_string()),
        days,
        forecast,
        csv,
    })
}

/// The least-squares line through the completed work of each day, to the
/// day it reaches the last day's scope; none with fewer than three days,
/// a line that doesn't rise, or all the work completed.
fn forecast(from: Day, days: &[BurnupDay]) -> Option<BurnupForecast> {
    let n = days.len();
    if n < FORECAST_MIN_POINTS {
        return None;
    }
    let last = days.last()?;
    if last.completed >= last.scope {
        return None;
    }
    let count = n as f64;
    let mean_x = (count - 1.0) / 2.0;
    let mean_y = days.iter().map(|d| d.completed).sum::<f64>() / count;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (i, d) in days.iter().enumerate() {
        let dx = i as f64 - mean_x;
        sxy += dx * (d.completed - mean_y);
        sxx += dx * dx;
    }
    let per_day = sxy / sxx;
    if per_day.is_nan() || per_day <= 0.0 {
        return None;
    }
    let start = mean_y - per_day * mean_x;
    // A hair under a whole day (rounding) is that day.
    let reach = ((last.scope - start) / per_day - 1e-9).ceil().max(count);
    if reach > FORECAST_DAYS_MAX + count {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let date = from.add_days(reach as i64).ok()?;
    Some(BurnupForecast {
        date: date.to_string(),
        start: round2(start),
        per_day: round2(per_day),
    })
}

/// A burn-up, as `report.burnup` gives it.
pub(crate) fn burnup_report(
    q: &QueryCtx<'_>,
    input: &BurnupInput,
) -> Result<BurnupReport, Failure> {
    burnup(q, input)
}

// ---------------------------------------------------------------------
// Estimate accuracy

/// One closed task, compared.
struct Compared {
    task: EstimateTask,
    closed_at: i64,
    /// The keys and names of the groups it is in.
    groups: Vec<(String, String)>,
}

fn estimates(q: &QueryCtx<'_>, input: &EstimatesInput) -> Result<EstimatesReport, Failure> {
    let tx = q.tx;
    let zone = &q.env.zone;
    let today = zone.day_of(q.now);
    let day = |field: &str, text: &str| {
        worklog::parse_work_day(text, today).map_err(|e| invalid(field, e))
    };
    let to = match &input.to {
        Some(t) => day("to", t)?,
        None => today,
    };
    let from = match &input.from {
        Some(t) => day("from", t)?,
        None => to
            .add_days(-(ESTIMATE_DAYS_DEFAULT - 1))
            .map_err(|e| invalid("from", e))?,
    };
    if to < from {
        return Err(messages::invalid("to", "activity-to-before-from", &[]));
    }
    let (start, stop) = (day_start(zone, from), day_end(zone, to));
    let group_by = input.group_by.unwrap_or_default();
    let projects: Option<HashSet<Id>> = match &input.project {
        Some(p) if !p.trim().is_empty() => {
            let p = lookup::project(tx, "project", p)?;
            Some(super::project::subtree(tx, p.id)?.into_iter().collect())
        }
        _ => None,
    };
    let matching: Option<HashSet<Id>> = match input.query.as_deref().map(str::trim) {
        Some(src) if !src.is_empty() => {
            let (_, checked) = super::query::compile(tx, zone, q.now, src, "query")?;
            let compiled = Compiled::new(
                &checked,
                &SqlEnv {
                    now: q.now,
                    scope: None,
                    default_sort: Vec::new(),
                },
            );
            Some(
                super::query::matching_ids(tx, &compiled)?
                    .into_iter()
                    .collect(),
            )
        }
        _ => None,
    };
    // Time logged per task, by kind of work.
    let mut spent: HashMap<Id, Vec<(Option<Id>, i64)>> = HashMap::new();
    {
        let mut stmt = tx.prepare_cached(
            "SELECT task_id, work_kind_id, sum(minutes) FROM time_entries
              WHERE deleted_at IS NULL GROUP BY task_id, work_kind_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((id_at(r, 0)?, opt_id_at(r, 1)?, r.get::<_, i64>(2)?))
        })?;
        for row in rows {
            let (task, kind, minutes) = row?;
            spent.entry(task).or_default().push((kind, minutes));
        }
    }
    let kinds: HashMap<Id, String> = {
        let mut stmt = tx.prepare_cached("SELECT id, name FROM work_kinds")?;
        stmt.query_map([], |r| Ok((id_at(r, 0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<_, _>>()?
    };
    let labels: HashMap<Id, Vec<String>> = if group_by == EstimateGroupBy::Label {
        let mut stmt = tx.prepare_cached(
            "SELECT tl.task_id, l.name FROM task_labels tl JOIN labels l ON l.id = tl.label_id
              ORDER BY l.name_fold",
        )?;
        let mut out: HashMap<Id, Vec<String>> = HashMap::new();
        for row in stmt.query_map([], |r| Ok((id_at(r, 0)?, r.get::<_, String>(1)?)))? {
            let (task, name) = row?;
            out.entry(task).or_default().push(name);
        }
        out
    } else {
        HashMap::new()
    };

    let mut compared: Vec<Compared> = Vec::new();
    let (mut without_estimate, mut without_time) = (0usize, 0usize);
    let mut allowed: HashMap<Id, bool> = HashMap::new();
    let mut stmt = tx.prepare_cached(
        "SELECT t.id, t.project_id, p.key, p.name, p.key || '-' || t.number, t.title, y.name,
                t.closed_at, t.estimate_min
           FROM tasks t
           JOIN projects p ON p.id = t.project_id
           JOIN task_types y ON y.id = t.type_id
           JOIN statuses s ON s.id = t.status_id
          WHERE t.deleted_at IS NULL AND p.deleted_at IS NULL
            AND t.closed_at IS NOT NULL AND s.category = 'done'
            AND t.closed_at BETWEEN ?1 AND ?2
          ORDER BY t.closed_at, p.key, t.number",
    )?;
    let rows = stmt.query_map([start, stop], |r| {
        Ok((
            id_at(r, 0)?,
            id_at(r, 1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, i64>(7)?,
            r.get::<_, Option<i64>>(8)?,
        ))
    })?;
    for row in rows {
        let (id, project, project_key, project_name, key, title, type_name, closed_at, estimate) =
            row?;
        if projects.as_ref().is_some_and(|p| !p.contains(&project))
            || matching.as_ref().is_some_and(|m| !m.contains(&id))
        {
            continue;
        }
        if crate::scope::active() {
            let ok = match allowed.get(&project) {
                Some(ok) => *ok,
                None => {
                    let ok = crate::scope::allows(tx, project)?;
                    allowed.insert(project, ok);
                    ok
                }
            };
            if !ok {
                continue;
            }
        }
        let Some(estimate) = estimate.filter(|e| *e > 0) else {
            without_estimate += 1;
            continue;
        };
        let logged = spent.get(&id).map_or(&[][..], Vec::as_slice);
        let total: i64 = logged.iter().map(|(_, m)| m).sum();
        if total <= 0 {
            without_time += 1;
            continue;
        }
        let closed_on = zone.day_of(closed_at).to_string();
        let groups = match group_by {
            EstimateGroupBy::None => Vec::new(),
            EstimateGroupBy::Type => vec![(fold(&type_name), type_name.clone())],
            EstimateGroupBy::Project => vec![(project_key, project_name)],
            EstimateGroupBy::Month => {
                let month = closed_on[..7].to_owned();
                vec![(month.clone(), month)]
            }
            EstimateGroupBy::Label => match labels.get(&id) {
                Some(names) => names.iter().map(|n| (fold(n), n.clone())).collect(),
                None => vec![(String::new(), String::new())],
            },
            EstimateGroupBy::WorkKind => {
                // The kind most of its time was logged as; ties by name.
                let mut by_kind: Vec<(String, i64)> = logged
                    .iter()
                    .map(|(k, m)| {
                        (
                            k.and_then(|k| kinds.get(&k).cloned()).unwrap_or_default(),
                            *m,
                        )
                    })
                    .collect();
                by_kind.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| fold(&a.0).cmp(&fold(&b.0))));
                let name = by_kind.first().map(|k| k.0.clone()).unwrap_or_default();
                vec![(fold(&name), name)]
            }
        };
        #[allow(clippy::cast_precision_loss)]
        let ratio = total as f64 / estimate as f64;
        compared.push(Compared {
            task: EstimateTask {
                key,
                title,
                task_type: type_name,
                closed_on,
                estimate_minutes: estimate,
                spent_minutes: total,
                ratio,
            },
            closed_at,
            groups,
        });
    }
    compared.sort_by(|a, b| {
        a.closed_at
            .cmp(&b.closed_at)
            .then_with(|| a.task.key.cmp(&b.task.key))
    });

    let all: Vec<&EstimateTask> = compared.iter().map(|c| &c.task).collect();
    let total = group(String::new(), String::new(), &all);
    let mut groups: Vec<EstimateGroup> = Vec::new();
    if group_by != EstimateGroupBy::None {
        let mut members: Vec<(String, String, Vec<&EstimateTask>)> = Vec::new();
        for c in &compared {
            for (key, name) in &c.groups {
                match members.iter_mut().find(|m| &m.0 == key) {
                    Some(m) => m.2.push(&c.task),
                    None => members.push((key.clone(), name.clone(), vec![&c.task])),
                }
            }
        }
        groups = members
            .into_iter()
            .map(|(key, name, tasks)| group(key, name, &tasks))
            .collect();
        if group_by == EstimateGroupBy::Month {
            groups.sort_by(|a, b| a.key.cmp(&b.key));
        } else {
            groups.sort_by(|a, b| {
                b.count
                    .cmp(&a.count)
                    .then_with(|| a.key.is_empty().cmp(&b.key.is_empty()))
                    .then_with(|| a.key.cmp(&b.key))
            });
        }
    }
    let tasks: Vec<EstimateTask> = compared.into_iter().map(|c| rounded(&c.task)).collect();
    let csv = input.csv.then(|| {
        let mut out = String::new();
        csv_line(
            &mut out,
            &[
                "key",
                "title",
                "type",
                "closed_on",
                "estimate_minutes",
                "spent_minutes",
                "ratio",
            ],
        );
        for t in &tasks {
            csv_line(
                &mut out,
                &[
                    &t.key,
                    &t.title,
                    &t.task_type,
                    &t.closed_on,
                    &t.estimate_minutes.to_string(),
                    &t.spent_minutes.to_string(),
                    &t.ratio.to_string(),
                ],
            );
        }
        out
    });
    Ok(EstimatesReport {
        from: from.to_string(),
        to: to.to_string(),
        group_by,
        total,
        groups,
        without_estimate: count(without_estimate),
        without_time: count(without_time),
        tasks: input.tasks.then_some(tasks),
        csv,
    })
}

/// A task with its ratio to two decimals.
fn rounded(t: &EstimateTask) -> EstimateTask {
    EstimateTask {
        ratio: round2(t.ratio),
        ..t.clone()
    }
}

/// How far off a ratio is, either way: 2 (twice as long) and 0.5 (half as
/// long) are as far.
fn distance(ratio: f64) -> f64 {
    ratio.ln().abs()
}

/// The median of `values`, which are sorted.
fn median(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n == 0 {
        return None;
    }
    Some(if n % 2 == 1 {
        values[n / 2]
    } else {
        f64::midpoint(values[n / 2 - 1], values[n / 2])
    })
}

fn group(key: String, name: String, tasks: &[&EstimateTask]) -> EstimateGroup {
    let mut ratios: Vec<f64> = tasks.iter().map(|t| t.ratio).collect();
    ratios.sort_by(f64::total_cmp);
    let within = ratios
        .iter()
        .filter(|r| (1.0 - ON_TARGET..=1.0 + ON_TARGET).contains(&round4(**r)))
        .count();
    let over = ratios
        .iter()
        .filter(|r| round4(**r) > 1.0 + ON_TARGET)
        .count();
    let under = ratios
        .iter()
        .filter(|r| round4(**r) < 1.0 - ON_TARGET)
        .count();
    let mut outliers: Vec<&EstimateTask> = tasks.to_vec();
    outliers.sort_by(|a, b| {
        distance(b.ratio)
            .total_cmp(&distance(a.ratio))
            .then_with(|| a.key.cmp(&b.key))
    });
    #[allow(clippy::cast_precision_loss)]
    let share = (!ratios.is_empty()).then(|| round4(within as f64 / ratios.len() as f64));
    EstimateGroup {
        key,
        name,
        count: count(tasks.len()),
        estimate_minutes: tasks.iter().map(|t| t.estimate_minutes).sum(),
        spent_minutes: tasks.iter().map(|t| t.spent_minutes).sum(),
        median_ratio: median(&ratios).map(round2),
        within_share: share,
        over: count(over),
        under: count(under),
        outliers: outliers.into_iter().take(OUTLIERS).map(rounded).collect(),
    }
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// Estimate accuracy, as `report.estimates` gives it.
pub(crate) fn estimates_report(
    q: &QueryCtx<'_>,
    input: &EstimatesInput,
) -> Result<EstimatesReport, Failure> {
    estimates(q, input)
}
