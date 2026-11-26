// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `rules.update`, and the rules themselves: status rules and rules for
//! agents, enforced for every command that changes tasks.
//!
//! Rules are kept as JSON on their status (`statuses.rules`) and project
//! (`projects.agent_rules`), and change through the recorded fields
//! `status.rules` and `project.agent_rules`, so history, revert and Undo
//! treat them like any other setting of a project.
//!
//! [`enforce`] runs after a command's handler and before its history rows
//! are written, so the app, the command line and agents meet the same
//! rules. It looks at the tasks the command changed:
//!
//! - a field that is read-only in the status the task was in before the
//!   command can't change (unless the task moved to another project);
//! - an agent can't change a field its project keeps from agents, nor move
//!   or create a task into a status agents may not enter;
//! - a task entering a status (created in it or moved to it) gets the
//!   status's actions (fields set or cleared, the timer stopped, a claim
//!   released), and then must have every field the status requires.
//!
//! Commands that change the configuration or put back earlier states
//! (history, import, trash, projects, statuses, types, labels) are not
//! held to the rules: they move tasks as a side effect, and Undo must
//! restore exactly what was.
//!
//! Custom fields are named `cf:<id>`, which is also how history records a
//! task's custom field values, so read-only fields and the rules for agents
//! cover them as they cover built-in fields. Requiring one reads
//! `custom_values`; setting or clearing one on entering a status is not
//! offered yet.

use std::collections::BTreeMap;

use gk_api::ops::history::FieldChange;
use gk_api::ops::project::ProjectChange;
use gk_api::ops::rules::{
    AgentRules, CLEARABLE_FIELDS, CUSTOM_FIELD_PREFIX, LOCKABLE_FIELDS, REQUIRABLE_FIELDS,
    RULE_FIELDS_MAX, RuleSet, RulesUpdate, RulesUpdateInput, SETTABLE_FIELDS, StatusRules,
};
use gk_core::Id;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use super::desc;
use super::fields::{self, PROJECT, STATUS, TASK, text};
use super::lookup::{self, List, ProjectRow, b};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::write::WriteCtx;

const PRIORITIES: [&str; 5] = ["none", "low", "normal", "high", "urgent"];

pub(super) fn register(r: &mut Registry) {
    r.command::<RulesUpdate, _>(desc::<RulesUpdate>(), update);
}

// ---------------------------------------------------------------------
// Reading

/// The rules stored as `text`; none when there is no text or it can't be
/// read.
pub(crate) fn parse(text: Option<&str>) -> StatusRules {
    text.and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or_default()
}

/// The rules of status `status`.
pub(crate) fn status_rules(tx: &Transaction<'_>, status: Id) -> Result<StatusRules, Failure> {
    let text: Option<String> = tx
        .query_row(
            "SELECT rules FROM statuses WHERE id = ?1",
            [b(status)],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    Ok(parse(text.as_deref()))
}

/// The rules for agents of project `project`.
pub(crate) fn agent_rules(tx: &Transaction<'_>, project: Id) -> Result<AgentRules, Failure> {
    let text: Option<String> = tx
        .query_row(
            "SELECT agent_rules FROM projects WHERE id = ?1",
            [b(project)],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    Ok(text
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default())
}

// ---------------------------------------------------------------------
// Changing

fn update(w: &mut WriteCtx<'_>, input: RulesUpdateInput) -> Result<ProjectChange, Failure> {
    let p = lookup::project(w.tx(), "project", &input.project)?;
    lookup::ensure_active(&p)?;
    let pid = Some(p.id);
    for (i, change) in input.statuses.iter().enumerate() {
        let at = |e: Failure| at_path(e, &format!("statuses[{i}]"));
        let status =
            lookup::named(w.tx(), List::Statuses, &p, "status", &change.status).map_err(at)?;
        check_status_rules(w.tx(), &p, &change.rules).map_err(at)?;
        let value = stored(&change.rules, change.rules.is_empty())?;
        fields::set(w, STATUS, status.id, pid, "rules", value)?;
    }
    if let Some(agents) = &input.agents {
        check_list(
            w.tx(),
            &p,
            "agents.read_only",
            "agents",
            &agents.read_only,
            |f| LOCKABLE_FIELDS.contains(&f),
        )?;
        let value = stored(agents, agents.is_empty())?;
        fields::set(w, PROJECT, p.id, pid, "agent_rules", value)?;
    }
    super::project::changed(w, p.id)
}

/// `rules` as stored: no value when there are none.
fn stored<T: serde::Serialize>(rules: &T, empty: bool) -> Result<Option<Value>, Failure> {
    if empty {
        return Ok(None);
    }
    serde_json::to_value(rules).map(Some).map_err(|e| {
        Failure::Op(messages::storage(
            "storage-json",
            &[("error", e.to_string())],
        ))
    })
}

/// Puts `prefix` in front of an error's field path.
fn at_path(e: Failure, prefix: &str) -> Failure {
    match e {
        Failure::Op(mut op) => {
            let inner = op
                .details
                .get("path")
                .or_else(|| op.details.get("field"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            let path = match inner {
                Some(f) if !f.is_empty() => format!("{prefix}.{f}"),
                _ => prefix.to_owned(),
            };
            op.details.insert("path".to_owned(), Value::String(path));
            Failure::Op(op)
        }
        other => other,
    }
}

fn check_status_rules(
    tx: &Transaction<'_>,
    p: &ProjectRow,
    rules: &StatusRules,
) -> Result<(), Failure> {
    check_list(tx, p, "rules.required", "required", &rules.required, |f| {
        REQUIRABLE_FIELDS.contains(&f)
    })?;
    check_list(
        tx,
        p,
        "rules.read_only",
        "read_only",
        &rules.read_only,
        |f| LOCKABLE_FIELDS.contains(&f),
    )?;
    // Custom fields can't be set or cleared on entering a status yet.
    check_list(tx, p, "rules.clear", "clear", &rules.clear, |f| {
        CLEARABLE_FIELDS.contains(&f)
    })?;
    let set: Vec<String> = rules.set.iter().map(|s| s.field.clone()).collect();
    check_list(tx, p, "rules.set", "set", &set, |f| {
        SETTABLE_FIELDS.contains(&f)
    })?;
    for (i, s) in rules.set.iter().enumerate() {
        check_value(s).map_err(|e| at_path(e, &format!("rules.set[{i}]")))?;
        if rules.clear.contains(&s.field) {
            return Err(messages::invalid(
                &format!("rules.set[{i}].field"),
                "rule-set-and-cleared",
                &[("field", s.field.clone())],
            ));
        }
    }
    for (i, f) in rules.required.iter().enumerate() {
        if rules.clear.contains(f) {
            return Err(messages::invalid(
                &format!("rules.required[{i}]"),
                "rule-required-and-cleared",
                &field_args(tx, f)?,
            ));
        }
    }
    Ok(())
}

/// Checks that each field of a rule's list is one `allowed` accepts or a
/// custom field of the project, and that none is listed twice.
fn check_list(
    tx: &Transaction<'_>,
    p: &ProjectRow,
    path: &str,
    rule: &str,
    list: &[String],
    allowed: impl Fn(&str) -> bool,
) -> Result<(), Failure> {
    if list.len() > RULE_FIELDS_MAX {
        return Err(messages::invalid(
            path,
            "rule-too-many-fields",
            &[("max", RULE_FIELDS_MAX.to_string())],
        ));
    }
    for (i, field) in list.iter().enumerate() {
        let at = format!("{path}[{i}]");
        if list[..i].contains(field) {
            return Err(messages::invalid(
                &at,
                "rule-field-twice",
                &[("field", field.clone())],
            ));
        }
        if let Some(id) = field.strip_prefix(CUSTOM_FIELD_PREFIX) {
            let settable = matches!(rule, "required" | "read_only" | "agents");
            if !settable {
                return Err(messages::invalid(
                    &at,
                    "rule-field-not-allowed",
                    &[("rule", rule.to_owned()), ("field", field.clone())],
                ));
            }
            if custom_field_name(tx, p.id, id)?.is_none() {
                return Err(messages::invalid(
                    &at,
                    "rule-custom-field-unknown",
                    &[("id", id.to_owned()), ("project", p.key.clone())],
                ));
            }
            continue;
        }
        if !allowed(field) {
            return Err(messages::invalid(
                &at,
                "rule-field-not-allowed",
                &[("rule", rule.to_owned()), ("field", field.clone())],
            ));
        }
    }
    Ok(())
}

fn check_value(s: &RuleSet) -> Result<(), Failure> {
    let ok = match s.field.as_str() {
        "start_date" | "due_date" => s.value == "today",
        "progress" => s.value.parse::<u8>().is_ok_and(|n| n <= 100),
        "priority" => PRIORITIES.contains(&s.value.as_str()),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(messages::invalid(
            "value",
            "rule-set-value",
            &[("field", s.field.clone()), ("value", s.value.clone())],
        ))
    }
}

// ---------------------------------------------------------------------
// Custom fields

/// Whether the workspace has the custom field tables.
fn has_custom_fields(tx: &Transaction<'_>) -> Result<bool, Failure> {
    let n: i64 = tx.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type = 'table'
            AND name IN ('custom_fields', 'custom_values')",
        [],
        |r| r.get(0),
    )?;
    Ok(n == 2)
}

/// The name of custom field `id`, when it exists for project `project`
/// (its own, an ancestor's, or one for every project).
fn custom_field_name(
    tx: &Transaction<'_>,
    project: Id,
    id: &str,
) -> Result<Option<String>, Failure> {
    let Ok(id) = id.parse::<Id>() else {
        return Ok(None);
    };
    if !has_custom_fields(tx)? {
        return Ok(None);
    }
    Ok(tx
        .query_row(
            "WITH RECURSIVE up(id) AS (
                 SELECT ?2 UNION ALL
                 SELECT p.parent_id FROM projects p JOIN up ON p.id = up.id
                  WHERE p.parent_id IS NOT NULL)
             SELECT name FROM custom_fields
              WHERE id = ?1 AND (project_id IS NULL OR project_id IN (SELECT id FROM up))",
            params![b(id), b(project)],
            |r| r.get(0),
        )
        .optional()?)
}

/// A message's arguments naming `field`: `field` is the built-in field, or
/// `custom` with the custom field's `name`.
fn field_args(tx: &Transaction<'_>, field: &str) -> Result<Vec<(&'static str, String)>, Failure> {
    Ok(match field.strip_prefix(CUSTOM_FIELD_PREFIX) {
        Some(id) => {
            let name = match id.parse::<Id>() {
                Ok(parsed) if has_custom_fields(tx)? => tx
                    .query_row(
                        "SELECT name FROM custom_fields WHERE id = ?1",
                        [b(parsed)],
                        |r| r.get(0),
                    )
                    .optional()?
                    .unwrap_or_else(|| id.to_owned()),
                _ => id.to_owned(),
            };
            vec![("field", "custom".to_owned()), ("name", name)]
        }
        None => vec![("field", field.to_owned()), ("name", field.to_owned())],
    })
}

// ---------------------------------------------------------------------
// Enforcing

/// Whether command `op` is held to the rules. Commands that change the
/// configuration or bring back earlier states are not.
pub(crate) fn applies_to(op: &str) -> bool {
    const FREE: [&str; 13] = [
        "history.",
        "transfer.",
        "backup.",
        "trash.",
        "project.",
        "status.",
        "type.",
        "label.",
        "rules.",
        "custom_field.",
        "workspace.",
        "settings.",
        "agent.",
    ];
    !FREE.iter().any(|prefix| op.starts_with(prefix))
}

/// The task facts the rules look at.
struct TaskFacts {
    key: String,
    project: Id,
    project_key: String,
    status: Id,
    status_name: String,
    deleted: bool,
}

fn facts(tx: &Transaction<'_>, task: Id) -> Result<Option<TaskFacts>, Failure> {
    Ok(tx
        .query_row(
            "SELECT p.key || '-' || t.number, t.project_id, p.key, t.status_id, s.name,
                    t.deleted_at IS NOT NULL
               FROM tasks t JOIN projects p ON p.id = t.project_id
               JOIN statuses s ON s.id = t.status_id
              WHERE t.id = ?1",
            [b(task)],
            |r| {
                Ok(TaskFacts {
                    key: r.get(0)?,
                    project: lookup::id_at(r, 1)?,
                    project_key: r.get(2)?,
                    status: lookup::id_at(r, 3)?,
                    status_name: r.get(4)?,
                    deleted: r.get(5)?,
                })
            },
        )
        .optional()?)
}

fn status_name(tx: &Transaction<'_>, status: Id) -> Result<String, Failure> {
    Ok(tx
        .query_row(
            "SELECT name FROM statuses WHERE id = ?1",
            [b(status)],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_default())
}

/// The ID a recorded value holds.
fn id_of(v: Option<&Value>) -> Option<Id> {
    v.and_then(Value::as_str).and_then(|s| s.parse().ok())
}

/// Holds the tasks the command changed to the rules: refuses what they
/// forbid and applies what statuses do to tasks entering them.
pub(crate) fn enforce(w: &mut WriteCtx<'_>) -> Result<(), Failure> {
    if !applies_to(w.op()) {
        return Ok(());
    }
    let agent = w.caller().is_agent();
    for (task, _, changed) in w.changes_of(TASK) {
        let Some(now) = facts(w.tx(), task)? else {
            continue;
        };
        if now.deleted {
            continue;
        }
        let created = changed.contains_key("created");
        let entered = changed
            .get("status")
            .filter(|c| created || c.before != c.after)
            .is_some();
        if !created {
            check_unchanged(w, &now, &changed, agent)?;
        }
        if created || entered {
            enter(w, task, &now, agent)?;
        }
    }
    Ok(())
}

/// Refuses changes to fields that are read-only in the status the task was
/// in, and, for agents, to fields the project keeps from them.
fn check_unchanged(
    w: &WriteCtx<'_>,
    now: &TaskFacts,
    changed: &BTreeMap<String, FieldChange>,
    agent: bool,
) -> Result<(), Failure> {
    let moved_project = changed.contains_key("project");
    let before = changed
        .get("status")
        .and_then(|c| id_of(c.before.as_ref()))
        .unwrap_or(now.status);
    if !moved_project {
        let rules = status_rules(w.tx(), before)?;
        if let Some(field) = rules.read_only.iter().find(|f| changed.contains_key(*f)) {
            let mut args = field_args(w.tx(), field)?;
            args.push(("task", now.key.clone()));
            args.push(("status", status_name(w.tx(), before)?));
            let error =
                messages::forbidden("rule-read-only", &args).with_detail("path", field.clone());
            return Err(messages::hint(error, "rule-read-only-hint", &[]).into());
        }
    }
    if agent {
        let rules = agent_rules(w.tx(), now.project)?;
        if let Some(field) = rules.read_only.iter().find(|f| changed.contains_key(*f)) {
            let mut args = field_args(w.tx(), field)?;
            args.push(("task", now.key.clone()));
            args.push(("project", now.project_key.clone()));
            let error = messages::forbidden("rule-agent-field", &args)
                .with_detail("path", field.clone())
                .with_detail("rule", "agents.read_only");
            return Err(messages::hint(error, "rule-agent-hint", &[]).into());
        }
    }
    Ok(())
}

/// A task entered its status: refuses an agent where agents may not
/// enter, applies the status's actions, then checks its required fields.
fn enter(w: &mut WriteCtx<'_>, task: Id, now: &TaskFacts, agent: bool) -> Result<(), Failure> {
    let rules = status_rules(w.tx(), now.status)?;
    if rules.is_empty() {
        return Ok(());
    }
    if agent && rules.agents_may_not_enter {
        let error = messages::forbidden(
            "rule-agents-may-not-enter",
            &[
                ("task", now.key.clone()),
                ("status", now.status_name.clone()),
                ("project", now.project_key.clone()),
            ],
        )
        .with_detail("rule", "agents_may_not_enter")
        .with_detail("status", now.status_name.clone());
        return Err(messages::hint(error, "rule-agent-hint", &[]).into());
    }
    let pid = Some(now.project);
    for field in &rules.clear {
        if fields::is_column(TASK, field) {
            fields::set(w, TASK, task, pid, field, None)?;
        }
    }
    for s in &rules.set {
        let current = fields::read(w.tx(), TASK, task, &s.field)?;
        let empty = match &current {
            None => true,
            Some(Value::String(t)) => s.field == "priority" && t == "none",
            _ => false,
        };
        if s.only_if_empty && !empty {
            continue;
        }
        let value = match s.field.as_str() {
            "start_date" | "due_date" => {
                let today = w.zone().day_of(w.now()).to_string();
                if !dates_allow(w.tx(), task, &s.field, &today)? {
                    continue;
                }
                text(today)
            }
            "progress" => s.value.parse::<u8>().ok().map(Value::from),
            "priority" => text(s.value.clone()),
            _ => None,
        };
        if value.is_some() {
            fields::set(w, TASK, task, pid, &s.field, value)?;
        }
    }
    if rules.stop_timer {
        super::time::stop_before_trash(w, |_, id| Ok(id == task))?;
    }
    if rules.release_claim && super::claim::raw(w.tx(), task)?.is_some() {
        super::claim::apply(w, task, pid, None)?;
    }
    let missing = missing(w.tx(), task, &rules.required)?;
    if let Some(first) = missing.first() {
        let mut args = field_args(w.tx(), first)?;
        args.push(("task", now.key.clone()));
        args.push(("status", now.status_name.clone()));
        let more = (missing.len() - 1).to_string();
        let error = messages::invalid_at(first, "rule-required", &args)
            .with_detail("missing", Value::from(missing.clone()))
            .with_detail("status", now.status_name.clone());
        return Err(messages::hint(error, "rule-required-hint", &[("more", more)]).into());
    }
    Ok(())
}

/// Whether `field` may become `day` without the start date falling after
/// the due date.
fn dates_allow(tx: &Transaction<'_>, task: Id, field: &str, day: &str) -> Result<bool, Failure> {
    let (start, due): (Option<String>, Option<String>) = tx.query_row(
        "SELECT start_date, due_date FROM tasks WHERE id = ?1",
        [b(task)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(match field {
        "start_date" => due.is_none_or(|d| day <= d.as_str()),
        _ => start.is_none_or(|s| s.as_str() <= day),
    })
}

/// The fields of `required` that task `task` has no value in, in order.
fn missing(tx: &Transaction<'_>, task: Id, required: &[String]) -> Result<Vec<String>, Failure> {
    let mut out = Vec::new();
    for field in required {
        let has = match field.as_str() {
            "description" => "description <> ''",
            "priority" => "priority > 0",
            "start_date" => "start_date IS NOT NULL",
            "due_date" => "due_date IS NOT NULL",
            "estimate" => "estimate_min IS NOT NULL",
            "parent" => "parent_id IS NOT NULL",
            "labels" => "EXISTS (SELECT 1 FROM task_labels l WHERE l.task_id = tasks.id)",
            _ => "",
        };
        let present = if let Some(id) = field.strip_prefix(CUSTOM_FIELD_PREFIX) {
            custom_value_present(tx, task, id)?
        } else if has.is_empty() {
            true
        } else {
            tx.query_row(
                &format!("SELECT {has} FROM tasks WHERE id = ?1"),
                [b(task)],
                |r| r.get::<_, bool>(0),
            )?
        };
        if !present {
            out.push(field.clone());
        }
    }
    Ok(out)
}

/// Whether task `task` has a value in custom field `id`. A field that no
/// longer exists counts as filled in, so it can't block a task.
fn custom_value_present(tx: &Transaction<'_>, task: Id, id: &str) -> Result<bool, Failure> {
    let Ok(id) = id.parse::<Id>() else {
        return Ok(true);
    };
    if !has_custom_fields(tx)? {
        return Ok(true);
    }
    let exists: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM custom_fields WHERE id = ?1)",
        [b(id)],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(true);
    }
    Ok(tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM custom_values WHERE task_id = ?1 AND field_id = ?2)",
        params![b(task), b(id)],
        |r| r.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_and_history_are_free_of_the_rules() {
        for op in [
            "history.undo",
            "transfer.import",
            "status.delete",
            "project.update",
        ] {
            assert!(!applies_to(op), "{op}");
        }
        for op in [
            "task.update",
            "task.create",
            "bulk.apply",
            "task.claim",
            "board.move",
        ] {
            assert!(applies_to(op), "{op}");
        }
    }

    #[test]
    fn stored_rules_that_cant_be_read_are_no_rules() {
        assert!(parse(None).is_empty());
        assert!(parse(Some("not json")).is_empty());
        let rules = parse(Some(r#"{"required":["estimate"],"stop_timer":true}"#));
        assert_eq!(rules.required, vec!["estimate".to_owned()]);
        assert!(rules.stop_timer);
    }
}
