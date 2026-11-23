// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Durations: estimates and logged time, stored as whole minutes.
//!
//! People type them as `90m`, `1.5h`, `1,5h`, `1:30`, `2d`, `1w` or
//! combinations such as `1h 30m` and `1w 2d 4h`. Both a decimal point and a
//! decimal comma are accepted. A day and a week are working days and weeks:
//! 8 hours and 5 days unless the workspace settings say otherwise
//! ([`DurationUnits`]). Durations are shown in weeks, days, hours and minutes
//! with the same units ([`format_duration`]), or in hours and minutes only
//! ([`format_minutes`]). The setting changes only how minutes are read and
//! shown, never what is stored.

/// Most minutes a duration may hold (about 4,000 years of round-the-clock
/// work; enough to never matter, small enough to never overflow).
pub const MAX_MINUTES: i64 = i32::MAX as i64;

/// Most digits after the decimal point.
const MAX_FRACTION_DIGITS: u32 = 6;

/// The lengths of a working day and week, from the workspace settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurationUnits {
    hours_per_day: u32,
    days_per_week: u32,
}

impl Default for DurationUnits {
    /// 8 hours a day, 5 days a week.
    fn default() -> Self {
        Self {
            hours_per_day: 8,
            days_per_week: 5,
        }
    }
}

impl DurationUnits {
    /// Units with `hours_per_day` (1–24) and `days_per_week` (1–7).
    pub fn new(hours_per_day: u32, days_per_week: u32) -> Result<Self, DurationError> {
        if !(1..=24).contains(&hours_per_day) || !(1..=7).contains(&days_per_week) {
            return Err(DurationError::Units {
                hours_per_day,
                days_per_week,
            });
        }
        Ok(Self {
            hours_per_day,
            days_per_week,
        })
    }

    /// Hours in a working day.
    #[must_use]
    pub fn hours_per_day(self) -> u32 {
        self.hours_per_day
    }

    /// Days in a working week.
    #[must_use]
    pub fn days_per_week(self) -> u32 {
        self.days_per_week
    }

    fn minutes_per(self, unit: Unit) -> u128 {
        let day = u128::from(self.hours_per_day) * 60;
        match unit {
            Unit::Minute => 1,
            Unit::Hour => 60,
            Unit::Day => day,
            Unit::Week => day * u128::from(self.days_per_week),
        }
    }
}

/// Why text is not a duration. The messages show the accepted forms.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DurationError {
    /// Nothing was typed.
    #[error("enter a duration, for example 90m, 1.5h, 1:30, 2d or 1w")]
    Empty,
    /// The text could not be read.
    #[error("\"{0}\" is not a duration: use for example 90m, 1.5h, 1:30, 2d, 1w or 1h 30m")]
    Malformed(String),
    /// A number without a unit.
    #[error("\"{0}\" needs a unit: m, h, d or w (for example {0}m or {0}h)")]
    MissingUnit(String),
    /// The same unit twice, as in `1h 2h`.
    #[error("\"{0}\" names the same unit twice")]
    RepeatedUnit(String),
    /// Longer than [`MAX_MINUTES`].
    #[error("\"{0}\" is too long a duration")]
    TooLong(String),
    /// Settings out of range.
    #[error(
        "a working day has 1 to 24 hours and a week 1 to 7 days, not {hours_per_day} and {days_per_week}"
    )]
    Units {
        /// The rejected hours per day.
        hours_per_day: u32,
        /// The rejected days per week.
        days_per_week: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Minute,
    Hour,
    Day,
    Week,
}

impl Unit {
    fn parse(word: &str) -> Option<Self> {
        match word.to_ascii_lowercase().as_str() {
            "m" | "min" | "mins" => Some(Self::Minute),
            "h" | "hr" | "hrs" => Some(Self::Hour),
            "d" => Some(Self::Day),
            "w" => Some(Self::Week),
            _ => None,
        }
    }

    fn bit(self) -> u8 {
        match self {
            Self::Minute => 1,
            Self::Hour => 2,
            Self::Day => 4,
            Self::Week => 8,
        }
    }
}

/// Parses a duration into whole minutes. Fractions are rounded to the
/// nearest minute (`0.01h` is 1 minute). Zero is a valid duration; callers
/// that need a positive one check that themselves.
pub fn parse_minutes(input: &str, units: DurationUnits) -> Result<i64, DurationError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(DurationError::Empty);
    }
    let malformed = || DurationError::Malformed(input.to_owned());
    let too_long = || DurationError::TooLong(input.to_owned());

    // `h:mm`
    if let Some((h, m)) = text.split_once(':') {
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if !digits(h) || m.len() != 2 || !digits(m) {
            return Err(malformed());
        }
        let hours: i64 = h.parse().map_err(|_| too_long())?;
        let minutes: i64 = m.parse().map_err(|_| malformed())?;
        if minutes >= 60 {
            return Err(malformed());
        }
        let total = hours
            .checked_mul(60)
            .and_then(|v| v.checked_add(minutes))
            .ok_or_else(too_long)?;
        return if total > MAX_MINUTES {
            Err(too_long())
        } else {
            Ok(total)
        };
    }

    let mut rest = text;
    let mut seen = 0u8;
    let mut total: u128 = 0;
    while !rest.is_empty() {
        // A number: digits, optionally a point and more digits.
        let int_len = rest.bytes().take_while(u8::is_ascii_digit).count();
        let (int_part, after) = rest.split_at(int_len);
        let (frac_part, after) = match after.strip_prefix('.').or_else(|| after.strip_prefix(',')) {
            Some(tail) => {
                let len = tail.bytes().take_while(u8::is_ascii_digit).count();
                tail.split_at(len)
            }
            None => ("", after),
        };
        if int_part.is_empty() && frac_part.is_empty() {
            return Err(malformed());
        }
        if frac_part.len() > MAX_FRACTION_DIGITS as usize || int_part.len() > 12 {
            return Err(too_long());
        }
        let after = after.trim_start();
        let unit_len = after.bytes().take_while(u8::is_ascii_alphabetic).count();
        let (word, after) = after.split_at(unit_len);
        if word.is_empty() {
            return Err(if after.is_empty() && frac_part.is_empty() && seen == 0 {
                DurationError::MissingUnit(text.to_owned())
            } else {
                malformed()
            });
        }
        let unit = Unit::parse(word).ok_or_else(malformed)?;
        if seen & unit.bit() != 0 {
            return Err(DurationError::RepeatedUnit(input.to_owned()));
        }
        seen |= unit.bit();

        // value = int + frac / 10^k, in minutes, rounded half up.
        let scale = 10u128.pow(frac_part.len() as u32);
        let int: u128 = if int_part.is_empty() {
            0
        } else {
            int_part.parse().map_err(|_| too_long())?
        };
        let frac: u128 = if frac_part.is_empty() {
            0
        } else {
            frac_part.parse().map_err(|_| malformed())?
        };
        let scaled = (int * scale + frac) * units.minutes_per(unit);
        total += (scaled + scale / 2) / scale;
        if total > MAX_MINUTES as u128 {
            return Err(too_long());
        }
        rest = after.trim_start();
    }
    i64::try_from(total).map_err(|_| too_long())
}

/// Shows minutes in weeks, days, hours and minutes of working time:
/// `1w 2d 4h 30m`, `3d`, `45m`, `0m`, with the day and week lengths of
/// `units`. Units that are zero are left out. Negative values get a leading
/// minus sign (`-1d 2h`), for "over by". The result parses back to the same
/// minutes with the same units.
#[must_use]
pub fn format_duration(minutes: i64, units: DurationUnits) -> String {
    let sign = if minutes < 0 { "-" } else { "" };
    let mut rest = u128::from(minutes.unsigned_abs());
    let mut parts = Vec::with_capacity(4);
    for (unit, suffix) in [
        (Unit::Week, "w"),
        (Unit::Day, "d"),
        (Unit::Hour, "h"),
        (Unit::Minute, "m"),
    ] {
        let size = units.minutes_per(unit);
        let count = rest / size;
        rest %= size;
        if count > 0 {
            parts.push(format!("{count}{suffix}"));
        }
    }
    if parts.is_empty() {
        return "0m".to_owned();
    }
    format!("{sign}{}", parts.join(" "))
}

/// Shows minutes as hours and minutes: `1h 30m`, `2h`, `45m`, `0m`.
/// Negative values get a leading minus sign (`-1h 5m`), for "over by".
#[must_use]
pub fn format_minutes(minutes: i64) -> String {
    let sign = if minutes < 0 { "-" } else { "" };
    let abs = minutes.unsigned_abs();
    let (h, m) = (abs / 60, abs % 60);
    match (h, m) {
        (0, m) => format!("{sign}{m}m"),
        (h, 0) => format!("{sign}{h}h"),
        (h, m) => format!("{sign}{h}h {m}m"),
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn p(s: &str) -> Result<i64, DurationError> {
        parse_minutes(s, DurationUnits::default())
    }

    #[test]
    fn the_three_spellings_of_ninety_minutes() {
        for s in [
            "1.5h", "90m", "1:30", "1h 30m", "1h30m", " 90 min ", "1.5H", ".5h 60m",
        ] {
            assert_eq!(p(s), Ok(90), "{s}");
        }
        assert_eq!(format_minutes(90), "1h 30m");
    }

    #[test]
    fn days_and_weeks_follow_the_settings() {
        assert_eq!(p("2d"), Ok(2 * 8 * 60));
        assert_eq!(p("1w"), Ok(5 * 8 * 60));
        assert_eq!(p("1w 2d 3h 4m"), Ok(2400 + 960 + 180 + 4));
        let units = DurationUnits::new(6, 4).unwrap();
        assert_eq!(parse_minutes("1d", units), Ok(360));
        assert_eq!(parse_minutes("1w", units), Ok(1440));
        assert!(DurationUnits::new(0, 5).is_err());
        assert!(DurationUnits::new(8, 8).is_err());
    }

    #[test]
    fn fractions_round_to_the_nearest_minute() {
        assert_eq!(p("0.01h"), Ok(1));
        assert_eq!(p("0.005h"), Ok(0));
        assert_eq!(p("1.25h"), Ok(75));
        assert_eq!(p("0.1d"), Ok(48));
        assert_eq!(p("0m"), Ok(0));
        assert_eq!(p("0:00"), Ok(0));
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!(p(""), Err(DurationError::Empty));
        assert_eq!(p("  "), Err(DurationError::Empty));
        assert!(matches!(p("90"), Err(DurationError::MissingUnit(_))));
        assert!(matches!(p("1h 2h"), Err(DurationError::RepeatedUnit(_))));
        for bad in [
            "h", "1x", "1:5", "1:60", "-1h", "1,,5h", "1.,5h", "1h -5m", "1..5h", "1:30m", "abc",
            "1h 30",
        ] {
            assert!(p(bad).is_err(), "{bad}");
        }
        assert!(matches!(p("99999999999h"), Err(DurationError::TooLong(_))));
        assert!(matches!(p("1.0000001h"), Err(DurationError::TooLong(_))));
    }

    #[test]
    fn a_decimal_comma_is_a_decimal_point() {
        assert_eq!(p("1,5h"), Ok(90));
        assert_eq!(p("0,25d"), Ok(120));
        assert_eq!(p("1,5h 10m"), Ok(100));
        assert_eq!(p(",5h"), Ok(30));
    }

    #[test]
    fn formats_in_weeks_and_days() {
        let units = DurationUnits::default();
        assert_eq!(format_duration(0, units), "0m");
        assert_eq!(format_duration(45, units), "45m");
        assert_eq!(format_duration(90, units), "1h 30m");
        assert_eq!(format_duration(8 * 60, units), "1d");
        assert_eq!(
            format_duration(2400 + 960 + 240 + 30, units),
            "1w 2d 4h 30m"
        );
        assert_eq!(format_duration(-(8 * 60 + 120), units), "-1d 2h");
        let short = DurationUnits::new(6, 4).unwrap();
        assert_eq!(format_duration(6 * 60 * 4 + 60, short), "1w 1h");
    }

    #[test]
    fn formats() {
        assert_eq!(format_minutes(0), "0m");
        assert_eq!(format_minutes(45), "45m");
        assert_eq!(format_minutes(120), "2h");
        assert_eq!(format_minutes(16 * 60), "16h");
        assert_eq!(format_minutes(-65), "-1h 5m");
    }

    proptest! {
        #[test]
        fn formatted_durations_parse_back(minutes in 0i64..=MAX_MINUTES) {
            prop_assert_eq!(p(&format_minutes(minutes)), Ok(minutes));
        }

        #[test]
        fn weeks_and_days_parse_back(
            minutes in 0i64..=MAX_MINUTES,
            hours in 1u32..=24,
            days in 1u32..=7,
        ) {
            let units = DurationUnits::new(hours, days).unwrap();
            prop_assert_eq!(parse_minutes(&format_duration(minutes, units), units), Ok(minutes));
        }
    }
}
