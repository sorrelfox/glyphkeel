// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Schedule rules for the Gantt chart, on whole days.
//!
//! A task's dates read as a range: it begins on its start date (or its due
//! date when it has no start) and ends on its due date (or its start date
//! when it has no due date). Both days count, so a task due on Monday is
//! still running on Monday.
//!
//! A `blocks` link is a finish-to-start dependency: the blocked task may
//! begin on the day after its blocker ends at the earliest, plus the link's
//! lag in days. Days are calendar days; there are no working-day calendars.
//!
//! Tasks are numbered `0..n` here; callers map their own IDs to the
//! numbers. Nothing in this module reads a clock.

use std::collections::VecDeque;

use crate::day::{Day, DayError};

/// A task's own dates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dates {
    /// The start date.
    pub start: Option<Day>,
    /// The due date.
    pub due: Option<Day>,
}

impl Dates {
    /// Dates from a start and a due date.
    #[must_use]
    pub const fn new(start: Option<Day>, due: Option<Day>) -> Self {
        Self { start, due }
    }

    /// The first day of the range: the start, else the due date.
    #[must_use]
    pub fn begin(&self) -> Option<Day> {
        self.start.or(self.due)
    }

    /// The last day of the range: the due date, else the start.
    #[must_use]
    pub fn end(&self) -> Option<Day> {
        self.due.or(self.start)
    }

    /// Whether the task has any date.
    #[must_use]
    pub fn is_dated(&self) -> bool {
        self.start.is_some() || self.due.is_some()
    }

    /// Both dates moved by `days`; a missing date stays missing.
    pub fn shifted(&self, days: i64) -> Result<Self, DayError> {
        Ok(Self {
            start: self.start.map(|d| d.add_days(days)).transpose()?,
            due: self.due.map(|d| d.add_days(days)).transpose()?,
        })
    }
}

/// A finish-to-start dependency between two numbered tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dependency {
    /// The blocking task.
    pub from: usize,
    /// The blocked task.
    pub to: usize,
    /// Extra days between the blocker's end and the blocked task's
    /// beginning.
    pub lag_days: i64,
}

/// The first day a task blocked by a task ending on `blocker_end` may
/// begin, with `lag_days` in between.
pub fn earliest_begin(blocker_end: Day, lag_days: i64) -> Result<Day, DayError> {
    blocker_end.add_days(1 + lag_days.max(0))
}

/// When the blocked task begins too early: the day it begins and the day
/// its blocker ends. `None` when it doesn't, or when either has no dates.
#[must_use]
pub fn starts_too_early(blocker: &Dates, blocked: &Dates, lag_days: i64) -> Option<(Day, Day)> {
    let end = blocker.end()?;
    let begin = blocked.begin()?;
    let earliest = earliest_begin(end, lag_days).ok()?;
    (begin < earliest).then_some((begin, end))
}

/// The range a parent's summary bar spans: from the earliest beginning to
/// the latest end among the parent and its descendants. `None` when none
/// of them has a date.
#[must_use]
pub fn summary<'a>(dates: impl IntoIterator<Item = &'a Dates>) -> Option<(Day, Day)> {
    let mut out: Option<(Day, Day)> = None;
    for d in dates {
        let (Some(b), Some(e)) = (d.begin(), d.end()) else {
            continue;
        };
        out = Some(match out {
            Some((lo, hi)) => (lo.min(b), hi.max(e)),
            None => (b, e),
        });
    }
    out
}

/// The links of `n` tasks, by task, both ways.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    /// For each task, the indexes into `deps` of the links it blocks.
    out: Vec<Vec<usize>>,
    /// For each task, the indexes into `deps` of the links that block it.
    into: Vec<Vec<usize>>,
    deps: Vec<Dependency>,
}

impl Graph {
    /// The graph of `n` tasks with these links. Links naming a task `>= n`
    /// are left out.
    #[must_use]
    pub fn new(n: usize, deps: impl IntoIterator<Item = Dependency>) -> Self {
        let mut g = Self {
            out: vec![Vec::new(); n],
            into: vec![Vec::new(); n],
            deps: Vec::new(),
        };
        for d in deps {
            if d.from < n && d.to < n {
                let i = g.deps.len();
                g.out[d.from].push(i);
                g.into[d.to].push(i);
                g.deps.push(d);
            }
        }
        g
    }

    /// How many tasks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.out.len()
    }

    /// Whether there are no tasks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
    }

    /// Every link.
    #[must_use]
    pub fn dependencies(&self) -> &[Dependency] {
        &self.deps
    }

    /// The links task `i` blocks.
    pub fn blocks(&self, i: usize) -> impl Iterator<Item = &Dependency> {
        self.out
            .get(i)
            .into_iter()
            .flatten()
            .map(|&d| &self.deps[d])
    }

    /// The links that block task `i`.
    pub fn blocked_by(&self, i: usize) -> impl Iterator<Item = &Dependency> {
        self.into
            .get(i)
            .into_iter()
            .flatten()
            .map(|&d| &self.deps[d])
    }

    /// The tasks that depend on `i`, directly or through others, without
    /// `i` itself, in breadth-first order.
    #[must_use]
    pub fn dependents(&self, i: usize) -> Vec<usize> {
        let mut seen = vec![false; self.len()];
        let mut out = Vec::new();
        let mut queue = VecDeque::new();
        if i < self.len() {
            seen[i] = true;
            queue.push_back(i);
        }
        while let Some(u) = queue.pop_front() {
            for d in self.blocks(u) {
                if !seen[d.to] {
                    seen[d.to] = true;
                    out.push(d.to);
                    queue.push_back(d.to);
                }
            }
        }
        out
    }

    /// The loops of links: every group of two or more tasks that block
    /// each other through a chain of links, each group sorted, the groups
    /// ordered by their first task.
    #[must_use]
    pub fn loops(&self) -> Vec<Vec<usize>> {
        // Tarjan's strongly connected components, without recursion.
        let n = self.len();
        let mut index = vec![usize::MAX; n];
        let mut low = vec![0; n];
        let mut on_stack = vec![false; n];
        let mut stack = Vec::new();
        let mut next = 0;
        let mut out = Vec::new();
        for root in 0..n {
            if index[root] != usize::MAX {
                continue;
            }
            // (task, position in its list of outgoing links)
            let mut work = vec![(root, 0usize)];
            index[root] = next;
            low[root] = next;
            next += 1;
            stack.push(root);
            on_stack[root] = true;
            while let Some(&mut (u, ref mut pos)) = work.last_mut() {
                if let Some(&d) = self.out[u].get(*pos) {
                    *pos += 1;
                    let v = self.deps[d].to;
                    if index[v] == usize::MAX {
                        index[v] = next;
                        low[v] = next;
                        next += 1;
                        stack.push(v);
                        on_stack[v] = true;
                        work.push((v, 0));
                    } else if on_stack[v] {
                        low[u] = low[u].min(index[v]);
                    }
                    continue;
                }
                work.pop();
                if let Some(&(parent, _)) = work.last() {
                    low[parent] = low[parent].min(low[u]);
                }
                if low[u] == index[u] {
                    let mut group = Vec::new();
                    while let Some(v) = stack.pop() {
                        on_stack[v] = false;
                        group.push(v);
                        if v == u {
                            break;
                        }
                    }
                    if group.len() > 1 {
                        group.sort_unstable();
                        out.push(group);
                    }
                }
            }
        }
        out.sort();
        out
    }
}

/// Why dependents can't be moved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShiftError {
    /// The tasks that depend on the moved one block each other in a loop.
    #[error("the tasks that depend on it block each other in a loop")]
    Loop {
        /// The tasks of the loop.
        tasks: Vec<usize>,
    },
    /// A date would leave the calendar's range.
    #[error(transparent)]
    Day(#[from] DayError),
}

/// One task moved by [`shift_dependents`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shift {
    /// The task.
    pub task: usize,
    /// By how many days it moved later.
    pub days: i64,
    /// Its dates after the move.
    pub dates: Dates,
}

/// Which tasks move, and by how much, so that every task depending on
/// `root` (directly or through others) still begins after its blockers end,
/// once `root` has the dates in `dates[root]`.
///
/// Only blockers that are `root` or moved themselves are looked at: a
/// conflict with a blocker that didn't move is left alone. A task moves
/// later by as many days as it must, both dates together, so its length
/// stays; nothing moves earlier. Tasks for which `movable` is false keep
/// their dates (and their dependents are pushed by those dates). Tasks
/// without dates have nothing to move.
///
/// The result is in the order the moves were worked out: blockers before
/// the tasks they block.
pub fn shift_dependents(
    graph: &Graph,
    dates: &[Dates],
    root: usize,
    movable: &[bool],
) -> Result<Vec<Shift>, ShiftError> {
    let n = graph.len();
    if root >= n || dates.len() < n {
        return Ok(Vec::new());
    }
    let reach = graph.dependents(root);
    let mut inside = vec![false; n];
    inside[root] = true;
    for &t in &reach {
        inside[t] = true;
    }
    // Kahn's order over the tasks reached, counting only links among them.
    let mut waiting = vec![0usize; n];
    for &t in &reach {
        waiting[t] = graph.blocked_by(t).filter(|d| inside[d.from]).count();
    }
    let mut ready = VecDeque::from([root]);
    let mut order = Vec::with_capacity(reach.len() + 1);
    // A link back to `root` means `root` is in a loop with its dependents.
    let mut back_to_root = false;
    while let Some(u) = ready.pop_front() {
        order.push(u);
        for d in graph.blocks(u) {
            if d.to == root {
                back_to_root = true;
            } else if inside[d.to] {
                waiting[d.to] -= 1;
                if waiting[d.to] == 0 {
                    ready.push_back(d.to);
                }
            }
        }
    }
    if back_to_root || order.len() <= reach.len() {
        // Tasks after a loop wait too; name only the loop's own.
        let mut tasks: Vec<usize> = graph
            .loops()
            .into_iter()
            .filter(|group| group.iter().any(|&t| inside[t]))
            .flatten()
            .collect();
        tasks.sort_unstable();
        return Err(ShiftError::Loop { tasks });
    }
    let mut now: Vec<Dates> = dates[..n].to_vec();
    let mut changed = vec![false; n];
    changed[root] = true;
    let mut out = Vec::new();
    for &u in order.iter().skip(1) {
        let mine = now[u];
        let Some(begin) = mine.begin() else {
            continue;
        };
        if !movable.get(u).copied().unwrap_or(false) {
            continue;
        }
        let mut need: Option<Day> = None;
        for d in graph.blocked_by(u).filter(|d| changed[d.from]) {
            if let Some(end) = now[d.from].end() {
                let earliest = earliest_begin(end, d.lag_days)?;
                need = Some(need.map_or(earliest, |n| n.max(earliest)));
            }
        }
        let Some(need) = need else {
            continue;
        };
        if begin >= need {
            continue;
        }
        let days = need.days_since_epoch() - begin.days_since_epoch();
        let moved = mine.shifted(days)?;
        now[u] = moved;
        changed[u] = true;
        out.push(Shift {
            task: u,
            days,
            dates: moved,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> Day {
        s.parse().unwrap_or_else(|_| panic!("{s}"))
    }

    fn dates(start: Option<&str>, due: Option<&str>) -> Dates {
        Dates::new(start.map(day), due.map(day))
    }

    fn dep(from: usize, to: usize) -> Dependency {
        Dependency {
            from,
            to,
            lag_days: 0,
        }
    }

    #[test]
    fn ranges_fall_back_to_the_other_date() {
        let only_due = dates(None, Some("2026-12-04"));
        assert_eq!(only_due.begin(), Some(day("2026-12-04")));
        assert_eq!(only_due.end(), Some(day("2026-12-04")));
        let only_start = dates(Some("2026-12-01"), None);
        assert_eq!(only_start.end(), Some(day("2026-12-01")));
        assert!(!Dates::default().is_dated());
    }

    #[test]
    fn a_blocked_task_may_begin_the_day_after_its_blocker_ends() {
        let blocker = dates(Some("2026-12-01"), Some("2026-12-04"));
        let same_day = dates(Some("2026-12-04"), Some("2026-12-08"));
        let next_day = dates(Some("2026-12-05"), Some("2026-12-08"));
        assert_eq!(
            starts_too_early(&blocker, &same_day, 0),
            Some((day("2026-12-04"), day("2026-12-04")))
        );
        assert_eq!(starts_too_early(&blocker, &next_day, 0), None);
        assert!(starts_too_early(&blocker, &next_day, 2).is_some());
        assert_eq!(starts_too_early(&blocker, &Dates::default(), 0), None);
    }

    #[test]
    fn summaries_span_every_dated_task() {
        let all = [
            Dates::default(),
            dates(Some("2026-12-03"), Some("2026-12-05")),
            dates(None, Some("2026-12-20")),
            dates(Some("2026-11-30"), None),
        ];
        assert_eq!(summary(&all), Some((day("2026-11-30"), day("2026-12-20"))));
        assert_eq!(summary(&[Dates::default()]), None);
    }

    #[test]
    fn loops_are_found_and_chains_are_not() {
        let g = Graph::new(6, [dep(0, 1), dep(1, 2), dep(2, 0), dep(3, 4), dep(4, 5)]);
        assert_eq!(g.loops(), vec![vec![0, 1, 2]]);
        let two = Graph::new(4, [dep(3, 1), dep(1, 3), dep(0, 2), dep(2, 0)]);
        assert_eq!(two.loops(), vec![vec![0, 2], vec![1, 3]]);
        assert!(
            Graph::new(3, [dep(0, 1), dep(0, 2), dep(1, 2)])
                .loops()
                .is_empty()
        );
    }

    #[test]
    fn dependents_are_found_through_others_once_each() {
        let g = Graph::new(5, [dep(0, 1), dep(0, 2), dep(1, 3), dep(2, 3), dep(4, 0)]);
        assert_eq!(g.dependents(0), vec![1, 2, 3]);
        assert_eq!(g.dependents(3), Vec::<usize>::new());
        assert_eq!(g.dependents(4).len(), 4);
    }

    #[test]
    fn dependents_move_later_only_as_far_as_they_must() {
        // 0 now ends on the 10th; 1 began on the 8th and lasts 3 days; 2
        // follows 1 with room to spare; 3 already begins late enough.
        let g = Graph::new(4, [dep(0, 1), dep(1, 2), dep(0, 3)]);
        let d = [
            dates(Some("2026-12-01"), Some("2026-12-10")),
            dates(Some("2026-12-08"), Some("2026-12-10")),
            dates(Some("2026-12-12"), Some("2026-12-12")),
            dates(Some("2026-12-15"), None),
        ];
        let moves = shift_dependents(&g, &d, 0, &[true; 4]).unwrap();
        assert_eq!(
            moves,
            vec![
                Shift {
                    task: 1,
                    days: 3,
                    dates: dates(Some("2026-12-11"), Some("2026-12-13")),
                },
                Shift {
                    task: 2,
                    days: 2,
                    dates: dates(Some("2026-12-14"), Some("2026-12-14")),
                },
            ]
        );
    }

    #[test]
    fn a_task_waits_for_the_latest_of_its_moved_blockers() {
        // 0 → 1 → 3 and 0 → 2 → 3: 3 must wait for whichever of 1 and 2
        // ends later after both moved.
        let g = Graph::new(4, [dep(0, 1), dep(0, 2), dep(1, 3), dep(2, 3)]);
        let d = [
            dates(None, Some("2026-12-10")),
            dates(Some("2026-12-05"), Some("2026-12-06")),
            dates(Some("2026-12-05"), Some("2026-12-12")),
            dates(Some("2026-12-13"), Some("2026-12-14")),
        ];
        let moves = shift_dependents(&g, &d, 0, &[true; 4]).unwrap();
        let three = moves.iter().find(|m| m.task == 3).unwrap();
        assert_eq!(three.dates, dates(Some("2026-12-19"), Some("2026-12-20")));
    }

    #[test]
    fn fixed_and_undated_tasks_stay_and_lag_counts() {
        let g = Graph::new(
            4,
            [
                Dependency {
                    from: 0,
                    to: 1,
                    lag_days: 2,
                },
                dep(1, 2),
                dep(0, 3),
            ],
        );
        let d = [
            dates(None, Some("2026-12-10")),
            dates(Some("2026-12-01"), Some("2026-12-02")),
            Dates::default(),
            dates(Some("2026-12-01"), None),
        ];
        let moves = shift_dependents(&g, &d, 0, &[true, true, true, false]).unwrap();
        assert_eq!(
            moves,
            vec![Shift {
                task: 1,
                days: 12,
                dates: dates(Some("2026-12-13"), Some("2026-12-14")),
            }]
        );
    }

    #[test]
    fn a_loop_among_the_dependents_is_refused() {
        let g = Graph::new(4, [dep(0, 1), dep(1, 2), dep(2, 1), dep(2, 3)]);
        let d = [dates(None, Some("2026-12-10")); 4];
        assert_eq!(
            shift_dependents(&g, &d, 0, &[true; 4]),
            Err(ShiftError::Loop { tasks: vec![1, 2] })
        );
        // A loop elsewhere doesn't matter.
        let g = Graph::new(4, [dep(0, 1), dep(2, 3), dep(3, 2)]);
        assert!(shift_dependents(&g, &d, 0, &[true; 4]).is_ok());
    }

    #[test]
    fn a_loop_through_the_moved_task_is_refused() {
        let d = [dates(None, Some("2026-12-10")); 4];
        let g = Graph::new(4, [dep(0, 1), dep(1, 2), dep(2, 0), dep(0, 3)]);
        assert_eq!(
            shift_dependents(&g, &d, 0, &[true; 4]),
            Err(ShiftError::Loop {
                tasks: vec![0, 1, 2]
            })
        );
        // Two tasks blocking each other directly.
        let g = Graph::new(2, [dep(0, 1), dep(1, 0)]);
        assert_eq!(
            shift_dependents(&g, &d, 1, &[true; 2]),
            Err(ShiftError::Loop { tasks: vec![0, 1] })
        );
    }

    #[test]
    fn five_hundred_tasks_with_three_hundred_links_are_quick() {
        // A long chain of 300 links through 500 tasks: every task moves.
        let n = 500;
        let deps: Vec<Dependency> = (0..300).map(|i| dep(i, i + 1)).collect();
        let g = Graph::new(n, deps);
        let start = day("2026-01-01");
        let d: Vec<Dates> = (0..n)
            .map(|i| {
                let s = start.add_days(i as i64).unwrap();
                Dates::new(Some(s), Some(s))
            })
            .collect();
        let mut first = d.clone();
        first[0] = first[0].shifted(30).unwrap();
        let at = std::time::Instant::now();
        let moves = shift_dependents(&g, &first, 0, &vec![true; n]).unwrap();
        assert_eq!(moves.len(), 300);
        assert!(g.loops().is_empty());
        let total: usize = (0..n).map(|i| g.dependents(i).len()).sum();
        assert!(total > 0);
        assert!(at.elapsed().as_millis() < 500, "{:?}", at.elapsed());
    }
}
