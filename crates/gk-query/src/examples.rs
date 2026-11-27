// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Worked examples of friendly dates ([`crate::parse_natural`]): what each
//! text means on a given day with a given first day of the week.
//!
//! They are this crate's tests, and `cargo xtask gen` writes them, with the
//! day the parser gives today, into the page's generated files. The page's
//! in-memory engine for tests is checked against that file, so it reads
//! dates exactly as the engine does.

use gk_core::{Day, Weekday};

use crate::{FixedCalendar, parse_natural};

/// One example: on `today`, with weeks starting on `week_start`, `text`
/// means `day` (`None`: it is not a date).
#[derive(Debug, Clone, Copy)]
pub struct NaturalExample {
    /// The reference day, `YYYY-MM-DD`.
    pub today: &'static str,
    /// The first day of the week.
    pub week_start: Weekday,
    /// What was typed.
    pub text: &'static str,
    /// The day it means, `YYYY-MM-DD`.
    pub day: Option<&'static str>,
}

impl NaturalExample {
    /// The first day of the week as settings name it: `monday`.
    #[must_use]
    pub const fn week_start_name(&self) -> &'static str {
        match self.week_start {
            Weekday::Monday => "monday",
            Weekday::Tuesday => "tuesday",
            Weekday::Wednesday => "wednesday",
            Weekday::Thursday => "thursday",
            Weekday::Friday => "friday",
            Weekday::Saturday => "saturday",
            Weekday::Sunday => "sunday",
        }
    }

    /// The day the parser gives for the text now, which may differ from
    /// [`Self::day`] only while the parser and its examples disagree.
    #[must_use]
    pub fn read(&self) -> Option<Day> {
        let cal = FixedCalendar {
            today: self.today.parse().ok()?,
            offset_minutes: 0,
            week_start: self.week_start,
        };
        parse_natural(self.text, &cal)
    }
}

/// Monday 23 November 2026.
const MON: &str = "2026-11-23";
/// Sunday 29 November 2026.
const SUN: &str = "2026-11-29";
/// A leap day, Tuesday 29 February 2028.
const LEAP: &str = "2028-02-29";
/// Thursday 31 December 2026.
const NEW_YEARS_EVE: &str = "2026-12-31";

const fn ex(today: &'static str, text: &'static str, day: Option<&'static str>) -> NaturalExample {
    NaturalExample {
        today,
        week_start: Weekday::Monday,
        text,
        day,
    }
}

const fn on(
    week_start: Weekday,
    today: &'static str,
    text: &'static str,
    day: Option<&'static str>,
) -> NaturalExample {
    NaturalExample {
        today,
        week_start,
        text,
        day,
    }
}

/// Every example, by kind.
pub const NATURAL_EXAMPLES: &[NaturalExample] = &[
    // Days, offsets and periods (what queries also take).
    ex(MON, "today", Some("2026-11-23")),
    ex(MON, "Tomorrow", Some("2026-11-24")),
    ex(MON, "yesterday", Some("2026-11-22")),
    ex(MON, "  TODAY  ", Some("2026-11-23")),
    ex(MON, "+3d", Some("2026-11-26")),
    ex(MON, "-7d", Some("2026-11-16")),
    ex(MON, "+2w", Some("2026-12-07")),
    ex(MON, "-1m", Some("2026-10-23")),
    ex(MON, "+1y", Some("2027-11-23")),
    ex(MON, "+1D", Some("2026-11-24")),
    ex(MON, "+", None),
    ex(MON, "+7x", None),
    ex(MON, "7d", None),
    ex(MON, "+1234567d", None),
    ex(MON, "this-week", Some("2026-11-23")),
    ex(MON, "last-week", Some("2026-11-16")),
    ex(MON, "next-month", Some("2026-12-01")),
    ex(MON, "last-year", Some("2025-01-01")),
    ex(MON, "next-decade", None),
    ex(NEW_YEARS_EVE, "+2m", Some("2027-02-28")),
    ex(NEW_YEARS_EVE, "tomorrow", Some("2027-01-01")),
    ex(LEAP, "+1y", Some("2029-02-28")),
    ex(LEAP, "this-month", Some("2028-02-01")),
    // Written dates.
    ex(MON, "2026-12-01", Some("2026-12-01")),
    ex(MON, "2026-02-28", Some("2026-02-28")),
    ex(MON, "2026-2-30", None),
    ex(MON, "2026-02-30", None),
    ex(MON, "31-12-2026", None),
    ex(MON, "11/27/2026", None),
    // Weekdays: the next one after today.
    ex(MON, "fri", Some("2026-11-27")),
    ex(MON, "Friday", Some("2026-11-27")),
    ex(MON, "frid", Some("2026-11-27")),
    ex(MON, "mon", Some("2026-11-30")),
    ex(MON, "sun", Some("2026-11-29")),
    ex(SUN, "sun", Some("2026-12-06")),
    ex(SUN, "mon", Some("2026-11-30")),
    ex(MON, "fr", None),
    ex(MON, "fridays", None),
    // This and next: in this week or the next, as the workspace counts weeks.
    ex(MON, "this fri", Some("2026-11-27")),
    ex(MON, "this sun", Some("2026-11-29")),
    ex(MON, "this mon", Some("2026-11-23")),
    ex(MON, "next fri", Some("2026-12-04")),
    ex(MON, "next mon", Some("2026-11-30")),
    ex(MON, "next week", Some("2026-11-30")),
    ex(MON, "next month", Some("2026-12-01")),
    ex(MON, "next year", Some("2027-01-01")),
    ex(MON, "this week", Some("2026-11-23")),
    ex(MON, "next lunch", None),
    ex(MON, "last fri", None),
    on(Weekday::Sunday, MON, "next fri", Some("2026-12-04")),
    on(Weekday::Sunday, MON, "next week", Some("2026-11-29")),
    on(Weekday::Sunday, MON, "this sun", Some("2026-11-22")),
    on(Weekday::Sunday, MON, "this-week", Some("2026-11-22")),
    on(Weekday::Saturday, MON, "this-week", Some("2026-11-21")),
    on(Weekday::Saturday, MON, "this sat", Some("2026-11-21")),
    on(Weekday::Saturday, MON, "next sat", Some("2026-11-28")),
    on(Weekday::Saturday, MON, "end of week", Some("2026-11-27")),
    on(Weekday::Sunday, SUN, "next sun", Some("2026-12-06")),
    // Spans from today.
    ex(MON, "in 3 days", Some("2026-11-26")),
    ex(MON, "in 1 day", Some("2026-11-24")),
    ex(MON, "in 2 weeks", Some("2026-12-07")),
    ex(MON, "in a week", Some("2026-11-30")),
    ex(MON, "in an hour", None),
    ex(MON, "in one year", Some("2027-11-23")),
    ex(MON, "in a month", Some("2026-12-23")),
    ex(MON, "in 3 d", Some("2026-11-26")),
    ex(MON, "in 3d", Some("2026-11-26")),
    ex(MON, "in 2w", Some("2026-12-07")),
    ex(MON, "in 1m", Some("2026-12-23")),
    ex(MON, "in 1y", Some("2027-11-23")),
    ex(MON, "in 10days", Some("2026-12-03")),
    ex(MON, "in d", None),
    ex(MON, "in 3x", None),
    ex(MON, "in 10001 days", None),
    ex(MON, "in", None),
    ex(LEAP, "in 1 year", Some("2029-02-28")),
    // Month and day: the next such date.
    ex(MON, "dec 1", Some("2026-12-01")),
    ex(MON, "1 Dec", Some("2026-12-01")),
    ex(MON, "Dec. 1st", Some("2026-12-01")),
    ex(MON, "december 24th", Some("2026-12-24")),
    ex(MON, "nov 23", Some("2026-11-23")),
    ex(MON, "nov 1", Some("2027-11-01")),
    ex(MON, "december 1 2028", Some("2028-12-01")),
    ex(MON, "december 1, 2027", Some("2027-12-01")),
    ex(MON, "1 dec 2027", Some("2027-12-01")),
    ex(MON, "feb 29", Some("2028-02-29")),
    ex(MON, "feb 29 2027", None),
    ex(MON, "feb 30", None),
    ex(MON, "de 1", None),
    ex(MON, "dec dec", None),
    ex(MON, "dec 1 0", None),
    ex(MON, "dec 1 year", None),
    ex(NEW_YEARS_EVE, "jan 1", Some("2027-01-01")),
    // Ends of periods.
    ex(MON, "end of month", Some("2026-11-30")),
    ex(MON, "end of the month", Some("2026-11-30")),
    ex(MON, "end of week", Some("2026-11-29")),
    ex(MON, "end of year", Some("2026-12-31")),
    ex(MON, "end of the year", Some("2026-12-31")),
    ex(MON, "end of day", None),
    ex(LEAP, "end of month", Some("2028-02-29")),
    // Not dates.
    ex(MON, "", None),
    ex(MON, "   ", None),
    ex(MON, "someday", None),
    ex(MON, "soon", None),
    ex(MON, "the day after tomorrow", None),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_example_reads_as_written() {
        for e in NATURAL_EXAMPLES {
            assert!(e.today.parse::<Day>().is_ok(), "{}", e.today);
            let got = e.read().map(|d| d.to_string());
            assert_eq!(
                got.as_deref(),
                e.day,
                "{:?} on {} (weeks from {:?})",
                e.text,
                e.today,
                e.week_start
            );
        }
    }
}
