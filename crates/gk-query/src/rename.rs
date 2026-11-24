// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Keeping saved queries in step with the names they use.
//!
//! When a status, type, label or project key is renamed, [`rename`]
//! rewrites only the values that name it and leaves the rest of the text as
//! the person wrote it. When a label is deleted, [`remove_label`] takes it
//! out of the query and prints the rest in its normal form.

use crate::check::NameKind;
use crate::fields::{self, FieldType};
use crate::parse::parse;
use crate::print::{print, value_needs_quotes};
use crate::syntax::{Expr, Predicate, Query, Span, ValueKind};

/// `src` with every use of the name `old` (any case) of `kind` written as
/// `new`: in conditions on that field and, for a project key, in task keys
/// too (`TM-41` becomes `TK-41`). `None` when nothing names it or the text
/// doesn't parse (a saved query that no longer parses is left alone).
#[must_use]
pub fn rename(src: &str, kind: NameKind, old: &str, new: &str) -> Option<String> {
    let query = parse(src).ok()?;
    let mut edits: Vec<(Span, String)> = Vec::new();
    if let Some(filter) = &query.filter {
        collect(filter, kind, old, new, &mut edits);
    }
    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    let mut out = src.to_owned();
    for (span, text) in edits {
        out.replace_range(span.start..span.end, &text);
    }
    (out != src).then_some(out)
}

fn collect(e: &Expr, kind: NameKind, old: &str, new: &str, edits: &mut Vec<(Span, String)>) {
    match e {
        Expr::And(items) | Expr::Or(items) => {
            for item in items {
                collect(item, kind, old, new, edits);
            }
        }
        Expr::Not(inner, _, _) | Expr::Group(inner, _) => collect(inner, kind, old, new, edits),
        Expr::Pred(p) => predicate(p, kind, old, new, edits),
        Expr::Text(_) => {}
        Expr::Key(k) => {
            if kind == NameKind::Project && k.project.eq_ignore_ascii_case(old) {
                edits.push((k.span, format!("{new}-{}", k.number)));
            }
        }
    }
}

fn predicate(p: &Predicate, kind: NameKind, old: &str, new: &str, edits: &mut Vec<(Span, String)>) {
    let Some(field) = fields::find(&p.field) else {
        return;
    };
    for v in &p.values {
        let ValueKind::Word { text, quoted } = &v.kind else {
            continue;
        };
        let renamed = match (kind, field.ty) {
            (NameKind::Status, FieldType::Status)
            | (NameKind::Type, FieldType::Type)
            | (NameKind::Label, FieldType::Label) => same_name(text, old).then(|| new.to_owned()),
            (NameKind::Project, FieldType::Project) => match text.strip_suffix("/*") {
                Some(key) if key.eq_ignore_ascii_case(old) => Some(format!("{new}/*")),
                None if text.eq_ignore_ascii_case(old) => Some(new.to_owned()),
                _ => None,
            },
            (
                NameKind::Project,
                FieldType::Key | FieldType::Parent | FieldType::Under | FieldType::Link(_),
            ) => text
                .split_once('-')
                .filter(|(key, number)| {
                    key.eq_ignore_ascii_case(old)
                        && !number.is_empty()
                        && number.bytes().all(|b| b.is_ascii_digit())
                })
                .map(|(_, number)| format!("{new}-{number}")),
            _ => None,
        };
        if let Some(renamed) = renamed {
            edits.push((v.span, written(&renamed, *quoted)));
        }
    }
}

/// Names compare as the workspace compares them: ignoring case.
fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// A value as query text: in quotes where it was quoted or must be.
fn written(text: &str, quoted: bool) -> String {
    if !quoted && !value_needs_quotes(text) {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// `src` without the label `name` (any case): it leaves every condition on
/// labels, and a condition left with no label goes as a whole, with the
/// negation or parentheses around it. The rest is printed in its normal
/// form. `None` when the query doesn't use the label or doesn't parse.
#[must_use]
pub fn remove_label(src: &str, name: &str) -> Option<String> {
    let query = parse(src).ok()?;
    let mut removed = false;
    let filter = query
        .filter
        .clone()
        .and_then(|f| without_label(f, name, &mut removed));
    if !removed {
        return None;
    }
    Some(print(&Query {
        filter,
        sort: query.sort,
    }))
}

fn without_label(e: Expr, name: &str, removed: &mut bool) -> Option<Expr> {
    match e {
        Expr::And(items) => {
            let mut kept: Vec<Expr> = items
                .into_iter()
                .filter_map(|i| without_label(i, name, removed))
                .collect();
            match kept.len() {
                0 => None,
                1 => kept.pop(),
                _ => Some(Expr::And(kept)),
            }
        }
        Expr::Or(items) => {
            let mut kept: Vec<Expr> = items
                .into_iter()
                .filter_map(|i| without_label(i, name, removed))
                .collect();
            match kept.len() {
                0 => None,
                1 => kept.pop(),
                _ => Some(Expr::Or(kept)),
            }
        }
        Expr::Not(inner, negation, span) => {
            without_label(*inner, name, removed).map(|i| Expr::Not(Box::new(i), negation, span))
        }
        Expr::Group(inner, span) => {
            without_label(*inner, name, removed).map(|i| Expr::Group(Box::new(i), span))
        }
        Expr::Pred(mut p) => {
            if fields::find(&p.field).is_some_and(|f| f.ty == FieldType::Label) {
                let before = p.values.len();
                p.values.retain(|v| match &v.kind {
                    ValueKind::Word { text, .. } => !same_name(text, name),
                    ValueKind::Range(..) => true,
                });
                if p.values.len() != before {
                    *removed = true;
                }
                if p.values.is_empty() {
                    return None;
                }
            }
            Some(Expr::Pred(p))
        }
        other => Some(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_only_the_values_that_name_it() {
        assert_eq!(
            rename(
                "status:Review,\"to do\"  label:review sort:due",
                NameKind::Status,
                "review",
                "In review"
            )
            .as_deref(),
            Some("status:\"In review\",\"to do\"  label:review sort:due")
        );
        assert_eq!(
            rename("-label:bug OR label:Bug", NameKind::Label, "BUG", "defect").as_deref(),
            Some("-label:defect OR label:defect")
        );
        assert_eq!(
            rename("type:Bug status:Bug", NameKind::Type, "bug", "Fault").as_deref(),
            Some("type:Fault status:Bug")
        );
        assert_eq!(
            rename("title:review", NameKind::Status, "review", "x"),
            None
        );
        assert_eq!(rename("status:(", NameKind::Status, "x", "y"), None);
    }

    #[test]
    fn a_project_key_is_renamed_in_task_keys_too() {
        assert_eq!(
            rename(
                "project:TM/* parent:tm-4 TM-41 OTM-1 key:TM-2,OT-3",
                NameKind::Project,
                "TM",
                "TK"
            )
            .as_deref(),
            Some("project:TK/* parent:TK-4 TK-41 OTM-1 key:TK-2,OT-3")
        );
        assert_eq!(
            rename("project:tm", NameKind::Project, "TM", "TK").as_deref(),
            Some("project:TK")
        );
    }

    #[test]
    fn a_deleted_label_leaves_the_query() {
        assert_eq!(
            remove_label("label:old,new is:open", "OLD").as_deref(),
            Some("label:new is:open")
        );
        assert_eq!(
            remove_label("-label:old (label:old OR due<today) sort:due", "old").as_deref(),
            Some("(due<today) sort:due")
        );
        assert_eq!(remove_label("label:old", "old").as_deref(), Some(""));
        assert_eq!(remove_label("label:other", "old"), None);
    }
}
