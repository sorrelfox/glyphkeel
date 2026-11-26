// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Status rules and rules for agents.
//!
//! A status's rules apply to every task that enters it, whoever moves it:
//! fields set or cleared on the way in, the timer stopped, a claim
//! released, fields that must have a value first, and fields that can't
//! change while the task is in it. A project's rules for agents name the
//! task fields agents may not change; each status says whether agents may
//! move tasks into it. The engine enforces them for the app, the command
//! line and agents alike.
//!
//! Rules name task fields by their API name (`estimate`, `start_date`, …)
//! or a custom field as `cf:` followed by its ID.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::project::ProjectChange;

/// The prefix of a custom field in a rule: `cf:` and the field's ID.
pub const CUSTOM_FIELD_PREFIX: &str = "cf:";

/// The task fields a status may require before a task enters it.
pub const REQUIRABLE_FIELDS: &[&str] = &[
    "description",
    "priority",
    "start_date",
    "due_date",
    "estimate",
    "labels",
    "parent",
    "fix_version",
];

/// The task fields a status may make read-only, and agents may be kept
/// from changing.
pub const LOCKABLE_FIELDS: &[&str] = &[
    "title",
    "description",
    "type",
    "priority",
    "start_date",
    "due_date",
    "estimate",
    "progress",
    "labels",
    "parent",
    "fix_version",
];

/// The task fields a status may set when a task enters it.
pub const SETTABLE_FIELDS: &[&str] = &["start_date", "due_date", "progress", "priority"];

/// The task fields a status may clear when a task enters it.
pub const CLEARABLE_FIELDS: &[&str] = &[
    "start_date",
    "due_date",
    "estimate",
    "progress",
    "fix_version",
];

/// At most a list of fields in a rule holds.
pub const RULE_FIELDS_MAX: usize = 50;

fn is_false(b: &bool) -> bool {
    !*b
}

/// One field a status sets when a task enters it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleSet {
    /// The field: `start_date`, `due_date`, `progress`, `priority`, or a
    /// custom field as `cf:<id>`.
    pub field: String,
    /// The value as text: `today` for a date, 0 to 100 for progress, a
    /// priority's name (`high`), or a custom field's value as it is
    /// written (`today` for a date field, one option of a select field).
    pub value: String,
    /// Only when the field has no value yet (a start date already set is
    /// kept).
    #[serde(default, skip_serializing_if = "is_false")]
    pub only_if_empty: bool,
}

/// The rules of one status. Everything left out is no rule.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusRules {
    /// Fields set when a task enters the status.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set: Vec<RuleSet>,
    /// Fields cleared when a task enters the status: `start_date`,
    /// `due_date`, `estimate`, `progress`, `fix_version`, or a custom field
    /// that isn't required.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<String>,
    /// Stop the timer when its task enters the status; the time is logged.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stop_timer: bool,
    /// Release an agent's claim when the task enters the status.
    #[serde(default, skip_serializing_if = "is_false")]
    pub release_claim: bool,
    /// Fields that must have a value before a task enters the status:
    /// `description`, `priority`, `start_date`, `due_date`, `estimate`,
    /// `labels`, `parent`, `fix_version`, or a custom field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
    /// Fields that can't change while a task is in the status: `title`,
    /// `description`, `type`, `priority`, `start_date`, `due_date`,
    /// `estimate`, `progress`, `labels`, `parent`, `fix_version`, or a
    /// custom field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_only: Vec<String>,
    /// Agents may not move tasks into the status, nor create tasks in it
    /// (for example Done, so that a person reviews first).
    #[serde(default, skip_serializing_if = "is_false")]
    pub agents_may_not_enter: bool,
}

impl StatusRules {
    /// Whether the status has no rule at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// A project's rules for agents. Everything left out is no rule.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentRules {
    /// Task fields agents may not change in this project: `title`,
    /// `description`, `type`, `priority`, `start_date`, `due_date`,
    /// `estimate`, `progress`, `labels`, `parent`, `fix_version`, or a
    /// custom field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_only: Vec<String>,
}

impl AgentRules {
    /// Whether there is no rule at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// New rules for one status inside [`RulesUpdateInput`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusRulesChange {
    /// The status: its name or ID.
    pub status: String,
    /// Its rules, replacing the ones it had; empty removes them all.
    pub rules: StatusRules,
}

/// New rules for a project's statuses and its agents. What is left out
/// keeps its rules; everything is one change with one history group.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RulesUpdateInput {
    /// The project: its key (current or old) or ID.
    pub project: String,
    /// Statuses whose rules to replace.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<StatusRulesChange>,
    /// The rules for agents, replacing the ones the project had.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<AgentRules>,
}

op!(
    /// Change a project's status rules and rules for agents.
    RulesUpdate,
    "rules.update",
    RulesUpdateInput,
    ProjectChange
);

pub(crate) const RULES_UPDATE: OpDescriptor = OpDescriptor::of::<RulesUpdate>(Meta {
    summary: "Change a project's status rules (fields set, cleared or required when a task enters a status, fields read-only in it, the timer and claims) and its rules for agents (statuses they may not move tasks into, fields they may not change).",
    kind: OpKind::Command,
    safety: Safety::App,
    exposure: Exposure::App {
        reason: "It sets what agents may do; an agent that could call it would lift the rules the person set for agents.",
    },
    cli: Some("project rules <project>"),
    input_name: "RulesUpdateInput",
    output_name: "ProjectChange",
});
