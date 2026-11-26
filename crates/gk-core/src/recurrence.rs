// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Recurrence rules: when the next occurrence of a recurring task falls.
//!
//! A rule repeats every N days, weeks, months or years from its start day:
//!
//! - weekly rules fall on chosen weekdays (default: the start's weekday);
//!   weeks start on Monday for counting "every 2 weeks";
//! - monthly and yearly rules fall on a day of the month (default: the
//!   start's; `-1` is the last day, and the 31st is the last day in shorter
//!   months), or on the first to fifth or last weekday of the month (a
//!   month without a fifth Monday is passed over);
//! - yearly rules fall in one month (default: the start's); 29 February is
//!   28 February in other years.
//!
//! A rule can end on a day (`until`) or after a number of occurrences
//! (`count`, counted by the caller). Repeating from completion ignores the
//! calendar pattern: the next occurrence is N units after the day the last
//! one was done.
//!
//! Everything here works on days: the caller turns moments into days in the
//! person's time zone.

use std::fmt;

use crate::day::{Day, Weekday, days_in_month};

/// The largest interval: every 999 days, weeks, months or years.
pub const INTERVAL_MAX: u16 = 999;
/// The largest number of occurrences a rule can be limited to.
pub const COUNT_MAX: u32 = 10_000;

/// The unit a rule repeats in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Frequency {
    /// Every N days.
    Day,
    /// Every N weeks.
    Week,
    /// Every N months.
    Month,
    /// Every N years.
    Year,
}

/// What the next occurrence is counted from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RepeatFrom {
    /// The calendar: the rule's dates, whenever the last one was done.
    #[default]
    Schedule,
    /// The day the last occurrence was done ("3 days after done").
    Completion,
}

/// The nth weekday of a month: `1`–`5`, or `-1` for the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NthWeekday {
    /// Which one: 1 to 5, or -1 for the last.
    pub nth: i8,
    /// The weekday.
    pub weekday: Weekday,
}

/// A recurrence rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rule {
    /// The unit.
    pub every: Frequency,
    /// Every this many units, 1 to [`INTERVAL_MAX`].
    pub interval: u16,
    /// Weekly: the weekdays, Monday first. Empty: the start's weekday.
    pub weekdays: Vec<Weekday>,
    /// Monthly and yearly: the day of the month, 1 to 31 or -1 for the
    /// last. With neither this nor `nth`, the start's day.
    pub month_day: Option<i8>,
    /// Monthly and yearly: the nth weekday instead of a day of the month.
    pub nth: Option<NthWeekday>,
    /// Yearly: the month, 1 to 12; default the start's.
    pub month: Option<u8>,
    /// The first possible day; the pattern is counted from it.
    pub start: Day,
    /// The last possible day.
    pub until: Option<Day>,
    /// At most this many occurrences, the first included.
    pub count: Option<u32>,
    /// From the calendar or from completion.
    pub from: RepeatFrom,
}

/// Why a rule is not valid, or a rule's text could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    /// The interval is 0 or too large.
    #[error("the interval must be 1 to {max}")]
    Interval {
        /// The largest interval.
        max: u16,
    },
    /// The day of the month is not 1 to 31 or -1.
    #[error("the day of the month must be 1 to 31, or -1 for the last day")]
    MonthDay,
    /// The week of the month is not 1 to 5 or -1.
    #[error("the week of the month must be 1 to 5, or -1 for the last")]
    WeekOfMonth,
    /// The month is not 1 to 12.
    #[error("the month must be 1 to 12")]
    Month,
    /// Both a day of the month and a weekday of the month.
    #[error("give a day of the month or a weekday of the month, not both")]
    DayAndWeekday,
    /// Parts that don't belong to the unit (weekdays for a monthly rule…).
    #[error("these parts don't fit a rule that repeats every {unit}")]
    Parts {
        /// `day`, `week`, `month` or `year`.
        unit: &'static str,
    },
    /// Repeating from completion with a calendar pattern.
    #[error("a rule that repeats from completion can't fall on chosen days")]
    CompletionPattern,
    /// The end day comes before the start.
    #[error("the rule ends on {until}, before it starts on {start}")]
    UntilBeforeStart {
        /// The start.
        start: Day,
        /// The end.
        until: Day,
    },
    /// The count is 0 or too large.
    #[error("the number of occurrences must be 1 to {max}")]
    Count {
        /// The largest count.
        max: u32,
    },
    /// The text of a rule could not be read.
    #[error(
        "\"{0}\" is not a rule: write, for example, every monday, every 2 weeks on mon and thu, monthly on the last fri, or 3 days after done"
    )]
    Text(String),
}

impl Frequency {
    /// The word used in rules' text and the API (`week`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }
}

/// The weekdays, Monday first.
pub const WEEKDAYS: [Weekday; 7] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
];

/// Monday is 0, Sunday 6.
#[must_use]
pub fn weekday_index(w: Weekday) -> usize {
    WEEKDAYS.iter().position(|d| *d == w).unwrap_or(0)
}

/// The three-letter name of a weekday, as rules' text and the API write it.
#[must_use]
pub const fn weekday_code(w: Weekday) -> &'static str {
    match w {
        Weekday::Monday => "mon",
        Weekday::Tuesday => "tue",
        Weekday::Wednesday => "wed",
        Weekday::Thursday => "thu",
        Weekday::Friday => "fri",
        Weekday::Saturday => "sat",
        Weekday::Sunday => "sun",
    }
}

/// The weekday a word names (`mon`, `monday`, `tues`…).
#[must_use]
pub fn parse_weekday(word: &str) -> Option<Weekday> {
    let w = word.trim().to_ascii_lowercase();
    let w = w.trim_end_matches('s');
    Some(match w {
        "mon" | "monday" => Weekday::Monday,
        "tue" | "tuesday" => Weekday::Tuesday,
        "wed" | "wednesday" => Weekday::Wednesday,
        "thu" | "thur" | "thursday" => Weekday::Thursday,
        "fri" | "friday" => Weekday::Friday,
        "sat" | "saturday" => Weekday::Saturday,
        "sun" | "sunday" => Weekday::Sunday,
        _ => return None,
    })
}

fn month_index(d: Day) -> i64 {
    i64::from(d.year()) * 12 + i64::from(d.month()) - 1
}

fn month_of(index: i64) -> Option<(u16, u8)> {
    let year = u16::try_from(index.div_euclid(12)).ok()?;
    let month = u8::try_from(index.rem_euclid(12) + 1).ok()?;
    (1..=9999).contains(&year).then_some((year, month))
}

/// Day `day` of a month (`-1`: the last); a day past the month's end is
/// its last day.
fn day_in_month(year: u16, month: u8, day: i8) -> Option<Day> {
    let last = days_in_month(year, month);
    let d = if day < 0 {
        last
    } else {
        u8::try_from(day).ok()?.min(last)
    };
    Day::new(year, month, d).ok()
}

/// The nth weekday of a month, if the month has one.
fn nth_in_month(year: u16, month: u8, nth: NthWeekday) -> Option<Day> {
    let target = weekday_index(nth.weekday) as i64;
    if nth.nth < 0 {
        let last = Day::new(year, month, days_in_month(year, month)).ok()?;
        let back = (weekday_index(last.weekday()) as i64 - target).rem_euclid(7);
        return last.add_days(-back).ok();
    }
    let first = Day::new(year, month, 1).ok()?;
    let ahead = (target - weekday_index(first.weekday()) as i64).rem_euclid(7);
    let d = first.add_days(ahead + 7 * (i64::from(nth.nth) - 1)).ok()?;
    (d.month() == month).then_some(d)
}

fn ceil_div(a: i64, b: i64) -> i64 {
    -((-a).div_euclid(b))
}

impl Rule {
    /// A rule repeating every `interval` units from `start`, with no other
    /// parts.
    #[must_use]
    pub fn every(every: Frequency, interval: u16, start: Day) -> Self {
        Self {
            every,
            interval,
            weekdays: Vec::new(),
            month_day: None,
            nth: None,
            month: None,
            start,
            until: None,
            count: None,
            from: RepeatFrom::Schedule,
        }
    }

    /// Checks the rule's parts against each other.
    pub fn check(&self) -> Result<(), RuleError> {
        if self.interval == 0 || self.interval > INTERVAL_MAX {
            return Err(RuleError::Interval { max: INTERVAL_MAX });
        }
        if let Some(d) = self.month_day
            && !(d == -1 || (1..=31).contains(&d))
        {
            return Err(RuleError::MonthDay);
        }
        if let Some(n) = self.nth
            && !(n.nth == -1 || (1..=5).contains(&n.nth))
        {
            return Err(RuleError::WeekOfMonth);
        }
        if let Some(m) = self.month
            && !(1..=12).contains(&m)
        {
            return Err(RuleError::Month);
        }
        if self.month_day.is_some() && self.nth.is_some() {
            return Err(RuleError::DayAndWeekday);
        }
        let unit = self.every.as_str();
        let misfit = match self.every {
            Frequency::Day => {
                !self.weekdays.is_empty()
                    || self.month_day.is_some()
                    || self.nth.is_some()
                    || self.month.is_some()
            }
            Frequency::Week => {
                self.month_day.is_some() || self.nth.is_some() || self.month.is_some()
            }
            Frequency::Month => !self.weekdays.is_empty() || self.month.is_some(),
            Frequency::Year => !self.weekdays.is_empty(),
        };
        if misfit {
            return Err(RuleError::Parts { unit });
        }
        if self.from == RepeatFrom::Completion
            && (!self.weekdays.is_empty()
                || self.month_day.is_some()
                || self.nth.is_some()
                || self.month.is_some())
        {
            return Err(RuleError::CompletionPattern);
        }
        if let Some(until) = self.until
            && until < self.start
        {
            return Err(RuleError::UntilBeforeStart {
                start: self.start,
                until,
            });
        }
        if let Some(c) = self.count
            && (c == 0 || c > COUNT_MAX)
        {
            return Err(RuleError::Count { max: COUNT_MAX });
        }
        Ok(())
    }

    /// The weekdays of a weekly rule, Monday first, never empty.
    #[must_use]
    pub fn days_of_week(&self) -> Vec<Weekday> {
        if self.weekdays.is_empty() {
            return vec![self.start.weekday()];
        }
        let mut days = self.weekdays.clone();
        days.sort_by_key(|d| weekday_index(*d));
        days.dedup();
        days
    }

    fn within(&self, d: Day) -> Option<Day> {
        match self.until {
            Some(u) if d > u => None,
            _ => Some(d),
        }
    }

    /// The first day of the calendar pattern after `after` (and not before
    /// the start, nor after `until`). The count is the caller's.
    #[must_use]
    pub fn next_after(&self, after: Day) -> Option<Day> {
        let from = after.add_days(1).ok()?.max(self.start);
        let interval = i64::from(self.interval.max(1));
        let found = match self.every {
            Frequency::Day => {
                let gap = from.days_since_epoch() - self.start.days_since_epoch();
                self.start.add_days(ceil_div(gap, interval) * interval).ok()
            }
            Frequency::Week => {
                let days = self.days_of_week();
                let monday0 = self
                    .start
                    .add_days(-(weekday_index(self.start.weekday()) as i64))
                    .ok()?;
                let mut d = from;
                let mut found = None;
                for _ in 0..64 {
                    let week = (d.days_since_epoch() - monday0.days_since_epoch()).div_euclid(7);
                    if week.rem_euclid(interval) != 0 {
                        let next = ceil_div(week, interval) * interval;
                        d = monday0.add_days(next * 7).ok()?;
                        continue;
                    }
                    if days.contains(&d.weekday()) {
                        found = Some(d);
                        break;
                    }
                    d = d.add_days(1).ok()?;
                }
                found
            }
            Frequency::Month | Frequency::Year => {
                let step = if self.every == Frequency::Year {
                    interval * 12
                } else {
                    interval
                };
                let first = if self.every == Frequency::Year {
                    let month = self.month.unwrap_or_else(|| self.start.month());
                    i64::from(self.start.year()) * 12 + i64::from(month) - 1
                } else {
                    month_index(self.start)
                };
                let gap = month_index(from) - first;
                let mut k = first + ceil_div(gap.max(0), step) * step;
                if gap < 0 {
                    k = first;
                }
                // A step back covers a candidate earlier in `from`'s month.
                if k - step >= first {
                    k -= step;
                }
                let mut found = None;
                for _ in 0..200 {
                    let (year, month) = month_of(k)?;
                    let candidate = match (self.nth, self.month_day) {
                        (Some(n), _) => nth_in_month(year, month, n),
                        (None, Some(day)) => day_in_month(year, month, day),
                        (None, None) => {
                            day_in_month(year, month, i8::try_from(self.start.day()).ok()?)
                        }
                    };
                    if let Some(c) = candidate
                        && c >= from
                    {
                        found = Some(c);
                        break;
                    }
                    k += step;
                }
                found
            }
        }?;
        self.within(found)
    }

    /// The first occurrence: the first day of the pattern from the start.
    #[must_use]
    pub fn first(&self) -> Option<Day> {
        match self.start.add_days(-1) {
            Ok(before) => self.next_after(before),
            Err(_) => Some(self.start),
        }
    }

    /// For a rule repeating from completion: `interval` units after `done`,
    /// if before `until`. Adding months keeps the day of the month, or the
    /// month's last day.
    #[must_use]
    pub fn after_completion(&self, done: Day) -> Option<Day> {
        let n = i64::from(self.interval.max(1));
        let next = match self.every {
            Frequency::Day => done.add_days(n).ok()?,
            Frequency::Week => done.add_days(7 * n).ok()?,
            Frequency::Month | Frequency::Year => {
                let months = if self.every == Frequency::Year {
                    n * 12
                } else {
                    n
                };
                let (year, month) = month_of(month_index(done) + months)?;
                day_in_month(year, month, i8::try_from(done.day()).ok()?)?
            }
        };
        self.within(next)
    }

    /// The next occurrence after one on `last` that was done on `done`:
    /// the calendar's next day after `last`, else `interval` units after
    /// `done`.
    #[must_use]
    pub fn next(&self, last: Day, done: Day) -> Option<Day> {
        match self.from {
            RepeatFrom::Schedule => self.next_after(last),
            RepeatFrom::Completion => self.after_completion(done),
        }
    }

    /// Up to `n` days of the calendar pattern after `after`, with `made`
    /// occurrences already counted against `count`.
    #[must_use]
    pub fn upcoming(&self, after: Day, n: usize, made: u32) -> Vec<Day> {
        let mut out = Vec::new();
        let mut last = after;
        let mut made = made;
        while out.len() < n {
            if self.count.is_some_and(|c| made >= c) {
                break;
            }
            let next = match self.from {
                RepeatFrom::Schedule => self.next_after(last),
                RepeatFrom::Completion => self.after_completion(last),
            };
            let Some(next) = next else {
                break;
            };
            out.push(next);
            made += 1;
            last = next;
        }
        out
    }
}

impl fmt::Display for Rule {
    /// The rule in the text form [`parse`] reads, for logs and tests.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "every {} {}", self.interval, self.every.as_str())?;
        if self.every == Frequency::Week {
            let days: Vec<&str> = self.days_of_week().into_iter().map(weekday_code).collect();
            write!(f, " on {}", days.join(","))?;
        }
        if let Some(n) = self.nth {
            write!(f, " on the {} {}", n.nth, weekday_code(n.weekday))?;
        }
        if let Some(d) = self.month_day {
            write!(f, " on day {d}")?;
        }
        if let Some(m) = self.month {
            write!(f, " in month {m}")?;
        }
        if self.from == RepeatFrom::Completion {
            write!(f, " after done")?;
        }
        if let Some(u) = self.until {
            write!(f, " until {u}")?;
        }
        if let Some(c) = self.count {
            write!(f, " for {c} times")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------
// Reading a rule's text

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

fn parse_month(word: &str) -> Option<u8> {
    let w = word.to_ascii_lowercase();
    if w.len() < 3 {
        return None;
    }
    let i = MONTHS.iter().position(|m| w.starts_with(m))?;
    let full = [
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
    (full[i].starts_with(&w) || w == "sept").then(|| u8::try_from(i + 1).unwrap_or(1))
}

fn parse_unit(word: &str) -> Option<Frequency> {
    Some(match word.trim_end_matches('s') {
        "day" | "d" => Frequency::Day,
        "week" | "w" | "wk" => Frequency::Week,
        "month" | "mo" => Frequency::Month,
        "year" | "y" | "yr" => Frequency::Year,
        _ => return None,
    })
}

/// `1st`, `2nd`, `15th`, `last`, or a plain number.
fn parse_ordinal(word: &str) -> Option<i8> {
    if word == "last" {
        return Some(-1);
    }
    let words = [
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
    ];
    if let Some((_, n)) = words.iter().find(|(w, _)| *w == word) {
        return Some(*n);
    }
    let digits = word.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &word[digits.len()..];
    if !matches!(suffix, "" | "st" | "nd" | "rd" | "th") {
        return None;
    }
    digits.parse().ok()
}

fn tokens(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .replace([',', ';', '/', '+', '&'], " ")
        .split_whitespace()
        .filter(|w| !matches!(*w, "and" | "a" | "an"))
        .map(str::to_owned)
        .collect()
}

/// Reads a rule written in words, with `start` as its start day:
///
/// - `daily`, `weekly`, `monthly`, `yearly`, `weekdays`, `every day`,
///   `every 2 weeks`, `every other month`, `every monday`, `every mon, thu`;
/// - `on mon and fri`, `on the 1st`, `on the last fri`, `on the 15th`,
///   `the 1st of each month`, `in march`;
/// - `after done` (or `after completion`): counted from completion, as in
///   `3 days after done`;
/// - `until 2027-06-30`, `for 10 times`, `10 times`.
pub fn parse(text: &str, start: Day) -> Result<Rule, RuleError> {
    let bad = || RuleError::Text(text.trim().to_owned());
    let words = tokens(text);
    if words.is_empty() {
        return Err(bad());
    }
    let mut rule = Rule::every(Frequency::Day, 1, start);
    let mut unit: Option<Frequency> = None;
    let mut i = 0;
    let at = |i: usize| words.get(i).map(String::as_str);
    let mut weekdays: Vec<Weekday> = Vec::new();
    let mut ordinal: Option<i8> = None;
    let mut ordinal_weekday: Option<Weekday> = None;
    while let Some(w) = at(i) {
        i += 1;
        match w {
            "daily" => unit = Some(Frequency::Day),
            "weekly" => unit = Some(Frequency::Week),
            "biweekly" | "fortnightly" => {
                unit = Some(Frequency::Week);
                rule.interval = 2;
            }
            "monthly" => unit = Some(Frequency::Month),
            "yearly" | "annually" => unit = Some(Frequency::Year),
            "weekday" | "weekdays" | "workday" | "workdays" => {
                unit = Some(Frequency::Week);
                weekdays.extend(&WEEKDAYS[..5]);
            }
            "every" | "each" | "repeat" | "repeats" | "the" | "of" | "on" | "in" => {}
            "other" => rule.interval = 2,
            "after" => {
                if matches!(
                    at(i),
                    Some("done" | "completion" | "completed" | "closing" | "close" | "finish")
                ) {
                    i += 1;
                }
                rule.from = RepeatFrom::Completion;
            }
            "until" | "till" | "ending" | "ends" => {
                let d = at(i).ok_or_else(bad)?;
                i += 1;
                rule.until = Some(d.parse().map_err(|_| bad())?);
            }
            "for" | "x" | "times" | "time" | "occurrences" => {}
            _ => {
                if let Some(d) = parse_weekday(w) {
                    if ordinal.is_some() && ordinal_weekday.is_none() {
                        ordinal_weekday = Some(d);
                    } else {
                        weekdays.push(d);
                    }
                    continue;
                }
                if let Some(u) = parse_unit(w) {
                    unit = Some(u);
                    continue;
                }
                if let Some(m) = parse_month(w) {
                    rule.month = Some(m);
                    if unit.is_none() {
                        unit = Some(Frequency::Year);
                    }
                    continue;
                }
                if let Ok(n) = w.parse::<u32>() {
                    match at(i) {
                        Some("times" | "time" | "x" | "occurrences") => {
                            rule.count = Some(n);
                            i += 1;
                        }
                        Some(next) if parse_unit(next).is_some() => {
                            rule.interval = u16::try_from(n)
                                .map_err(|_| RuleError::Interval { max: INTERVAL_MAX })?;
                        }
                        _ if matches!(
                            words.get(i.wrapping_sub(2)).map(String::as_str),
                            Some("for")
                        ) =>
                        {
                            rule.count = Some(n);
                        }
                        _ if ordinal.is_none() && n <= 31 && unit.is_some() => {
                            ordinal = i8::try_from(n).ok();
                        }
                        _ => {
                            rule.interval = u16::try_from(n)
                                .map_err(|_| RuleError::Interval { max: INTERVAL_MAX })?;
                        }
                    }
                    continue;
                }
                if let Some(n) = parse_ordinal(w) {
                    ordinal = Some(n);
                    continue;
                }
                return Err(bad());
            }
        }
    }
    let every = match unit {
        Some(u) => u,
        None if !weekdays.is_empty() => Frequency::Week,
        None if ordinal.is_some() => Frequency::Month,
        None => return Err(bad()),
    };
    rule.every = every;
    match (ordinal, ordinal_weekday) {
        (Some(n), Some(d)) => rule.nth = Some(NthWeekday { nth: n, weekday: d }),
        (Some(n), None) => rule.month_day = Some(n),
        (None, _) => {}
    }
    if !weekdays.is_empty() {
        if every == Frequency::Week {
            weekdays.sort_by_key(|d| weekday_index(*d));
            weekdays.dedup();
            rule.weekdays = weekdays;
        } else {
            return Err(RuleError::Parts {
                unit: every.as_str(),
            });
        }
    }
    rule.check()?;
    Ok(rule)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn d(s: &str) -> Day {
        s.parse().unwrap()
    }

    fn dates(rule: &Rule, after: &str, n: usize) -> Vec<String> {
        rule.upcoming(d(after), n, 0)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn daily_rules_step_from_the_start() {
        let r = Rule::every(Frequency::Day, 3, d("2026-11-01"));
        assert_eq!(
            dates(&r, "2026-10-01", 3),
            ["2026-11-01", "2026-11-04", "2026-11-07"]
        );
        assert_eq!(dates(&r, "2026-11-04", 1), ["2026-11-07"]);
        assert_eq!(dates(&r, "2026-11-05", 1), ["2026-11-07"]);
    }

    #[test]
    fn weekly_rules_fall_on_their_weekdays_in_every_nth_week() {
        // 2026-11-04 is a Wednesday.
        let mut r = Rule::every(Frequency::Week, 2, d("2026-11-04"));
        r.weekdays = vec![Weekday::Thursday, Weekday::Monday];
        assert_eq!(
            dates(&r, "2026-11-03", 4),
            ["2026-11-05", "2026-11-16", "2026-11-19", "2026-11-30"]
        );
        let plain = Rule::every(Frequency::Week, 1, d("2026-11-04"));
        assert_eq!(dates(&plain, "2026-11-04", 2), ["2026-11-11", "2026-11-18"]);
    }

    #[test]
    fn monthly_rules_keep_the_day_or_the_last_day() {
        let r = Rule::every(Frequency::Month, 1, d("2027-01-31"));
        assert_eq!(
            dates(&r, "2027-01-01", 4),
            ["2027-01-31", "2027-02-28", "2027-03-31", "2027-04-30"]
        );
        let mut last = Rule::every(Frequency::Month, 2, d("2026-11-10"));
        last.month_day = Some(-1);
        assert_eq!(dates(&last, "2026-11-10", 2), ["2026-11-30", "2027-01-31"]);
        let mut first = Rule::every(Frequency::Month, 1, d("2026-11-26"));
        first.month_day = Some(1);
        assert_eq!(dates(&first, "2026-11-26", 2), ["2026-12-01", "2027-01-01"]);
    }

    #[test]
    fn monthly_rules_by_weekday_skip_months_without_one() {
        let mut fifth = Rule::every(Frequency::Month, 1, d("2026-11-01"));
        fifth.nth = Some(NthWeekday {
            nth: 5,
            weekday: Weekday::Monday,
        });
        // November 2026 has five Mondays (the 30th); December and January
        // 2027 don't have a fifth Monday until March 2027 (the 29th).
        assert_eq!(dates(&fifth, "2026-11-01", 2), ["2026-11-30", "2027-03-29"]);
        let mut last_fri = Rule::every(Frequency::Month, 1, d("2026-11-01"));
        last_fri.nth = Some(NthWeekday {
            nth: -1,
            weekday: Weekday::Friday,
        });
        assert_eq!(
            dates(&last_fri, "2026-11-01", 2),
            ["2026-11-27", "2026-12-25"]
        );
    }

    #[test]
    fn yearly_rules_and_leap_days() {
        let r = Rule::every(Frequency::Year, 1, d("2028-02-29"));
        assert_eq!(
            dates(&r, "2028-01-01", 3),
            ["2028-02-29", "2029-02-28", "2030-02-28"]
        );
        let mut march = Rule::every(Frequency::Year, 1, d("2026-11-26"));
        march.month = Some(3);
        march.month_day = Some(1);
        assert_eq!(dates(&march, "2026-11-26", 2), ["2027-03-01", "2028-03-01"]);
    }

    #[test]
    fn until_and_count_end_a_rule() {
        let mut r = Rule::every(Frequency::Day, 1, d("2026-11-01"));
        r.until = Some(d("2026-11-03"));
        assert_eq!(dates(&r, "2026-10-31", 5).len(), 3);
        r.until = None;
        r.count = Some(2);
        assert_eq!(r.upcoming(d("2026-10-31"), 5, 0).len(), 2);
        assert_eq!(r.upcoming(d("2026-10-31"), 5, 1).len(), 1);
    }

    #[test]
    fn repeating_from_completion_counts_from_the_day_it_was_done() {
        let mut r = Rule::every(Frequency::Day, 3, d("2026-11-01"));
        r.from = RepeatFrom::Completion;
        assert_eq!(
            r.next(d("2026-11-01"), d("2026-11-10")),
            Some(d("2026-11-13"))
        );
        let mut m = Rule::every(Frequency::Month, 1, d("2026-01-31"));
        m.from = RepeatFrom::Completion;
        assert_eq!(m.after_completion(d("2027-01-31")), Some(d("2027-02-28")));
    }

    #[test]
    fn checks_refuse_parts_that_dont_fit() {
        let mut r = Rule::every(Frequency::Day, 1, d("2026-11-01"));
        r.weekdays = vec![Weekday::Monday];
        assert_eq!(r.check(), Err(RuleError::Parts { unit: "day" }));
        let zero = Rule::every(Frequency::Day, 0, d("2026-11-01"));
        assert!(matches!(zero.check(), Err(RuleError::Interval { .. })));
        let mut both = Rule::every(Frequency::Month, 1, d("2026-11-01"));
        both.month_day = Some(3);
        both.nth = Some(NthWeekday {
            nth: 1,
            weekday: Weekday::Monday,
        });
        assert_eq!(both.check(), Err(RuleError::DayAndWeekday));
        let mut done = Rule::every(Frequency::Week, 1, d("2026-11-01"));
        done.from = RepeatFrom::Completion;
        done.weekdays = vec![Weekday::Friday];
        assert_eq!(done.check(), Err(RuleError::CompletionPattern));
        let mut ends = Rule::every(Frequency::Week, 1, d("2026-11-01"));
        ends.until = Some(d("2026-10-01"));
        assert!(matches!(
            ends.check(),
            Err(RuleError::UntilBeforeStart { .. })
        ));
    }

    #[test]
    fn text_is_read_into_rules() {
        let s = d("2026-11-26");
        let r = parse("every monday", s).unwrap();
        assert_eq!((r.every, r.interval), (Frequency::Week, 1));
        assert_eq!(r.weekdays, [Weekday::Monday]);
        let r = parse("every 2 weeks on mon and thu", s).unwrap();
        assert_eq!(r.interval, 2);
        assert_eq!(r.weekdays, [Weekday::Monday, Weekday::Thursday]);
        let r = parse("the 1st of each month", s).unwrap();
        assert_eq!((r.every, r.month_day), (Frequency::Month, Some(1)));
        let r = parse("monthly on the last fri", s).unwrap();
        assert_eq!(
            r.nth,
            Some(NthWeekday {
                nth: -1,
                weekday: Weekday::Friday
            })
        );
        let r = parse("3 days after done", s).unwrap();
        assert_eq!(
            (r.every, r.interval, r.from),
            (Frequency::Day, 3, RepeatFrom::Completion)
        );
        let r = parse("every other week until 2027-06-30", s).unwrap();
        assert_eq!((r.interval, r.until), (2, Some(d("2027-06-30"))));
        let r = parse("daily for 10 times", s).unwrap();
        assert_eq!(r.count, Some(10));
        let r = parse("weekly, 5 times", s).unwrap();
        assert_eq!(r.count, Some(5));
        let r = parse("weekdays", s).unwrap();
        assert_eq!(r.weekdays.len(), 5);
        let r = parse("yearly in march on the 1st", s).unwrap();
        assert_eq!(
            (r.every, r.month, r.month_day),
            (Frequency::Year, Some(3), Some(1))
        );
        let r = parse("every month on 15", s).unwrap();
        assert_eq!(r.month_day, Some(15));
        assert!(parse("sometimes", s).is_err());
        assert!(parse("", s).is_err());
        assert!(parse("every monday after done", s).is_err());
    }

    proptest::proptest! {
        #[test]
        fn the_next_day_is_later_and_the_same_from_any_day_in_between(
            every in 0usize..4,
            interval in 1u16..5,
            start in 19_000i64..21_000,
            after in 19_000i64..21_500,
            pick in 0u8..4,
            day in 1i8..32,
            nth in 1i8..6,
            wd in 0usize..7,
        ) {
            let start = Day::from_days_since_epoch(start).unwrap();
            let after = Day::from_days_since_epoch(after).unwrap();
            let unit = [Frequency::Day, Frequency::Week, Frequency::Month, Frequency::Year][every];
            let mut r = Rule::every(unit, interval, start);
            match (unit, pick) {
                (Frequency::Week, 1) => r.weekdays = vec![WEEKDAYS[wd], WEEKDAYS[(wd + 3) % 7]],
                (Frequency::Month | Frequency::Year, 1) => r.month_day = Some(day),
                (Frequency::Month | Frequency::Year, 2) => {
                    r.nth = Some(NthWeekday { nth, weekday: WEEKDAYS[wd] });
                }
                (Frequency::Month | Frequency::Year, 3) => r.month_day = Some(-1),
                _ => {}
            }
            let next = r.next_after(after).unwrap();
            proptest::prop_assert!(next > after && next >= start);
            let mut d = after.add_days(1).unwrap();
            while d < next {
                proptest::prop_assert_eq!(r.next_after(d), Some(next), "from {}", d);
                d = d.add_days(1).unwrap();
                if d.days_since_epoch() - after.days_since_epoch() > 40 {
                    break;
                }
            }
            if unit == Frequency::Week {
                proptest::prop_assert!(r.days_of_week().contains(&next.weekday()));
            }
        }
    }
}
