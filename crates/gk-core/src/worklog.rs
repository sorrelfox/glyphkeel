// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Rules for logged time, the timer and reminders.
//!
//! - A time entry is 1 minute to 24 hours of work on one day. Zero makes
//!   no sense without billing; more than a day on one day can't be right.
//! - Durations are typed as everywhere else (`90m`, `1.5h`, `1:30`), and a
//!   leading `+` is allowed for quick entries (`+30m`).
//! - The timer logs its time rounded to the nearest minute; less than half
//!   a minute logs nothing.
//! - A deadline alert fires the day before the due date, at the reminder
//!   time; a reminder that follows the due date fires a number of days
//!   before it.
//! - Days are typed as `YYYY-MM-DD`, `today` or `yesterday`.

use crate::day::{Day, DayError, Weekday};
use crate::duration::{DurationError, DurationUnits, parse_minutes};

/// The longest time entry, in minutes: one day.
pub const ENTRY_MAX_MINUTES: i64 = 24 * 60;
/// The longest note on a time entry or the timer, in characters.
pub const ENTRY_NOTE_MAX_CHARS: usize = 2000;
/// The longest text of a reminder, in characters.
pub const REMINDER_NOTE_MAX_CHARS: usize = 500;
/// The most days a reminder can come before the due date.
pub const REMINDER_MAX_DAYS_BEFORE: u32 = 365;

/// Why logged time was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorklogError {
    /// Not a duration.
    #[error(transparent)]
    Duration(#[from] DurationError),
    /// Zero or less.
    #[error("log at least one minute")]
    Zero,
    /// Over a day.
    #[error("{0} minutes is more than a day; log the time on each day it was done")]
    OverADay(i64),
    /// A note over the limit.
    #[error("the note is {len} characters long; the limit is {max}")]
    NoteTooLong {
        /// Its length.
        len: usize,
        /// The limit.
        max: usize,
    },
    /// Not a day.
    #[error(transparent)]
    Day(#[from] DayError),
    /// Too many days before the due date.
    #[error("a reminder can come at most {REMINDER_MAX_DAYS_BEFORE} days before the due date")]
    TooEarly,
}

/// The minutes of a time entry typed as `text`: a duration in the
/// workspace's units, with an optional leading `+`, from 1 minute to a day.
pub fn entry_minutes(text: &str, units: DurationUnits) -> Result<i64, WorklogError> {
    let text = text.trim();
    let text = text.strip_prefix('+').unwrap_or(text);
    check_entry_minutes(parse_minutes(text, units)?)
}

/// Checks the minutes of a time entry: 1 minute to a day.
pub fn check_entry_minutes(minutes: i64) -> Result<i64, WorklogError> {
    if minutes <= 0 {
        Err(WorklogError::Zero)
    } else if minutes > ENTRY_MAX_MINUTES {
        Err(WorklogError::OverADay(minutes))
    } else {
        Ok(minutes)
    }
}

/// The note of a time entry, trimmed, at most [`ENTRY_NOTE_MAX_CHARS`].
pub fn entry_note(text: &str) -> Result<String, WorklogError> {
    note(text, ENTRY_NOTE_MAX_CHARS)
}

/// The text of a reminder, trimmed, at most [`REMINDER_NOTE_MAX_CHARS`].
pub fn reminder_note(text: &str) -> Result<String, WorklogError> {
    note(text, REMINDER_NOTE_MAX_CHARS)
}

fn note(text: &str, max: usize) -> Result<String, WorklogError> {
    let text = text.trim();
    let len = text.chars().count();
    if len > max {
        Err(WorklogError::NoteTooLong { len, max })
    } else {
        Ok(text.to_owned())
    }
}

/// The minutes a timer logs after `elapsed_ms`: rounded to the nearest
/// minute; 0 (nothing to log) under half a minute.
#[must_use]
pub fn timer_minutes(elapsed_ms: i64) -> i64 {
    (elapsed_ms.max(0) + 30_000) / 60_000
}

/// The day typed as `text`: `YYYY-MM-DD`, `today` or `yesterday`, with
/// `today` the local day.
pub fn parse_work_day(text: &str, today: Day) -> Result<Day, WorklogError> {
    match text.trim().to_ascii_lowercase().as_str() {
        "today" => Ok(today),
        "yesterday" => Ok(today.add_days(-1)?),
        other => Ok(other.parse::<Day>()?),
    }
}

/// The first day of the week that holds `day`, for weeks starting on
/// `start`.
#[must_use]
pub fn week_start(day: Day, start: Weekday) -> Day {
    let index = |w: Weekday| w as i64;
    let back = (index(day.weekday()) - index(start)).rem_euclid(7);
    day.add_days(-back).unwrap_or(day)
}

/// The day a deadline alert for `due` fires: the day before.
#[must_use]
pub fn deadline_alert_day(due: Day) -> Day {
    due.add_days(-1).unwrap_or(due)
}

/// The day a reminder `days_before` the due date `due` fires.
pub fn reminder_day(due: Day, days_before: u32) -> Result<Day, WorklogError> {
    if days_before > REMINDER_MAX_DAYS_BEFORE {
        return Err(WorklogError::TooEarly);
    }
    Ok(due.add_days(-i64::from(days_before))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> Day {
        text.parse().unwrap()
    }

    #[test]
    fn entries_are_a_minute_to_a_day() {
        let units = DurationUnits::default();
        assert_eq!(entry_minutes("+30m", units), Ok(30));
        assert_eq!(entry_minutes(" 1h 30m ", units), Ok(90));
        assert_eq!(entry_minutes("1,5h", units), Ok(90));
        assert_eq!(entry_minutes("3d", units), Ok(1440));
        assert_eq!(entry_minutes("0m", units), Err(WorklogError::Zero));
        assert_eq!(
            entry_minutes("25h", units),
            Err(WorklogError::OverADay(1500))
        );
        assert!(matches!(
            entry_minutes("soon", units),
            Err(WorklogError::Duration(_))
        ));
    }

    #[test]
    fn notes_are_trimmed_and_limited() {
        assert_eq!(entry_note("  fixed it \n").unwrap(), "fixed it");
        assert!(entry_note(&"x".repeat(2001)).is_err());
        assert!(reminder_note(&"é".repeat(500)).is_ok());
        assert!(reminder_note(&"é".repeat(501)).is_err());
    }

    #[test]
    fn the_timer_rounds_to_the_nearest_minute() {
        assert_eq!(timer_minutes(0), 0);
        assert_eq!(timer_minutes(29_999), 0);
        assert_eq!(timer_minutes(30_000), 1);
        assert_eq!(timer_minutes(89_999), 1);
        assert_eq!(timer_minutes(90_000), 2);
        assert_eq!(timer_minutes(-5), 0);
    }

    #[test]
    fn work_days_are_words_or_dates() {
        let today = day("2026-03-01");
        assert_eq!(parse_work_day("today", today).unwrap(), today);
        assert_eq!(
            parse_work_day("Yesterday", today).unwrap(),
            day("2026-02-28")
        );
        assert_eq!(
            parse_work_day("2026-01-31", today).unwrap(),
            day("2026-01-31")
        );
        assert!(parse_work_day("2026-02-30", today).is_err());
    }

    #[test]
    fn weeks_start_on_the_chosen_day() {
        // 2026-11-25 is a Wednesday.
        let wed = day("2026-11-25");
        assert_eq!(week_start(wed, Weekday::Monday), day("2026-11-23"));
        assert_eq!(week_start(wed, Weekday::Sunday), day("2026-11-22"));
        assert_eq!(week_start(wed, Weekday::Saturday), day("2026-11-21"));
        assert_eq!(week_start(wed, Weekday::Wednesday), wed);
    }

    #[test]
    fn deadline_alerts_and_reminders_come_before_the_due_date() {
        assert_eq!(deadline_alert_day(day("2026-03-01")), day("2026-02-28"));
        assert_eq!(
            reminder_day(day("2026-03-01"), 0).unwrap(),
            day("2026-03-01")
        );
        assert_eq!(
            reminder_day(day("2026-03-01"), 2).unwrap(),
            day("2026-02-27")
        );
        assert_eq!(
            reminder_day(day("2026-03-01"), 366),
            Err(WorklogError::TooEarly)
        );
    }
}
