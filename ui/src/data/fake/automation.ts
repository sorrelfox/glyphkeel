// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Automation rules in the fake engine: the rules themselves and their run
// log, checked as the engine checks them, with its message IDs. The engine
// class runs them after each command (see `followRules` there).

import type { AutomationAction, AutomationCreateInput, AutomationRule, AutomationRun, AutomationRunsInput, AutomationTrigger, AutomationUpdateInput, OpError, TriggerKind } from "../../generated/ops";

/** A rule keeps this many runs. */
export const RUNS_KEPT = 50;
/** How many rules setting each other off one change can lead to. */
export const DEPTH_MAX = 5;

type Fail = (code: OpError["code"], message: string, extra?: { hint?: string; details?: Record<string, unknown> }) => never;

/** What happened to one task in one command. */
export interface RuleEvent {
  taskId: string;
  created: boolean;
  /** The status changed (or the task was created in one). */
  entered: boolean;
  fields: string[];
  commented: boolean;
}

/** The rules a project sees: its own and the workspace's; all without a project. */
export function listRules(rules: readonly AutomationRule[], project: string | undefined): AutomationRule[] {
  return rules.filter((rule) => project === undefined || rule.project === undefined || rule.project === project);
}

/** The rule `ref` names: its ID, or its name (in `project` when given). */
export function findRule(rules: readonly AutomationRule[], ref: string, project: string | null | undefined, fail: Fail): AutomationRule {
  const byId = rules.find((rule) => rule.id === ref);
  if (byId) {
    return byId;
  }
  const named = rules.filter((rule) => rule.name.toLowerCase() === ref.trim().toLowerCase() && (project === null || project === undefined || rule.project === project));
  const found = named.length === 1 ? named[0] : named.find((rule) => rule.project === undefined);
  if (found) {
    return found;
  }
  return fail("not_found", `There is no automation rule "${ref}".`, { details: { message_id: "automation-not-found", args: { rule: ref }, field: "rule" } });
}

/** Checks what a rule does, as the engine does. */
function check(trigger: AutomationTrigger, condition: string, actions: readonly AutomationAction[], fail: Fail): void {
  if (trigger.on === "schedule" && condition.trim() === "") {
    fail("invalid", "A scheduled rule acts on every task its condition matches, so it needs a condition, for example `is:open`.", { details: { message_id: "automation-schedule-needs-condition", path: "condition" } });
  }
  if (actions.length === 0 || actions.length > 20) {
    fail("invalid", "A rule has 1 to 20 actions.", { details: { message_id: "automation-action-count", args: { max: "20" }, path: "actions" } });
  }
  actions.forEach((action, index) => {
    const needs = (field: string, present: boolean) => {
      if (!present) {
        fail("invalid", `This kind of trigger or action needs "${field}".`, { details: { message_id: "automation-field-needed", args: { field }, path: `actions[${String(index)}].${field}` } });
      }
    };
    switch (action.kind) {
      case "set_fields":
        needs("fields", Object.keys(action.fields ?? {}).length > 0);
        break;
      case "add_label":
      case "remove_label":
        needs("label", (action.label ?? "").trim() !== "");
        break;
      case "move_to_status":
        needs("status", (action.status ?? "").trim() !== "");
        break;
      case "add_comment":
      case "notify":
        needs("text", (action.text ?? "").trim() !== "");
        break;
      default:
        needs("text", (action.text ?? "").trim() !== "" || (action.template ?? "") !== "");
    }
  });
}

function checkName(rules: readonly AutomationRule[], name: string, project: string | undefined, except: string | undefined, fail: Fail): string {
  const trimmed = name.trim();
  if (trimmed === "") {
    fail("invalid", "The name is empty.", { details: { path: "name" } });
  }
  if (rules.some((rule) => rule.id !== except && rule.project === project && rule.name.toLowerCase() === trimmed.toLowerCase())) {
    fail("conflict", `There is already an automation rule named "${trimmed}" here.`, { details: { message_id: "automation-name-taken", args: { rule: trimmed }, field: "name" } });
  }
  return trimmed;
}

export function createRule(rules: AutomationRule[], input: AutomationCreateInput, project: string | undefined, now: string, id: string, fail: Fail): AutomationRule {
  const name = checkName(rules, input.name, project, undefined, fail);
  const condition = (input.condition ?? "").trim();
  check(input.trigger, condition, input.actions, fail);
  const rule: AutomationRule = {
    id,
    name,
    ...(project === undefined ? {} : { project }),
    enabled: input.enabled ?? true,
    trigger: input.trigger,
    ...(condition === "" ? {} : { condition }),
    actions: input.actions,
    created_by: { kind: "user", name: "You" },
    created_at: now,
    updated_at: now,
  };
  rules.push(rule);
  return rule;
}

export function updateRule(rules: AutomationRule[], input: AutomationUpdateInput, now: string, fail: Fail): AutomationRule {
  const rule = findRule(rules, input.rule, input.project, fail);
  const name = input.name === null || input.name === undefined ? rule.name : checkName(rules, input.name, rule.project, rule.id, fail);
  const trigger = input.trigger ?? rule.trigger;
  const condition = (input.condition ?? rule.condition ?? "").trim();
  const actions = input.actions ?? rule.actions;
  check(trigger, condition, actions, fail);
  const next: AutomationRule = { ...rule, name, enabled: input.enabled ?? rule.enabled, trigger, condition, actions, updated_at: now };
  if (condition === "") {
    delete next.condition;
  }
  rules.splice(rules.indexOf(rule), 1, next);
  return next;
}

/** Whether a rule's trigger fits what happened to a task now in status `status`. */
export function fits(rule: AutomationRule, event: RuleEvent, status: string): boolean {
  const t = rule.trigger;
  switch (t.on) {
    case "task_created":
      return event.created;
    case "field_changed":
      return !event.created && ((t.fields ?? []).length === 0 ? event.fields.length > 0 : (t.fields ?? []).some((field) => event.fields.includes(field)));
    case "moved_to_status":
      return !event.created && event.entered && (t.status === undefined || t.status.toLowerCase() === status.toLowerCase());
    case "comment_added":
      return event.commented;
    default:
      return false;
  }
}

/** Adds a run to the log, keeping each rule's last runs; the newest first. */
export function logRun(runs: AutomationRun[], run: AutomationRun): void {
  runs.unshift(run);
  const ofRule = runs.filter((each) => each.rule === run.rule);
  for (const old of ofRule.slice(RUNS_KEPT)) {
    runs.splice(runs.indexOf(old), 1);
  }
}

/** The runs a list asks for, newest first. */
export function listRuns(runs: readonly AutomationRun[], rules: readonly AutomationRule[], input: AutomationRunsInput, ruleId: string | undefined, project: string | undefined): { items: AutomationRun[]; total: number } {
  const seen = new Set(listRules(rules, project).map((rule) => rule.id));
  const chosen = runs.filter((run) => (ruleId === undefined ? project === undefined || seen.has(run.rule) : run.rule === ruleId) && (input.problems_only !== true || run.result !== "done"));
  return { items: chosen.slice(0, input.limit ?? 50), total: chosen.length };
}

/** A run as the log keeps it. */
export function run(rule: AutomationRule, at: string, id: string, result: AutomationRun["result"], trigger: TriggerKind, task: string | undefined, extra: { changes?: number; depth?: number; error?: OpError; group?: string } = {}): AutomationRun {
  return {
    id,
    rule: rule.id,
    rule_name: rule.name,
    at,
    result,
    trigger,
    ...(task === undefined ? {} : { task }),
    changes: extra.changes ?? 0,
    depth: extra.depth ?? 0,
    ...(extra.error === undefined ? {} : { error: extra.error }),
    ...(extra.group === undefined ? {} : { group: extra.group }),
  };
}

/** Fills the placeholders of a rule's text. */
export function fill(text: string, values: Readonly<Record<string, string>>): string {
  return text.replace(/\{([a-z_]+)\}/g, (whole, name: string) => values[name] ?? whole);
}
