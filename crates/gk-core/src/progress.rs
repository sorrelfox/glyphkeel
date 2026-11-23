// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! A task's progress, from its own values and its subtasks'.
//!
//! The rule, in order: progress set by hand; 100 % once the task is closed;
//! the share of ticked checklist items; the subtasks' progress; else 0 %.
//!
//! Subtasks are weighted by their estimates, so a two-minute subtask doesn't
//! count as much as a two-day one. Subtasks without an estimate get the
//! average weight of those with one (all weigh the same when none has one),
//! and a closed subtask counts as 100 %.

/// What a parent needs of one subtask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubtaskProgress {
    /// The subtask's own progress, 0 to 100.
    pub percent: u8,
    /// Its estimate in minutes, if it has one.
    pub estimate_minutes: Option<i64>,
    /// Whether it is closed.
    pub closed: bool,
}

/// `part` of `whole` in percent, rounded to the nearest whole number.
fn percent(part: u128, whole: u128) -> u8 {
    if whole == 0 {
        return 0;
    }
    let p = (part * 200 + whole) / (whole * 2);
    u8::try_from(p.min(100)).unwrap_or(100)
}

/// The progress of a task.
///
/// ```
/// use gk_core::progress::{SubtaskProgress, progress};
///
/// assert_eq!(progress(Some(40), true, (0, 0), &[]), 40);
/// assert_eq!(progress(None, true, (1, 4), &[]), 100);
/// assert_eq!(progress(None, false, (3, 7), &[]), 43);
/// let small = SubtaskProgress { percent: 0, estimate_minutes: Some(2), closed: false };
/// let big = SubtaskProgress { percent: 0, estimate_minutes: Some(960), closed: true };
/// assert_eq!(progress(None, false, (0, 0), &[small, big]), 100);
/// ```
#[must_use]
pub fn progress(
    set_by_hand: Option<u8>,
    closed: bool,
    checklist: (u32, u32),
    subtasks: &[SubtaskProgress],
) -> u8 {
    if let Some(p) = set_by_hand {
        return p.min(100);
    }
    if closed {
        return 100;
    }
    let (done, total) = checklist;
    if total > 0 {
        return percent(u128::from(done.min(total)), u128::from(total));
    }
    subtask_progress(subtasks).unwrap_or(0)
}

/// The subtasks' progress weighted by estimate; `None` without subtasks.
#[must_use]
pub fn subtask_progress(subtasks: &[SubtaskProgress]) -> Option<u8> {
    if subtasks.is_empty() {
        return None;
    }
    let estimated: Vec<u128> = subtasks
        .iter()
        .filter_map(|s| s.estimate_minutes.filter(|m| *m > 0))
        .map(|m| u128::try_from(m).unwrap_or(0))
        .collect();
    // Weights are scaled by the number of estimated subtasks, so the
    // average weight stays a whole number.
    let n = u128::try_from(estimated.len()).unwrap_or(1).max(1);
    let average = if estimated.is_empty() {
        1
    } else {
        estimated.iter().sum::<u128>()
    };
    let (mut weighted, mut weights) = (0u128, 0u128);
    for s in subtasks {
        let weight = match s.estimate_minutes.filter(|m| *m > 0) {
            Some(m) if !estimated.is_empty() => u128::try_from(m).unwrap_or(0) * n,
            _ => average,
        };
        let p = if s.closed { 100 } else { s.percent.min(100) };
        weighted += weight * u128::from(p);
        weights += weight;
    }
    Some(percent(weighted, weights * 100))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(percent: u8, estimate: Option<i64>, closed: bool) -> SubtaskProgress {
        SubtaskProgress {
            percent,
            estimate_minutes: estimate,
            closed,
        }
    }

    #[test]
    fn the_rule_goes_in_order() {
        assert_eq!(progress(Some(150), false, (1, 1), &[]), 100);
        assert_eq!(progress(None, false, (0, 0), &[]), 0);
        assert_eq!(progress(None, false, (2, 4), &[sub(0, None, true)]), 50);
        assert_eq!(progress(None, false, (0, 0), &[sub(0, None, true)]), 100);
    }

    #[test]
    fn subtasks_are_weighted_by_estimate() {
        // 1 h done, 3 h at 0 %: 25 %.
        let s = [sub(0, Some(60), true), sub(0, Some(180), false)];
        assert_eq!(subtask_progress(&s), Some(25));
        // Without estimates every subtask weighs the same.
        let s = [sub(50, None, false), sub(0, None, false)];
        assert_eq!(subtask_progress(&s), Some(25));
        // An unestimated subtask gets the average weight (2 h here).
        let s = [
            sub(0, Some(60), false),
            sub(0, Some(180), false),
            sub(100, None, false),
        ];
        assert_eq!(subtask_progress(&s), Some(33));
        assert_eq!(subtask_progress(&[]), None);
    }
}
