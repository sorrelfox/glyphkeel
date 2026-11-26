// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Keeping saved queries in step with the names they use.
//!
//! When a status, type, label or project key is renamed, [`rename`]
//! rewrites only the values that name it and leaves the rest of the text as
//! the person wrote it. When a label is deleted, [`remove_label`] takes it
//! out of the query and prints the rest in its normal form.
//!
//! Custom fields work the same way: [`rename_field`] and [`rename_option`]
//! rewrite in place, [`remove_field`] and [`remove_option`] take the field
//! or the option out.

use crate::check::NameKind;
use crate::fields::{self, FieldType};
use crate::parse::parse;
use crate::print::{print, value_needs_quotes};
use crate::syntax::{Expr, Predicate, Query, SortClause, Span, ValueKind};

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

/// Whether `written` (a field as written in a condition or a sort key)
/// names the custom field with this query name: `cf.<name>`, or the bare
/// name where no built-in field has it.
fn names_custom(written: &str, query_name: &str) -> bool {
    match fields::strip_custom_prefix(written) {
        Some(rest) => gk_core::fold(rest) == query_name,
        None => fields::find(written).is_none() && gk_core::fold(written) == query_name,
    }
}

/// `src` with the custom field `old` (a query name) written as `cf.<new>`
/// in every condition and sort key that names it. `None` when nothing
/// names it or the text doesn't parse.
#[must_use]
pub fn rename_field(src: &str, old: &str, new: &str) -> Option<String> {
    let query = parse(src).ok()?;
    let mut edits: Vec<(Span, String)> = Vec::new();
    let written = format!("{}{new}", fields::CUSTOM_PREFIX);
    if let Some(filter) = &query.filter {
        each_pred(filter, &mut |p| {
            if names_custom(&p.field, old) {
                edits.push((p.field_span, written.clone()));
            }
        });
    }
    if let Some(sort) = &query.sort {
        for key in &sort.keys {
            if names_custom(&key.field, old) {
                let dash = if key.descending { "-" } else { "" };
                edits.push((key.span, format!("{dash}{written}")));
            }
        }
    }
    apply(src, edits)
}

/// `src` with the option `old` of the custom field `field` (a query name)
/// written as `new` in the conditions on that field.
#[must_use]
pub fn rename_option(src: &str, field: &str, old: &str, new: &str) -> Option<String> {
    let query = parse(src).ok()?;
    let mut edits: Vec<(Span, String)> = Vec::new();
    if let Some(filter) = &query.filter {
        each_pred(filter, &mut |p| {
            if !names_custom(&p.field, field) {
                return;
            }
            for v in &p.values {
                if let ValueKind::Word { text, quoted } = &v.kind
                    && same_name(text, old)
                {
                    edits.push((v.span, written(new, *quoted)));
                }
            }
        });
    }
    apply(src, edits)
}

/// `src` without the custom field `field` (a query name): its conditions
/// go as a whole, with the negation or parentheses around them, and so do
/// its sort keys. The rest is printed in its normal form.
#[must_use]
pub fn remove_field(src: &str, field: &str) -> Option<String> {
    let query = parse(src).ok()?;
    let mut removed = false;
    let filter = query.filter.clone().and_then(|f| {
        prune(f, &mut |p| {
            if names_custom(&p.field, field) {
                removed = true;
                false
            } else {
                true
            }
        })
    });
    let sort = query.sort.clone().and_then(|s| {
        let keys: Vec<_> = s
            .keys
            .into_iter()
            .filter(|k| {
                let gone = names_custom(&k.field, field);
                removed |= gone;
                !gone
            })
            .collect();
        (!keys.is_empty()).then_some(SortClause { keys, span: s.span })
    });
    removed.then(|| print(&Query { filter, sort }))
}

/// `src` without the option `option` of the custom field `field`: it
/// leaves the conditions on that field, and a condition left with no value
/// goes as a whole.
#[must_use]
pub fn remove_option(src: &str, field: &str, option: &str) -> Option<String> {
    let query = parse(src).ok()?;
    let mut removed = false;
    let filter = query.filter.clone().and_then(|f| {
        prune_values(f, &mut |p| {
            if !names_custom(&p.field, field) {
                return;
            }
            let before = p.values.len();
            p.values.retain(|v| match &v.kind {
                ValueKind::Word { text, .. } => !same_name(text, option),
                ValueKind::Range(..) => true,
            });
            removed |= p.values.len() != before;
        })
    });
    removed.then(|| {
        print(&Query {
            filter,
            sort: query.sort,
        })
    })
}

fn apply(src: &str, mut edits: Vec<(Span, String)>) -> Option<String> {
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

fn each_pred(e: &Expr, f: &mut dyn FnMut(&Predicate)) {
    match e {
        Expr::And(items) | Expr::Or(items) => {
            for item in items {
                each_pred(item, f);
            }
        }
        Expr::Not(inner, _, _) | Expr::Group(inner, _) => each_pred(inner, f),
        Expr::Pred(p) => f(p),
        Expr::Text(_) | Expr::Key(_) => {}
    }
}

/// `e` without the conditions `keep` refuses; `None` when nothing is left.
fn prune(e: Expr, keep: &mut dyn FnMut(&Predicate) -> bool) -> Option<Expr> {
    prune_values(e, &mut |p: &mut Predicate| {
        if !keep(p) {
            p.values.clear();
        }
    })
}

/// `e` after `change` edited the values of each condition; a condition
/// left without values goes, with the negation or parentheses around it.
fn prune_values(e: Expr, change: &mut dyn FnMut(&mut Predicate)) -> Option<Expr> {
    let list = |items: Vec<Expr>, change: &mut dyn FnMut(&mut Predicate), or: bool| {
        let mut kept: Vec<Expr> = items
            .into_iter()
            .filter_map(|i| prune_values(i, change))
            .collect();
        match kept.len() {
            0 => None,
            1 => kept.pop(),
            _ => Some(if or { Expr::Or(kept) } else { Expr::And(kept) }),
        }
    };
    match e {
        Expr::And(items) => list(items, change, false),
        Expr::Or(items) => list(items, change, true),
        Expr::Not(inner, negation, span) => {
            prune_values(*inner, change).map(|i| Expr::Not(Box::new(i), negation, span))
        }
        Expr::Group(inner, span) => {
            prune_values(*inner, change).map(|i| Expr::Group(Box::new(i), span))
        }
        Expr::Pred(mut p) => {
            change(&mut p);
            (!p.values.is_empty()).then_some(Expr::Pred(p))
        }
        other => Some(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_renamed_custom_field_is_renamed_in_conditions_and_sort_keys() {
        assert_eq!(
            rename_field(
                "cf.points>3 Points:none label:x sort:-cf.POINTS,due",
                "points",
                "story-points"
            )
            .as_deref(),
            Some("cf.story-points>3 cf.story-points:none label:x sort:-cf.story-points,due")
        );
        assert_eq!(
            rename_field("due:today", "due", "x"),
            None,
            "a built-in field"
        );
        assert_eq!(
            rename_option(
                "cf.area:front,Back -cf.area:front",
                "area",
                "FRONT",
                "Front end"
            )
            .as_deref(),
            Some("cf.area:\"Front end\",Back -cf.area:\"Front end\"")
        );
    }

    #[test]
    fn a_deleted_custom_field_or_option_leaves_the_query() {
        assert_eq!(
            remove_field("cf.points>3 is:open sort:cf.points,due", "points").as_deref(),
            Some("is:open sort:due")
        );
        assert_eq!(
            remove_field("-cf.points:none (cf.points:1 OR due<today)", "points").as_deref(),
            Some("(due<today)")
        );
        assert_eq!(remove_field("is:open", "points"), None);
        assert_eq!(
            remove_option("cf.area:front,back is:open", "area", "front").as_deref(),
            Some("cf.area:back is:open")
        );
        assert_eq!(
            remove_option("cf.area:front", "area", "front").as_deref(),
            Some("")
        );
    }

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
