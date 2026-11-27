// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Keeping saved queries in step with the names they use.
//!
//! When a status, type, label, version or project key is renamed, [`rename`]
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
    let Some(field) = fields::find_target(&p.field) else {
        return;
    };
    for v in &p.values {
        let ValueKind::Word { text, quoted } = &v.kind else {
            continue;
        };
        let renamed = match (kind, field.ty) {
            (NameKind::Status, FieldType::Status)
            | (NameKind::Type, FieldType::Type)
            | (NameKind::Label, FieldType::Label)
            | (NameKind::Version, FieldType::Version)
            | (NameKind::Sprint, FieldType::Sprint) => same_name(text, old).then(|| new.to_owned()),
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
    remove_name(src, FieldType::Label, name)
}

/// `src` without the version `name` (any case), as [`remove_label`] takes
/// out a label.
#[must_use]
pub fn remove_version(src: &str, name: &str) -> Option<String> {
    remove_name(src, FieldType::Version, name)
}

/// `src` without the sprint `name` (any case), as [`remove_label`] takes
/// out a label.
#[must_use]
pub fn remove_sprint(src: &str, name: &str) -> Option<String> {
    remove_name(src, FieldType::Sprint, name)
}

fn remove_name(src: &str, ty: FieldType, name: &str) -> Option<String> {
    let query = parse(src).ok()?;
    let mut removed = false;
    let filter = query
        .filter
        .clone()
        .and_then(|f| without_label(f, ty, name, &mut removed));
    if !removed {
        return None;
    }
    Some(print(&Query {
        filter,
        sort: query.sort,
    }))
}

fn without_label(e: Expr, ty: FieldType, name: &str, removed: &mut bool) -> Option<Expr> {
    match e {
        Expr::And(items) => {
            let mut kept: Vec<Expr> = items
                .into_iter()
                .filter_map(|i| without_label(i, ty, name, removed))
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
                .filter_map(|i| without_label(i, ty, name, removed))
                .collect();
            match kept.len() {
                0 => None,
                1 => kept.pop(),
                _ => Some(Expr::Or(kept)),
            }
        }
        Expr::Not(inner, negation, span) => {
            without_label(*inner, ty, name, removed).map(|i| Expr::Not(Box::new(i), negation, span))
        }
        Expr::Group(inner, span) => {
            without_label(*inner, ty, name, removed).map(|i| Expr::Group(Box::new(i), span))
        }
        Expr::Pred(mut p) => {
            if fields::find_target(&p.field).is_some_and(|f| f.ty == ty) {
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

/// The field a condition's values belong to: the field itself, or for
/// `was:cf.area` the custom field. `changed:` holds dates or field names,
/// not values.
fn value_field(p: &Predicate) -> Option<&str> {
    match fields::split_history(&p.field) {
        Some((fields::History::Was, inner)) => Some(inner),
        Some((fields::History::Changed, _)) => None,
        None if is_changed_list(p) => None,
        None => Some(&p.field),
    }
}

/// Whether the condition is `changed:<fields>`, which lists field names.
fn is_changed_list(p: &Predicate) -> bool {
    fields::find(&p.field).is_some_and(|f| f.ty == FieldType::Changed)
}

/// Whether the condition looks at the custom field `query_name`: on it,
/// on its history (`was:cf.area:…`, `changed:cf.area>…`), or with it among
/// the fields of `changed:`.
fn uses_custom(p: &Predicate, query_name: &str) -> bool {
    match fields::split_history(&p.field) {
        Some((_, inner)) => names_custom(inner, query_name),
        None if is_changed_list(p) => false,
        None => names_custom(&p.field, query_name),
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
            if let Some((_, inner)) = fields::split_history(&p.field) {
                if names_custom(inner, old) {
                    let end = p.field_span.end;
                    edits.push((Span::new(end - inner.len(), end), written.clone()));
                }
            } else if is_changed_list(p) {
                for v in &p.values {
                    if let ValueKind::Word { text, .. } = &v.kind
                        && names_custom(text, old)
                    {
                        edits.push((v.span, written.clone()));
                    }
                }
            } else if names_custom(&p.field, old) {
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
            if !value_field(p).is_some_and(|f| names_custom(f, field)) {
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
        prune_values(f, &mut |p| {
            if uses_custom(p, field) {
                removed = true;
                p.values.clear();
            } else if is_changed_list(p) {
                let before = p.values.len();
                p.values.retain(|v| match &v.kind {
                    ValueKind::Word { text, .. } => !names_custom(text, field),
                    ValueKind::Range(..) => true,
                });
                removed |= p.values.len() != before;
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
            if !value_field(p).is_some_and(|f| names_custom(f, field)) {
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
    fn history_conditions_are_renamed_by_their_field() {
        assert_eq!(
            rename(
                "was:status:Review changed:status:Review status:Review",
                NameKind::Status,
                "review",
                "In review"
            )
            .as_deref(),
            Some("was:status:\"In review\" changed:status:Review status:\"In review\"")
        );
        assert_eq!(
            remove_label("was:label:bug is:open", "bug").as_deref(),
            Some("is:open")
        );
    }

    #[test]
    fn sprints_are_renamed_and_removed() {
        assert_eq!(
            rename(
                "sprint:\"Sprint 3\",active version:\"Sprint 3\"",
                NameKind::Sprint,
                "sprint 3",
                "Sprint 3b"
            )
            .as_deref(),
            Some("sprint:\"Sprint 3b\",active version:\"Sprint 3\"")
        );
        assert_eq!(
            remove_sprint("sprint:\"Sprint 3\" is:open", "Sprint 3").as_deref(),
            Some("is:open")
        );
        assert_eq!(remove_sprint("sprint:active", "Sprint 3"), None);
    }

    #[test]
    fn versions_are_renamed_and_removed() {
        assert_eq!(
            rename(
                "version:1.0,\"2.0 beta\" label:1.0",
                NameKind::Version,
                "1.0",
                "1.0.1"
            )
            .as_deref(),
            Some("version:1.0.1,\"2.0 beta\" label:1.0")
        );
        assert_eq!(
            remove_version("version:1.0 is:open", "1.0").as_deref(),
            Some("is:open")
        );
        assert_eq!(remove_version("label:1.0", "1.0"), None);
    }

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
    fn custom_fields_are_renamed_and_removed_in_history_conditions() {
        assert_eq!(
            rename_field(
                "was:cf.points:3 changed:status,cf.points changed:cf.Points>-7d",
                "points",
                "size"
            )
            .as_deref(),
            Some("was:cf.size:3 changed:status,cf.size changed:cf.size>-7d")
        );
        assert_eq!(
            rename_option(
                "was:cf.area:front changed:cf.area:today",
                "area",
                "front",
                "Front end"
            )
            .as_deref(),
            Some("was:cf.area:\"Front end\" changed:cf.area:today")
        );
        assert_eq!(
            remove_field(
                "was:cf.points:3 changed:status,cf.points changed:cf.points>-7d is:open",
                "points"
            )
            .as_deref(),
            Some("changed:status is:open")
        );
        assert_eq!(
            remove_option("was:cf.area:front,back", "area", "front").as_deref(),
            Some("was:cf.area:back")
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
