// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Golden tests of the SQL a query compiles to. A change in the snapshot is
//! a change in what queries mean or how fast they run; review it as such.

use std::fmt::Write as _;

use gk_core::{DurationUnits, Weekday};
use gk_query::{CheckEnv, Compiled, FixedCalendar, Param, Scope, SqlEnv, compile};

fn golden(queries: &[&str], scope: Option<Scope>) -> String {
    let cal = FixedCalendar {
        today: "2026-11-23".parse().expect("day"),
        offset_minutes: 60,
        week_start: Weekday::Monday,
    };
    let mut out = String::new();
    for src in queries {
        let (_, checked) = compile(
            src,
            CheckEnv {
                calendar: &cal,
                units: DurationUnits::default(),
            },
        )
        .unwrap_or_else(|e| panic!("{src}: {e}"));
        let compiled = Compiled::new(
            &checked,
            &SqlEnv {
                now: 1_795_431_300_000,
                scope: scope.clone(),
                default_sort: Vec::new(),
            },
        );
        let sql = compiled.page(50, 0);
        let params: Vec<String> = sql
            .params
            .iter()
            .map(|p| match p {
                Param::Int(n) => n.to_string(),
                Param::Text(t) => format!("{t:?}"),
            })
            .collect();
        let _ = writeln!(
            out,
            "-- {src}\n{}\n-- params: {}\n",
            sql.text,
            params.join(", ")
        );
    }
    out
}

#[test]
fn a_span_of_days() {
    insta::assert_snapshot!(golden(
        &[
            "during:2026-11-01..2026-11-30",
            "during:this-week,2026-12-24 sort:start",
            "is:open -during:today",
        ],
        None,
    ));
}

#[test]
fn filters() {
    insta::assert_snapshot!(golden(
        &[
            "",
            "is:open due<+7d",
            "label:bug,crash -status:Done updated>-3d",
            "type:Feature priority>=High sort:due,-priority",
            "project:TM/* is:overdue",
            "(label:bug OR type:Bug) is:open -is:blocked",
            "estimate:1h..4h -has:start",
            "created:this-month author:agent",
            "parent:TM-12",
            "parent:none",
            "claimed:\"Build Bot\"",
            "TM-41",
            "title:\"login page\" title=Exact",
            "category:closed progress>=50% comments>0",
            "due:none,today remaining<1h",
            "blocks:any relates:TM-2 under:TM-1",
        ],
        None,
    ));
}

#[test]
fn text_and_relevance() {
    insta::assert_snapshot!(golden(
        &[
            "timeout",
            "\"connection timeout\" -crash",
            "description:\"stack trace\" comment:flaky sort:updated",
            "%%",
        ],
        None,
    ));
}

#[test]
fn a_project_scope() {
    insta::assert_snapshot!(golden(
        &["is:open"],
        Some(Scope {
            project: "TM".to_owned(),
            subprojects: true,
        }),
    ));
}
