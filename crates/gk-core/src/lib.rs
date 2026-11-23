// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Value types shared by every other crate: task keys, ranks, durations,
//! dates, priorities and status categories, with their validation and the
//! values derived from them (progress, roll-ups, "blocked", "overdue").
//!
//! This crate does no I/O: no files, no database, no clock. Callers pass the
//! current time in where a rule needs it.

pub mod actor;
pub mod day;
pub mod duration;
pub mod id;
pub mod key;
pub mod kinds;
pub mod rank;
pub mod status;
pub mod text;

pub use actor::{ActorKind, Via};
pub use day::{Day, DayError, Weekday, check_date_range};
pub use duration::{DurationError, DurationUnits, format_duration, format_minutes, parse_minutes};
pub use id::{Id, IdError};
pub use key::{KeyError, ProjectKey, TaskKey};
pub use kinds::{Glyph, Hue, KindError, Priority};
pub use rank::{Rank, RankError};
pub use status::{DEFAULT_STATUSES, DEFAULT_TASK_TYPES, StatusCategory, closed_at_after_move};
pub use text::{
    TextError, check_comment, check_line, check_markdown, check_title, clean_line, fold,
};
