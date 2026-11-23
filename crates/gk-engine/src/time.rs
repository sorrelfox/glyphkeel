// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Time stamps: milliseconds since 1970 in storage, RFC 3339 in outputs.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use gk_core::Day;

/// Where the engine gets the current time from: milliseconds since 1970 UTC.
/// Tests pass a fixed clock.
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// The system clock.
#[must_use]
pub fn system_clock() -> Clock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
    })
}

/// `ms` as RFC 3339 in UTC with milliseconds: `2026-11-23T09:15:00.000Z`.
#[must_use]
pub fn rfc3339(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rest = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The time zone of "today", of local times of day and of the days time
/// entries fall on: the computer's own, or one named for tests.
#[derive(Debug, Clone)]
pub(crate) struct Zone(jiff::tz::TimeZone);

impl Zone {
    /// The zone named `name` (IANA, `Europe/Berlin`), or the system's when
    /// `None` or unknown.
    pub(crate) fn new(name: Option<&str>) -> Self {
        let named = name.and_then(|n| match jiff::tz::TimeZone::get(n) {
            Ok(tz) => Some(tz),
            Err(e) => {
                tracing::warn!(zone = n, error = %e, "unknown time zone; using the system's");
                None
            }
        });
        Self(named.unwrap_or_else(jiff::tz::TimeZone::system))
    }

    fn local(&self, ms: i64) -> Option<jiff::civil::DateTime> {
        let ts = jiff::Timestamp::from_millisecond(ms).ok()?;
        Some(self.0.to_datetime(ts))
    }

    /// The local day of moment `ms`.
    pub(crate) fn day_of(&self, ms: i64) -> Day {
        self.local(ms)
            .and_then(|dt| {
                let d = dt.date();
                Day::new(
                    u16::try_from(d.year()).ok()?,
                    u8::try_from(d.month()).ok()?,
                    u8::try_from(d.day()).ok()?,
                )
                .ok()
            })
            .unwrap_or_else(|| {
                Day::from_days_since_epoch(ms.div_euclid(86_400_000))
                    .unwrap_or_else(|_| Day::new(1970, 1, 1).unwrap_or_else(|_| unreachable!()))
            })
    }

    /// The moment of `minute` (minutes after midnight) on local `day`. A
    /// time skipped by a clock change is taken after the change; a time
    /// that happens twice, the first time.
    pub(crate) fn moment(&self, day: Day, minute: u32) -> i64 {
        let local = jiff::civil::DateTime::new(
            i16::try_from(day.year()).unwrap_or(1970),
            i8::try_from(day.month()).unwrap_or(1),
            i8::try_from(day.day()).unwrap_or(1),
            i8::try_from(minute / 60 % 24).unwrap_or(0),
            i8::try_from(minute % 60).unwrap_or(0),
            0,
            0,
        );
        local
            .ok()
            .and_then(|dt| self.0.to_timestamp(dt).ok())
            .map_or_else(
                || (day.days_since_epoch() * 1440 + i64::from(minute)) * 60_000,
                jiff::Timestamp::as_millisecond,
            )
    }

    /// Reads a moment: RFC 3339 with an offset, or a local `YYYY-MM-DD
    /// HH:MM` (also with `T`, and with seconds).
    pub(crate) fn parse_moment(&self, text: &str) -> Option<i64> {
        let text = text.trim();
        if let Ok(ts) = text.parse::<jiff::Timestamp>() {
            return Some(ts.as_millisecond());
        }
        let local = text
            .replacen(' ', "T", 1)
            .parse::<jiff::civil::DateTime>()
            .ok()?;
        self.0
            .to_timestamp(local)
            .ok()
            .map(jiff::Timestamp::as_millisecond)
    }
}

/// `HH:MM` as minutes after midnight.
pub(crate) fn clock_minutes(text: &str) -> Option<u32> {
    let (h, m) = text.trim().split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// Minutes after midnight as `HH:MM`.
pub(crate) fn clock_text(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60 % 24, minutes % 60)
}

/// Year, month and day of a day number counted from 1970-01-01 (Howard
/// Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(1_795_431_300_123), "2026-11-23T10:55:00.123Z");
        assert_eq!(rfc3339(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(rfc3339(-1), "1969-12-31T23:59:59.999Z");
    }

    #[test]
    fn local_days_and_moments_follow_the_zone() {
        let berlin = Zone::new(Some("Europe/Berlin"));
        // 2026-11-23T23:30Z is already the 24th in Berlin (UTC+1).
        let late = 1_795_476_600_000;
        assert_eq!(rfc3339(late), "2026-11-23T23:30:00.000Z");
        assert_eq!(berlin.day_of(late).to_string(), "2026-11-24");
        assert_eq!(
            Zone::new(Some("UTC")).day_of(late).to_string(),
            "2026-11-23"
        );
        let day: Day = "2026-11-24".parse().unwrap();
        assert_eq!(
            rfc3339(berlin.moment(day, 9 * 60)),
            "2026-11-24T08:00:00.000Z"
        );
        // The clock goes forward at 02:00 on 2026-03-29: 02:30 doesn't exist.
        let spring: Day = "2026-03-29".parse().unwrap();
        assert_eq!(
            rfc3339(berlin.moment(spring, 150)),
            "2026-03-29T01:30:00.000Z"
        );
    }

    #[test]
    fn moments_are_read_as_rfc_3339_or_local_time() {
        let berlin = Zone::new(Some("Europe/Berlin"));
        let at = |t: &str| berlin.parse_moment(t).map(rfc3339);
        assert_eq!(
            at("2026-11-24T09:00:00Z").unwrap(),
            "2026-11-24T09:00:00.000Z"
        );
        assert_eq!(
            at("2026-11-24T09:00:00+02:00").unwrap(),
            "2026-11-24T07:00:00.000Z"
        );
        assert_eq!(at("2026-11-24 09:00").unwrap(), "2026-11-24T08:00:00.000Z");
        assert_eq!(at("2026-11-24T09:00").unwrap(), "2026-11-24T08:00:00.000Z");
        assert_eq!(at("tomorrow"), None);
        assert_eq!(clock_minutes("09:30"), Some(570));
        assert_eq!(clock_minutes("24:00"), None);
        assert_eq!(clock_text(570), "09:30");
    }
}
