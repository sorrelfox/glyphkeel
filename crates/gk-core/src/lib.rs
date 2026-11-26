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
pub mod file;
pub mod id;
pub mod key;
pub mod kinds;
pub mod link;
pub mod merge;
pub mod message;
pub mod palette;
pub mod progress;
pub mod rank;
pub mod schedule;
pub mod status;
pub mod text;
pub mod wiki;
pub mod worklog;

pub use actor::{ActorKind, Via};
pub use day::{Day, DayError, Weekday, check_date_range};
pub use duration::{DurationError, DurationUnits, format_duration, format_minutes, parse_minutes};
pub use file::{
    DEFAULT_FILE_MAX_MB, FILE_NAME_MAX_BYTES, FileError, FileKind, base64_decode, base64_encode,
    clean_file_name, hex, image_size, kind_of, sniff,
};
pub use id::{Id, IdError};
pub use key::{KeyError, ProjectKey, TaskKey};
pub use kinds::{Glyph, Hue, KindError, Priority, ProjectIcon};
pub use link::{LinkKind, Relation};
pub use merge::{Edit, EditError, Merge, apply_edits, merge3, unified_diff};
pub use message::Message;
pub use palette::{PALETTE, suggest_hue};
pub use progress::{SubtaskProgress, progress, subtask_progress};
pub use rank::{Rank, RankError};
pub use schedule::{Dates, Dependency, Graph, Shift, ShiftError, shift_dependents};
pub use status::{DEFAULT_STATUSES, DEFAULT_TASK_TYPES, StatusCategory, closed_at_after_move};
pub use text::{
    TextError, check_comment, check_line, check_markdown, check_title, clean_line, fold,
};
pub use wiki::{PAGE_TITLE_MAX_CHARS, PageTitleError, check_page_title, slug};
