// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The query corpus (`corpus/queries.txt`) over data set S, with its
//! expected results in four settings: one instant seen from UTC−10, UTC and
//! UTC+14, and the day Europe/Berlin moves its clocks forward. The expected
//! results are the snapshots `query_corpus__*.snap`.

#![allow(clippy::unwrap_used)]

use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;

use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::json;
use tempfile::TempDir;

/// Tuesday 10 March 2026, 11:00 UTC: still the 10th in Honolulu (01:00),
/// already the 11th on Kiritimati (01:00).
const MARCH_10: i64 = 1_773_140_400_000;
/// Sunday 29 March 2026, 12:00 UTC: Berlin moved from UTC+1 to UTC+2 at
/// 01:00 UTC, so its day runs from 23:00 UTC the day before to 22:00 UTC,
/// 23 hours.
const BERLIN_SPRING: i64 = 1_774_785_600_000;

/// The corpus: one query per line, without comments and blank lines.
fn corpus() -> Vec<String> {
    include_str!("corpus/queries.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// A workspace with data set S, seed 1, imported.
fn data_set_s() -> TempDir {
    let dir = TempDir::new().unwrap();
    let archive = dir.path().join("seed-S");
    gk_seed::generate(gk_seed::DataSet::Small, 1)
        .unwrap()
        .write_to(&archive)
        .unwrap();
    let e = Engine::open(&layout(dir.path()), EngineConfig::new(Role::Cli)).unwrap();
    e.call(
        &Context::user(Via::Cli),
        "transfer.import",
        json!({ "path": archive.to_string_lossy() }),
    )
    .unwrap();
    e.close();
    dir
}

fn layout(dir: &Path) -> WorkspaceLayout {
    WorkspaceLayout::new(dir.join("Personal"))
}

/// The workspace in `dir` on a clock stopped at `now`, in `zone`.
fn open_at(dir: &Path, zone: &str, now: i64) -> Engine {
    let mut config = EngineConfig::new(Role::Cli);
    config.clock = Arc::new(move || now);
    config.time_zone = Some(zone.to_owned());
    Engine::open(&layout(dir), config).unwrap()
}

/// The keys `query` finds, in the order of the result.
fn run(e: &Engine, query: &str) -> Vec<String> {
    let out = e
        .call(
            &Context::user(Via::App),
            "task.search",
            json!({ "query": query, "limit": 200 }),
        )
        .unwrap_or_else(|err| panic!("{query}: {err:?}"));
    assert!(out.get("next_cursor").is_none(), "{query}: one page");
    let keys: Vec<String> = out["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(keys.len() as u64, out["total"].as_u64().unwrap(), "{query}");
    keys
}

/// Every query with what it found: the expected results.
fn report(e: &Engine) -> String {
    let mut out = String::new();
    for query in corpus() {
        let keys = run(e, &query);
        let _ = writeln!(out, "{query}\n    {}: {}", keys.len(), keys.join(" "));
    }
    out
}

#[test]
fn the_corpus_has_at_least_fifty_queries_that_all_run() {
    let queries = corpus();
    assert!(queries.len() >= 50, "{} queries", queries.len());
    let mut unique = queries.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), queries.len(), "every query once");
}

#[test]
fn the_corpus_finds_the_expected_tasks_in_three_time_zones() {
    let dir = data_set_s();
    let mut reports = Vec::new();
    for (name, zone) in [
        ("utc_minus_10", "Pacific/Honolulu"),
        ("utc", "UTC"),
        ("utc_plus_14", "Pacific/Kiritimati"),
    ] {
        let e = open_at(dir.path(), zone, MARCH_10);
        let r = report(&e);
        e.close();
        insta::assert_snapshot!(name, r);
        reports.push(r);
    }
    // The same instant is another day on Kiritimati, so relative days
    // select other tasks there.
    assert_ne!(reports[0], reports[2]);
    // Fixed dates mean the same everywhere.
    let e = open_at(dir.path(), "Pacific/Kiritimati", MARCH_10);
    let far = run(&e, "due:2026-03-01..2026-03-31");
    e.close();
    let e = open_at(dir.path(), "Pacific/Honolulu", MARCH_10);
    assert_eq!(run(&e, "due:2026-03-01..2026-03-31"), far);
    e.close();
}

#[test]
fn the_corpus_finds_the_expected_tasks_on_a_daylight_saving_day() {
    let dir = data_set_s();
    let e = open_at(dir.path(), "Europe/Berlin", BERLIN_SPRING);
    insta::assert_snapshot!("berlin_spring_forward", report(&e));

    // TMAPP-22 was closed at 23:05 UTC on the 28th: 00:05 on the 29th in
    // Berlin (still UTC+1). GARDEN-25 was closed at 22:46 UTC on the 29th:
    // 00:46 on the 30th (now UTC+2). With one offset for the whole day
    // either would land on the wrong day.
    let today = run(&e, "closed:today");
    assert!(today.contains(&"TMAPP-22".to_owned()), "{today:?}");
    assert!(!today.contains(&"GARDEN-25".to_owned()), "{today:?}");
    let yesterday = run(&e, "closed:yesterday");
    assert!(!yesterday.contains(&"TMAPP-22".to_owned()), "{yesterday:?}");
    let tomorrow = run(&e, "closed:2026-03-30");
    assert!(tomorrow.contains(&"GARDEN-25".to_owned()), "{tomorrow:?}");
    e.close();
}

/// Rows of `sql` as task keys, for checking a result against the data
/// itself.
fn keys_where(e: &Engine, sql: &str) -> Vec<String> {
    e.store()
        .read(|tx| {
            let mut stmt = tx.prepare(sql)?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, gk_store::StoreError>(rows)
        })
        .unwrap()
}

#[test]
fn some_corpus_results_agree_with_the_data_itself() {
    let dir = data_set_s();
    let e = open_at(dir.path(), "UTC", MARCH_10);
    let sorted = |mut v: Vec<String>| {
        v.sort();
        v
    };
    let key = "p.key || '-' || t.number";
    let from = "FROM tasks t JOIN projects p ON p.id = t.project_id
                WHERE t.deleted_at IS NULL AND p.deleted_at IS NULL AND p.state = 'active'";
    let check = |query: &str, condition: &str| {
        let expected = sorted(keys_where(
            &e,
            &format!("SELECT {key} {from} AND {condition}"),
        ));
        assert_eq!(sorted(run(&e, query)), expected, "{query}");
        expected.len()
    };
    assert!(check("due<2026-02-01", "t.due_date < '2026-02-01'") > 0);
    check("due:none", "t.due_date IS NULL");
    check("estimate>=1d", "t.estimate_min >= 480");
    check("spent>0m", "t.spent_min > 0");
    check(
        "created:yesterday",
        "t.created_at >= 1773014400000 AND t.created_at < 1773100800000",
    );
    check(
        "closed:today",
        "t.closed_at >= 1773100800000 AND t.closed_at < 1773187200000",
    );
    e.close();
}

#[test]
fn every_field_of_the_registry_filters_data_set_s() {
    use gk_query::fields::{FIELDS, FieldType};
    let dir = data_set_s();
    let e = open_at(dir.path(), "UTC", MARCH_10);
    // Data set S has every kind of link but duplicates.
    e.call(
        &Context::user(Via::App),
        "relation.add",
        json!({ "key": "TM-2", "kind": "duplicates", "other": "TM-3" }),
    )
    .unwrap();
    let all = run(&e, "").len();
    let parent = run(&e, "is:parent sort:key")[0].clone();
    let mut walked = 0;
    for field in FIELDS {
        let name = field.name;
        // One condition per field type; a new type fails here until it
        // has one.
        let query = match field.ty {
            FieldType::Key => format!("{name}:TM-1,GARDEN-2"),
            FieldType::Project => format!("{name}:GARDEN"),
            FieldType::Title | FieldType::FullText(_) => format!("{name}:the"),
            FieldType::Status => format!("{name}:Review"),
            FieldType::Category => format!("{name}:done"),
            FieldType::Type => format!("{name}:Bug"),
            FieldType::Priority => format!("{name}>=high"),
            FieldType::Label => format!("{name}:frontend"),
            FieldType::Parent | FieldType::Under => format!("{name}:{parent}"),
            FieldType::Link(_) => format!("{name}:any"),
            FieldType::Date(_) | FieldType::Moment(_) | FieldType::Span => {
                format!("{name}:2026-03-01..2026-04-30")
            }
            FieldType::Duration(_) => format!("{name}>2h"),
            FieldType::Percent => format!("{name}>=50"),
            FieldType::Author => format!("{name}:agent"),
            FieldType::Claimed => format!("{name}:any"),
            FieldType::Count(_) => format!("{name}>0"),
            FieldType::Is => format!("{name}:open"),
            FieldType::Has => format!("{name}:due"),
        };
        let found = run(&e, &query).len();
        assert!(
            found > 0 && found < all,
            "{query} selects {found} of {all} tasks"
        );
        // The same condition negated selects the others.
        let others = run(&e, &format!("-({query})")).len();
        assert_eq!(found + others, all, "{query} and its negation");
        walked += 1;
    }
    assert_eq!(walked, FIELDS.len());
    e.close();
}
