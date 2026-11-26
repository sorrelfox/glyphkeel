// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Calendar days without a time zone: start dates, due dates and the dates
//! of time entries.
//!
//! A due date of 1 December is 1 December wherever the user is and whatever
//! the system's time zone later becomes. Days are stored as `YYYY-MM-DD`
//! text, which sorts in date order.

use std::fmt;
use std::str::FromStr;

/// A day in the proleptic Gregorian calendar, years 1 to 9999.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    year: u16,
    month: u8,
    day: u8,
}

/// A day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weekday {
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

/// Why a day is not valid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DayError {
    /// Not in the form `YYYY-MM-DD`.
    #[error("\"{0}\" is not a date: expected YYYY-MM-DD, for example 2026-12-01")]
    Format(String),
    /// The month or day does not exist (`2026-02-30`), or the year is 0.
    #[error("{0} is not a day in the calendar")]
    NoSuchDay(String),
    /// Moving by a number of days left the years 1 to 9999.
    #[error("the date is outside the years 1 to 9999")]
    OutOfRange,
    /// A start date after the due date.
    #[error("the start date {start} is after the due date {due}")]
    StartAfterDue {
        /// The start date.
        start: Day,
        /// The due date.
        due: Day,
    },
}

const fn is_leap(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

pub(crate) const fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl Day {
    /// The day `year-month-day`, if it exists.
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, DayError> {
        let text = || format!("{year:04}-{month:02}-{day:02}");
        if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
            return Err(DayError::NoSuchDay(text()));
        }
        if day == 0 || day > days_in_month(year, month) {
            return Err(DayError::NoSuchDay(text()));
        }
        Ok(Self { year, month, day })
    }

    /// The year.
    #[must_use]
    pub fn year(self) -> u16 {
        self.year
    }

    /// The month, 1 to 12.
    #[must_use]
    pub fn month(self) -> u8 {
        self.month
    }

    /// The day of the month, 1 to 31.
    #[must_use]
    pub fn day(self) -> u8 {
        self.day
    }

    /// Days since 1970-01-01 (negative before it).
    #[must_use]
    pub fn days_since_epoch(self) -> i64 {
        // Howard Hinnant's days_from_civil.
        let (m, d) = (i64::from(self.month), i64::from(self.day));
        let y = i64::from(self.year) - i64::from(m <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The day `days` after 1970-01-01.
    pub fn from_days_since_epoch(days: i64) -> Result<Self, DayError> {
        // Howard Hinnant's civil_from_days.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        let year = u16::try_from(year).map_err(|_| DayError::OutOfRange)?;
        Self::new(year, month as u8, day as u8).map_err(|_| DayError::OutOfRange)
    }

    /// The day `days` later (earlier when negative).
    pub fn add_days(self, days: i64) -> Result<Self, DayError> {
        let total = self
            .days_since_epoch()
            .checked_add(days)
            .ok_or(DayError::OutOfRange)?;
        Self::from_days_since_epoch(total)
    }

    /// The day of the week.
    #[must_use]
    pub fn weekday(self) -> Weekday {
        // 1970-01-01 was a Thursday.
        const ORDER: [Weekday; 7] = [
            Weekday::Thursday,
            Weekday::Friday,
            Weekday::Saturday,
            Weekday::Sunday,
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
        ];
        ORDER[self.days_since_epoch().rem_euclid(7) as usize]
    }
}

/// Checks that a start date is not after a due date. Either may be missing.
pub fn check_date_range(start: Option<Day>, due: Option<Day>) -> Result<(), DayError> {
    match (start, due) {
        (Some(start), Some(due)) if start > due => Err(DayError::StartAfterDue { start, due }),
        _ => Ok(()),
    }
}

impl FromStr for Day {
    type Err = DayError;

    /// Parses exactly `YYYY-MM-DD`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let b = s.as_bytes();
        let shape = b.len() == 10
            && b[4] == b'-'
            && b[7] == b'-'
            && b.iter()
                .enumerate()
                .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
        if !shape {
            return Err(DayError::Format(s.to_owned()));
        }
        let number = |range: std::ops::Range<usize>| -> u16 {
            b[range]
                .iter()
                .fold(0, |acc, c| acc * 10 + u16::from(c - b'0'))
        };
        Self::new(number(0..4), number(5..7) as u8, number(8..10) as u8)
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl fmt::Debug for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Day({self})")
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn d(s: &str) -> Day {
        s.parse().unwrap()
    }

    #[test]
    fn parses_and_prints() {
        assert_eq!(d("2026-12-01").to_string(), "2026-12-01");
        assert_eq!(d("2024-02-29").day(), 29);
        assert_eq!(d("0001-01-01").year(), 1);
    }

    #[test]
    fn rejects_invalid_days() {
        for bad in [
            "2026-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "0000-01-01",
            "2026-1-01",
            "2026/01/01",
            "26-01-01",
            "2026-01-01T00:00",
            "２０２６-01-01",
        ] {
            assert!(bad.parse::<Day>().is_err(), "{bad}");
        }
        assert!(matches!(
            "1900-02-29".parse::<Day>(),
            Err(DayError::NoSuchDay(_))
        ));
    }

    #[test]
    fn epoch_arithmetic() {
        assert_eq!(d("1970-01-01").days_since_epoch(), 0);
        assert_eq!(d("2026-01-05").days_since_epoch(), 20_458);
        assert_eq!(d("1969-12-31").days_since_epoch(), -1);
        assert_eq!(d("2026-02-28").add_days(1).unwrap(), d("2026-03-01"));
        assert_eq!(d("2024-02-28").add_days(1).unwrap(), d("2024-02-29"));
        assert_eq!(d("2026-01-01").add_days(-1).unwrap(), d("2025-12-31"));
        assert!(d("9999-12-31").add_days(1).is_err());
        assert!(d("0001-01-01").add_days(-1).is_err());
    }

    #[test]
    fn weekdays() {
        assert_eq!(d("2026-01-05").weekday(), Weekday::Monday);
        assert_eq!(d("2026-12-01").weekday(), Weekday::Tuesday);
        assert_eq!(d("1970-01-01").weekday(), Weekday::Thursday);
    }

    #[test]
    fn start_after_due_is_rejected() {
        let (a, b) = (d("2026-03-01"), d("2026-03-02"));
        assert!(check_date_range(Some(a), Some(b)).is_ok());
        assert!(check_date_range(Some(a), Some(a)).is_ok());
        assert!(check_date_range(None, Some(a)).is_ok());
        assert_eq!(
            check_date_range(Some(b), Some(a)),
            Err(DayError::StartAfterDue { start: b, due: a })
        );
    }

    proptest! {
        #[test]
        fn days_round_trip(days in -719_162i64..=2_932_896) {
            let day = Day::from_days_since_epoch(days).unwrap();
            prop_assert_eq!(day.days_since_epoch(), days);
            prop_assert_eq!(day.to_string().parse::<Day>().unwrap(), day);
        }

        #[test]
        fn text_order_is_date_order(a in -719_162i64..=2_932_896, b in -719_162i64..=2_932_896) {
            let (x, y) = (Day::from_days_since_epoch(a).unwrap(), Day::from_days_since_epoch(b).unwrap());
            prop_assert_eq!(x.cmp(&y), a.cmp(&b));
            prop_assert_eq!(x.to_string().cmp(&y.to_string()), a.cmp(&b));
        }
    }
}
