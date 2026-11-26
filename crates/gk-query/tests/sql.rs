// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Golden tests of the SQL a query compiles to. A change in the snapshot is
//! a change in what queries mean or how fast they run; review it as such.

use std::fmt::Write as _;

use gk_core::{DurationUnits, FieldKind, Weekday};
use gk_query::fields::CustomField;
use gk_query::{CheckEnv, Compiled, FixedCalendar, Param, Scope, SqlEnv, compile};

fn custom_fields() -> Vec<CustomField> {
    let field = |name: &str, query_name: &str, kind, options: &[&str]| CustomField {
        name: name.to_owned(),
        query_name: query_name.to_owned(),
        kind,
        options: options.iter().map(|o| (*o).to_owned()).collect(),
    };
    vec![
        field("Points", "points", FieldKind::Integer, &[]),
        field("Cost", "cost", FieldKind::Decimal, &[]),
        field("Review on", "review-on", FieldKind::Date, &[]),
        field("Approved", "approved", FieldKind::Boolean, &[]),
        field(
            "Area",
            "area",
            FieldKind::Select,
            &["Front end", "Back end"],
        ),
        field(
            "Platforms",
            "platforms",
            FieldKind::MultiSelect,
            &["Linux", "Windows"],
        ),
        field("Ticket", "ticket", FieldKind::Text, &[]),
    ]
}

fn golden(queries: &[&str], scope: Option<Scope>) -> String {
    let cal = FixedCalendar {
        today: "2026-11-23".parse().expect("day"),
        offset_minutes: 60,
        week_start: Weekday::Monday,
    };
    let custom = custom_fields();
    let mut out = String::new();
    for src in queries {
        let (_, checked) = compile(
            src,
            CheckEnv {
                calendar: &cal,
                units: DurationUnits::default(),
                custom: &custom,
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
fn custom_fields_by_kind() {
    insta::assert_snapshot!(golden(
        &[
            "cf.points>=3 cf.cost<2.5",
            "points:1..5,none sort:-cf.points",
            "cf.review-on:this-week -cf.approved:yes",
            "cf.approved:no",
            "cf.area:\"front END\" cf.platforms!=linux",
            "cf.ticket:abc cf.ticket=\"ABC-1\" sort:cf.area",
            "cf.platforms:none",
        ],
        None,
    ));
}

#[test]
fn the_history() {
    insta::assert_snapshot!(golden(
        &[
            "was:status:Review",
            "was:category:closed -was:type:Bug",
            "was:label:bug,crash was:priority:high was:version:1.0",
            "changed:due>-7d",
            "changed:status:this-week,2026-11-01..2026-11-02",
            "changed:due,start -changed-by:agent",
            "changed-by:me,Helper",
        ],
        None,
    ));
}

#[test]
fn the_history_of_custom_fields() {
    insta::assert_snapshot!(golden(
        &[
            "was:cf.area:\"front END\" -was:cf.approved:yes",
            "was:cf.points:3 was:cf.ticket=ABC-1",
            "changed:status,cf.points",
            "changed:cf.review-on>-7d",
        ],
        None,
    ));
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
