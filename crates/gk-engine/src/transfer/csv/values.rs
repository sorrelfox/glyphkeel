// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Values of a CSV file as this program's: dates in the forms Jira and
//! Redmine write, durations in their units, priority names, keys and
//! Redmine's list of related issues.

use gk_api::ops::common::Priority;
use gk_api::ops::csv_import::{CsvDateOrder, CsvUnit};
use gk_api::ops::task::RelationKind;
use gk_core::{Day, DurationUnits, fold};

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

fn month_named(text: &str) -> Option<u8> {
    let t = text.trim_end_matches('.').to_ascii_lowercase();
    if t.len() < 3 {
        return None;
    }
    MONTHS
        .iter()
        .position(|m| t.starts_with(m))
        .and_then(|i| u8::try_from(i + 1).ok())
}

fn year_of(text: &str) -> Option<u16> {
    let n: u16 = text.parse().ok()?;
    match text.len() {
        2 => Some(if n < 70 { 2000 + n } else { 1900 + n }),
        4 => Some(n),
        _ => None,
    }
}

/// The date part of `text`: what comes before a time (`T10:30`, ` 3:45
/// PM`, ` 10:30:00`).
fn date_part(text: &str) -> &str {
    let t = text.trim();
    let cut = t
        .find(['T', ' '])
        .filter(|&i| t[i + 1..].contains(':') || t[i..].starts_with('T'));
    match cut {
        // `15 Jan 2024` has spaces of its own: cut only before a time.
        Some(i) if t[..i].bytes().any(|b| b.is_ascii_digit()) && t[..i].len() >= 6 => &t[..i],
        _ => t,
    }
}

/// The three numbers of a numeric date (`31/12/2025`, `2025-12-31`,
/// `31.12.25`), and its separator.
fn numeric_parts(text: &str) -> Option<([&str; 3], char)> {
    let sep = text.chars().find(|c| matches!(c, '/' | '.' | '-'))?;
    let parts: Vec<&str> = text.trim_end_matches('.').split(sep).collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    Some(([parts[0], parts[1], parts[2]], sep))
}

/// The order that `values` (numeric dates) must be read in, if they show
/// it: a first number above 12 means day first, a second one above 12
/// month first, four digits first year first. Else dates with dots are
/// day first and those with slashes month first, as Redmine writes them in
/// English.
pub(crate) fn detect_order<'a>(values: impl Iterator<Item = &'a str>) -> Option<CsvDateOrder> {
    let mut seen = false;
    let mut dots = false;
    let mut slashes = false;
    for v in values {
        let Some(([a, b, _], sep)) = numeric_parts(date_part(v)) else {
            continue;
        };
        seen = true;
        if a.len() == 4 {
            return Some(CsvDateOrder::Ymd);
        }
        let (a, b): (u32, u32) = (a.parse().unwrap_or(0), b.parse().unwrap_or(0));
        if a > 12 {
            return Some(CsvDateOrder::Dmy);
        }
        if b > 12 {
            return Some(CsvDateOrder::Mdy);
        }
        dots |= sep == '.';
        slashes |= sep == '/';
    }
    match (seen, dots, slashes) {
        (false, _, _) => None,
        (true, true, _) => Some(CsvDateOrder::Dmy),
        (true, false, true) => Some(CsvDateOrder::Mdy),
        (true, false, false) => Some(CsvDateOrder::Dmy),
    }
}

/// A date written as Jira (`15/Jan/24 3:45 PM`), Redmine (`2024-01-15`,
/// `01/15/2024`, `15.01.2024`) or a spreadsheet writes it; any time is
/// left out.
pub(crate) fn parse_date(text: &str, order: CsvDateOrder) -> Option<Day> {
    let t = date_part(text);
    if let Ok(day) = t.parse::<Day>() {
        return Some(day);
    }
    // Month names: `15/Jan/24`, `15-Jan-2024`, `15 Jan 2024`, `Jan 15, 2024`.
    let words: Vec<&str> = t
        .split(['/', '-', ' ', ','])
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() == 3 {
        if let Some(m) = month_named(words[1]) {
            let d: u8 = words[0].parse().ok()?;
            return Day::new(year_of(words[2])?, m, d).ok();
        }
        if let Some(m) = month_named(words[0]) {
            let d: u8 = words[1].parse().ok()?;
            return Day::new(year_of(words[2])?, m, d).ok();
        }
    }
    let ([a, b, c], _) = numeric_parts(t)?;
    let (y, m, d) = if a.len() == 4 {
        (a, b, c)
    } else {
        match order {
            CsvDateOrder::Dmy => (c, b, a),
            CsvDateOrder::Mdy => (c, a, b),
            CsvDateOrder::Ymd => (a, b, c),
        }
    };
    Day::new(year_of(y)?, m.parse().ok()?, d.parse().ok()?).ok()
}

/// A number written with a decimal point, or a comma when `comma`.
fn number(text: &str, comma: bool) -> Option<f64> {
    let t = text.trim();
    let t = if comma {
        t.replace('.', "").replace(',', ".")
    } else {
        t.replace(',', "")
    };
    t.parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0)
}

/// A duration in minutes, from `text` in `unit`.
pub(crate) fn parse_duration(
    text: &str,
    unit: CsvUnit,
    comma: bool,
    units: DurationUnits,
) -> Option<i64> {
    let per = match unit {
        CsvUnit::Seconds => 1.0 / 60.0,
        CsvUnit::Minutes => 1.0,
        CsvUnit::Hours => 60.0,
        CsvUnit::Days => f64::from(units.hours_per_day()) * 60.0,
        CsvUnit::Text => {
            if let Some(n) = number(text, comma) {
                n * 60.0
            } else {
                return gk_core::parse_minutes(text, units).ok();
            }
        }
    };
    let n = match unit {
        CsvUnit::Text => return finish(per),
        _ => number(text, comma)?,
    };
    finish(n * per)
}

fn finish(minutes: f64) -> Option<i64> {
    let m = minutes.round();
    #[allow(clippy::cast_possible_truncation)]
    let whole = m as i64;
    (m.is_finite() && (0..=gk_core::duration::MAX_MINUTES).contains(&whole)).then_some(whole)
}

/// The priority a name of Jira or Redmine stands for.
pub(crate) fn known_priority(text: &str) -> Option<Priority> {
    Some(match fold(text.trim()).as_str() {
        "" | "none" => Priority::None,
        "immediate" | "blocker" | "highest" | "urgent" => Priority::Urgent,
        "critical" | "high" | "major" => Priority::High,
        "medium" | "normal" => Priority::Normal,
        "low" | "minor" | "lowest" | "trivial" => Priority::Low,
        _ => return None,
    })
}

/// A key or ID as rows refer to it: `#12` and `12` are the same, and case
/// doesn't matter.
pub(crate) fn reference(text: &str) -> String {
    fold(text.trim().trim_start_matches('#').trim())
}

/// The links of Redmine's "Related issues": `Blocks #12, Related to #13,
/// Follows (2 days) #14`.
pub(crate) fn redmine_relations(text: &str) -> Vec<Result<(RelationKind, String), String>> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|item| {
            let Some(hash) = item.rfind('#') else {
                return Err(item.to_owned());
            };
            let label = item[..hash].trim();
            let label = match label.find('(') {
                Some(i) => label[..i].trim(),
                None => label,
            };
            let target = item[hash + 1..].trim().to_owned();
            let kind = match fold(label).as_str() {
                "related to" => RelationKind::Relates,
                "is duplicate of" => RelationKind::Duplicates,
                "has duplicate" => RelationKind::DuplicatedBy,
                "blocks" | "precedes" => RelationKind::Blocks,
                "blocked by" | "follows" => RelationKind::BlockedBy,
                "copied to" => RelationKind::ClonedBy,
                "copied from" => RelationKind::Clones,
                _ => return Err(item.to_owned()),
            };
            if target.is_empty() {
                Err(item.to_owned())
            } else {
                Ok((kind, target))
            }
        })
        .collect()
}

/// The text of a Jira comment column (`15/Jan/24 10:30 AM;accountId;text`),
/// without the date and author in front.
pub(crate) fn jira_comment(text: &str) -> &str {
    let mut parts = text.splitn(3, ';');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(date), Some(author), Some(body))
            if parse_date(date, CsvDateOrder::Dmy).is_some()
                && !author.contains(char::is_whitespace) =>
        {
            body
        }
        _ => text,
    }
}

/// What a status named `name` is likely to mean.
pub(crate) fn guess_category(name: &str) -> gk_api::ops::common::Category {
    use gk_api::ops::common::Category;
    let n = fold(name);
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["cancel", "reject", "won't", "wont", "invalid", "declined"]) {
        Category::Cancelled
    } else if has(&[
        "done", "closed", "resolved", "complete", "finished", "fixed",
    ]) {
        Category::Done
    } else if has(&[
        "progress", "review", "testing", "feedback", "doing", "develop",
    ]) {
        Category::InProgress
    } else {
        Category::Todo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Day {
        s.parse().expect("day")
    }

    #[test]
    fn dates_of_jira_redmine_and_spreadsheets() {
        let dmy = CsvDateOrder::Dmy;
        let mdy = CsvDateOrder::Mdy;
        assert_eq!(parse_date("15/Jan/24 3:45 PM", dmy), Some(d("2024-01-15")));
        assert_eq!(parse_date("03/Feb/2025", mdy), Some(d("2025-02-03")));
        assert_eq!(parse_date("2024-01-15", mdy), Some(d("2024-01-15")));
        assert_eq!(parse_date("2024-01-15 10:30", mdy), Some(d("2024-01-15")));
        assert_eq!(
            parse_date("2024-01-15T10:30:00Z", mdy),
            Some(d("2024-01-15"))
        );
        assert_eq!(parse_date("01/15/2024", mdy), Some(d("2024-01-15")));
        assert_eq!(parse_date("15.01.2024", dmy), Some(d("2024-01-15")));
        assert_eq!(parse_date("15 Jan 2024", dmy), Some(d("2024-01-15")));
        assert_eq!(parse_date("Jan 15, 2024", dmy), Some(d("2024-01-15")));
        assert_eq!(parse_date("2024/01/15", dmy), Some(d("2024-01-15")));
        assert_eq!(parse_date("31/02/2024", dmy), None);
        assert_eq!(parse_date("soon", dmy), None);
    }

    #[test]
    fn the_date_order_shows_in_the_dates() {
        let o = |v: &[&str]| detect_order(v.iter().copied());
        assert_eq!(o(&["01/02/2024", "25/02/2024"]), Some(CsvDateOrder::Dmy));
        assert_eq!(o(&["01/02/2024", "02/25/2024"]), Some(CsvDateOrder::Mdy));
        assert_eq!(o(&["01.02.2024"]), Some(CsvDateOrder::Dmy));
        assert_eq!(o(&["01/02/2024"]), Some(CsvDateOrder::Mdy));
        assert_eq!(o(&["2024/01/02"]), Some(CsvDateOrder::Ymd));
        assert_eq!(o(&["15/Jan/24"]), None);
    }

    #[test]
    fn durations_in_each_unit() {
        let u = DurationUnits::default();
        assert_eq!(
            parse_duration("7200", CsvUnit::Seconds, false, u),
            Some(120)
        );
        assert_eq!(parse_duration("2.50", CsvUnit::Hours, false, u), Some(150));
        assert_eq!(parse_duration("2,50", CsvUnit::Hours, true, u), Some(150));
        assert_eq!(parse_duration("1", CsvUnit::Days, false, u), Some(480));
        assert_eq!(parse_duration("1h 30m", CsvUnit::Text, false, u), Some(90));
        assert_eq!(parse_duration("1.5", CsvUnit::Text, false, u), Some(90));
        assert_eq!(parse_duration("-3", CsvUnit::Hours, false, u), None);
        assert_eq!(parse_duration("lots", CsvUnit::Hours, false, u), None);
    }

    #[test]
    fn priorities_references_and_relations() {
        assert_eq!(known_priority("Highest"), Some(Priority::Urgent));
        assert_eq!(known_priority("Medium"), Some(Priority::Normal));
        assert_eq!(known_priority("Immediate"), Some(Priority::Urgent));
        assert_eq!(known_priority("whenever"), None);
        assert_eq!(reference(" #12 "), "12");
        assert_eq!(reference("proj-3"), reference("PROJ-3"));
        let r = redmine_relations("Blocks #12, Related to #13, Follows (2 days) #14, Odd");
        assert_eq!(
            r,
            vec![
                Ok((RelationKind::Blocks, "12".to_owned())),
                Ok((RelationKind::Relates, "13".to_owned())),
                Ok((RelationKind::BlockedBy, "14".to_owned())),
                Err("Odd".to_owned()),
            ]
        );
        assert_eq!(
            jira_comment("15/Jan/24 10:30 AM;abc123;Looks good; ship it"),
            "Looks good; ship it"
        );
        assert_eq!(jira_comment("plain; text"), "plain; text");
    }
}
