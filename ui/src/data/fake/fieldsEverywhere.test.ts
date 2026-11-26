// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Board, type BulkResult, type MoveCheck, type Schedule, type TaskChange, type TaskList, type TaskSearchResult } from "../../generated/ops";
import { groupTaskInput, isFieldGrouping, isGrouping } from "../../screens/list/model";
import { searchInput } from "../list";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";

const TODAY = new Date(2026, 10, 23, 9, 0);

function workspace() {
  return new FakeWorkspace(seed(TODAY), () => TODAY);
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

function failure(run: () => unknown) {
  try {
    run();
  } catch (error: unknown) {
    if (error instanceof FakeError) {
      return error.error;
    }
    throw error;
  }
  throw new Error("expected the call to fail");
}

function newTask(ws: FakeWorkspace, project: string, title: string, fields?: Record<string, unknown>): string {
  return call<TaskChange>(ws, "task.create", { project, title, ...(fields ? { fields } : {}) }).task.key;
}

function valueOf(ws: FakeWorkspace, key: string, field: string): unknown {
  return call<TaskList>(ws, "task.get", { keys: [key] }).items[0]?.fields?.find((each) => each.name === field)?.value;
}

describe("custom fields across the views of the sample workspace", () => {
  it("shows the fields the layout chooses on board cards", () => {
    const ws = workspace();
    call(ws, "field.create", { project: "PER", name: "Story points", kind: "integer" });
    const key = newTask(ws, "PER", "Sized", { "Story points": 3 });
    const card = (board: Board) => board.columns.flatMap((column) => column.cards).find((each) => each.key === key);
    expect(card(call<Board>(ws, "board.get", { project: "PER" }))?.fields).toBeUndefined();
    call(ws, "layout.update", { project: "PER", card_fields: ["due", "cf.story-points"] });
    expect(card(call<Board>(ws, "board.get", { project: "PER" }))?.fields).toEqual({ "story-points": 3 });
  });

  it("sets and clears a field for many tasks in one bulk change", () => {
    const ws = workspace();
    call(ws, "field.create", { project: "PER", name: "Points", kind: "integer" });
    const keys = [newTask(ws, "PER", "One"), newTask(ws, "PER", "Two", { Points: 8 })];
    const preview = call<BulkResult>(ws, "task.bulk_update", { keys, changes: { fields: { points: 5 } } });
    expect(preview.changed).toBe(2);
    expect(preview.items[1]?.changes).toEqual([{ field: "cf.points", before: "8", after: "5" }]);
    expect(valueOf(ws, keys[0] ?? "", "Points")).toBeUndefined();
    call(ws, "task.bulk_update", { keys, changes: { fields: { points: 5 } }, dry_run: false, expected_count: 2 });
    expect(keys.map((key) => valueOf(ws, key, "Points"))).toEqual([5, 5]);
    call(ws, "task.bulk_update", { keys, changes: { fields: { points: null } }, dry_run: false, expected_count: 2 });
    expect(keys.map((key) => valueOf(ws, key, "Points"))).toEqual([undefined, undefined]);
  });

  it("groups a list by a select or yes/no field", () => {
    const ws = workspace();
    call(ws, "field.create", { project: "PER", name: "Area", kind: "select", options: ["Front end", "Back end"] });
    call(ws, "field.create", { project: "PER", name: "Approved", kind: "boolean" });
    const back = newTask(ws, "PER", "Back", { Area: "Back end", Approved: true });
    const front = newTask(ws, "PER", "Front", { Area: "Front end" });
    const scope = { project: "PER", includeSubprojects: false, query: "", groupBy: "cf.area" as const };
    expect(searchInput(scope, 0)).toMatchObject({ group_by_field: "cf.area" });
    expect(searchInput(scope, 0)).not.toHaveProperty("group_by");
    const grouped = call<TaskSearchResult>(ws, "task.search", searchInput(scope, 0));
    const values = grouped.groups?.map((group) => group.value);
    expect(values?.slice(0, 2)).toEqual(["Front end", "Back end"]);
    expect(values?.at(-1)).toBeUndefined();
    expect(grouped.items.findIndex((task) => task.key === front)).toBeLessThan(grouped.items.findIndex((task) => task.key === back));
    const approved = call<TaskSearchResult>(ws, "task.search", { query: "", project: "PER", group_by_field: "cf.approved" });
    expect(approved.groups?.map((group) => group.value)).toEqual(["yes", "no"]);
  });

  it("reads groupings by fields and fills a new task's value from its group", () => {
    expect(isGrouping("cf.area")).toBe(true);
    expect(isFieldGrouping("status")).toBe(false);
    expect(isGrouping("cf.")).toBe(false);
    const field = { id: "f", name: "Area", query_name: "cf.area", kind: "multi_select" as const, required: false, tasks: 0 };
    expect(groupTaskInput("cf.area", { value: "Docs" }, "TM", "New", field)).toEqual({ project: "TM", title: "New", fields: { area: ["Docs"] } });
    expect(groupTaskInput("cf.ok", { value: "no" }, "TM", "New", { ...field, query_name: "cf.ok", kind: "boolean" })).toEqual({ project: "TM", title: "New", fields: { ok: false } });
    expect(groupTaskInput("cf.area", {}, "TM", "New", field)).toEqual({ project: "TM", title: "New" });
    expect(groupTaskInput("cf.area", { value: "Docs" }, undefined, "New", field)).toBeUndefined();
  });

  it("checks required fields when the type changes or the task moves, as the engine does", () => {
    const ws = workspace();
    call(ws, "field.create", { project: "LAB", name: "Customer", kind: "text", required: true });
    call(ws, "field.create", { project: "LAB", name: "Team", kind: "select", options: ["Red", "Blue"], required: true, default: "Blue" });
    const key = newTask(ws, "PER", "Moving");
    const check = call<MoveCheck>(ws, "task.move_check", { keys: [key], project: "LAB" });
    expect(check.required_fields).toEqual([
      { name: "Customer", tasks: 1, has_default: false },
      { name: "Team", tasks: 1, has_default: true },
    ]);
    expect(failure(() => call(ws, "task.move", { key, project: "LAB" })).details?.message_id).toBe("field-required-after-change");
    const moved = call<TaskChange>(ws, "task.move", { key, project: "LAB", fields: { Customer: "ACME" } }).task.key;
    expect(valueOf(ws, moved, "Customer")).toBe("ACME");
    expect(valueOf(ws, moved, "Team")).toBe("Blue");

    call(ws, "field.create", { project: "PER", name: "Severity", kind: "text", required: true, types: ["Bug"] });
    const plain = newTask(ws, "PER", "Plain");
    expect(failure(() => call(ws, "task.update", { key: plain, type: "Bug" })).details?.message_id).toBe("field-required-after-change");
    call(ws, "task.update", { key: plain, type: "Bug", fields: { Severity: "High" } });
    expect(valueOf(ws, plain, "Severity")).toBe("High");
  });

  it("gives the schedule the tasks' values for the Gantt chart's tooltips", () => {
    const ws = workspace();
    call(ws, "field.create", { project: "PER", name: "Customer", kind: "text" });
    const key = call<TaskChange>(ws, "task.create", { project: "PER", title: "Dated", due_date: "2026-11-30", fields: { Customer: "ACME" } }).task.key;
    const schedule = call<Schedule>(ws, "schedule.get", { project: "PER" });
    expect(schedule.tasks.find((task) => task.key === key)?.fields).toEqual({ customer: "ACME" });
  });
});
