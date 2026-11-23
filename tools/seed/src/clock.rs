// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Fixed time, IDs, ranks and slugs. Nothing here reads the system clock or
//! the operating system's randomness.

use crate::rng::Rng;

pub const MINUTE_MS: i64 = 60_000;
pub const HOUR_MS: i64 = 60 * MINUTE_MS;
pub const DAY_MS: i64 = 24 * HOUR_MS;

/// Every data set starts on Monday 2026-01-05, 09:00 UTC, and spans the days
/// after it. The instant is recorded in the manifest.
pub fn anchor_ms() -> i64 {
    days_from_civil(2026, 1, 5) * DAY_MS + 9 * HOUR_MS
}

/// Days since 1970-01-01 for a proleptic Gregorian date (H. Hinnant's
/// algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The `YYYY-MM-DD` date of a day number (inverse of [`days_from_civil`]).
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The calendar day (UTC) of a moment, as `YYYY-MM-DD`.
pub fn day_of(ms: i64) -> String {
    day_string(ms.div_euclid(DAY_MS))
}

pub fn day_string(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A UUIDv7 for the moment `ms`, with the random bits drawn from `rng`.
pub fn uuid_v7(ms: i64, rng: &mut Rng) -> String {
    let ts = (ms as u64) & 0xFFFF_FFFF_FFFF;
    let a = rng.next_u64();
    let b = rng.next_u64();
    let mut bytes = [0u8; 16];
    bytes[..6].copy_from_slice(&ts.to_be_bytes()[2..]);
    bytes[6..8].copy_from_slice(&a.to_be_bytes()[..2]);
    bytes[6] = 0x70 | (bytes[6] & 0x0F);
    bytes[8..].copy_from_slice(&b.to_be_bytes());
    bytes[8] = 0x80 | (bytes[8] & 0x3F);
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Width of a rank in digits.
pub const RANK_WIDTH: usize = 7;
const RANK_DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const RANK_STEP: u64 = 36 * 36 * 36;

/// The rank of the `index`th item in an ordered list: lower-case base-36
/// digits of a fixed width, spaced so that many items fit between any two
/// neighbours. Ranks sort as plain byte strings.
pub fn rank(index: usize) -> String {
    let mut value = (index as u64 + 1) * RANK_STEP;
    let mut out = [b'0'; RANK_WIDTH];
    for slot in out.iter_mut().rev() {
        *slot = RANK_DIGITS[(value % 36) as usize];
        value /= 36;
    }
    debug_assert_eq!(value, 0, "rank overflow");
    out.iter().map(|&c| char::from(c)).collect()
}

/// A wiki slug: Unicode kept, spaces become `-`, characters that are unsafe
/// in file names and links are removed, and the result is never empty.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_whitespace() {
            if !out.ends_with('-') && !out.is_empty() {
                out.push('-');
            }
        } else if c.is_control()
            || matches!(
                c,
                '/' | '\\'
                    | ':'
                    | '*'
                    | '?'
                    | '"'
                    | '<'
                    | '>'
                    | '|'
                    | '#'
                    | '%'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '^'
                    | '`'
                    | '\u{200B}'..='\u{200F}'
                    | '\u{202A}'..='\u{202E}'
                    | '\u{2066}'..='\u{2069}'
                    | '\u{FEFF}'
            )
        {
            continue;
        } else {
            out.extend(c.to_lowercase());
        }
    }
    let trimmed = out.trim_matches(|c| c == '-' || c == '.');
    if trimmed.is_empty() {
        "page".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(day_string(days_from_civil(2026, 1, 5)), "2026-01-05");
        assert_eq!(day_string(days_from_civil(2028, 2, 29)), "2028-02-29");
        for d in -1000..40_000 {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
    }

    #[test]
    fn uuid_v7_shape() {
        let mut rng = Rng::new(1, 0);
        let id = uuid_v7(anchor_ms(), &mut rng);
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "7");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"));
        // The first 48 bits are the moment.
        let ts = u64::from_str_radix(&id[0..8], 16).unwrap() << 16
            | u64::from_str_radix(&id[9..13], 16).unwrap();
        assert_eq!(ts as i64, anchor_ms());
    }

    #[test]
    fn ranks_sort_in_order() {
        let ranks: Vec<String> = (0..30_000).map(rank).collect();
        assert!(ranks.windows(2).all(|w| w[0] < w[1]));
        assert!(ranks.iter().all(|r| r.len() == RANK_WIDTH));
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Release checklist"), "release-checklist");
        assert_eq!(slug("../../etc/passwd"), "etcpasswd");
        assert_eq!(slug("Задачи и планы"), "задачи-и-планы");
        assert_eq!(slug("<>"), "page");
    }
}
