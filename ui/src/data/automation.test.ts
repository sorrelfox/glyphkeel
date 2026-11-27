// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { actionsSummary, cleanAction, cleanTrigger, newAction, newDraft, setField, triggerSummary, weekdayDate, withTriggerKind, type Naming } from "./automation";

const t = Object.assign((id: string, vars?: Record<string, unknown>) => (vars === undefined ? id : `${id} ${JSON.stringify(vars)}`), { has: () => true });
const naming: Naming = {
  list: (items) => items.join(" + "),
  weekday: (day) => day.toUpperCase(),
};

describe("the rule editor's helpers", () => {
  it("start a rule with a trigger and one action", () => {
    const draft = newDraft("TM");
    expect(draft.project).toBe("TM");
    expect(draft.trigger).toEqual({ on: "task_created" });
    expect(draft.actions).toEqual([{ kind: "add_label" }]);
  });

  it("keep what fits when the trigger changes kind", () => {
    expect(withTriggerKind({ on: "task_created" }, "schedule")).toEqual({ on: "schedule", every: "daily", time: "09:00" });
    expect(withTriggerKind({ on: "field_changed", fields: ["priority"] }, "field_changed")).toEqual({ on: "field_changed", fields: ["priority"] });
    expect(withTriggerKind({ on: "moved_to_status", status: "Done" }, "comment_added")).toEqual({ on: "comment_added" });
  });

  it("send a weekly schedule with its day and a daily one without", () => {
    expect(cleanTrigger({ on: "schedule", every: "daily", time: "08:00", weekday: "fri" })).toEqual({ on: "schedule", every: "daily", time: "08:00" });
    expect(cleanTrigger({ on: "schedule", every: "weekly", time: "08:00" })).toEqual({ on: "schedule", every: "weekly", time: "08:00", weekday: "mon" });
  });

  it("send actions without empty texts, and a link only for a linked task", () => {
    expect(cleanAction({ kind: "add_comment", text: "  hi  ", template: "" })).toEqual({ kind: "add_comment", text: "hi" });
    expect(cleanAction({ kind: "create_subtask", text: "x", link: "blocks" })).toEqual({ kind: "create_subtask", text: "x" });
    expect(cleanAction(newAction("create_linked_task"))).toEqual({ kind: "create_linked_task", link: "relates" });
  });

  it("read the one field a set action sets", () => {
    expect(setField({ kind: "set_fields", fields: { due_date: "+3d" } })).toEqual({ field: "due_date", value: "+3d" });
    expect(setField({ kind: "set_fields" })).toEqual({ field: "priority", value: "high" });
  });

  it("sum a rule up in the person's words", () => {
    expect(triggerSummary(t, { on: "task_created" }, naming)).toBe("automation-trigger-task_created");
    expect(triggerSummary(t, { on: "field_changed", fields: ["priority", "due_date"] }, naming)).toBe('automation-when-fields {"fields":"automation-field-priority + automation-field-due_date","count":2}');
    expect(triggerSummary(t, { on: "schedule", every: "weekly", weekday: "fri", time: "08:00" }, naming)).toBe('automation-when-weekly {"day":"FRI","time":"08:00"}');
    expect(actionsSummary(t, [{ kind: "add_label" }, { kind: "notify" }], naming)).toBe("automation-action-add_label + automation-action-notify");
  });

  it("name weekdays from a week that starts on a Monday", () => {
    expect(weekdayDate("mon").getUTCDay()).toBe(1);
    expect(weekdayDate("sun").getUTCDay()).toBe(0);
  });
});
