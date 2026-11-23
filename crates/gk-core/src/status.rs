// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Status categories, the statuses and task types a new project starts
//! with, and the rule for a task's closing time.
//!
//! Every status (a board column) belongs to one project and has one of four
//! categories. The category, not the status name, decides whether a task is
//! open or closed.

use std::fmt;
use std::str::FromStr;

use crate::kinds::{Glyph, Hue, KindError};

/// What a status means for the tasks in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatusCategory {
    /// Not started.
    Todo,
    /// Being worked on.
    InProgress,
    /// Finished.
    Done,
    /// Will not be done.
    Cancelled,
}

impl StatusCategory {
    /// Every category, in workflow order.
    pub const ALL: [Self; 4] = [Self::Todo, Self::InProgress, Self::Done, Self::Cancelled];

    /// The stored name: `todo`, `in_progress`, `done` or `cancelled`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether tasks in this category count as closed.
    #[must_use]
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }
}

impl FromStr for StatusCategory {
    type Err = KindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|c| c.as_str() == s)
            .ok_or_else(|| KindError::new("status category", s, &Self::ALL.map(Self::as_str)))
    }
}

impl fmt::Display for StatusCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A status that new projects start with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultStatus {
    /// Its name.
    pub name: &'static str,
    /// Its category.
    pub category: StatusCategory,
    /// Its colour.
    pub hue: Hue,
}

/// The statuses of a new top-level project, in board order. Sub-projects
/// copy their parent's statuses instead.
pub const DEFAULT_STATUSES: [DefaultStatus; 5] = [
    DefaultStatus {
        name: "Backlog",
        category: StatusCategory::Todo,
        hue: Hue::Grey,
    },
    DefaultStatus {
        name: "To do",
        category: StatusCategory::Todo,
        hue: Hue::Blue,
    },
    DefaultStatus {
        name: "In progress",
        category: StatusCategory::InProgress,
        hue: Hue::Orange,
    },
    DefaultStatus {
        name: "Review",
        category: StatusCategory::InProgress,
        hue: Hue::Purple,
    },
    DefaultStatus {
        name: "Done",
        category: StatusCategory::Done,
        hue: Hue::Green,
    },
];

/// A task type that new projects start with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultTaskType {
    /// Its name.
    pub name: &'static str,
    /// Its shape on cards and in lists.
    pub glyph: Glyph,
    /// Its colour.
    pub hue: Hue,
    /// Whether new tasks get this type unless told otherwise.
    pub is_default: bool,
}

/// The task types of a new top-level project, in order.
pub const DEFAULT_TASK_TYPES: [DefaultTaskType; 4] = [
    DefaultTaskType {
        name: "Task",
        glyph: Glyph::Task,
        hue: Hue::Blue,
        is_default: true,
    },
    DefaultTaskType {
        name: "Bug",
        glyph: Glyph::Bug,
        hue: Hue::Red,
        is_default: false,
    },
    DefaultTaskType {
        name: "Feature",
        glyph: Glyph::Feature,
        hue: Hue::Green,
        is_default: false,
    },
    DefaultTaskType {
        name: "Milestone",
        glyph: Glyph::Milestone,
        hue: Hue::Purple,
        is_default: false,
    },
];

/// A task's closing time after it moves from a status of category `from` to
/// one of category `to` at `now` (milliseconds since 1970 UTC).
///
/// Entering a closed category sets it; leaving one clears it; moving between
/// two closed categories keeps the original time.
#[must_use]
pub fn closed_at_after_move(
    from: StatusCategory,
    to: StatusCategory,
    closed_at: Option<i64>,
    now: i64,
) -> Option<i64> {
    match (from.is_closed(), to.is_closed()) {
        (_, false) => None,
        (false, true) => Some(now),
        (true, true) => closed_at.or(Some(now)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_projects_get_the_five_default_statuses() {
        let names: Vec<_> = DEFAULT_STATUSES.iter().map(|s| s.name).collect();
        assert_eq!(names, ["Backlog", "To do", "In progress", "Review", "Done"]);
        let categories: Vec<_> = DEFAULT_STATUSES.iter().map(|s| s.category).collect();
        assert_eq!(
            categories,
            [
                StatusCategory::Todo,
                StatusCategory::Todo,
                StatusCategory::InProgress,
                StatusCategory::InProgress,
                StatusCategory::Done
            ]
        );
    }

    #[test]
    fn new_projects_get_four_types_with_one_default() {
        assert_eq!(
            DEFAULT_TASK_TYPES.iter().filter(|t| t.is_default).count(),
            1
        );
        assert_eq!(DEFAULT_TASK_TYPES[0].name, "Task");
        assert!(DEFAULT_TASK_TYPES[0].is_default);
    }

    #[test]
    fn categories_round_trip_and_classify() {
        for c in StatusCategory::ALL {
            assert_eq!(c.as_str().parse::<StatusCategory>(), Ok(c));
        }
        assert!("Done".parse::<StatusCategory>().is_err());
        assert!(StatusCategory::Done.is_closed());
        assert!(StatusCategory::Cancelled.is_closed());
        assert!(!StatusCategory::Todo.is_closed());
        assert!(!StatusCategory::InProgress.is_closed());
    }

    #[test]
    fn closing_time() {
        use StatusCategory::*;
        assert_eq!(closed_at_after_move(Todo, Done, None, 5), Some(5));
        assert_eq!(
            closed_at_after_move(InProgress, Cancelled, None, 5),
            Some(5)
        );
        assert_eq!(closed_at_after_move(Done, Todo, Some(3), 5), None);
        assert_eq!(closed_at_after_move(Done, Cancelled, Some(3), 5), Some(3));
        assert_eq!(closed_at_after_move(Todo, InProgress, None, 5), None);
        assert_eq!(closed_at_after_move(Done, Done, None, 5), Some(5));
    }
}
