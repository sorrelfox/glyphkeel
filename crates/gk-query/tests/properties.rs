// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Property tests: the parser never panics, the normal form parses back to
//! the same tree, and a long query parses quickly.

use std::time::{Duration, Instant};

use gk_core::{DurationUnits, Weekday};
use gk_query::syntax::{Expr, KeyTerm, Negation, Op, Predicate, Span, TextTerm, Value, ValueKind};
use gk_query::{
    CheckEnv, Compiled, FixedCalendar, Query, SqlEnv, check, chips, parse, parse_with_tokens, print,
};
use proptest::prelude::*;

fn calendar() -> FixedCalendar {
    FixedCalendar {
        today: "2026-11-23".parse().expect("day"),
        offset_minutes: 60,
        week_start: Weekday::Monday,
    }
}

/// Everything a query can go through without a workspace.
fn run_all(src: &str) {
    let (result, tokens) = parse_with_tokens(src);
    for t in &tokens {
        assert!(t.span.start <= t.span.end && t.span.end <= src.len());
        assert!(src.is_char_boundary(t.span.start) && src.is_char_boundary(t.span.end));
    }
    match result {
        Ok(query) => {
            let _ = chips(&query);
            let normal = print(&query);
            let again = parse(&normal).expect("the normal form parses");
            assert_eq!(query.shape(), again.shape(), "{src:?} -> {normal:?}");
            let cal = calendar();
            if let Ok(checked) = check(
                &query,
                CheckEnv {
                    calendar: &cal,
                    units: DurationUnits::default(),
                },
            ) {
                let _ = checked.names();
                let sql = Compiled::new(
                    &checked,
                    &SqlEnv {
                        now: 0,
                        scope: None,
                        default_sort: Vec::new(),
                    },
                )
                .page(50, 0);
                assert_eq!(sql.text.matches('?').count(), sql.params.len(), "{src:?}");
            }
        }
        Err(e) => {
            assert!(
                e.span.start <= e.span.end && e.span.end <= src.len(),
                "{src:?}"
            );
            assert!(!e.message().is_empty());
        }
    }
}

const PIECES: &[&str] = &[
    " ",
    "  ",
    "(",
    ")",
    "-",
    "\"",
    "'",
    ",",
    ":",
    "!=",
    "<",
    ">=",
    "..",
    "AND",
    "OR",
    "NOT",
    "status",
    "label",
    "due",
    "sort",
    "estimate",
    "is",
    "has",
    "TM-41",
    "open",
    "today",
    "+7d",
    "1h",
    "none",
    "any",
    "é",
    "日本",
    "\\",
    "relevance",
    "-due",
    "prio",
    "x",
    "\t",
];

fn piece_soup() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(PIECES), 0..24).prop_map(|v| v.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn any_text_is_safe(src in ".{0,64}") {
        run_all(&src);
    }

    #[test]
    fn any_mix_of_query_pieces_is_safe(src in piece_soup()) {
        run_all(&src);
    }
}

// Random syntax trees ------------------------------------------------------

const FIELDS: &[&str] = &[
    "status", "label", "due", "estimate", "is", "project", "title", "prio",
];
const OPS: &[Op] = &[Op::Is, Op::Eq, Op::Ne, Op::Lt, Op::Le, Op::Gt, Op::Ge];

fn value() -> impl Strategy<Value = Value> {
    prop_oneof![
        "[a-z][a-z0-9+]{0,5}".prop_map(|text| Value {
            kind: ValueKind::Word {
                text,
                quoted: false
            },
            span: Span::default(),
        }),
        "[a-z \"\\\\,()]{1,8}".prop_map(|text| Value {
            kind: ValueKind::Word { text, quoted: true },
            span: Span::default(),
        }),
        ("[0-9]{1,2}h", "[0-9]{1,2}h").prop_map(|(a, b)| Value {
            kind: ValueKind::Range(a, b),
            span: Span::default(),
        }),
    ]
}

fn leaf() -> impl Strategy<Value = Expr> {
    prop_oneof![
        (
            prop::sample::select(FIELDS),
            prop::sample::select(OPS),
            prop::collection::vec(value(), 1..3)
        )
            .prop_map(|(field, op, values)| Expr::Pred(Predicate {
                field: field.to_owned(),
                field_span: Span::default(),
                op,
                op_span: Span::default(),
                values,
                span: Span::default(),
            })),
        "[a-z]{1,6}"
            .prop_filter("not a field", |w| !w.is_empty())
            .prop_map(|text| Expr::Text(TextTerm {
                text,
                quoted: false,
                span: Span::default(),
            })),
        "[a-zA-Z0-9 :()\"-]{0,10}".prop_map(|text| Expr::Text(TextTerm {
            text,
            quoted: true,
            span: Span::default(),
        })),
        ("[A-Z][A-Z0-9]{1,3}", 1u32..5000).prop_map(|(project, number)| Expr::Key(KeyTerm {
            project,
            number,
            span: Span::default(),
        })),
    ]
}

fn tree() -> impl Strategy<Value = Expr> {
    leaf().prop_recursive(4, 32, 4, |inner| {
        let unary = prop_oneof![
            inner
                .clone()
                .prop_filter("an atom", |e| !matches!(e, Expr::And(_) | Expr::Or(_))),
            (
                inner
                    .clone()
                    .prop_filter("an atom", |e| !matches!(e, Expr::And(_) | Expr::Or(_))),
                prop::bool::ANY
            )
                .prop_map(|(e, minus)| Expr::Not(
                    Box::new(e),
                    if minus {
                        Negation::Minus
                    } else {
                        Negation::Not
                    },
                    Span::default()
                )),
            inner
                .clone()
                .prop_map(|e| Expr::Group(Box::new(e), Span::default())),
        ];
        let and = prop::collection::vec(unary.clone(), 2..4).prop_map(Expr::And);
        let branch = prop_oneof![unary.clone(), and.clone()];
        prop_oneof![
            unary,
            and,
            prop::collection::vec(branch, 2..4).prop_map(Expr::Or),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn printed_trees_parse_back_to_themselves(tree in tree()) {
        let query = Query { filter: Some(tree), sort: None };
        let text = print(&query);
        let parsed = parse(&text).map_err(|e| TestCaseError::fail(format!("{text:?}: {e}")))?;
        prop_assert_eq!(parsed.shape(), query.shape(), "{}", text);
    }
}

#[test]
fn a_four_kilobyte_query_parses_quickly() {
    let mut src = String::new();
    while src.len() < 4000 {
        src.push_str("(label:bug,crash OR -status:\"in progress\") due<+7d estimate:1h..4h word ");
    }
    let start = Instant::now();
    for _ in 0..10 {
        let (result, _) = parse_with_tokens(&src);
        assert!(result.is_ok());
    }
    let each = start.elapsed() / 10;
    let budget = if cfg!(debug_assertions) {
        Duration::from_millis(50)
    } else {
        Duration::from_millis(5)
    };
    assert!(each < budget, "{each:?} for {} bytes", src.len());
}
