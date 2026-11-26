// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Board, type BulkResult, type CustomFieldChange, type CustomFieldList, type Task, type TaskChange, type TaskList, type TemplateApplied, type TemplateChange, type TemplateList } from "../../generated/ops";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";

// A Thursday.
const TODAY = new Date(2026, 10, 26, 12, 0);

function workspace() {
  const ws = new FakeWorkspace(seed(TODAY), () => TODAY);
  ws.call("project.create", { name: "Team", key: "RT" });
  return ws;
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

const task = (ws: FakeWorkspace, key: string) => call<TaskList>(ws, "task.get", { keys: [key] }).items[0] as Task;

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

describe("the built-in Software project template", () => {
  it("is listed first, sets up epics and stories, and can't be changed", () => {
    const ws = workspace();
    const list = call<TemplateList>(ws, "template.list", { kind: "project" });
    expect(list.items[0]).toMatchObject({ name: "Software project", built_in: "software" });
    const used = call<TemplateApplied>(ws, "template.apply", { template: "software project", name: "App", key: "APP" });
    expect(used.project?.types.map((type) => type.glyph)).toEqual(["story", "task", "bug", "epic"]);
    expect(used.project?.statuses.map((status) => status.name)).toEqual(["Backlog", "To do", "In progress", "In review", "Done", "Won't do"]);
    expect(used.project?.statuses[4]?.rules?.agents_may_not_enter).toBe(true);
    expect(call<TemplateApplied>(ws, "template.apply", { template: "Story", project: "APP" }).task?.type.glyph).toBe("story");
    const id = list.items[0]?.id ?? "";
    expect(failure(() => ws.call("template.update", { template: id, name: "Mine" })).details?.message_id).toBe("template-built-in");
    expect(failure(() => ws.call("template.delete", { template: id })).details?.message_id).toBe("template-built-in");
  });
});

describe("project templates", () => {
  it("keep the project's own custom fields and the rules that name them", () => {
    const ws = workspace();
    const { field } = call<CustomFieldChange>(ws, "field.create", { project: "RT", name: "Area", kind: "select", options: ["UI", "API"], required: true });
    if (field === undefined) throw new Error("field.create returned no field");
    ws.call("rules.update", { project: "RT", statuses: [{ status: "Done", rules: { required: [`cf:${field.id}`], stop_timer: true } }], agents: { read_only: ["title"] } });
    const saved = call<TemplateChange>(ws, "template.create", { name: "Team setup", from_project: "RT" });
    const setup = saved.template.setup as { fields: { id: string; name: string; required?: boolean }[]; statuses: { name: string; rules?: { required?: string[] } }[]; agent_rules?: { read_only: string[] } };
    expect(setup.fields).toMatchObject([{ id: field.id, name: "Area", required: true }]);
    expect(setup.statuses.find((status) => status.name === "Done")?.rules?.required).toEqual([`cf:${field.id}`]);
    expect(setup.agent_rules?.read_only).toEqual(["title"]);

    const used = call<TemplateApplied>(ws, "template.apply", { template: "Team setup", name: "Copy", key: "CP" });
    const fields = call<CustomFieldList>(ws, "field.list", { project: "CP" }).fields.filter((each) => each.project === "CP");
    expect(fields.map((each) => each.name)).toEqual(["Area"]);
    const copy = fields[0]?.id ?? "";
    expect(copy).not.toBe(field.id);
    const done = used.project?.statuses.find((status) => status.name === "Done");
    expect(done?.rules).toMatchObject({ required: [`cf:${copy}`], stop_timer: true });
  });
});

describe("closing", () => {
  it("an occurrence answers with the series as a later read shows it and names the next one", () => {
    const ws = workspace();
    ws.call("task.create", { project: "RT", title: "Bins", due_date: "2026-11-30" });
    ws.call("recurrence.set", { key: "RT-1", text: "every week on mon" });
    const closed = call<TaskChange>(ws, "task.move", { key: "RT-1", status: "Done" });
    expect(closed.task.recurrence).toEqual(task(ws, "RT-1").recurrence);
    expect(closed.next_occurrence).toBe("RT-2");
    const board = call<Board>(ws, "board.get", { project: "RT" });
    const cards = board.columns.flatMap((column) => column.cards);
    expect(cards.find((card) => card.key === "RT-2")?.recurring).toBe(true);
    expect(cards.find((card) => card.key === "RT-1")?.recurring).toBeUndefined();
  });

  it("several tasks in bulk can close their duplicates too", () => {
    const ws = workspace();
    for (const title of ["Crash", "Crash again"]) {
      ws.call("task.create", { project: "RT", title });
    }
    ws.call("relation.add", { key: "RT-2", kind: "duplicates", other: "RT-1" });
    const result = call<BulkResult>(ws, "task.bulk_update", { keys: ["RT-1"], changes: { status: "Done", close_duplicates: true }, dry_run: false, expected_count: 1 });
    expect(result.changed).toBe(1);
    expect(task(ws, "RT-2").closed_at).toBeDefined();
  });

  it("a task template keeps a due date and priority given with it", () => {
    const ws = workspace();
    ws.call("template.create", { name: "Report", project: "RT", task: { title: "Report", priority: "low" } });
    const used = call<TemplateApplied>(ws, "template.apply", { template: "Report", project: "RT", due_date: "fri", priority: "urgent" });
    expect(used.task).toMatchObject({ due_date: "2026-11-27", priority: "urgent" });
    expect(failure(() => ws.call("template.apply", { template: "Report", project: "RT", due_date: "someday" })).details?.message_id).toBe("date-invalid");
  });
});
