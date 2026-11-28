// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The critical path method over `blocks` links, on working days.
//!
//! Each task has a length in working days and may have a day it can't
//! begin before (its own start date). A link is finish-to-start: the
//! blocked task begins on the working day after its blocker finishes at
//! the earliest, plus the link's lag in working days.
//!
//! The forward pass gives every task its earliest start and finish; the
//! backward pass, from the end target, its latest start and finish. Total
//! slack is the latest start less the earliest start; a task with no slack
//! (or less, when the target is earlier than the plan can make) is
//! critical.
//!
//! Tasks are numbered `0..n`, as in [`crate::schedule`]; tasks in a loop of
//! links are left out of the result. Nothing here reads a clock.

use std::collections::VecDeque;

use crate::day::{Day, DayError};
use crate::schedule::Graph;

/// Days since 1970 of the Monday 5 January 1970, which weeks count from.
const MONDAY: i64 = 4;

/// Which days of the week are working days: the first `per_week` days
/// from Monday (five: Monday to Friday).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkWeek {
    per_week: i64,
}

impl Default for WorkWeek {
    fn default() -> Self {
        Self { per_week: 5 }
    }
}

impl WorkWeek {
    /// A week of `days_per_week` working days (1 to 7; out of range values
    /// are brought into it).
    #[must_use]
    pub fn new(days_per_week: u32) -> Self {
        Self {
            per_week: i64::from(days_per_week.clamp(1, 7)),
        }
    }

    /// How many working days a week has.
    #[must_use]
    pub fn per_week(self) -> u32 {
        u32::try_from(self.per_week).unwrap_or(5)
    }

    /// Whether `day` is a working day.
    #[must_use]
    pub fn is_working(self, day: Day) -> bool {
        (day.days_since_epoch() - MONDAY).rem_euclid(7) < self.per_week
    }

    /// The working day number of `day` read as a beginning: a day off
    /// counts as the next working day.
    #[must_use]
    pub fn begin_index(self, day: Day) -> i64 {
        let n = day.days_since_epoch() - MONDAY;
        n.div_euclid(7) * self.per_week + n.rem_euclid(7).min(self.per_week)
    }

    /// The working day number of `day` read as an end: a day off counts
    /// as the working day before it.
    #[must_use]
    pub fn end_index(self, day: Day) -> i64 {
        let n = day.days_since_epoch() - MONDAY;
        n.div_euclid(7) * self.per_week + n.rem_euclid(7).min(self.per_week - 1)
    }

    /// The day of working day number `index`.
    pub fn day(self, index: i64) -> Result<Day, DayError> {
        let weeks = index.div_euclid(self.per_week);
        let rest = index.rem_euclid(self.per_week);
        Day::from_days_since_epoch(MONDAY + weeks * 7 + rest)
    }

    /// How many working days `begin..=end` holds; at least one.
    #[must_use]
    pub fn length(self, begin: Day, end: Day) -> i64 {
        (self.end_index(end) - self.begin_index(begin) + 1).max(1)
    }
}

/// What the method needs of one task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Work {
    /// Its length in working days (at least one).
    pub days: i64,
    /// The working day it can't begin before: its own start, when it has
    /// one; `None` lets it begin with the plan.
    pub not_before: Option<i64>,
    /// Whether the task takes part; a task without dates and estimate
    /// doesn't, and neither do the links through it.
    pub included: bool,
}

/// One task's times, as working day numbers (see [`WorkWeek`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// The earliest working day it can begin.
    pub earliest_start: i64,
    /// The earliest working day it can finish.
    pub earliest_finish: i64,
    /// The latest working day it can begin without moving the end.
    pub latest_start: i64,
    /// The latest working day it can finish without moving the end.
    pub latest_finish: i64,
}

impl Timing {
    /// Total slack in working days: how far it can slip without moving the
    /// end target. Negative when the target can't be met.
    #[must_use]
    pub fn slack(&self) -> i64 {
        self.latest_start - self.earliest_start
    }

    /// Whether it is on the critical path: no slack, or less.
    #[must_use]
    pub fn critical(&self) -> bool {
        self.slack() <= 0
    }
}

/// The result of [`critical_path`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Each task's times; `None` for tasks left out (not included, or in a
    /// loop of links).
    pub timing: Vec<Option<Timing>>,
    /// The latest earliest finish: when the plan ends at the soonest.
    pub finish: Option<i64>,
    /// The end target the latest times count back from.
    pub end: Option<i64>,
    /// Tasks in loops of links, which have no times.
    pub in_loops: Vec<usize>,
}

/// The critical path of `work` over the links of `graph` (lags in working
/// days). The plan begins on `begin` (a working day number) for tasks with
/// nothing to wait for; the latest times count back from `end`, or from
/// the plan's own finish when `end` is `None`.
#[must_use]
pub fn critical_path(graph: &Graph, work: &[Work], begin: i64, end: Option<i64>) -> Plan {
    let n = graph.len().min(work.len());
    let inside = |i: usize| i < n && work[i].included;
    // Kahn's order over the included tasks.
    let mut waiting = vec![0usize; n];
    for (i, w) in waiting.iter_mut().enumerate() {
        if inside(i) {
            *w = graph.blocked_by(i).filter(|d| inside(d.from)).count();
        }
    }
    let mut ready: VecDeque<usize> = (0..n).filter(|&i| inside(i) && waiting[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(u) = ready.pop_front() {
        order.push(u);
        for d in graph.blocks(u) {
            if inside(d.to) {
                waiting[d.to] -= 1;
                if waiting[d.to] == 0 {
                    ready.push_back(d.to);
                }
            }
        }
    }
    let mut placed = vec![false; n];
    for &u in &order {
        placed[u] = true;
    }
    let in_loops: Vec<usize> = (0..n).filter(|&i| inside(i) && !placed[i]).collect();

    // Forward: earliest times.
    let mut es = vec![0i64; n];
    let mut ef = vec![0i64; n];
    for &u in &order {
        let mut start = work[u].not_before.unwrap_or(begin);
        for d in graph.blocked_by(u) {
            if placed[d.from] {
                start = start.max(ef[d.from] + 1 + d.lag_days.max(0));
            }
        }
        es[u] = start;
        ef[u] = start + work[u].days.max(1) - 1;
    }
    let finish = order.iter().map(|&u| ef[u]).max();
    let Some(plan_end) = end.or(finish) else {
        return Plan {
            timing: vec![None; graph.len()],
            finish,
            end,
            in_loops,
        };
    };

    // Backward: latest times.
    let mut ls = vec![0i64; n];
    let mut lf = vec![0i64; n];
    for &u in order.iter().rev() {
        let mut latest = plan_end;
        for d in graph.blocks(u) {
            if placed[d.to] {
                latest = latest.min(ls[d.to] - 1 - d.lag_days.max(0));
            }
        }
        lf[u] = latest;
        ls[u] = latest - work[u].days.max(1) + 1;
    }

    let mut timing = vec![None; graph.len()];
    for &u in &order {
        timing[u] = Some(Timing {
            earliest_start: es[u],
            earliest_finish: ef[u],
            latest_start: ls[u],
            latest_finish: lf[u],
        });
    }
    Plan {
        timing,
        finish,
        end: Some(plan_end),
        in_loops,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::schedule::Dependency;

    fn day(s: &str) -> Day {
        s.parse().unwrap()
    }

    fn link(from: usize, to: usize, lag: i64) -> Dependency {
        Dependency {
            from,
            to,
            lag_days: lag,
        }
    }

    fn free(days: i64) -> Work {
        Work {
            days,
            not_before: None,
            included: true,
        }
    }

    /// `(ES, EF, LS, LF)` with the plan starting at working day 1, as
    /// textbooks number them.
    fn times(plan: &Plan) -> Vec<(i64, i64, i64, i64)> {
        plan.timing
            .iter()
            .map(|t| {
                let t = t.unwrap();
                (
                    t.earliest_start,
                    t.earliest_finish,
                    t.latest_start,
                    t.latest_finish,
                )
            })
            .collect()
    }

    #[test]
    fn working_days_skip_the_weekend() {
        let week = WorkWeek::default();
        // Friday 27 November 2026, then Saturday, Sunday and Monday.
        assert!(week.is_working(day("2026-11-27")));
        assert!(!week.is_working(day("2026-11-28")));
        assert!(!week.is_working(day("2026-11-29")));
        assert_eq!(
            week.begin_index(day("2026-11-28")),
            week.begin_index(day("2026-11-30"))
        );
        assert_eq!(
            week.end_index(day("2026-11-29")),
            week.end_index(day("2026-11-27"))
        );
        assert_eq!(
            week.day(week.begin_index(day("2026-11-27")) + 1).unwrap(),
            day("2026-11-30")
        );
        // Monday to the next Friday: ten working days.
        assert_eq!(week.length(day("2026-11-23"), day("2026-12-04")), 10);
        // A weekend only still lasts a day.
        assert_eq!(week.length(day("2026-11-28"), day("2026-11-29")), 1);
        let all = WorkWeek::new(7);
        assert_eq!(all.length(day("2026-11-23"), day("2026-12-04")), 12);
        for d in 0..30 {
            let x = day("2026-11-01").add_days(d).unwrap();
            if week.is_working(x) {
                assert_eq!(week.day(week.begin_index(x)).unwrap(), x);
                assert_eq!(week.begin_index(x), week.end_index(x));
            }
        }
    }

    /// The classic seven-activity network: A(3) → B(4), A → C(2),
    /// B → D(5), C → E(3), D,E → F(2), and G(1) after C alone. The critical
    /// path is A-B-D-F, 14 days long.
    #[test]
    fn a_textbook_network_has_its_known_times_and_path() {
        let work = [
            free(3),
            free(4),
            free(2),
            free(5),
            free(3),
            free(2),
            free(1),
        ];
        let g = Graph::new(
            7,
            [
                link(0, 1, 0),
                link(0, 2, 0),
                link(1, 3, 0),
                link(2, 4, 0),
                link(3, 5, 0),
                link(4, 5, 0),
                link(2, 6, 0),
            ],
        );
        let plan = critical_path(&g, &work, 1, None);
        assert_eq!(plan.finish, Some(14));
        assert_eq!(
            times(&plan),
            vec![
                (1, 3, 1, 3),
                (4, 7, 4, 7),
                (4, 5, 8, 9),
                (8, 12, 8, 12),
                (6, 8, 10, 12),
                (13, 14, 13, 14),
                (6, 6, 14, 14),
            ]
        );
        let slack: Vec<i64> = plan.timing.iter().map(|t| t.unwrap().slack()).collect();
        assert_eq!(slack, vec![0, 0, 4, 0, 4, 0, 8]);
        let critical: Vec<usize> = (0..7)
            .filter(|&i| plan.timing[i].unwrap().critical())
            .collect();
        assert_eq!(critical, vec![0, 1, 3, 5]);
    }

    /// Two parallel chains of 2+2 and 1+1+1 days: the longer is critical,
    /// the other has one day of slack each. A lag of two days on the
    /// shorter chain makes it the longer one.
    #[test]
    fn parallel_chains_and_lags() {
        let work = [free(2), free(2), free(1), free(1), free(1)];
        let plain = [link(0, 1, 0), link(2, 3, 0), link(3, 4, 0)];
        let plan = critical_path(&Graph::new(5, plain), &work, 0, None);
        let slack: Vec<i64> = plan.timing.iter().map(|t| t.unwrap().slack()).collect();
        assert_eq!(slack, vec![0, 0, 1, 1, 1]);
        assert_eq!(plan.finish, Some(3));

        let lagged = [link(0, 1, 0), link(2, 3, 2), link(3, 4, 0)];
        let plan = critical_path(&Graph::new(5, lagged), &work, 0, None);
        let slack: Vec<i64> = plan.timing.iter().map(|t| t.unwrap().slack()).collect();
        assert_eq!(slack, vec![1, 1, 0, 0, 0]);
        assert_eq!(plan.finish, Some(4));
        let t3 = plan.timing[3].unwrap();
        assert_eq!((t3.earliest_start, t3.earliest_finish), (3, 3));
    }

    #[test]
    fn an_end_target_gives_slack_or_takes_it_away() {
        let work = [free(2), free(3)];
        let g = Graph::new(2, [link(0, 1, 0)]);
        let later = critical_path(&g, &work, 0, Some(9));
        assert_eq!(later.timing[0].unwrap().slack(), 5);
        assert!(!later.timing[1].unwrap().critical());
        let sooner = critical_path(&g, &work, 0, Some(2));
        assert_eq!(sooner.timing[1].unwrap().slack(), -2);
        assert!(sooner.timing[0].unwrap().critical());
    }

    #[test]
    fn own_start_dates_hold_tasks_back_and_excluded_tasks_drop_out() {
        let mut work = [free(1), free(1), free(1)];
        work[1].not_before = Some(5);
        work[2].included = false;
        let g = Graph::new(3, [link(0, 1, 0), link(2, 1, 0)]);
        let plan = critical_path(&g, &work, 0, None);
        assert!(plan.timing[2].is_none());
        let t1 = plan.timing[1].unwrap();
        assert_eq!((t1.earliest_start, t1.earliest_finish), (5, 5));
        assert_eq!(plan.timing[0].unwrap().slack(), 4);
        assert!(t1.critical());
    }

    #[test]
    fn loops_are_left_out() {
        let work = [free(1), free(1), free(1)];
        let g = Graph::new(3, [link(0, 1, 0), link(1, 2, 0), link(2, 1, 0)]);
        let plan = critical_path(&g, &work, 0, None);
        assert_eq!(plan.in_loops, vec![1, 2]);
        assert!(plan.timing[0].is_some());
        assert!(plan.timing[1].is_none());
    }

    #[test]
    fn twenty_thousand_tasks_are_quick() {
        let n = 20_000;
        let work: Vec<Work> = (0..n).map(|i| free(1 + (i % 5) as i64)).collect();
        let deps = (0..n - 7).map(|i| link(i, i + 7, (i % 3) as i64));
        let g = Graph::new(n, deps);
        let at = std::time::Instant::now();
        let plan = critical_path(&g, &work, 0, None);
        let took = at.elapsed();
        assert!(plan.finish.is_some());
        assert!(took.as_millis() < 200, "{took:?}");
    }
}
