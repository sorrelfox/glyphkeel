// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Automation rules: a trigger, an optional query the task must match, and
//! actions, run by the engine as the rule itself.
//!
//! A rule is kept whole in history (one `record` field), so creating,
//! changing and deleting one can be undone like a saved view.
//!
//! **Running.** After a command's own changes, its status rules and its
//! history rows, [`follow`] looks at what it changed: tasks created, fields
//! changed, tasks entering a status, comments added. Each enabled rule
//! whose trigger fits and whose condition the task matches runs its actions
//! through the same handlers people and agents use (`task.update`,
//! `task.move`, `comment.add`, `task.create`, `template.apply`,
//! `relation.add`), as a command of its own whose caller is the rule (actor
//! kind `automation`), in the triggering command's history group: Undo
//! takes the change and what the rules did after it back together. Status
//! rules apply to the rule's changes; a rule an agent last changed, or
//! that follows an agent's change, is held to the rules for agents.
//!
//! What a rule's run changed can set off other rules in turn, never the
//! rules already in that chain, and at most [`AUTOMATION_DEPTH_MAX`] deep.
//! A run that fails (an action refused, a status rule unmet) keeps none of
//! its changes and is logged; the command that set it off still succeeds.
//! A rule runs at most [`AUTOMATION_RUNS_PER_HOUR`] times an hour.
//!
//! Due date and scheduled rules are started by `automation.run`, which
//! long-running programs call every minute: it finds the tasks whose due
//! date came or passed (once per task and due date) or the tasks a
//! scheduled rule's condition matches when its time has come, and asks for
//! their runs, which [`follow`] then makes as above.
//!
//! The run log, the due dates already acted on, the hourly counts and the
//! notifications rules leave are kept beside history, not in it.
//!
//! Rules belong to the workspace's data and name who made them; nothing in
//! them is personal. Rules never call a model and never reach the network.

use std::collections::VecDeque;
use std::sync::OnceLock;

use gk_api::ops::automation::{
    AUTOMATION_ACTIONS_MAX, AUTOMATION_DEPTH_MAX, AUTOMATION_RUNS_KEPT, AUTOMATION_RUNS_PER_HOUR,
    AUTOMATION_SETTABLE_FIELDS, AUTOMATION_TASKS_PER_RUN, ActionKind, AutomationAction,
    AutomationChange, AutomationCreate, AutomationCreateInput, AutomationDelete, AutomationGet,
    AutomationList, AutomationListInput, AutomationListOp, AutomationRef, AutomationRule,
    AutomationRun, AutomationRunInput, AutomationRunList, AutomationRunOp, AutomationRunsInput,
    AutomationRunsOp, AutomationTest, AutomationTestChange, AutomationTestInput,
    AutomationTestResult, AutomationTick, AutomationTrigger, AutomationUpdate,
    AutomationUpdateInput, RunResult, ScheduleEvery, TriggerKind, WATCHABLE_FIELDS,
};
use gk_api::ops::common::{Actor, ActorKind};
use gk_api::ops::history::FieldChange;
use gk_api::ops::rules::CUSTOM_FIELD_PREFIX;
use gk_api::ops::task::RelationKind;
use gk_api::{ErrorCode, OpError};
use gk_core::{Day, Id, Rank, fold};
use gk_query::{Compiled, Param, SqlEnv};
use gk_store::StoreError;
use gk_store::rusqlite::types::Value as Sql;
use gk_store::rusqlite::{OptionalExtension, Transaction, params, params_from_iter};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::desc;
use super::fields::TASK;
use super::lookup::{self, List, ProjectRow, b, id_at, opt_id_at};
use crate::context::Context;
use crate::failure::Failure;
use crate::messages;
use crate::registry::{Handler, Registry};
use crate::time::{clock_minutes, rfc3339};
use crate::write::{CommandMeta, Finished, Pending, WriteCtx};

/// History's name for automation rules.
pub(crate) const ENTITY: &str = "automation_rule";
/// The one recorded field: the whole rule.
pub(crate) const RECORD: &str = "record";
/// The operation a rule's run is recorded under.
const OP: &str = "automation.run";
/// The longest rule name, in characters.
const NAME_MAX: usize = 100;
/// The longest comment a rule adds, in characters.
const COMMENT_MAX: usize = 10_000;
/// The longest notification, in characters.
const NOTICE_MAX: usize = 2_000;
/// The most fields a `field_changed` trigger watches or a `set_fields`
/// action sets.
const FIELDS_MAX: usize = 50;
/// Due dates this many days past are no longer acted on, so a new rule
/// doesn't act on every old task at once.
const DUE_GRACE_DAYS: i64 = 7;
/// The length of the rate limit's window.
const HOUR_MS: i64 = 3_600_000;
/// Acknowledged notifications are kept this long.
const NOTICE_KEEP_MS: i64 = 30 * 24 * HOUR_MS;
/// The task fields a `set_fields` action may clear with `null`.
const CLEARABLE: &[&str] = &[
    "description",
    "start_date",
    "due_date",
    "estimate",
    "progress",
    "fix_version",
];
const PRIORITIES: [&str; 5] = ["none", "low", "normal", "high", "urgent"];

pub(super) fn register(r: &mut Registry) {
    r.query::<AutomationListOp, _>(desc::<AutomationListOp>(), |q, input| list(q.tx, &input));
    r.query::<AutomationGet, _>(desc::<AutomationGet>(), |q, input: AutomationRef| {
        let (id, record) = find(q.tx, &input.rule, input.project.as_deref())?;
        output(q.tx, id, &record, true)
    });
    r.query::<AutomationRunsOp, _>(desc::<AutomationRunsOp>(), |q, input| runs(q.tx, &input));
    r.command::<AutomationCreate, _>(desc::<AutomationCreate>(), create);
    r.command::<AutomationUpdate, _>(desc::<AutomationUpdate>(), update);
    r.command::<AutomationDelete, _>(desc::<AutomationDelete>(), delete);
    r.command::<AutomationTest, _>(desc::<AutomationTest>(), test);
    r.command::<AutomationRunOp, _>(desc::<AutomationRunOp>(), |w, _: AutomationRunInput| {
        tick(w)
    });
}

// ---------------------------------------------------------------------
// The stored rule

/// What a rule does, as its `definition` column keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Definition {
    trigger: AutomationTrigger,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    condition: String,
    actions: Vec<AutomationAction>,
}

/// A rule as history records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    name: String,
    enabled: bool,
    trigger: AutomationTrigger,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    condition: String,
    actions: Vec<AutomationAction>,
    rank: String,
    created_at: i64,
    created_by_kind: String,
    created_by_name: String,
    updated_at: i64,
    updated_by_kind: String,
}

impl Record {
    fn project_id(&self) -> Option<Id> {
        self.project.as_deref().and_then(|p| p.parse().ok())
    }

    /// Whether an agent made or last changed the rule.
    fn held_as_agent(&self) -> bool {
        self.updated_by_kind == ActorKind::Agent.as_str()
    }
}

const COLUMNS: &str = "id, project_id, name, enabled, definition, rank, created_at, \
                       created_by_kind, created_by_name, updated_at, updated_by_kind";

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<(Id, Record)> {
    let definition: String = r.get(4)?;
    let d: Definition = serde_json::from_str(&definition).map_err(|e| {
        gk_store::rusqlite::Error::FromSqlConversionFailure(
            4,
            gk_store::rusqlite::types::Type::Text,
            Box::new(e),
        )
    })?;
    Ok((
        id_at(r, 0)?,
        Record {
            project: opt_id_at(r, 1)?.map(|p| p.to_string()),
            name: r.get(2)?,
            enabled: r.get(3)?,
            trigger: d.trigger,
            condition: d.condition,
            actions: d.actions,
            rank: r.get(5)?,
            created_at: r.get(6)?,
            created_by_kind: r.get(7)?,
            created_by_name: r.get(8)?,
            updated_at: r.get(9)?,
            updated_by_kind: r.get(10)?,
        },
    ))
}

fn load(tx: &Transaction<'_>, id: Id) -> Result<Option<Record>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM automation_rules WHERE id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?
        .map(|(_, r)| r))
}

/// The stored rule `id` as history records it, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    load(tx, id)?
        .map(|r| serde_json::to_value(r).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

/// Writes rule `id` as history recorded it (`None` deletes it), without
/// recording. Its schedule and hourly count stay when it is rewritten.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    let Some(value) = value else {
        w.tx()
            .execute("DELETE FROM automation_rules WHERE id = ?1", [b(id)])?;
        return Ok(());
    };
    let r: Record = serde_json::from_value(value.clone())
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    let definition = serde_json::to_string(&Definition {
        trigger: r.trigger.clone(),
        condition: r.condition.clone(),
        actions: r.actions.clone(),
    })
    .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    w.tx().execute(
        "INSERT INTO automation_rules (id, project_id, name, name_fold, enabled, trigger_kind,
                                       definition, rank, created_at, created_by_kind,
                                       created_by_name, updated_at, updated_by_kind)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT (id) DO UPDATE SET
           project_id = excluded.project_id, name = excluded.name,
           name_fold = excluded.name_fold, enabled = excluded.enabled,
           trigger_kind = excluded.trigger_kind, definition = excluded.definition,
           rank = excluded.rank, created_at = excluded.created_at,
           created_by_kind = excluded.created_by_kind,
           created_by_name = excluded.created_by_name, updated_at = excluded.updated_at,
           updated_by_kind = excluded.updated_by_kind",
        params![
            b(id),
            r.project_id().map(b),
            r.name,
            fold(&r.name),
            r.enabled,
            r.trigger.on.as_str(),
            definition,
            r.rank,
            r.created_at,
            r.created_by_kind,
            r.created_by_name,
            r.updated_at,
            r.updated_by_kind,
        ],
    )?;
    Ok(())
}

/// Writes rule `id` and records the change.
fn set(w: &mut WriteCtx<'_>, id: Id, record: Option<&Record>) -> Result<(), Failure> {
    let before = raw(w.tx(), id)?;
    let after = record
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?;
    if before == after {
        return Ok(());
    }
    let project = record.and_then(Record::project_id).or_else(|| {
        before
            .as_ref()
            .and_then(|v| v.get("project"))
            .and_then(Value::as_str)
            .and_then(|p| p.parse().ok())
    });
    write_raw(w, id, after.as_ref())?;
    w.record_field(ENTITY, id, project, RECORD, before, after);
    Ok(())
}

/// Every rule, the workspace's first, then by project, each in order.
fn rows(tx: &Transaction<'_>, enabled_only: bool) -> Result<Vec<(Id, Record)>, Failure> {
    let filter = if enabled_only {
        "WHERE enabled = 1"
    } else {
        ""
    };
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM automation_rules {filter}
          ORDER BY project_id IS NOT NULL, project_id, rank, id"
    ))?;
    let rows = stmt
        .query_map([], read_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn project_key(tx: &Transaction<'_>, project: Option<Id>) -> Result<Option<String>, Failure> {
    Ok(match project {
        Some(p) => tx
            .query_row("SELECT key FROM projects WHERE id = ?1", [b(p)], |r| {
                r.get(0)
            })
            .optional()?,
        None => None,
    })
}

fn output(
    tx: &Transaction<'_>,
    id: Id,
    r: &Record,
    with_runs: bool,
) -> Result<AutomationRule, Failure> {
    let runs = run_rows(tx, Some(id), None, false, AUTOMATION_RUNS_KEPT as u32)?.0;
    Ok(AutomationRule {
        id: id.to_string(),
        name: r.name.clone(),
        project: project_key(tx, r.project_id())?,
        enabled: r.enabled,
        trigger: r.trigger.clone(),
        condition: r.condition.clone(),
        actions: r.actions.clone(),
        created_by: Actor {
            kind: ActorKind::from_stored(&r.created_by_kind),
            name: r.created_by_name.clone(),
        },
        created_at: rfc3339(r.created_at),
        updated_at: rfc3339(r.updated_at),
        held_as_agent: r.held_as_agent(),
        last_run: runs.first().cloned(),
        runs: with_runs.then_some(runs),
    })
}

fn list(tx: &Transaction<'_>, input: &AutomationListInput) -> Result<AutomationList, Failure> {
    let project = match &input.project {
        Some(p) => Some(lookup::project(tx, "project", p)?.id),
        None => None,
    };
    let items = rows(tx, false)?
        .into_iter()
        .filter(|(_, r)| project.is_none() || r.project_id().is_none() || r.project_id() == project)
        .filter(|(_, r)| r.project_id().is_none_or(crate::scope::sees))
        .map(|(id, r)| output(tx, id, &r, false))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AutomationList {
        total: items.len() as u64,
        items,
    })
}

/// The rule `text` names: its ID, or its name (in `project` when given; a
/// rule of the workspace, or the only rule with that name, when not).
fn find(tx: &Transaction<'_>, text: &str, project: Option<&str>) -> Result<(Id, Record), Failure> {
    let text = text.trim();
    if let Ok(id) = text.parse::<Id>()
        && let Some(r) = load(tx, id)?
    {
        return Ok((id, r));
    }
    let project = match project {
        Some(p) => Some(lookup::project(tx, "project", p)?.id),
        None => None,
    };
    let wanted = fold(text);
    let all = rows(tx, false)?;
    let named: Vec<&(Id, Record)> = all
        .iter()
        .filter(|(_, r)| fold(&r.name) == wanted)
        .filter(|(_, r)| project.is_none() || r.project_id() == project)
        .collect();
    let chosen = match named.as_slice() {
        [one] => Some(*one),
        [] => None,
        many => many.iter().find(|(_, r)| r.project_id().is_none()).copied(),
    };
    if let Some((id, r)) = chosen {
        return Ok((*id, r.clone()));
    }
    if named.len() > 1 {
        let error = messages::conflict("automation-name-ambiguous", &[("rule", text.to_owned())])
            .with_detail("field", "rule");
        return Err(messages::hint(error, "automation-name-ambiguous-hint", &[]).into());
    }
    let names: Vec<&str> = all.iter().map(|(_, r)| r.name.as_str()).take(50).collect();
    let error = messages::with("automation-not-found", &[("rule", text.to_owned())], |m| {
        OpError::not_found(m, &names)
    })
    .with_detail("field", "rule");
    Err(messages::hint(error, "automation-not-found-hint", &[]).into())
}

// ---------------------------------------------------------------------
// Checking a rule

fn invalid(field: &str, e: impl gk_core::Message) -> Failure {
    super::task::invalid(field, e)
}

/// The project a rule belongs to, active.
fn active_project(tx: &Transaction<'_>, text: &str) -> Result<ProjectRow, Failure> {
    let p = lookup::project(tx, "project", text)?;
    lookup::ensure_active(&p)?;
    Ok(p)
}

/// A custom field named in a rule, as `cf:` and its ID: one of the
/// project's (or, for a rule of the workspace, any).
fn check_custom_field(
    tx: &Transaction<'_>,
    project: Option<&ProjectRow>,
    path: &str,
    field: &str,
) -> Result<String, Failure> {
    let id = field
        .strip_prefix(CUSTOM_FIELD_PREFIX)
        .and_then(|id| id.parse::<Id>().ok());
    let known = match (id, project) {
        (Some(id), Some(p)) => {
            super::custom_field::of_project(tx, p.id, &id.to_string())?.is_some()
        }
        (Some(id), None) => tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM custom_fields WHERE id = ?1)",
            [b(id)],
            |r| r.get(0),
        )?,
        (None, _) => false,
    };
    match id {
        Some(id) if known => Ok(format!("{CUSTOM_FIELD_PREFIX}{id}")),
        _ => Err(messages::invalid(
            path,
            "automation-field-unknown",
            &[("field", field.to_owned())],
        )),
    }
}

/// Refuses a field of the trigger or an action that its kind doesn't use.
fn refuse_extra(path: &str, present: bool, field: &str) -> Result<(), Failure> {
    if present {
        return Err(messages::invalid(
            &format!("{path}.{field}"),
            "automation-field-not-used",
            &[("field", field.to_owned())],
        ));
    }
    Ok(())
}

fn require(path: &str, value: Option<&str>, field: &str) -> Result<String, Failure> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => Ok(v.to_owned()),
        None => Err(messages::invalid(
            &format!("{path}.{field}"),
            "automation-field-needed",
            &[("field", field.to_owned())],
        )),
    }
}

/// The trigger as it is kept: checked, its custom fields written as
/// history names them, its status named as the project names it.
fn check_trigger(
    tx: &Transaction<'_>,
    project: Option<&ProjectRow>,
    t: &AutomationTrigger,
) -> Result<AutomationTrigger, Failure> {
    let path = "trigger";
    let mut out = t.clone();
    let on = t.on;
    refuse_extra(
        path,
        on != TriggerKind::FieldChanged && !t.fields.is_empty(),
        "fields",
    )?;
    refuse_extra(
        path,
        on != TriggerKind::MovedToStatus && t.status.is_some(),
        "status",
    )?;
    let scheduled = on == TriggerKind::Schedule;
    refuse_extra(path, !scheduled && t.every.is_some(), "every")?;
    refuse_extra(path, !scheduled && t.time.is_some(), "time")?;
    refuse_extra(
        path,
        t.weekday.is_some() && t.every != Some(ScheduleEvery::Weekly),
        "weekday",
    )?;
    match on {
        TriggerKind::FieldChanged => {
            if t.fields.len() > FIELDS_MAX {
                return Err(messages::invalid(
                    "trigger.fields",
                    "automation-too-many-fields",
                    &[("max", FIELDS_MAX.to_string())],
                ));
            }
            let mut fields = Vec::new();
            for (i, f) in t.fields.iter().enumerate() {
                let at = format!("trigger.fields[{i}]");
                let f = f.trim();
                let kept = if WATCHABLE_FIELDS.contains(&f) {
                    f.to_owned()
                } else {
                    check_custom_field(tx, project, &at, f)?
                };
                if !fields.contains(&kept) {
                    fields.push(kept);
                }
            }
            out.fields = fields;
        }
        TriggerKind::MovedToStatus => {
            out.status = match (&t.status, project) {
                (Some(s), Some(p)) => {
                    Some(lookup::named(tx, List::Statuses, p, "trigger.status", s)?.name)
                }
                (Some(s), None) => Some(require(path, Some(s), "status")?),
                (None, _) => None,
            };
        }
        TriggerKind::Schedule => {
            if t.every.is_none() {
                require(path, None, "every")?;
            }
            let time = require(path, t.time.as_deref(), "time")?;
            let Some(minutes) = clock_minutes(&time) else {
                return Err(messages::invalid(
                    "trigger.time",
                    "automation-time",
                    &[("time", time)],
                ));
            };
            out.time = Some(crate::time::clock_text(minutes));
            if t.every == Some(ScheduleEvery::Weekly) && t.weekday.is_none() {
                require(path, None, "weekday")?;
            }
        }
        _ => {}
    }
    Ok(out)
}

/// Checks one action and returns it as it is kept.
fn check_action(
    tx: &Transaction<'_>,
    project: Option<&ProjectRow>,
    i: usize,
    a: &AutomationAction,
) -> Result<AutomationAction, Failure> {
    let path = format!("actions[{i}]");
    let k = a.kind;
    let mut out = a.clone();
    let uses_text = matches!(
        k,
        ActionKind::AddComment
            | ActionKind::Notify
            | ActionKind::CreateSubtask
            | ActionKind::CreateLinkedTask
    );
    let creates = matches!(k, ActionKind::CreateSubtask | ActionKind::CreateLinkedTask);
    refuse_extra(
        &path,
        k != ActionKind::SetFields && !a.fields.is_empty(),
        "fields",
    )?;
    refuse_extra(
        &path,
        !matches!(k, ActionKind::AddLabel | ActionKind::RemoveLabel) && a.label.is_some(),
        "label",
    )?;
    refuse_extra(
        &path,
        k != ActionKind::MoveToStatus && a.status.is_some(),
        "status",
    )?;
    refuse_extra(&path, !uses_text && a.text.is_some(), "text")?;
    refuse_extra(&path, !creates && a.template.is_some(), "template")?;
    refuse_extra(
        &path,
        k != ActionKind::CreateLinkedTask && a.link.is_some(),
        "link",
    )?;
    match k {
        ActionKind::SetFields => {
            if a.fields.is_empty() {
                require(&path, None, "fields")?;
            }
            if a.fields.len() > FIELDS_MAX {
                return Err(messages::invalid(
                    &format!("{path}.fields"),
                    "automation-too-many-fields",
                    &[("max", FIELDS_MAX.to_string())],
                ));
            }
            for (field, value) in &a.fields {
                check_set_value(&format!("{path}.fields.{field}"), field, value)?;
            }
        }
        ActionKind::AddLabel | ActionKind::RemoveLabel => {
            let label = require(&path, a.label.as_deref(), "label")?;
            out.label = Some(match project {
                Some(p) => {
                    lookup::named(tx, List::Labels, p, &format!("{path}.label"), &label)?.name
                }
                None => label,
            });
        }
        ActionKind::MoveToStatus => {
            let status = require(&path, a.status.as_deref(), "status")?;
            out.status = Some(match project {
                Some(p) => {
                    lookup::named(tx, List::Statuses, p, &format!("{path}.status"), &status)?.name
                }
                None => status,
            });
        }
        ActionKind::AddComment | ActionKind::Notify => {
            let max = if k == ActionKind::Notify {
                NOTICE_MAX
            } else {
                COMMENT_MAX
            };
            let text = require(&path, a.text.as_deref(), "text")?;
            if text.chars().count() > max {
                return Err(messages::invalid(
                    &format!("{path}.text"),
                    "automation-text-too-long",
                    &[("max", max.to_string())],
                ));
            }
            out.text = Some(text);
        }
        ActionKind::CreateSubtask | ActionKind::CreateLinkedTask => {
            let text = a.text.as_deref().map(str::trim).filter(|t| !t.is_empty());
            let template = a
                .template
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty());
            if text.is_none() && template.is_none() {
                require(&path, None, "text")?;
            }
            if let Some(t) = text
                && t.chars().count() > 500
            {
                return Err(messages::invalid(
                    &format!("{path}.text"),
                    "automation-text-too-long",
                    &[("max", "500".to_owned())],
                ));
            }
            out.text = text.map(str::to_owned);
            out.template = template.map(str::to_owned);
            if matches!(
                a.link,
                Some(RelationKind::SubtaskOf | RelationKind::ParentOf)
            ) {
                return Err(messages::invalid(
                    &format!("{path}.link"),
                    "automation-link-kind",
                    &[],
                ));
            }
        }
    }
    Ok(out)
}

/// Checks the value a `set_fields` action gives `field`.
fn check_set_value(path: &str, field: &str, value: &Value) -> Result<(), Failure> {
    let bad = || {
        Err(messages::invalid(
            path,
            "automation-set-value",
            &[("field", field.to_owned())],
        ))
    };
    if !AUTOMATION_SETTABLE_FIELDS.contains(&field) {
        // A custom field, by name, query name or ID: its value is checked
        // when the rule runs, against the task's project.
        return if field.trim().is_empty() {
            bad()
        } else {
            Ok(())
        };
    }
    match (field, value) {
        (f, Value::Null) if CLEARABLE.contains(&f) || f == "priority" => Ok(()),
        ("progress", Value::Number(n)) if n.as_u64().is_some_and(|n| n <= 100) => Ok(()),
        ("priority", Value::String(s)) if PRIORITIES.contains(&s.as_str()) => Ok(()),
        ("priority" | "progress", _) => bad(),
        (_, Value::String(s)) if !s.trim().is_empty() => Ok(()),
        _ => bad(),
    }
}

/// Checks what a rule does and returns it as it is kept.
fn check_definition(
    w: &WriteCtx<'_>,
    project: Option<&ProjectRow>,
    trigger: &AutomationTrigger,
    condition: &str,
    actions: &[AutomationAction],
) -> Result<Definition, Failure> {
    let trigger = check_trigger(w.tx(), project, trigger)?;
    let condition = condition.trim().to_owned();
    if !condition.is_empty() {
        super::query::compile(w.tx(), w.zone(), w.now(), &condition, "condition")?;
    } else if trigger.on == TriggerKind::Schedule {
        return Err(messages::invalid(
            "condition",
            "automation-schedule-needs-condition",
            &[],
        ));
    }
    if actions.is_empty() || actions.len() > AUTOMATION_ACTIONS_MAX {
        return Err(messages::invalid(
            "actions",
            "automation-action-count",
            &[("max", AUTOMATION_ACTIONS_MAX.to_string())],
        ));
    }
    let actions = actions
        .iter()
        .enumerate()
        .map(|(i, a)| check_action(w.tx(), project, i, a))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Definition {
        trigger,
        condition,
        actions,
    })
}

/// Refuses a name another rule of the same project (or of the workspace)
/// has.
fn check_unique(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), Failure> {
    let wanted = fold(&r.name);
    let taken = rows(tx, false)?.into_iter().any(|(other, o)| {
        other != id && o.project_id() == r.project_id() && fold(&o.name) == wanted
    });
    if taken {
        let error = messages::conflict("automation-name-taken", &[("rule", r.name.clone())])
            .with_detail("field", "name");
        return Err(messages::hint(error, "automation-name-taken-hint", &[]).into());
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Changing rules

fn create(w: &mut WriteCtx<'_>, input: AutomationCreateInput) -> Result<AutomationChange, Failure> {
    let name =
        gk_core::check_line("name", &input.name, NAME_MAX).map_err(|e| invalid("name", e))?;
    let project = match &input.project {
        Some(p) => Some(active_project(w.tx(), p)?),
        None => None,
    };
    let d = check_definition(
        w,
        project.as_ref(),
        &input.trigger,
        input.condition.as_deref().unwrap_or_default(),
        &input.actions,
    )?;
    let pid = project.as_ref().map(|p| p.id);
    let last = rows(w.tx(), false)?
        .into_iter()
        .filter(|(_, r)| r.project_id() == pid)
        .filter_map(|(_, r)| Rank::parse(&r.rank).ok())
        .max_by(|a, b| a.as_str().cmp(b.as_str()));
    let rank = last.map_or_else(Rank::first, |r| r.after());
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let caller = w.caller().actor.clone();
    let record = Record {
        project: pid.map(|p| p.to_string()),
        name,
        enabled: input.enabled.unwrap_or(true),
        trigger: d.trigger,
        condition: d.condition,
        actions: d.actions,
        rank: rank.as_str().to_owned(),
        created_at: w.now(),
        created_by_kind: caller.kind.as_str().to_owned(),
        created_by_name: caller.name,
        updated_at: w.now(),
        updated_by_kind: caller.kind.as_str().to_owned(),
    };
    check_unique(w.tx(), id, &record)?;
    set(w, id, Some(&record))?;
    Ok(AutomationChange {
        rule: output(w.tx(), id, &record, false)?,
        deleted: false,
        group: w.group_if_changed(),
    })
}

fn update(w: &mut WriteCtx<'_>, input: AutomationUpdateInput) -> Result<AutomationChange, Failure> {
    let (id, mut record) = find(w.tx(), &input.rule, input.project.as_deref())?;
    let project = match record.project_id() {
        Some(p) => {
            let row = lookup::project_by_id(w.tx(), p)?.ok_or_else(|| {
                Failure::Op(messages::not_found(
                    "automation-not-found",
                    &[("rule", input.rule.clone())],
                ))
            })?;
            lookup::ensure_active(&row)?;
            Some(row)
        }
        None => None,
    };
    if let Some(name) = &input.name {
        record.name =
            gk_core::check_line("name", name, NAME_MAX).map_err(|e| invalid("name", e))?;
    }
    if let Some(enabled) = input.enabled {
        record.enabled = enabled;
    }
    let redefined = input.trigger.is_some() || input.condition.is_some() || input.actions.is_some();
    if redefined {
        let d = check_definition(
            w,
            project.as_ref(),
            input.trigger.as_ref().unwrap_or(&record.trigger),
            input.condition.as_deref().unwrap_or(&record.condition),
            input.actions.as_deref().unwrap_or(&record.actions),
        )?;
        record.trigger = d.trigger;
        record.condition = d.condition;
        record.actions = d.actions;
    }
    let before = load(w.tx(), id)?;
    if before.as_ref() != Some(&record) {
        record.updated_at = w.now();
        record.updated_by_kind = w.caller().actor.kind.as_str().to_owned();
    }
    check_unique(w.tx(), id, &record)?;
    set(w, id, Some(&record))?;
    Ok(AutomationChange {
        rule: output(w.tx(), id, &record, false)?,
        deleted: false,
        group: w.group_if_changed(),
    })
}

fn delete(w: &mut WriteCtx<'_>, input: AutomationRef) -> Result<AutomationChange, Failure> {
    let (id, record) = find(w.tx(), &input.rule, input.project.as_deref())?;
    let rule = output(w.tx(), id, &record, false)?;
    set(w, id, None)?;
    Ok(AutomationChange {
        rule,
        deleted: true,
        group: w.group_if_changed(),
    })
}

/// Rewrites the condition of every rule of `project` (every rule for
/// `None`) that `rewrite` changes, in the running command, as saved views'
/// queries are rewritten when a name they use changes.
pub(crate) fn rewrite_conditions(
    w: &mut WriteCtx<'_>,
    project: Option<Id>,
    rewrite: &dyn Fn(&str) -> Option<String>,
) -> Result<(), Failure> {
    for (id, mut record) in rows(w.tx(), false)? {
        if project.is_some() && record.project_id() != project {
            continue;
        }
        if record.condition.is_empty() {
            continue;
        }
        if let Some(condition) = rewrite(&record.condition) {
            record.condition = condition;
            set(w, id, Some(&record))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// The run log

fn run_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<AutomationRun> {
    let result: String = r.get(4)?;
    let trigger: String = r.get(5)?;
    let error: Option<String> = r.get(10)?;
    Ok(AutomationRun {
        id: id_at(r, 0)?.to_string(),
        rule: id_at(r, 1)?.to_string(),
        rule_name: r.get(2)?,
        at: rfc3339(r.get(3)?),
        result: match result.as_str() {
            "failed" => RunResult::Failed,
            "limited" => RunResult::Limited,
            _ => RunResult::Done,
        },
        trigger: serde_json::from_value(Value::String(trigger)).unwrap_or(TriggerKind::TaskCreated),
        task: r.get(6)?,
        group: opt_id_at(r, 7)?.map(|g| g.to_string()),
        changes: r.get(8)?,
        depth: r.get(9)?,
        error: error.and_then(|e| serde_json::from_str(&e).ok()),
    })
}

/// Runs, newest first: of one rule, of the rules a project sees, or all;
/// at most `limit`, with how many there are.
fn run_rows(
    tx: &Transaction<'_>,
    rule: Option<Id>,
    project: Option<Id>,
    problems_only: bool,
    limit: u32,
) -> Result<(Vec<AutomationRun>, u64), Failure> {
    let mut filter = vec!["1 = 1".to_owned()];
    let mut args: Vec<Sql> = Vec::new();
    if let Some(rule) = rule {
        args.push(Sql::Blob(b(rule)));
        filter.push(format!("u.rule_id = ?{}", args.len()));
    }
    if let Some(p) = project {
        args.push(Sql::Blob(b(p)));
        filter.push(format!(
            "u.rule_id IN (SELECT id FROM automation_rules
                            WHERE project_id IS NULL OR project_id = ?{})",
            args.len()
        ));
    }
    if problems_only {
        filter.push("u.result <> 'done'".to_owned());
    }
    let filter = filter.join(" AND ");
    let total: i64 = tx.query_row(
        &format!("SELECT count(*) FROM automation_runs u WHERE {filter}"),
        params_from_iter(args.iter()),
        |r| r.get(0),
    )?;
    let mut stmt = tx.prepare(&format!(
        "SELECT u.id, u.rule_id, u.rule_name, u.at, u.result, u.trigger_kind,
                (SELECT p.key || '-' || t.number FROM tasks t JOIN projects p ON p.id = t.project_id
                  WHERE t.id = u.task_id AND t.deleted_at IS NULL),
                u.group_id, u.changes, u.depth, u.error
           FROM automation_runs u WHERE {filter}
          ORDER BY u.at DESC, u.rowid DESC LIMIT {limit}"
    ))?;
    let items = stmt
        .query_map(params_from_iter(args.iter()), run_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok((items, u64::try_from(total).unwrap_or(0)))
}

pub(crate) fn runs(
    tx: &Transaction<'_>,
    input: &AutomationRunsInput,
) -> Result<AutomationRunList, Failure> {
    let limit = input.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(messages::invalid(
            "limit",
            "input-limit",
            &[("max", "200".to_owned())],
        ));
    }
    let rule = match &input.rule {
        Some(r) => Some(find(tx, r, input.project.as_deref())?.0),
        None => None,
    };
    let project = match (&input.project, rule) {
        (Some(p), None) => Some(lookup::project(tx, "project", p)?.id),
        _ => None,
    };
    let (items, total) = run_rows(tx, rule, project, input.problems_only, limit)?;
    Ok(AutomationRunList { items, total })
}

/// One entry of the run log.
#[derive(Clone, Copy)]
struct Logged<'a> {
    rule: Id,
    name: &'a str,
    result: RunResult,
    trigger: TriggerKind,
    task: Option<Id>,
    group: Option<Id>,
    changes: usize,
    depth: u32,
    error: Option<&'a OpError>,
}

fn log(tx: &Transaction<'_>, at: i64, entry: &Logged<'_>) -> Result<(), Failure> {
    let result = match entry.result {
        RunResult::Done => "done",
        RunResult::Failed => "failed",
        RunResult::Limited => "limited",
    };
    let error = entry
        .error
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?;
    tx.execute(
        "INSERT INTO automation_runs (id, rule_id, rule_name, at, result, trigger_kind, task_id,
                                      group_id, changes, depth, error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            b(Id::generate_at(u64::try_from(at).unwrap_or(0))),
            b(entry.rule),
            entry.name,
            at,
            result,
            entry.trigger.as_str(),
            entry.task.map(b),
            entry.group.map(b),
            i64::try_from(entry.changes).unwrap_or(i64::MAX),
            entry.depth,
            error,
        ],
    )?;
    tx.execute(
        "DELETE FROM automation_runs WHERE rule_id = ?1 AND rowid NOT IN (
            SELECT rowid FROM automation_runs WHERE rule_id = ?1
             ORDER BY at DESC, rowid DESC LIMIT ?2)",
        params![b(entry.rule), AUTOMATION_RUNS_KEPT as i64],
    )?;
    Ok(())
}

/// Whether the rule may run once more this hour: counts the run. `None`:
/// it may; `Some(true)`: it may not, and this is the first refusal of the
/// hour (logged); `Some(false)`: refused again (not logged).
fn rate_limited(tx: &Transaction<'_>, rule: Id, now: i64) -> Result<Option<bool>, Failure> {
    let (start, count): (Option<i64>, u32) = tx.query_row(
        "SELECT window_start, window_runs FROM automation_rules WHERE id = ?1",
        [b(rule)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let fresh = start.is_none_or(|s| now - s >= HOUR_MS || now < s);
    let (start, count) = if fresh {
        (now, 0)
    } else {
        (start.unwrap_or(now), count)
    };
    let refused = match count.cmp(&AUTOMATION_RUNS_PER_HOUR) {
        std::cmp::Ordering::Less => None,
        std::cmp::Ordering::Equal => Some(true),
        std::cmp::Ordering::Greater => Some(false),
    };
    tx.execute(
        "UPDATE automation_rules SET window_start = ?2, window_runs = ?3 WHERE id = ?1",
        params![
            b(rule),
            start,
            count.saturating_add(1).min(AUTOMATION_RUNS_PER_HOUR + 1)
        ],
    )?;
    Ok(refused)
}

// ---------------------------------------------------------------------
// Events

/// A run of a rule a command asks for once it is done.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Queued {
    pub(crate) rule: Id,
    pub(crate) task: Id,
    pub(crate) trigger: TriggerKind,
}

/// What happened to one task in one command.
#[derive(Debug, Clone)]
struct Event {
    task: Id,
    created: bool,
    /// It entered its status (moved, or was created in it).
    entered: bool,
    /// The watched fields that changed.
    fields: Vec<String>,
    commented: bool,
    /// A run asked for by the schedule: that rule, set off by that.
    queued: Option<(Id, TriggerKind)>,
}

/// What one command, or one rule's run, did, for the rules that may follow.
#[derive(Debug, Clone, Default)]
pub(crate) struct Events {
    items: Vec<Event>,
    /// The rules that led to these events, first to last.
    chain: Vec<Id>,
    /// Whether the changes came from an agent, or from rules held to the
    /// rules for agents.
    held: bool,
    /// Who made the changes, for the `{actor}` placeholder.
    actor: String,
}

impl Event {
    fn on(task: Id) -> Self {
        Self {
            task,
            created: false,
            entered: false,
            fields: Vec::new(),
            commented: false,
            queued: None,
        }
    }
}

fn watchable(field: &str) -> bool {
    WATCHABLE_FIELDS.contains(&field) || field.starts_with(CUSTOM_FIELD_PREFIX)
}

/// The events in what `w` recorded.
fn events_in(w: &WriteCtx<'_>) -> Result<Vec<Event>, Failure> {
    fn at(out: &mut Vec<Event>, task: Id) -> usize {
        if let Some(i) = out.iter().position(|e| e.task == task) {
            i
        } else {
            out.push(Event::on(task));
            out.len() - 1
        }
    }
    let mut out: Vec<Event> = Vec::new();
    for (entity, id, _, fields) in w.pending() {
        let differs = |c: &FieldChange| c.before != c.after;
        if entity == TASK {
            let created = fields.contains_key("created");
            let entered = created || fields.get("status").is_some_and(differs);
            let changed: Vec<String> = fields
                .iter()
                .filter(|(k, c)| watchable(k) && differs(c))
                .map(|(k, _)| k.clone())
                .collect();
            if !created && !entered && changed.is_empty() {
                continue;
            }
            let i = at(&mut out, id);
            let e = &mut out[i];
            e.created |= created;
            e.entered |= entered;
            for f in changed {
                if !e.fields.contains(&f) {
                    e.fields.push(f);
                }
            }
        } else if entity == super::comment::ENTITY
            && fields
                .get("created")
                .is_some_and(|c| c.after == Some(Value::Bool(true)))
        {
            let task: Option<Vec<u8>> = w
                .tx()
                .query_row("SELECT task_id FROM comments WHERE id = ?1", [b(id)], |r| {
                    r.get(0)
                })
                .optional()?;
            if let Some(task) = task.and_then(|t| Id::from_slice(&t).ok()) {
                let i = at(&mut out, task);
                out[i].commented = true;
            }
        }
    }
    Ok(out)
}

/// What the command running in `w` did that rules may follow, and the
/// runs it asked for. Commands that change the configuration or put back
/// earlier states (history, imports, the trash) set no rule off.
pub(crate) fn events_of(w: &mut WriteCtx<'_>) -> Result<Events, Failure> {
    let queued = w.take_queued();
    let mut items = if super::rules::applies_to(w.op()) {
        events_in(w)?
    } else {
        Vec::new()
    };
    items.extend(queued.into_iter().map(|q| Event {
        queued: Some((q.rule, q.trigger)),
        ..Event::on(q.task)
    }));
    Ok(Events {
        items,
        chain: Vec::new(),
        held: w.acts_as_agent(),
        actor: w.caller().actor.name.clone(),
    })
}

/// The task facts a rule's run needs.
struct TaskNow {
    project: Id,
    status: String,
    live: bool,
}

fn task_now(tx: &Transaction<'_>, task: Id) -> Result<Option<TaskNow>, Failure> {
    Ok(tx
        .query_row(
            "SELECT t.project_id, s.name,
                    t.deleted_at IS NULL AND p.deleted_at IS NULL AND p.state = 'active'
               FROM tasks t JOIN projects p ON p.id = t.project_id
               JOIN statuses s ON s.id = t.status_id WHERE t.id = ?1",
            [b(task)],
            |r| {
                Ok(TaskNow {
                    project: id_at(r, 0)?,
                    status: r.get(1)?,
                    live: r.get(2)?,
                })
            },
        )
        .optional()?)
}

/// Whether `rule`'s trigger fits what happened to the task.
fn fits(record: &Record, event: &Event, now: &TaskNow) -> bool {
    let t = &record.trigger;
    match t.on {
        TriggerKind::TaskCreated => event.created,
        TriggerKind::FieldChanged => {
            !event.created
                && if t.fields.is_empty() {
                    !event.fields.is_empty()
                } else {
                    t.fields.iter().any(|f| event.fields.contains(f))
                }
        }
        TriggerKind::MovedToStatus => {
            !event.created
                && event.entered
                && t.status
                    .as_deref()
                    .is_none_or(|s| fold(s) == fold(&now.status))
        }
        TriggerKind::CommentAdded => event.commented,
        TriggerKind::DueDateReached | TriggerKind::Overdue | TriggerKind::Schedule => false,
    }
}

// ---------------------------------------------------------------------
// Conditions

fn sql_params(params: &[Param]) -> Vec<Sql> {
    params
        .iter()
        .map(|p| match p {
            Param::Int(n) => Sql::Integer(*n),
            Param::Text(t) => Sql::Text(t.clone()),
        })
        .collect()
}

/// The compiled condition, or the mistake in it.
fn compile_condition(
    tx: &Transaction<'_>,
    meta: &CommandMeta,
    condition: &str,
    scope: Option<String>,
) -> Result<Result<Compiled, OpError>, Failure> {
    let compiled = match super::query::try_compile(tx, &meta.zone, meta.at, condition)? {
        Ok((_, checked)) => Compiled::new(
            &checked,
            &SqlEnv {
                now: meta.at,
                scope: scope.map(|project| gk_query::Scope {
                    project,
                    subprojects: false,
                }),
                default_sort: Vec::new(),
            },
        ),
        Err(e) => {
            return Ok(Err(
                super::query::query_error(&e, condition, "condition").into()
            ));
        }
    };
    Ok(Ok(compiled))
}

/// Whether task `task` matches `condition` (every task when empty), or
/// the mistake in it.
fn matches(
    tx: &Transaction<'_>,
    meta: &CommandMeta,
    condition: &str,
    task: Id,
) -> Result<Result<bool, OpError>, Failure> {
    if condition.trim().is_empty() {
        return Ok(Ok(true));
    }
    let compiled = match compile_condition(tx, meta, condition, None)? {
        Ok(c) => c,
        Err(e) => return Ok(Err(e)),
    };
    let ids = compiled.ids();
    let mut args = sql_params(&ids.params);
    args.push(Sql::Blob(b(task)));
    let found: bool = tx.query_row(
        &format!(
            "SELECT EXISTS (SELECT 1 FROM ({}) q WHERE q.id = ?{})",
            ids.text,
            args.len()
        ),
        params_from_iter(args.iter()),
        |r| r.get(0),
    )?;
    Ok(Ok(found))
}

// ---------------------------------------------------------------------
// Actions

/// The handlers rules act through: the catalog's own.
fn handlers() -> &'static Registry {
    static HANDLERS: OnceLock<Registry> = OnceLock::new();
    HANDLERS.get_or_init(Registry::standard)
}

/// Runs catalog command `op` with `input` in `w`, as a rule's action.
fn call(w: &mut WriteCtx<'_>, op: &str, input: Value) -> Result<Value, Failure> {
    let entry = handlers()
        .get(op)
        .ok_or_else(|| Failure::Op(OpError::storage(format!("no handler for {op}"))))?;
    (entry.validate)(&input)?;
    match &entry.handler {
        Handler::Command(f) => f(w, input),
        _ => Err(Failure::Op(OpError::storage(format!(
            "{op} is not a command"
        )))),
    }
}

/// What the placeholders of a rule's texts stand for.
struct Placeholders {
    values: Vec<(&'static str, String)>,
}

impl Placeholders {
    fn of(
        tx: &Transaction<'_>,
        task: Id,
        rule: &str,
        actor: &str,
        today: Day,
    ) -> Result<Self, Failure> {
        let (key, title, status, priority, due, project): (
            String,
            String,
            String,
            i64,
            Option<String>,
            String,
        ) = tx.query_row(
            "SELECT p.key || '-' || t.number, t.title, s.name, t.priority, t.due_date, p.key
               FROM tasks t JOIN projects p ON p.id = t.project_id
               JOIN statuses s ON s.id = t.status_id WHERE t.id = ?1",
            [b(task)],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )?;
        Ok(Self {
            values: vec![
                ("key", key),
                ("title", title),
                ("status", status),
                (
                    "priority",
                    super::fields::priority_name(priority).to_owned(),
                ),
                ("due_date", due.unwrap_or_default()),
                ("project", project),
                ("rule", rule.to_owned()),
                ("actor", actor.to_owned()),
                ("today", today.to_string()),
            ],
        })
    }

    fn fill(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for (name, value) in &self.values {
            out = out.replace(&format!("{{{name}}}"), value);
        }
        out
    }
}

fn task_key(tx: &Transaction<'_>, task: Id) -> Result<String, Failure> {
    Ok(tx.query_row(
        "SELECT p.key || '-' || t.number FROM tasks t JOIN projects p ON p.id = t.project_id
          WHERE t.id = ?1",
        [b(task)],
        |r| r.get(0),
    )?)
}

/// Puts which action failed into its error.
fn at_action(e: Failure, i: usize) -> Failure {
    match e {
        Failure::Op(op) => Failure::Op(op.with_detail("action", i)),
        other => other,
    }
}

/// The context a rule's actions see.
struct Acting<'a> {
    rule: Id,
    name: &'a str,
    actor: &'a str,
}

/// Makes `record`'s actions on task `task`, in order.
fn act(w: &mut WriteCtx<'_>, a: &Acting<'_>, record: &Record, task: Id) -> Result<(), Failure> {
    for (i, action) in record.actions.iter().enumerate() {
        act_one(w, a, action, task).map_err(|e| at_action(e, i))?;
    }
    Ok(())
}

fn act_one(
    w: &mut WriteCtx<'_>,
    a: &Acting<'_>,
    action: &AutomationAction,
    task: Id,
) -> Result<(), Failure> {
    let today = w.zone().day_of(w.now());
    let names = Placeholders::of(w.tx(), task, a.name, a.actor, today)?;
    let key = task_key(w.tx(), task)?;
    let project: String = w.tx().query_row(
        "SELECT p.key FROM tasks t JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
        [b(task)],
        |r| r.get(0),
    )?;
    let text = action.text.as_deref().map(|t| names.fill(t));
    match action.kind {
        ActionKind::SetFields => {
            let mut input = Map::new();
            let mut clear = Vec::new();
            let mut custom = Map::new();
            input.insert("key".into(), Value::String(key));
            for (field, value) in &action.fields {
                let value = match value {
                    Value::String(s) => Value::String(names.fill(s)),
                    other => other.clone(),
                };
                if AUTOMATION_SETTABLE_FIELDS.contains(&field.as_str()) {
                    match (field.as_str(), value) {
                        ("priority", Value::Null) => {
                            input.insert("priority".into(), Value::from("none"));
                        }
                        (f, Value::Null) => clear.push(Value::from(f)),
                        (f, v) => {
                            input.insert(f.to_owned(), v);
                        }
                    }
                } else {
                    custom.insert(field.clone(), value);
                }
            }
            if !clear.is_empty() {
                input.insert("clear".into(), Value::Array(clear));
            }
            if !custom.is_empty() {
                input.insert("fields".into(), Value::Object(custom));
            }
            call(w, "task.update", Value::Object(input))?;
        }
        ActionKind::AddLabel | ActionKind::RemoveLabel => {
            let list = if action.kind == ActionKind::AddLabel {
                "add_labels"
            } else {
                "remove_labels"
            };
            let label = action.label.clone().unwrap_or_default();
            call(w, "task.update", json!({ "key": key, list: [label] }))?;
        }
        ActionKind::MoveToStatus => {
            call(
                w,
                "task.move",
                json!({ "key": key, "status": action.status }),
            )?;
        }
        ActionKind::AddComment => {
            call(
                w,
                "comment.add",
                json!({ "key": key, "body": text.unwrap_or_default() }),
            )?;
        }
        ActionKind::CreateSubtask | ActionKind::CreateLinkedTask => {
            let subtask = action.kind == ActionKind::CreateSubtask;
            let made = match &action.template {
                Some(template) => {
                    let mut input = json!({ "template": template, "project": project });
                    if let Some(title) = &text {
                        input["title"] = Value::String(title.clone());
                    }
                    if subtask {
                        input["parent"] = Value::String(key.clone());
                    }
                    call(w, "template.apply", input)?
                }
                None => {
                    let mut input = json!({
                        "project": project,
                        "title": text.clone().unwrap_or_default(),
                    });
                    if subtask {
                        input["parent"] = Value::String(key.clone());
                    }
                    call(w, "task.create", input)?
                }
            };
            let new_key = made
                .get("task")
                .and_then(|t| t.get("key"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            if !subtask && let Some(other) = new_key {
                let kind = action.link.unwrap_or(RelationKind::Relates);
                call(
                    w,
                    "relation.add",
                    json!({ "key": key, "kind": kind, "other": other }),
                )?;
            }
        }
        ActionKind::Notify => {
            let text: String = text.unwrap_or_default().chars().take(NOTICE_MAX).collect();
            if !text.trim().is_empty() {
                w.tx().execute(
                    "INSERT INTO automation_notices (id, rule_id, task_id, at, text)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        b(Id::generate_at(u64::try_from(w.now()).unwrap_or(0))),
                        b(a.rule),
                        b(task),
                        w.now(),
                        text
                    ],
                )?;
            }
        }
    }
    Ok(())
}

/// The command facts of a rule's run inside the command `base`.
fn rule_meta(base: &CommandMeta, name: &str, held: bool) -> CommandMeta {
    CommandMeta {
        op: OP,
        group: base.group,
        at: base.at,
        ctx: Context::automation(name, base.ctx.via),
        workspace_id: base.workspace_id,
        layout: base.layout.clone(),
        zone: base.zone.clone(),
        as_agent: held,
    }
}

/// How a rule's run ended.
enum Outcome {
    Done {
        finished: Finished,
        events: Vec<Event>,
    },
    Failed(OpError),
}

/// Runs `record`'s actions on `task` as a command of its own in the
/// running transaction, with its own history rows in `base`'s group. A
/// refusal keeps nothing of the run; a storage failure ends the command.
fn run_rule(
    tx: &Transaction<'_>,
    base: &CommandMeta,
    acting: &Acting<'_>,
    record: &Record,
    task: Id,
    held: bool,
) -> Result<Outcome, Failure> {
    let meta = rule_meta(base, acting.name, held);
    tx.execute_batch("SAVEPOINT automation_rule")?;
    let result = (|| {
        let mut w = WriteCtx::new(tx, &meta);
        act(&mut w, acting, record, task)?;
        super::recurrence::after_command(&mut w)?;
        super::rules::enforce(&mut w)?;
        let events = events_in(&w)?;
        let finished = w.finish()?;
        Ok::<_, Failure>((finished, events))
    })();
    match result {
        Ok((finished, events)) => {
            tx.execute_batch("RELEASE automation_rule")?;
            Ok(Outcome::Done { finished, events })
        }
        Err(e) => {
            tx.execute_batch("ROLLBACK TO automation_rule; RELEASE automation_rule")?;
            match e {
                Failure::Op(op) => Ok(Outcome::Failed(op)),
                other => Err(other),
            }
        }
    }
}

/// Runs the rules that follow what a command did (`events`), and in turn
/// the rules that follow what those did, adding their changes to the
/// command's (`finished`).
pub(crate) fn follow(
    tx: &Transaction<'_>,
    base: &CommandMeta,
    events: Events,
    finished: &mut Finished,
) -> Result<(), Failure> {
    if events.items.is_empty() {
        return Ok(());
    }
    let rules = rows(tx, true)?;
    if rules.is_empty() {
        return Ok(());
    }
    let mut waiting = VecDeque::from([events]);
    while let Some(batch) = waiting.pop_front() {
        let depth = u32::try_from(batch.chain.len()).unwrap_or(u32::MAX);
        for event in &batch.items {
            let Some(now) = task_now(tx, event.task)? else {
                continue;
            };
            if !now.live {
                continue;
            }
            let chosen: Vec<&(Id, Record)> = match event.queued {
                Some((rule, _)) => rules.iter().filter(|(id, _)| *id == rule).collect(),
                None => rules
                    .iter()
                    .filter(|(id, _)| !batch.chain.contains(id))
                    .filter(|(_, r)| r.project_id().is_none_or(|p| p == now.project))
                    .filter(|(_, r)| fits(r, event, &now))
                    .collect(),
            };
            for (id, record) in chosen {
                let trigger = event.queued.map_or(record.trigger.on, |(_, t)| t);
                let entry = Logged {
                    rule: *id,
                    name: &record.name,
                    result: RunResult::Done,
                    trigger,
                    task: Some(event.task),
                    group: None,
                    changes: 0,
                    depth,
                    error: None,
                };
                let failed = |result: RunResult, e: &OpError| {
                    log(
                        tx,
                        base.at,
                        &Logged {
                            result,
                            error: Some(e),
                            ..entry
                        },
                    )
                };
                match matches(tx, base, &record.condition, event.task)? {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(e) => {
                        failed(RunResult::Failed, &e)?;
                        continue;
                    }
                }
                if depth >= AUTOMATION_DEPTH_MAX {
                    let e = messages::error(
                        ErrorCode::Limit,
                        "automation-depth-limit",
                        &[("max", AUTOMATION_DEPTH_MAX.to_string())],
                    );
                    failed(RunResult::Limited, &e)?;
                    continue;
                }
                if let Some(first) = rate_limited(tx, *id, base.at)? {
                    if first {
                        let e = messages::error(
                            ErrorCode::Limit,
                            "automation-rate-limit",
                            &[("max", AUTOMATION_RUNS_PER_HOUR.to_string())],
                        );
                        failed(RunResult::Limited, &e)?;
                    }
                    continue;
                }
                let held = batch.held || record.held_as_agent();
                let acting = Acting {
                    rule: *id,
                    name: &record.name,
                    actor: &batch.actor,
                };
                match run_rule(tx, base, &acting, record, event.task, held)? {
                    Outcome::Done {
                        finished: done,
                        events,
                    } => {
                        let changes = done.items();
                        let group = (changes > 0).then_some(base.group);
                        log(
                            tx,
                            base.at,
                            &Logged {
                                group,
                                changes,
                                ..entry
                            },
                        )?;
                        finished.absorb(done);
                        if !events.is_empty() {
                            let mut chain = batch.chain.clone();
                            chain.push(*id);
                            waiting.push_back(Events {
                                items: events,
                                chain,
                                held,
                                actor: record.name.clone(),
                            });
                        }
                    }
                    Outcome::Failed(e) => failed(RunResult::Failed, &e)?,
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// The dry run

fn test(w: &mut WriteCtx<'_>, input: AutomationTestInput) -> Result<AutomationTestResult, Failure> {
    let task = lookup::task(w.tx(), "task", &input.task)?;
    let key = task_key(w.tx(), task.id)?;
    let (rule, record) = match (&input.rule, &input.draft) {
        (Some(r), None) => find(w.tx(), r, input.project.as_deref())?,
        (None, Some(d)) => {
            let project = lookup::project_by_id(w.tx(), task.project)?;
            let def = check_definition(
                w,
                project.as_ref(),
                &d.trigger,
                d.condition.as_deref().unwrap_or_default(),
                &d.actions,
            )?;
            let name = d
                .name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map_or_else(
                    || messages::text("automation-draft-name", &[]),
                    str::to_owned,
                );
            let caller = w.caller().actor.kind.as_str().to_owned();
            let record = Record {
                project: Some(task.project.to_string()),
                name,
                enabled: true,
                trigger: def.trigger,
                condition: def.condition,
                actions: def.actions,
                rank: Rank::first().as_str().to_owned(),
                created_at: w.now(),
                created_by_kind: caller.clone(),
                created_by_name: w.caller().actor.name.clone(),
                updated_at: w.now(),
                updated_by_kind: caller,
            };
            (Id::generate_at(u64::try_from(w.now()).unwrap_or(0)), record)
        }
        _ => {
            return Err(messages::invalid(
                "rule",
                "automation-test-rule-or-draft",
                &[],
            ));
        }
    };
    let mut result = AutomationTestResult {
        task: key,
        matches: false,
        ok: false,
        changes: Vec::new(),
        notices: Vec::new(),
        error: None,
        failed_action: None,
    };
    match matches(w.tx(), w.meta(), &record.condition, task.id)? {
        Ok(true) => result.matches = true,
        Ok(false) => return Ok(result),
        Err(e) => {
            result.error = Some(e);
            return Ok(result);
        }
    }
    let meta = rule_meta(
        w.meta(),
        &record.name,
        w.acts_as_agent() || record.held_as_agent(),
    );
    let actor = w.caller().actor.name.clone();
    let (outcome, changes, notices) = w.preview(|w| {
        let tx = w.tx();
        let mut inner = WriteCtx::new(tx, &meta);
        let acting = Acting {
            rule,
            name: &record.name,
            actor: &actor,
        };
        let outcome = (|| {
            act(&mut inner, &acting, &record, task.id)?;
            super::recurrence::after_command(&mut inner)?;
            super::rules::enforce(&mut inner)
        })();
        let changes = describe(tx, &inner.pending())?;
        let mut stmt =
            tx.prepare("SELECT text FROM automation_notices WHERE rule_id = ?1 ORDER BY rowid")?;
        let notices = stmt
            .query_map([b(rule)], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((outcome, changes, notices))
    })?;
    result.changes = changes;
    result.notices = notices;
    match outcome {
        Ok(()) => result.ok = true,
        Err(Failure::Op(e)) => {
            result.failed_action = e
                .details
                .get("action")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok());
            result.error = Some(e);
        }
        Err(other) => return Err(other),
    }
    Ok(result)
}

/// The changes recorded so far, as a dry run shows them.
fn describe(
    tx: &Transaction<'_>,
    pending: &[Pending],
) -> Result<Vec<AutomationTestChange>, Failure> {
    let mut out = Vec::new();
    for (entity, id, _, fields) in pending {
        let task = if entity == TASK {
            Some(*id)
        } else if entity == super::comment::ENTITY {
            tx.query_row(
                "SELECT task_id FROM comments WHERE id = ?1",
                [b(*id)],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .optional()?
            .and_then(|t| Id::from_slice(&t).ok())
        } else {
            None
        };
        let key = match task {
            Some(t) => Some(task_key(tx, t)?),
            None => None,
        };
        for (field, change) in fields {
            if entity == TASK && matches!(field.as_str(), "rank" | "subtask_rank" | "number") {
                continue;
            }
            let shown = |v: &Option<Value>| -> Result<Option<Value>, Failure> {
                match v {
                    Some(v) if entity == TASK => named(tx, field, v).map(Some),
                    other => Ok(other.clone()),
                }
            };
            out.push(AutomationTestChange {
                entity: entity.clone(),
                task: key.clone(),
                field: field.clone(),
                before: shown(&change.before)?,
                after: shown(&change.after)?,
            });
        }
    }
    Ok(out)
}

/// A task field's value as history records it, with IDs replaced by what
/// people know them by: a status's, type's, label's or version's name, a
/// task's key.
fn named(tx: &Transaction<'_>, field: &str, value: &Value) -> Result<Value, Failure> {
    let sql = match field {
        "status" => "SELECT name FROM statuses WHERE id = ?1",
        "type" => "SELECT name FROM task_types WHERE id = ?1",
        "labels" => "SELECT name FROM labels WHERE id = ?1",
        "fix_version" => "SELECT name FROM versions WHERE id = ?1",
        "project" => "SELECT key FROM projects WHERE id = ?1",
        "parent" => {
            "SELECT p.key || '-' || t.number FROM tasks t JOIN projects p ON p.id = t.project_id
              WHERE t.id = ?1"
        }
        _ => return Ok(value.clone()),
    };
    let one = |v: &Value| -> Result<Value, Failure> {
        let Some(id) = v.as_str().and_then(|s| s.parse::<Id>().ok()) else {
            return Ok(v.clone());
        };
        let name: Option<String> = tx.query_row(sql, [b(id)], |r| r.get(0)).optional()?;
        Ok(name.map_or_else(|| v.clone(), Value::String))
    };
    match value {
        Value::Array(items) => Ok(Value::Array(
            items.iter().map(one).collect::<Result<Vec<_>, _>>()?,
        )),
        other => one(other),
    }
}

// ---------------------------------------------------------------------
// Due dates and the schedule

/// The latest moment at or before `now` that a scheduled trigger names.
fn latest_slot(t: &AutomationTrigger, zone: &crate::time::Zone, now: i64) -> Option<i64> {
    let minutes = clock_minutes(t.time.as_deref()?)?;
    let today = zone.day_of(now);
    for back in 0..8 {
        let day = today.add_days(-back).ok()?;
        if t.every == Some(ScheduleEvery::Weekly) {
            let wanted = super::recurrence::to_core_weekday(t.weekday?);
            if day.weekday() != wanted {
                continue;
            }
        }
        let at = zone.moment(day, minutes);
        if at <= now {
            return Some(at);
        }
    }
    None
}

/// The tasks `record` acts on now, for a due date or scheduled rule, at
/// most [`AUTOMATION_TASKS_PER_RUN`]: open tasks whose due date came or
/// passed and that it hasn't acted on for that due date yet, or every task
/// its condition matches. Due date tasks are remembered as acted on.
fn due_tasks(
    w: &WriteCtx<'_>,
    rule: Id,
    record: &Record,
) -> Result<Result<Vec<Id>, OpError>, Failure> {
    let tx = w.tx();
    let scope = project_key(tx, record.project_id())?;
    if record.project_id().is_some() && scope.is_none() {
        return Ok(Ok(Vec::new()));
    }
    let condition = if record.condition.trim().is_empty() {
        None
    } else {
        match compile_condition(tx, w.meta(), &record.condition, scope.clone())? {
            Ok(c) => Some(c.ids()),
            Err(e) => return Ok(Err(e)),
        }
    };
    let today = w.zone().day_of(w.now());
    let mut args: Vec<Sql> = Vec::new();
    let mut filter = vec![
        "t.deleted_at IS NULL".to_owned(),
        "p.deleted_at IS NULL".to_owned(),
        "p.state = 'active'".to_owned(),
    ];
    if let Some(p) = record.project_id() {
        args.push(Sql::Blob(b(p)));
        filter.push(format!("t.project_id = ?{}", args.len()));
    }
    let due = matches!(
        record.trigger.on,
        TriggerKind::DueDateReached | TriggerKind::Overdue
    );
    if due {
        let oldest = today
            .add_days(-DUE_GRACE_DAYS)
            .map_or_else(|_| "0000-00-00".to_owned(), |d| d.to_string());
        args.push(Sql::Text(today.to_string()));
        let cmp = if record.trigger.on == TriggerKind::Overdue {
            "<"
        } else {
            "<="
        };
        filter.push(format!(
            "t.closed_at IS NULL AND t.due_date IS NOT NULL AND t.due_date {cmp} ?{}",
            args.len()
        ));
        args.push(Sql::Text(oldest));
        filter.push(format!("t.due_date >= ?{}", args.len()));
        args.push(Sql::Blob(b(rule)));
        filter.push(format!(
            "NOT EXISTS (SELECT 1 FROM automation_fired f
                          WHERE f.rule_id = ?{} AND f.task_id = t.id AND f.mark = t.due_date)",
            args.len()
        ));
    }
    let mut text = format!(
        "SELECT t.id, t.due_date FROM tasks t JOIN projects p ON p.id = t.project_id WHERE {}",
        filter.join(" AND ")
    );
    if let Some(ids) = condition {
        // The condition's parameters are plain `?`s after ours, which SQLite
        // numbers on from the highest one before them.
        args.extend(sql_params(&ids.params));
        text.push_str(&format!(" AND t.id IN (SELECT id FROM ({}))", ids.text));
    }
    text.push_str(&format!(
        " ORDER BY t.due_date, t.id LIMIT {AUTOMATION_TASKS_PER_RUN}"
    ));
    let mut stmt = tx.prepare(&text)?;
    let found = stmt
        .query_map(params_from_iter(args.iter()), |r| {
            Ok((id_at(r, 0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if due {
        for (task, due_date) in &found {
            tx.execute(
                "INSERT OR IGNORE INTO automation_fired (rule_id, task_id, mark, fired_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![b(rule), b(*task), due_date, w.now()],
            )?;
        }
    }
    Ok(Ok(found.into_iter().map(|(t, _)| t).collect()))
}

/// `automation.run`: asks for the runs of due date rules and of scheduled
/// rules whose time has come; [`follow`] makes them once the command is
/// done. Also forgets old acknowledged notifications and what it acted on
/// for due dates long past.
fn tick(w: &mut WriteCtx<'_>) -> Result<AutomationTick, Failure> {
    let now = w.now();
    w.tx().execute(
        "DELETE FROM automation_notices WHERE acked_at IS NOT NULL AND acked_at < ?1",
        [now - NOTICE_KEEP_MS],
    )?;
    w.tx().execute(
        "DELETE FROM automation_fired WHERE fired_at < ?1",
        [now - 4 * DUE_GRACE_DAYS * 24 * HOUR_MS],
    )?;
    let mut queued = Vec::new();
    for (id, record) in rows(w.tx(), true)? {
        let on = record.trigger.on;
        match on {
            TriggerKind::DueDateReached | TriggerKind::Overdue => {}
            TriggerKind::Schedule => {
                let Some(slot) = latest_slot(&record.trigger, w.zone(), now) else {
                    continue;
                };
                let last: Option<i64> = w.tx().query_row(
                    "SELECT scheduled_at FROM automation_rules WHERE id = ?1",
                    [b(id)],
                    |r| r.get(0),
                )?;
                if slot < record.updated_at || last.is_some_and(|l| l >= slot) {
                    continue;
                }
                w.tx().execute(
                    "UPDATE automation_rules SET scheduled_at = ?2 WHERE id = ?1",
                    params![b(id), slot],
                )?;
            }
            _ => continue,
        }
        match due_tasks(w, id, &record)? {
            Ok(tasks) => queued.extend(tasks.into_iter().map(|task| Queued {
                rule: id,
                task,
                trigger: on,
            })),
            Err(e) => log(
                w.tx(),
                now,
                &Logged {
                    rule: id,
                    name: &record.name,
                    result: RunResult::Failed,
                    trigger: on,
                    task: None,
                    group: None,
                    changes: 0,
                    depth: 0,
                    error: Some(&e),
                },
            )?,
        }
    }
    let runs = u32::try_from(queued.len()).unwrap_or(u32::MAX);
    for q in queued {
        w.queue_automation(q);
    }
    Ok(AutomationTick { runs })
}

/// [`tick`], for the jobs that run it.
pub(crate) fn tick_command(w: &mut WriteCtx<'_>) -> Result<AutomationTick, Failure> {
    tick(w)
}

/// Removes the rules of project `project` and what is kept beside them,
/// when the project is purged.
pub(crate) fn purge_project(tx: &Transaction<'_>, project: Id) -> Result<(), Failure> {
    let ids: Vec<Vec<u8>> = tx
        .prepare("SELECT id FROM automation_rules WHERE project_id = ?1")?
        .query_map([b(project)], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for id in ids {
        for sql in [
            "DELETE FROM automation_runs WHERE rule_id = ?1",
            "DELETE FROM automation_fired WHERE rule_id = ?1",
            "DELETE FROM automation_notices WHERE rule_id = ?1",
            "DELETE FROM automation_rules WHERE id = ?1",
        ] {
            tx.execute(sql, [&id])?;
        }
    }
    Ok(())
}

/// The notifications rules left that nobody acknowledged: ID, task,
/// moment and text.
pub(crate) fn open_notices(tx: &Transaction<'_>) -> Result<Vec<(Id, Id, i64, String)>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT n.id, n.task_id, n.at, n.text FROM automation_notices n
           JOIN tasks t ON t.id = n.task_id JOIN projects p ON p.id = t.project_id
          WHERE n.acked_at IS NULL AND t.deleted_at IS NULL AND p.deleted_at IS NULL",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((id_at(r, 0)?, id_at(r, 1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Marks notification `id` as shown.
pub(crate) fn acknowledge_notice(tx: &Transaction<'_>, id: Id, at: i64) -> Result<(), Failure> {
    tx.execute(
        "UPDATE automation_notices SET acked_at = ?2 WHERE id = ?1",
        params![b(id), at],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_fill_what_they_name_and_leave_the_rest() {
        let p = Placeholders {
            values: vec![("key", "TM-1".to_owned()), ("title", "Fix".to_owned())],
        };
        assert_eq!(p.fill("{key}: {title} {other}"), "TM-1: Fix {other}");
    }

    #[test]
    fn set_values_are_checked_by_field() {
        assert!(check_set_value("v", "priority", &json!("high")).is_ok());
        assert!(check_set_value("v", "priority", &json!("loud")).is_err());
        assert!(check_set_value("v", "progress", &json!(50)).is_ok());
        assert!(check_set_value("v", "progress", &json!(150)).is_err());
        assert!(check_set_value("v", "due_date", &Value::Null).is_ok());
        assert!(check_set_value("v", "title", &Value::Null).is_err());
        assert!(check_set_value("v", "Points", &json!(3)).is_ok());
    }
}
