// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The condition builder's model: a query that is a plain AND of
//! conditions shows as chips, one per condition. Free-text words become one
//! "text contains" chip (and negated ones one "doesn't contain" chip). A
//! query with `OR` or parentheses has no chips: it is edited as text.
//!
//! Each chip keeps the spans of its text, so editing a chip replaces only
//! that text and the rest stays as typed.

use crate::syntax::{Expr, Op, Query, Span};

/// What a chip shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChipKind {
    /// A condition on a field.
    Condition {
        /// The field's name in the registry (an alias becomes the name;
        /// unknown fields stay as written).
        field: String,
        /// The operator.
        op: Op,
        /// The values as text.
        values: Vec<String>,
    },
    /// Words or phrases to search for.
    Text {
        /// The words and phrases.
        words: Vec<String>,
    },
}

/// One chip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    /// What it shows.
    pub kind: ChipKind,
    /// "is not" or "doesn't contain".
    pub negated: bool,
    /// The text it stands for, negation included; several pieces for text.
    pub spans: Vec<Span>,
}

/// The chips of `query`, or `None` when it needs the text field (it has
/// `OR` or parentheses).
#[must_use]
pub fn chips(query: &Query) -> Option<Vec<Chip>> {
    let items: Vec<&Expr> = match &query.filter {
        None => Vec::new(),
        Some(Expr::And(items)) => items.iter().collect(),
        Some(e) => vec![e],
    };
    let mut out = Vec::new();
    let mut words: (Vec<String>, Vec<Span>) = (Vec::new(), Vec::new());
    let mut not_words: (Vec<String>, Vec<Span>) = (Vec::new(), Vec::new());
    for item in items {
        let (inner, negated, span) = match item {
            Expr::Not(inner, _, span) => (&**inner, true, *span),
            e => (e, false, e.span()),
        };
        match inner {
            Expr::Pred(p) => {
                let field = crate::fields::find(&p.field)
                    .map_or_else(|| p.field.clone(), |f| f.name.to_owned());
                let (op, negated) = match (p.op, negated) {
                    (Op::Ne, n) => (Op::Is, !n),
                    (op, n) => (op, n),
                };
                out.push(Chip {
                    kind: ChipKind::Condition {
                        field,
                        op,
                        values: p.values.iter().map(crate::syntax::Value::text).collect(),
                    },
                    negated,
                    spans: vec![span],
                });
            }
            Expr::Key(k) => out.push(Chip {
                kind: ChipKind::Condition {
                    field: "key".to_owned(),
                    op: Op::Is,
                    values: vec![format!("{}-{}", k.project, k.number)],
                },
                negated,
                spans: vec![span],
            }),
            Expr::Text(t) => {
                let target = if negated { &mut not_words } else { &mut words };
                target.0.push(t.text.clone());
                target.1.push(span);
            }
            _ => return None,
        }
    }
    for ((words, spans), negated) in [(words, false), (not_words, true)] {
        if !words.is_empty() {
            out.push(Chip {
                kind: ChipKind::Text { words },
                negated,
                spans,
            });
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    #[test]
    fn a_plain_and_becomes_chips() {
        let src = "prio>=high -label:bug login \"time out\" TM-4 status!=Done";
        let chips = chips(&parse(src).expect("query")).expect("chips");
        let shown: Vec<(String, bool, Vec<&str>)> = chips
            .iter()
            .map(|c| {
                let what = match &c.kind {
                    ChipKind::Condition { field, op, values } => {
                        format!("{field}{op}{}", values.join(","))
                    }
                    ChipKind::Text { words } => format!("text:{}", words.join("|")),
                };
                (what, c.negated, c.spans.iter().map(|s| s.of(src)).collect())
            })
            .collect();
        assert_eq!(
            shown,
            [
                ("priority>=high".to_owned(), false, vec!["prio>=high"]),
                ("label:bug".to_owned(), true, vec!["-label:bug"]),
                ("key:TM-4".to_owned(), false, vec!["TM-4"]),
                ("status:Done".to_owned(), true, vec!["status!=Done"]),
                (
                    "text:login|time out".to_owned(),
                    false,
                    vec!["login", "\"time out\""]
                ),
            ]
        );
    }

    #[test]
    fn or_and_parentheses_need_the_text() {
        assert!(chips(&parse("a OR b").expect("q")).is_none());
        assert!(chips(&parse("(a b) c").expect("q")).is_none());
        assert!(chips(&parse("-(a b)").expect("q")).is_none());
        assert_eq!(chips(&parse("sort:due").expect("q")), Some(Vec::new()));
    }
}
