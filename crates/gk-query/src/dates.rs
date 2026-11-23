// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Dates in queries and in date fields.
//!
//! Queries take only unambiguous forms: `2026-11-24`, `today`, `yesterday`,
//! `tomorrow`, offsets in calendar units (`-7d`, `+2w`, `-1m`, `+1y`) and
//! periods (`this-week`, `last-month`, `next-year`). Date fields, quick add
//! and agents' date inputs also take friendlier forms ([`parse_natural`]):
//! `fri`, `next fri`, `in 2 weeks`, `dec 1`, `end of month`.
//!
//! Everything is relative to a [`Calendar`]: today's date and the moment a
//! local day starts, in the workspace's time zone, and the first day of the
//! week. Tests inject one.

use gk_core::{Day, Weekday};

/// Today, the local days' start moments and the first day of the week.
pub trait Calendar {
    /// Today in the workspace's time zone.
    fn today(&self) -> Day;
    /// The moment `day` starts there, in milliseconds since 1970 UTC.
    fn start_of(&self, day: Day) -> i64;
    /// The first day of the week.
    fn week_start(&self) -> Weekday;
}

/// A calendar at a fixed offset from UTC, for tests and tools.
#[derive(Debug, Clone, Copy)]
pub struct FixedCalendar {
    /// Today.
    pub today: Day,
    /// Minutes east of UTC (UTC+14 is 840, UTC−10 is −600).
    pub offset_minutes: i64,
    /// The first day of the week.
    pub week_start: Weekday,
}

impl Calendar for FixedCalendar {
    fn today(&self) -> Day {
        self.today
    }

    fn start_of(&self, day: Day) -> i64 {
        (day.days_since_epoch() * 1440 - self.offset_minutes) * 60_000
    }

    fn week_start(&self) -> Weekday {
        self.week_start
    }
}

/// Days from the first to the last, both included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Days {
    /// The first day.
    pub first: Day,
    /// The last day.
    pub last: Day,
}

impl Days {
    const fn one(day: Day) -> Self {
        Self {
            first: day,
            last: day,
        }
    }
}

fn weekday_index(w: Weekday) -> i64 {
    match w {
        Weekday::Monday => 0,
        Weekday::Tuesday => 1,
        Weekday::Wednesday => 2,
        Weekday::Thursday => 3,
        Weekday::Friday => 4,
        Weekday::Saturday => 5,
        Weekday::Sunday => 6,
    }
}

fn shift(day: Day, days: i64) -> Option<Day> {
    day.add_days(days).ok()
}

/// `day` moved by `months` calendar months; the day of the month is kept,
/// or the month's last day when it has fewer.
#[must_use]
pub fn add_months(day: Day, months: i64) -> Option<Day> {
    let index = i64::from(day.year()) * 12 + i64::from(day.month()) - 1 + months;
    let year = u16::try_from(index.div_euclid(12)).ok()?;
    let month = u8::try_from(index.rem_euclid(12) + 1).ok()?;
    (1..=day.day())
        .rev()
        .find_map(|d| Day::new(year, month, d).ok())
}

/// The first day of the week holding `day`.
#[must_use]
pub fn week_of(day: Day, start: Weekday) -> Days {
    let back = (weekday_index(day.weekday()) - weekday_index(start)).rem_euclid(7);
    let first = shift(day, -back).unwrap_or(day);
    Days {
        first,
        last: shift(first, 6).unwrap_or(first),
    }
}

fn month_of(day: Day) -> Days {
    let first = Day::new(day.year(), day.month(), 1).unwrap_or(day);
    let last = add_months(first, 1)
        .and_then(|d| shift(d, -1))
        .unwrap_or(first);
    Days { first, last }
}

fn year_of(day: Day) -> Days {
    Days {
        first: Day::new(day.year(), 1, 1).unwrap_or(day),
        last: Day::new(day.year(), 12, 31).unwrap_or(day),
    }
}

/// A signed offset in calendar units: `-7d`, `+2w`, `-1m`, `+1y`.
fn offset(text: &str, today: Day) -> Option<Day> {
    let (sign, rest) = match text.as_bytes().first()? {
        b'+' => (1, &text[1..]),
        b'-' => (-1, &text[1..]),
        _ => return None,
    };
    let unit = rest.chars().last()?;
    let digits = &rest[..rest.len() - unit.len_utf8()];
    if digits.is_empty() || digits.len() > 6 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: i64 = digits.parse().ok()?;
    match unit.to_ascii_lowercase() {
        'd' => shift(today, sign * n),
        'w' => shift(today, sign * n * 7),
        'm' => add_months(today, sign * n),
        'y' => add_months(today, sign * n * 12),
        _ => None,
    }
}

/// A date or period in a query, as the days it covers.
#[must_use]
pub fn parse_query_date(text: &str, cal: &dyn Calendar) -> Option<Days> {
    let today = cal.today();
    let lower = text.to_ascii_lowercase();
    let one = |d: Option<Day>| d.map(Days::one);
    match lower.as_str() {
        "today" => return Some(Days::one(today)),
        "yesterday" => return one(shift(today, -1)),
        "tomorrow" => return one(shift(today, 1)),
        _ => {}
    }
    if let Some((which, unit)) = lower.split_once('-') {
        let step = match which {
            "this" => Some(0),
            "last" => Some(-1),
            "next" => Some(1),
            _ => None,
        };
        if let Some(step) = step {
            return match unit {
                "week" => shift(today, step * 7).map(|d| week_of(d, cal.week_start())),
                "month" => add_months(today, step).map(month_of),
                "year" => add_months(today, step * 12).map(year_of),
                _ => None,
            };
        }
    }
    if let Some(day) = offset(&lower, today) {
        return Some(Days::one(day));
    }
    lower.parse::<Day>().ok().map(Days::one)
}

const WEEKDAYS: [(&str, Weekday); 7] = [
    ("mon", Weekday::Monday),
    ("tue", Weekday::Tuesday),
    ("wed", Weekday::Wednesday),
    ("thu", Weekday::Thursday),
    ("fri", Weekday::Friday),
    ("sat", Weekday::Saturday),
    ("sun", Weekday::Sunday),
];

const WEEKDAY_NAMES: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

fn weekday(word: &str) -> Option<Weekday> {
    let (short, day) = WEEKDAYS.iter().find(|(s, _)| word.starts_with(s))?;
    let i = usize::try_from(weekday_index(*day)).ok()?;
    let full = WEEKDAY_NAMES[i];
    (word.len() >= short.len() && full.starts_with(word)).then_some(*day)
}

fn month(word: &str) -> Option<u8> {
    let word = word.trim_end_matches('.');
    if word.len() < 3 {
        return None;
    }
    MONTHS
        .iter()
        .position(|m| m.starts_with(word))
        .and_then(|i| u8::try_from(i + 1).ok())
}

fn unit_days(n: i64, unit: &str, today: Day) -> Option<Day> {
    match unit.trim_end_matches('s') {
        "day" | "d" => shift(today, n),
        "week" | "w" => shift(today, n * 7),
        "month" => add_months(today, n),
        "year" | "y" => add_months(today, n * 12),
        _ => None,
    }
}

/// The next `month`/`day` on or after `today` (this year or next).
fn next_date(today: Day, month: u8, day: u8, year: Option<u16>) -> Option<Day> {
    if let Some(year) = year {
        return Day::new(year, month, day).ok();
    }
    let this = Day::new(today.year(), month, day).ok();
    match this {
        Some(d) if d >= today => Some(d),
        _ => (1..=4).find_map(|k| Day::new(today.year() + k, month, day).ok()),
    }
}

/// A date written the friendly way, for date fields, quick add and agents'
/// date inputs. Takes everything [`parse_query_date`] takes (a period means
/// its first day) and also:
///
/// - a weekday (`fri`, `friday`): the next one after today;
/// - `this fri`: that day of this week; `next fri`: that day of next week;
/// - `in 3 days`, `in 2 weeks`, `in a month`, `in 1 year`;
/// - `dec 1`, `1 dec`, `december 1 2027`: the next such date;
/// - `next week`, `next month`, `next year`: the first day of it;
/// - `end of week`, `end of month`, `end of year`.
///
/// Words are English for now.
#[must_use]
pub fn parse_natural(text: &str, cal: &dyn Calendar) -> Option<Day> {
    let today = cal.today();
    let lower = text.trim().to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty())
        .collect();
    match words.as_slice() {
        [] => None,
        [one] => {
            if let Some(days) = parse_query_date(one, cal) {
                return Some(days.first);
            }
            weekday(one).and_then(|w| {
                let ahead = (weekday_index(w) - weekday_index(today.weekday())).rem_euclid(7);
                shift(today, if ahead == 0 { 7 } else { ahead })
            })
        }
        [which @ ("this" | "next"), unit] => {
            let step = i64::from(*which == "next");
            if let Some(w) = weekday(unit) {
                let week = week_of(shift(today, step * 7)?, cal.week_start());
                let into = (weekday_index(w) - weekday_index(cal.week_start())).rem_euclid(7);
                return shift(week.first, into);
            }
            let period = format!("{which}-{unit}");
            parse_query_date(&period, cal).map(|d| d.first)
        }
        ["in", n, unit] => {
            let n: i64 = match *n {
                "a" | "an" | "one" => 1,
                n => n.parse().ok().filter(|n: &i64| *n <= 10_000)?,
            };
            unit_days(n, unit, today)
        }
        ["end", "of", unit] | ["end", "of", "the", unit] => match *unit {
            "week" => Some(week_of(today, cal.week_start()).last),
            "month" => Some(month_of(today).last),
            "year" => Some(year_of(today).last),
            _ => None,
        },
        [a, b] | [a, b, _] => {
            let year = match words.as_slice() {
                [_, _, y] => Some(y.parse::<u16>().ok().filter(|y| *y >= 1)?),
                _ => None,
            };
            let (m, d) = match (month(a), month(b)) {
                (Some(m), None) => (m, b),
                (None, Some(m)) => (m, a),
                _ => return None,
            };
            let d: u8 = d
                .trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '.')
                .parse()
                .ok()?;
            next_date(today, m, d, year)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Day {
        s.parse().expect("day")
    }

    /// Monday 23 November 2026.
    fn cal(week_start: Weekday) -> FixedCalendar {
        FixedCalendar {
            today: d("2026-11-23"),
            offset_minutes: 60,
            week_start,
        }
    }

    fn q(text: &str, c: &FixedCalendar) -> (String, String) {
        let r = parse_query_date(text, c).expect(text);
        (r.first.to_string(), r.last.to_string())
    }

    fn pair(a: &str, b: &str) -> (String, String) {
        (a.to_owned(), b.to_owned())
    }

    #[test]
    fn query_dates_and_periods() {
        let c = cal(Weekday::Monday);
        assert_eq!(q("today", &c), pair("2026-11-23", "2026-11-23"));
        assert_eq!(q("Tomorrow", &c), pair("2026-11-24", "2026-11-24"));
        assert_eq!(q("-7d", &c), pair("2026-11-16", "2026-11-16"));
        assert_eq!(q("+2w", &c), pair("2026-12-07", "2026-12-07"));
        assert_eq!(q("-1m", &c), pair("2026-10-23", "2026-10-23"));
        assert_eq!(q("+1y", &c), pair("2027-11-23", "2027-11-23"));
        assert_eq!(q("this-week", &c), pair("2026-11-23", "2026-11-29"));
        assert_eq!(q("last-week", &c), pair("2026-11-16", "2026-11-22"));
        assert_eq!(q("next-month", &c), pair("2026-12-01", "2026-12-31"));
        assert_eq!(q("last-year", &c), pair("2025-01-01", "2025-12-31"));
        assert_eq!(q("2026-02-28", &c), pair("2026-02-28", "2026-02-28"));
        assert!(parse_query_date("31-12-2026", &c).is_none());
        assert!(parse_query_date("7d", &c).is_none());
        assert!(parse_query_date("+7x", &c).is_none());
        assert!(parse_query_date("+", &c).is_none());
    }

    #[test]
    fn weeks_start_on_the_configured_day() {
        let sunday = cal(Weekday::Sunday);
        assert_eq!(q("this-week", &sunday), pair("2026-11-22", "2026-11-28"));
        let saturday = cal(Weekday::Saturday);
        assert_eq!(q("this-week", &saturday), pair("2026-11-21", "2026-11-27"));
    }

    #[test]
    fn month_ends_and_leap_days() {
        assert_eq!(add_months(d("2026-01-31"), 1), Some(d("2026-02-28")));
        assert_eq!(add_months(d("2028-01-31"), 1), Some(d("2028-02-29")));
        assert_eq!(add_months(d("2028-02-29"), 12), Some(d("2029-02-28")));
        assert_eq!(add_months(d("2026-03-31"), -1), Some(d("2026-02-28")));
        let leap = FixedCalendar {
            today: d("2028-02-29"),
            offset_minutes: 0,
            week_start: Weekday::Monday,
        };
        assert_eq!(q("this-month", &leap), pair("2028-02-01", "2028-02-29"));
        assert_eq!(q("+1y", &leap), pair("2029-02-28", "2029-02-28"));
    }

    #[test]
    fn days_start_at_local_midnight() {
        let east = FixedCalendar {
            today: d("2026-11-23"),
            offset_minutes: 14 * 60,
            week_start: Weekday::Monday,
        };
        let west = FixedCalendar {
            offset_minutes: -10 * 60,
            ..east
        };
        let day = d("2026-11-23");
        let utc = day.days_since_epoch() * 86_400_000;
        assert_eq!(east.start_of(day), utc - 14 * 3_600_000);
        assert_eq!(west.start_of(day), utc + 10 * 3_600_000);
    }

    #[test]
    fn natural_dates() {
        let c = cal(Weekday::Monday);
        let n = |t: &str| parse_natural(t, &c).map(|d| d.to_string());
        assert_eq!(n("fri").as_deref(), Some("2026-11-27"));
        assert_eq!(n("Friday").as_deref(), Some("2026-11-27"));
        assert_eq!(n("mon").as_deref(), Some("2026-11-30"));
        assert_eq!(n("next fri").as_deref(), Some("2026-12-04"));
        assert_eq!(n("this sun").as_deref(), Some("2026-11-29"));
        assert_eq!(n("in 2 weeks").as_deref(), Some("2026-12-07"));
        assert_eq!(n("in a month").as_deref(), Some("2026-12-23"));
        assert_eq!(n("in 3 days").as_deref(), Some("2026-11-26"));
        assert_eq!(n("dec 1").as_deref(), Some("2026-12-01"));
        assert_eq!(n("1 Dec").as_deref(), Some("2026-12-01"));
        assert_eq!(n("nov 1").as_deref(), Some("2027-11-01"));
        assert_eq!(n("december 1 2028").as_deref(), Some("2028-12-01"));
        assert_eq!(n("feb 29").as_deref(), Some("2028-02-29"));
        assert_eq!(n("end of month").as_deref(), Some("2026-11-30"));
        assert_eq!(n("end of week").as_deref(), Some("2026-11-29"));
        assert_eq!(n("next week").as_deref(), Some("2026-11-30"));
        assert_eq!(n("today").as_deref(), Some("2026-11-23"));
        assert_eq!(n("+3d").as_deref(), Some("2026-11-26"));
        assert_eq!(n("2026-12-24").as_deref(), Some("2026-12-24"));
        assert_eq!(n("someday"), None);
        assert_eq!(n("fr"), None);
        assert_eq!(n(""), None);
    }
}
