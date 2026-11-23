// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The printer: a syntax tree back to query text, in a normal form that
//! parses to the same tree. Values are quoted only where needed (or where
//! they were quoted), and spacing is one space.

use std::fmt::Write as _;

use crate::syntax::{Expr, Negation, Predicate, Query, ValueKind};

/// The query as normal text.
#[must_use]
pub fn print(query: &Query) -> String {
    let mut out = String::new();
    if let Some(filter) = &query.filter {
        expr(&mut out, filter, false);
    }
    if let Some(sort) = &query.sort {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str("sort:");
        let keys: Vec<String> = sort
            .keys
            .iter()
            .map(|k| format!("{}{}", if k.descending { "-" } else { "" }, k.field))
            .collect();
        out.push_str(&keys.join(","));
    }
    out
}

/// One expression as normal text.
#[must_use]
pub fn print_expr(e: &Expr) -> String {
    let mut out = String::new();
    expr(&mut out, e, false);
    out
}

fn expr(out: &mut String, e: &Expr, in_and: bool) {
    let after_minus = out.ends_with('-');
    match e {
        Expr::And(items) => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                expr(out, item, true);
            }
        }
        Expr::Or(items) => {
            if in_and {
                out.push('(');
            }
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(" OR ");
                }
                expr(out, item, false);
            }
            if in_and {
                out.push(')');
            }
        }
        Expr::Not(inner, negation, _) => {
            let wrap = matches!(**inner, Expr::And(_) | Expr::Or(_));
            if *negation == Negation::Minus
                && let Expr::Not(inner2, Negation::Not, span) = &**inner
            {
                // `-NOT x` would read as text; `--x` means the same.
                out.push('-');
                let as_minus = Expr::Not(inner2.clone(), Negation::Minus, *span);
                expr(out, &as_minus, false);
                return;
            }
            match negation {
                Negation::Minus if !wrap => out.push('-'),
                Negation::Minus => out.push_str("-("),
                Negation::Not if !wrap => out.push_str("NOT "),
                Negation::Not => out.push_str("NOT ("),
            }
            expr(out, inner, false);
            if wrap {
                out.push(')');
            }
        }
        Expr::Group(inner, _) => {
            out.push('(');
            expr(out, inner, false);
            out.push(')');
        }
        Expr::Pred(p) => predicate(out, p),
        Expr::Text(t) => {
            let keyword_after_minus =
                after_minus && matches!(t.text.as_str(), "AND" | "OR" | "NOT");
            if t.quoted || (text_needs_quotes(&t.text) && !keyword_after_minus) {
                quote(out, &t.text);
            } else {
                out.push_str(&t.text);
            }
        }
        Expr::Key(k) => {
            let _ = write!(out, "{}-{}", k.project, k.number);
        }
    }
}

fn predicate(out: &mut String, p: &Predicate) {
    out.push_str(&p.field);
    out.push_str(p.op.as_str());
    for (i, v) in p.values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        match &v.kind {
            ValueKind::Word { text, quoted } => {
                if *quoted || value_needs_quotes(text) {
                    quote(out, text);
                } else {
                    out.push_str(text);
                }
            }
            ValueKind::Range(a, b) => {
                out.push_str(a);
                out.push_str("..");
                out.push_str(b);
            }
        }
    }
}

/// Characters that end a bare word.
fn special(c: char) -> bool {
    c.is_whitespace() || matches!(c, '"' | '\'' | '(' | ')')
}

/// Whether a value must be quoted to read back as itself.
#[must_use]
pub fn value_needs_quotes(text: &str) -> bool {
    text.is_empty() || text.contains("..") || text.chars().any(|c| special(c) || c == ',')
}

/// Whether free text must be quoted to read back as the same text.
#[must_use]
pub fn text_needs_quotes(text: &str) -> bool {
    text.is_empty()
        || text.starts_with('-')
        || matches!(text, "AND" | "OR" | "NOT")
        || text.chars().any(special)
        || crate::parse::task_key(text).is_some()
        || looks_like_condition(text)
}

fn looks_like_condition(text: &str) -> bool {
    let mut chars = text.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    let len = text
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
        .map_or(text.len(), |(i, _)| i);
    let rest = &text[len..];
    ["!=", "<", ">", ":", "="]
        .iter()
        .any(|op| rest.starts_with(op))
}

fn quote(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use crate::parse::parse;

    use super::*;

    fn normal(src: &str) -> String {
        print(&parse(src).expect(src))
    }

    #[test]
    fn prints_a_normal_form() {
        assert_eq!(normal("  is:open    due<+7d "), "is:open due<+7d");
        assert_eq!(
            normal("label:bug,crash -status:Done \"login error\" sort:due,-priority"),
            "label:bug,crash -status:Done \"login error\" sort:due,-priority"
        );
        assert_eq!(normal("a AND b"), "a b");
        assert_eq!(normal("( a  OR b ) c"), "(a OR b) c");
        assert_eq!(normal("NOT x"), "NOT x");
        assert_eq!(normal(r#"title:'say "hi"'"#), r#"title:"say \"hi\"""#);
    }

    #[test]
    fn the_normal_form_parses_to_the_same_tree() {
        for src in [
            "is:open due<+7d",
            "(label:bug OR type:Bug) is:open -is:blocked",
            "estimate:1h..4h -has:start",
            "-(a OR NOT b) TM-41 \"TM-41\" 'x y' sort:-due",
            "status:\"in progress\",Done",
        ] {
            let first = parse(src).expect(src);
            let again = parse(&print(&first)).expect("normal form");
            assert_eq!(first.shape(), again.shape(), "{src}");
        }
    }
}
