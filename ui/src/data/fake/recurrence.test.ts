// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Project, type RecurrenceChange, type Task, type TaskList, type TaskSearchResult, type TemplateApplied, type TemplateChange, type TemplateList } from "../../generated/ops";
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
const close = (ws: FakeWorkspace, key: string) => ws.call("task.move", { key, status: "Done" });
const openKeys = (ws: FakeWorkspace) => call<TaskSearchResult>(ws, "task.search", { query: "project:RT is:open" }).items.map((each) => each.key);

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

describe("recurring tasks", () => {
  it("make the next occurrence with the series' details when one closes, and Undo takes it back", () => {
    const ws = workspace();
    ws.call("label.create", { project: "RT", name: "chores" });
    ws.call("task.create", { project: "RT", title: "Water the plants", labels: ["chores"], description: "- [ ] kitchen\n- [ ] office\n", estimate: "15m" });
    ws.call("task.create", { project: "RT", title: "Fill the can", parent: "RT-1" });
    const set = call<RecurrenceChange>(ws, "recurrence.set", { key: "RT-1", text: "every monday and thursday" });
    expect(set.recurrence?.summary).toBe("Every week on Monday and Thursday");
    expect(set.due_date).toBe("2026-11-26");
    expect(set.recurrence?.next_date).toBe("2026-11-30");

    ws.call("task.update", { key: "RT-1", description: "- [x] kitchen\n- [ ] office\n", title: "Water the plants (balcony too)" });
    close(ws, "RT-1");
    expect(task(ws, "RT-1").recurrence).toBeUndefined();
    const next = task(ws, "RT-3");
    expect(next.title).toBe("Water the plants");
    expect(next.due_date).toBe("2026-11-30");
    expect(next.description).toBe("- [ ] kitchen\n- [ ] office\n");
    expect(next.labels[0]?.name).toBe("chores");
    expect(next.estimate_minutes).toBe(15);
    expect(next.subtask_count.total).toBe(1);
    expect(next.recurrence?.made).toBe(2);
    expect(next.recurrence?.next_date).toBe("2026-12-03");
    const listed = call<TaskSearchResult>(ws, "task.search", { query: "project:RT is:open" }).items.find((each) => each.key === "RT-3");
    expect(listed?.recurring).toBe(true);

    ws.call("history.undo", {});
    expect(task(ws, "RT-1").closed_at).toBeUndefined();
    expect(task(ws, "RT-1").recurrence?.made).toBe(1);
    expect(openKeys(ws)).not.toContain("RT-3");
  });

  it("keep the series' details until asked to take the task's", () => {
    const ws = workspace();
    ws.call("task.create", { project: "RT", title: "Report", due_date: "2026-11-27" });
    ws.call("recurrence.set", { key: "RT-1", text: "monthly on the last fri" });
    ws.call("task.update", { key: "RT-1", title: "Monthly report" });
    expect(task(ws, "RT-1").recurrence?.title).toBe("Report");
    const changed = call<RecurrenceChange>(ws, "recurrence.set", { key: "RT-1", update_details: true });
    expect(changed.recurrence?.title).toBe("Monthly report");
    expect(changed.recurrence?.summary).toBe("Every month on the last Friday");
    close(ws, "RT-1");
    expect(task(ws, "RT-2").title).toBe("Monthly report");
    expect(task(ws, "RT-2").due_date).toBe("2026-12-25");
  });

  it("count from completion and pass over dates already past", () => {
    const ws = workspace();
    ws.call("task.create", { project: "RT", title: "Descale", due_date: "2026-11-20" });
    ws.call("recurrence.set", { key: "RT-1", every: "day", interval: 3, from: "completion" });
    close(ws, "RT-1");
    expect(task(ws, "RT-2").due_date).toBe("2026-11-29");

    ws.call("task.create", { project: "RT", title: "Stand-up", due_date: "2026-11-02" });
    ws.call("recurrence.set", { key: "RT-3", text: "every monday, 5 times" });
    close(ws, "RT-3");
    expect(task(ws, "RT-4").due_date).toBe("2026-11-30");
    expect(task(ws, "RT-4").recurrence?.made).toBe(5);
    close(ws, "RT-4");
    expect(openKeys(ws)).toEqual(["RT-2"]);
  });

  it("skip: the dates move, and the last one can't be skipped", () => {
    const ws = workspace();
    ws.call("task.create", { project: "RT", title: "Backup", start_date: "2026-11-28", due_date: "2026-11-30" });
    ws.call("recurrence.set", { key: "RT-1", text: "weekly on mon, 2 times" });
    expect(call<RecurrenceChange>(ws, "recurrence.skip", { key: "RT-1" }).due_date).toBe("2026-12-07");
    expect(task(ws, "RT-1").start_date).toBe("2026-12-05");
    expect(failure(() => ws.call("recurrence.skip", { key: "RT-1" })).code).toBe("conflict");
    ws.call("history.undo", {});
    expect(task(ws, "RT-1").due_date).toBe("2026-11-30");
  });

  it("stop: the task stays and nothing more is made", () => {
    const ws = workspace();
    ws.call("task.create", { project: "RT", title: "Water", due_date: "2026-11-26" });
    ws.call("recurrence.set", { key: "RT-1", text: "daily" });
    expect(call<RecurrenceChange>(ws, "recurrence.clear", { key: "RT-1" }).recurrence).toBeUndefined();
    close(ws, "RT-1");
    expect(openKeys(ws)).toEqual([]);
    expect(failure(() => ws.call("recurrence.set", { key: "RT-1", text: "daily" })).code).toBe("invalid");
  });

  it("preview a rule and say what can't be read", () => {
    const ws = workspace();
    expect(call<{ dates: string[] }>(ws, "recurrence.preview", { text: "the 31st of each month", starts: "2027-01-01", limit: 3 }).dates).toEqual(["2027-01-31", "2027-02-28", "2027-03-31"]);
    const error = failure(() => ws.call("recurrence.preview", { text: "now and then" }));
    expect(error.details?.message_id).toBe("value-recurrence-text");
  });
});

describe("a type's default due date", () => {
  it("is given to new tasks without one, and can be taken away", () => {
    const ws = workspace();
    ws.call("type.update", { project: "RT", type: "Bug", due_offset_days: 3 });
    const bug = call<Project>(ws, "project.get", { project: "RT" }).types.find((type) => type.name === "Bug");
    expect(bug?.due_offset_days).toBe(3);
    expect(call<{ task: Task }>(ws, "task.create", { project: "RT", title: "Crash", type: "Bug" }).task.due_date).toBe("2026-11-29");
    expect(call<{ task: Task }>(ws, "task.create", { project: "RT", title: "Given", type: "Bug", due_date: "2026-12-24" }).task.due_date).toBe("2026-12-24");
    expect(call<{ task: Task }>(ws, "task.create", { project: "RT", title: "Plain" }).task.due_date).toBeUndefined();
    ws.call("type.update", { project: "RT", type: "Bug", clear_due_offset: true });
    expect(call<{ task: Task }>(ws, "task.create", { project: "RT", title: "Later", type: "Bug" }).task.due_date).toBeUndefined();
  });
});

describe("templates", () => {
  it("keep a task's names, dates, subtasks and unticked checklist", () => {
    const ws = workspace();
    ws.call("label.create", { project: "RT", name: "crash" });
    ws.call("task.create", { project: "RT", title: "Bug report", type: "Bug", labels: ["crash"], priority: "high", description: "## Steps\n\n- [x] reproduce\n- [ ] fix\n", due_date: "2026-12-01" });
    ws.call("task.create", { project: "RT", title: "Write a test", parent: "RT-1" });
    ws.call("task.create", { project: "RT", title: "Check CI", parent: "RT-2" });
    const made = call<TemplateChange>(ws, "template.create", { name: "Bug", from_task: "RT-1", description: "For crashes" });
    const bp = made.template.task as { due_in_days: number; description: string; subtasks: { under?: number }[] };
    expect(bp.due_in_days).toBe(5);
    expect(bp.description).toBe("## Steps\n\n- [ ] reproduce\n- [ ] fix\n");
    expect(bp.subtasks[1]?.under).toBe(0);
    expect(failure(() => ws.call("template.create", { name: "bug", from_task: "RT-1" })).code).toBe("conflict");

    const used = call<TemplateApplied>(ws, "template.apply", { template: "Bug", project: "RT", title: "Crash on save", date: "2027-01-10" });
    expect(used.tasks).toBe(3);
    expect(used.task?.title).toBe("Crash on save");
    expect(used.task?.type.name).toBe("Bug");
    expect(used.task?.priority).toBe("high");
    expect(used.task?.due_date).toBe("2027-01-15");
    expect(used.task?.subtask_count.total).toBe(1);

    ws.call("project.create", { name: "Other", key: "OT" });
    const elsewhere = call<TemplateApplied>(ws, "template.apply", { template: made.template.id, project: "OT" });
    expect(elsewhere.left_out).toEqual(["crash"]);
    expect(elsewhere.task?.key).toBe("OT-1");

    expect(call<TemplateList>(ws, "template.list", { project: "RT" }).items[0]?.items).toBe(3);
    ws.call("template.delete", { template: "Bug", project: "RT" });
    expect(call<TemplateList>(ws, "template.list", {}).total).toBe(0);
    ws.call("history.undo", {});
    expect(call<TemplateList>(ws, "template.list", {}).total).toBe(1);
  });

  it("set up a project from a template, and save a project as one", () => {
    const ws = workspace();
    ws.call("template.create", {
      name: "Software project",
      setup: {
        description: "Code and releases",
        statuses: [
          { name: "Ideas", category: "todo", hue: "grey" },
          { name: "Building", category: "in_progress", hue: "blue", wip_limit: 3 },
          { name: "Shipped", category: "done", hue: "green" },
        ],
        types: [
          { name: "Story", glyph: "feature", hue: "blue", is_default: true },
          { name: "Bug", glyph: "bug", hue: "red", due_offset_days: 7 },
        ],
        labels: [{ name: "frontend", hue: "purple" }],
        wiki: [{ title: "Overview", body: "# Overview" }, { title: "Releases", parent: "Overview" }],
        tasks: [{ title: "Set up CI", due_in_days: 2, labels: ["frontend"], subtasks: [{ title: "Pick a runner" }] }],
        task_templates: [{ name: "Bug report", task: { title: "Bug", type: "Bug" } }],
      },
    });
    const used = call<TemplateApplied>(ws, "template.apply", { template: "Software project", name: "App", key: "APP" });
    expect(used.project?.statuses.map((status) => status.name)).toEqual(["Ideas", "Building", "Shipped"]);
    expect(used.project?.types[1]?.due_offset_days).toBe(7);
    expect(used.tasks).toBe(2);
    expect(task(ws, "APP-1").status.name).toBe("Ideas");
    expect(task(ws, "APP-1").due_date).toBe("2026-11-28");
    expect(call<TemplateApplied>(ws, "template.apply", { template: "Bug report", project: "APP" }).task?.due_date).toBe("2026-12-03");

    const again = call<TemplateChange>(ws, "template.create", { name: "Copy", from_project: "APP", include_tasks: true });
    const setup = again.template.setup as { statuses: { wip_limit?: number }[]; wiki: { parent?: string }[]; task_templates: { name: string }[]; tasks: unknown[] };
    expect(setup.statuses[1]?.wip_limit).toBe(3);
    expect(setup.wiki[1]?.parent).toBe("Overview");
    expect(setup.task_templates[0]?.name).toBe("Bug report");
    expect(setup.tasks).toHaveLength(2);
  });
});
