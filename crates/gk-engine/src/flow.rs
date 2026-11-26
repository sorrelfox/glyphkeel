// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The arithmetic of the flow reports, apart from storage: the periods of a
//! range, a task's history replayed into states, and percentiles.
//!
//! A task's history is a list of events: moves into a status and moves into
//! or out of the trash. Replaying it gives the task's state after each
//! event; the reports read the state at the end of each period. A close is
//! a move from an open status into a closed one while the task is out of the
//! trash; a reopen the move back.

use gk_api::ops::report::{FlowBucket, FlowStats};
use gk_core::{Day, Weekday, worklog};

/// One period: its first and last day (both included).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Period {
    pub(crate) first: Day,
    pub(crate) last: Day,
}

/// The first day of the period that `day` falls in.
fn natural_start(day: Day, bucket: FlowBucket, week_start: Weekday) -> Day {
    match bucket {
        FlowBucket::Day => day,
        FlowBucket::Week => worklog::week_start(day, week_start),
        FlowBucket::Month => Day::new(day.year(), day.month(), 1).unwrap_or(day),
    }
}

/// The first day of the period after the one that starts on `start`.
fn next_start(start: Day, bucket: FlowBucket) -> Option<Day> {
    match bucket {
        FlowBucket::Day => start.add_days(1).ok(),
        FlowBucket::Week => start.add_days(7).ok(),
        FlowBucket::Month => {
            let (y, m) = if start.month() == 12 {
                (start.year().checked_add(1)?, 1)
            } else {
                (start.year(), start.month() + 1)
            };
            Day::new(y, m, 1).ok()
        }
    }
}

/// The periods from `from` to `to`, the first and last cut to the range;
/// `None` when there would be more than `max`.
pub(crate) fn periods(
    from: Day,
    to: Day,
    bucket: FlowBucket,
    week_start: Weekday,
    max: usize,
) -> Option<Vec<Period>> {
    let mut out = Vec::new();
    let mut start = natural_start(from, bucket, week_start);
    while start <= to {
        let next = next_start(start, bucket)?;
        let first = if start < from { from } else { start };
        let end = next.add_days(-1).ok()?;
        let last = if end > to { to } else { end };
        out.push(Period { first, last });
        if out.len() > max {
            return None;
        }
        start = next;
    }
    Some(out)
}

/// The default first day: twelve periods back from `to`.
pub(crate) fn default_from(to: Day, bucket: FlowBucket, week_start: Weekday) -> Day {
    match bucket {
        FlowBucket::Day => to.add_days(-29).unwrap_or(to),
        FlowBucket::Week => {
            let start = worklog::week_start(to, week_start);
            start.add_days(-77).unwrap_or(start)
        }
        FlowBucket::Month => {
            let months = i32::from(to.year()) * 12 + i32::from(to.month()) - 1 - 11;
            let y = u16::try_from(months.div_euclid(12)).unwrap_or(to.year());
            let m = u8::try_from(months.rem_euclid(12) + 1).unwrap_or(1);
            Day::new(y, m, 1).unwrap_or(to)
        }
    }
}

/// What happened to a task at one moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    /// It moved into the status of this index.
    Status(usize),
    /// It went into the trash (`true`) or came back.
    Trashed(bool),
}

/// One event of a task's history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Event<A> {
    pub(crate) at: i64,
    pub(crate) change: Change,
    /// Who did it, when known.
    pub(crate) actor: Option<A>,
}

/// A task's state from a moment on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) at: i64,
    /// Its status; `None` before the first status is known.
    pub(crate) status: Option<usize>,
    /// Whether it is out of the trash.
    pub(crate) live: bool,
}

/// What a status means to the replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Todo,
    InProgress,
    Closed,
}

/// A task's history, replayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Replay<A> {
    /// The state after each event, by time; the first is the creation.
    pub(crate) states: Vec<State>,
    /// When it was closed, and by whom.
    pub(crate) closes: Vec<(i64, Option<A>)>,
    /// When it was opened again, and by whom.
    pub(crate) reopens: Vec<(i64, Option<A>)>,
    /// When it first moved into progress.
    pub(crate) first_progress: Option<i64>,
}

impl<A> Replay<A> {
    /// The state at moment `at`: the last one that began at or before it;
    /// `None` before the task existed.
    pub(crate) fn at(&self, at: i64) -> Option<State> {
        let n = self.states.partition_point(|s| s.at <= at);
        n.checked_sub(1).map(|i| self.states[i])
    }
}

/// Replays `events` (in the order they happened) of a task created at
/// `created`; `kind` says what each status means. Events before the
/// creation count at the creation.
pub(crate) fn replay<A: Clone>(
    created: i64,
    events: &[Event<A>],
    kind: impl Fn(usize) -> Kind,
) -> Replay<A> {
    let mut out = Replay {
        states: vec![State {
            at: created,
            status: None,
            live: true,
        }],
        closes: Vec::new(),
        reopens: Vec::new(),
        first_progress: None,
    };
    let mut current = out.states[0];
    for e in events {
        let at = e.at.max(created).max(current.at);
        let mut next = State { at, ..current };
        match e.change {
            Change::Status(s) => {
                let was = current.status.map(&kind);
                let now = kind(s);
                if current.live {
                    let was_closed = was == Some(Kind::Closed);
                    if now == Kind::Closed && !was_closed {
                        out.closes.push((at, e.actor.clone()));
                    } else if now != Kind::Closed && was_closed {
                        out.reopens.push((at, e.actor.clone()));
                    }
                }
                if now == Kind::InProgress && out.first_progress.is_none() {
                    out.first_progress = Some(at);
                }
                next.status = Some(s);
            }
            Change::Trashed(t) => next.live = !t,
        }
        if next == current {
            continue;
        }
        match out.states.last_mut() {
            Some(last) if last.at == at => *last = next,
            _ => out.states.push(next),
        }
        current = next;
    }
    out
}

/// Milliseconds as days.
pub(crate) fn days(ms: i64) -> f64 {
    ms as f64 / 86_400_000.0
}

/// `x` to two decimals.
pub(crate) fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// The `p`th percentile (0–100) of `sorted`, interpolating between the two
/// nearest values; `None` when it is empty.
pub(crate) fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    let last = sorted.len().checked_sub(1)?;
    let rank = (p / 100.0).clamp(0.0, 1.0) * last as f64;
    let below = rank.floor() as usize;
    let above = rank.ceil() as usize;
    let (lo, hi) = (sorted[below], sorted[above.min(last)]);
    Some(lo + (hi - lo) * (rank - below as f64))
}

/// The summary figures of `values`, in days.
pub(crate) fn stats(mut values: Vec<f64>) -> FlowStats {
    values.sort_by(f64::total_cmp);
    let count = u32::try_from(values.len()).unwrap_or(u32::MAX);
    let mean = (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64);
    let pick = |p: f64| percentile(&values, p).map(round2);
    FlowStats {
        count,
        mean: mean.map(round2),
        p50: pick(50.0),
        p85: pick(85.0),
        p95: pick(95.0),
        min: values.first().copied().map(round2),
        max: values.last().copied().map(round2),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;

    fn day(s: &str) -> Day {
        s.parse().unwrap()
    }

    fn spans(p: &[Period]) -> Vec<String> {
        p.iter()
            .map(|p| format!("{}..{}", p.first, p.last))
            .collect()
    }

    #[test]
    fn weeks_start_on_the_first_weekday_and_are_cut_to_the_range() {
        // 2026-11-04 is a Wednesday.
        let p = periods(
            day("2026-11-04"),
            day("2026-11-18"),
            FlowBucket::Week,
            Weekday::Monday,
            100,
        )
        .unwrap();
        assert_eq!(
            spans(&p),
            [
                "2026-11-04..2026-11-08",
                "2026-11-09..2026-11-15",
                "2026-11-16..2026-11-18"
            ]
        );
        let sunday = periods(
            day("2026-11-04"),
            day("2026-11-08"),
            FlowBucket::Week,
            Weekday::Sunday,
            100,
        )
        .unwrap();
        assert_eq!(
            spans(&sunday),
            ["2026-11-04..2026-11-07", "2026-11-08..2026-11-08"]
        );
    }

    #[test]
    fn months_and_days_cover_the_range() {
        let m = periods(
            day("2026-11-15"),
            day("2027-02-10"),
            FlowBucket::Month,
            Weekday::Monday,
            100,
        )
        .unwrap();
        assert_eq!(
            spans(&m),
            [
                "2026-11-15..2026-11-30",
                "2026-12-01..2026-12-31",
                "2027-01-01..2027-01-31",
                "2027-02-01..2027-02-10"
            ]
        );
        let d = periods(
            day("2028-02-28"),
            day("2028-03-01"),
            FlowBucket::Day,
            Weekday::Monday,
            100,
        )
        .unwrap();
        assert_eq!(d.len(), 3, "a leap day");
        assert!(
            periods(
                day("2020-01-01"),
                day("2026-01-01"),
                FlowBucket::Day,
                Weekday::Monday,
                1000
            )
            .is_none()
        );
    }

    #[test]
    fn the_default_range_is_twelve_periods() {
        let to = day("2026-11-25");
        assert_eq!(
            default_from(to, FlowBucket::Day, Weekday::Monday),
            day("2026-10-27")
        );
        assert_eq!(
            default_from(to, FlowBucket::Week, Weekday::Monday),
            day("2026-09-07")
        );
        assert_eq!(
            default_from(to, FlowBucket::Month, Weekday::Monday),
            day("2025-12-01")
        );
        let p = periods(
            default_from(to, FlowBucket::Week, Weekday::Monday),
            to,
            FlowBucket::Week,
            Weekday::Monday,
            100,
        )
        .unwrap();
        assert_eq!(p.len(), 12);
    }

    #[test]
    fn percentiles_interpolate() {
        let v = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(&v, 50.0), Some(2.5));
        assert_eq!(percentile(&v, 0.0), Some(1.0));
        assert_eq!(percentile(&v, 100.0), Some(4.0));
        assert_eq!(round2(percentile(&v, 85.0).unwrap()), 3.55);
        assert_eq!(percentile(&[7.0], 95.0), Some(7.0));
        assert_eq!(percentile(&[], 50.0), None);
        let s = stats(vec![3.0, 1.0, 2.0, 10.0]);
        assert_eq!(s.count, 4);
        assert_eq!(s.mean, Some(4.0));
        assert_eq!(s.p50, Some(2.5));
        assert_eq!(s.min, Some(1.0));
        assert_eq!(s.max, Some(10.0));
        let empty = stats(Vec::new());
        assert_eq!((empty.count, empty.p50, empty.mean), (0, None, None));
    }

    const TODO: usize = 0;
    const DOING: usize = 1;
    const DONE: usize = 2;

    fn kind(s: usize) -> Kind {
        match s {
            TODO => Kind::Todo,
            DOING => Kind::InProgress,
            _ => Kind::Closed,
        }
    }

    fn ev(at: i64, change: Change) -> Event<&'static str> {
        Event {
            at,
            change,
            actor: Some("me"),
        }
    }

    #[test]
    fn a_reopened_task_closes_twice_and_keeps_its_first_start() {
        let events = [
            ev(10, Change::Status(TODO)),
            ev(20, Change::Status(DOING)),
            ev(30, Change::Status(DONE)),
            ev(40, Change::Status(TODO)),
            ev(50, Change::Status(DOING)),
            ev(60, Change::Status(DONE)),
        ];
        let r = replay(10, &events, kind);
        assert_eq!(r.closes.iter().map(|c| c.0).collect::<Vec<_>>(), [30, 60]);
        assert_eq!(r.reopens.iter().map(|c| c.0).collect::<Vec<_>>(), [40]);
        assert_eq!(r.first_progress, Some(20));
        assert_eq!(r.at(35).unwrap().status, Some(DONE));
        assert_eq!(r.at(45).unwrap().status, Some(TODO));
        assert_eq!(r.at(9), None, "before it existed");
    }

    #[test]
    fn the_trash_hides_a_task_and_is_not_a_close() {
        let events = [
            ev(0, Change::Status(TODO)),
            ev(5, Change::Trashed(true)),
            ev(6, Change::Status(DONE)),
            ev(9, Change::Trashed(false)),
        ];
        let r = replay(0, &events, kind);
        assert!(r.closes.is_empty(), "closed while in the trash");
        assert!(!r.at(7).unwrap().live);
        assert!(r.at(9).unwrap().live);
        assert_eq!(r.at(9).unwrap().status, Some(DONE));
    }

    #[test]
    fn events_before_the_creation_count_at_the_creation_and_same_moments_merge() {
        let events = [ev(-5, Change::Status(TODO)), ev(100, Change::Status(DONE))];
        let r = replay(100, &events, kind);
        assert_eq!(r.states.len(), 1, "{:?}", r.states);
        assert_eq!(r.states[0].status, Some(DONE));
        assert_eq!(r.closes.len(), 1, "created straight into a closed status");
    }
}
