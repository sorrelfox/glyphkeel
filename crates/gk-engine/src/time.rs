// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Time stamps: milliseconds since 1970 in storage, RFC 3339 in outputs.

use std::sync::{Arc, PoisonError, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use gk_api::OpError;
use gk_core::Day;
use gk_store::{SYSTEM_TIME_ZONE, UserSettings};

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
/// entries fall on: the person's chosen zone, or the computer's own. The app
/// can change it while it runs ([`crate::Engine::set_time_zone`]); every
/// copy follows.
#[derive(Debug, Clone)]
pub(crate) struct Zone(Arc<RwLock<jiff::tz::TimeZone>>);

impl Zone {
    /// The zone named `name` (IANA, `Europe/Berlin`), or the system's when
    /// `None` or unknown.
    pub(crate) fn new(name: Option<&str>) -> Self {
        Self(Arc::new(RwLock::new(resolve(name))))
    }

    /// Switches every copy of this zone to `name` (`None`: the system's).
    pub(crate) fn set(&self, name: Option<&str>) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = resolve(name);
    }

    /// The zone's IANA name, when it has one.
    pub(crate) fn name(&self) -> Option<String> {
        self.tz().iana_name().map(str::to_owned)
    }

    fn tz(&self) -> jiff::tz::TimeZone {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn local(&self, ms: i64) -> Option<jiff::civil::DateTime> {
        let ts = jiff::Timestamp::from_millisecond(ms).ok()?;
        Some(self.tz().to_datetime(ts))
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
            .and_then(|dt| self.tz().to_timestamp(dt).ok())
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
        self.tz()
            .to_timestamp(local)
            .ok()
            .map(jiff::Timestamp::as_millisecond)
    }
}

/// The zone named `name`, or the system's when `None` or unknown.
fn resolve(name: Option<&str>) -> jiff::tz::TimeZone {
    let named = name.and_then(|n| match jiff::tz::TimeZone::get(n) {
        Ok(tz) => Some(tz),
        Err(e) => {
            tracing::warn!(zone = n, error = %e, "unknown time zone; using the system's");
            None
        }
    });
    named.unwrap_or_else(jiff::tz::TimeZone::system)
}

/// Checks a personal time zone: `"system"` (or blank) gives `None`, the
/// computer's own zone; an IANA name known to the zone database (the
/// system's, or the copy built into the program) gives that name.
///
/// # Errors
/// `invalid` when the name is no known zone.
pub fn check_time_zone(name: &str) -> Result<Option<String>, OpError> {
    let name = name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case(SYSTEM_TIME_ZONE) {
        return Ok(None);
    }
    match jiff::tz::TimeZone::get(name) {
        Ok(tz) => Ok(Some(tz.iana_name().unwrap_or(name).to_owned())),
        Err(_) => Err(
            OpError::invalid(format!("\"{name}\" is not a known time zone."))
                .with_hint("Use \"system\" or an IANA name such as \"Europe/Berlin\"."),
        ),
    }
}

/// The time zone a program runs in: `given` when the person named one for
/// this run (the command line's `--tz`), else the one in the personal
/// settings `settings`, else the computer's (`None`). A zone named for the
/// run must be valid; an unknown one in the settings file is reported in
/// the log and the computer's zone is used.
///
/// # Errors
/// `invalid` when `given` is no known zone.
pub fn personal_time_zone(
    given: Option<&str>,
    settings: &UserSettings,
) -> Result<Option<String>, OpError> {
    if let Some(given) = given {
        return check_time_zone(given);
    }
    let Some(stored) = settings.zone_name() else {
        return Ok(None);
    };
    match check_time_zone(stored) {
        Ok(zone) => Ok(zone),
        Err(e) => {
            tracing::warn!(zone = stored, error = %e.message, "ignoring the personal time zone");
            Ok(None)
        }
    }
}

/// One time zone a person can choose, with its offset from UTC now.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ZoneChoice {
    /// The IANA name (`Europe/Berlin`).
    pub name: String,
    /// The offset from UTC at the moment asked for, in seconds.
    pub offset_seconds: i32,
}

/// Every zone of the zone database with its offset at `now` (milliseconds
/// since 1970), sorted by name, for a picker. Names that are only aliases
/// kept for old data (`US/Pacific`) are left in; the picker searches.
#[must_use]
pub fn time_zones(now: i64) -> Vec<ZoneChoice> {
    let at = jiff::Timestamp::from_millisecond(now).unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    let db = jiff::tz::db();
    let mut zones: Vec<ZoneChoice> = db
        .available()
        .filter_map(|name| {
            let name = name.as_str().to_owned();
            let tz = db.get(&name).ok()?;
            Some(ZoneChoice {
                offset_seconds: tz.to_offset(at).seconds(),
                name,
            })
        })
        .collect();
    zones.sort_by(|a, b| a.name.cmp(&b.name));
    zones.dedup_by(|a, b| a.name == b.name);
    zones
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
    fn a_copy_of_the_zone_database_is_built_in_for_systems_without_one() {
        // Used only when the system has no zone database; without it, named
        // zones would silently become the system's (UTC on such systems).
        let built_in = jiff::tz::TimeZoneDatabase::bundled();
        assert!(built_in.get("Europe/Berlin").is_ok());
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
