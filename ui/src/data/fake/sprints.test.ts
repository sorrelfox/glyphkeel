// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ListedTask, type MoveCheck, type SprintBurndown, type SprintChange, type SprintList, type Task, type TaskBlueprint, type TaskChange, type TemplateApplied, type TemplateChange } from "../../generated/ops";
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

function code(run: () => unknown): string {
  try {
    run();
  } catch (error: unknown) {
    if (error instanceof FakeError) {
      return error.error.code;
    }
    throw error;
  }
  throw new Error("expected the call to fail");
}

describe("the sample workspace's sprints", () => {
  it("lists Task Manager's sprints with their tasks and the backlog", () => {
    const ws = workspace();
    const list = call<SprintList>(ws, "sprint.list", { project: "TM", include_tasks: true });
    expect(list.items.map((s) => [s.name, s.state])).toEqual([
      ["Sprint 1", "closed"],
      ["Sprint 2", "active"],
      ["Sprint 3", "planned"],
    ]);
    const active = list.items[1];
    expect(active?.tasks?.length).toBeGreaterThan(0);
    expect(list.backlog?.every((task) => !task.closed)).toBe(true);
    const found = call<{ items: ListedTask[] }>(ws, "task.search", { query: "sprint:active" }).items;
    expect(found.map((task) => task.key).sort()).toEqual(active?.tasks?.map((task) => task.key).sort());
  });

  it("starts, completes and undoes like the engine", () => {
    const ws = workspace();
    expect(code(() => ws.call("sprint.start", { sprint: "Sprint 3", project: "TM" }))).toBe("conflict");
    const active = call<SprintList>(ws, "sprint.list", { project: "TM", include_tasks: true }).items[1];
    const open = (active?.tasks ?? []).filter((task) => !task.closed).map((task) => task.key);
    const done = call<SprintChange>(ws, "sprint.complete", { sprint: "active", project: "TM", move_open_tasks_to: "Sprint 3" });
    expect(done.sprint?.state).toBe("closed");
    expect(done.tasks_changed?.sort()).toEqual([...open].sort());
    const started = call<SprintChange>(ws, "sprint.start", { sprint: "Sprint 3", project: "TM", start_date: "2026-11-23", end_date: "2026-12-04" });
    expect(started.sprint?.start_date).toBe("2026-11-23");
    expect(started.sprint?.end_date).toBe("2026-12-04");
    ws.call("history.undo", {});
    ws.call("history.undo", {});
    expect(call<SprintList>(ws, "sprint.list", { project: "TM", state: ["active"] }).items[0]?.name).toBe("Sprint 2");
  });

  it("puts tasks in a sprint and takes them out, and the task shows it", () => {
    const ws = workspace();
    const made = call<SprintChange>(ws, "sprint.create", { project: "TM", name: "Hardening" });
    const backlog = call<SprintList>(ws, "sprint.list", { project: "TM", include_tasks: true }).backlog ?? [];
    const keys = backlog.slice(0, 2).map((task) => task.key);
    const added = call<SprintChange>(ws, "sprint.add_tasks", { sprint: made.sprint?.id, tasks: keys });
    expect(added.tasks_changed).toEqual(keys);
    expect(call<{ items: Task[] }>(ws, "task.get", { keys: [keys[0]] }).items[0]?.sprint?.name).toBe("Hardening");
    const removed = call<SprintChange>(ws, "sprint.remove_tasks", { tasks: [keys[0]] });
    expect(removed.tasks_changed).toEqual([keys[0]]);
    expect(code(() => ws.call("sprint.create", { project: "TM", name: "none" }))).toBe("invalid");
    expect(code(() => ws.call("sprint.delete", { sprint: "Sprint 2", project: "TM" }))).toBe("conflict");
  });

  it("has a burndown for the running sprint up to today", () => {
    const ws = workspace();
    const burndown = call<SprintBurndown>(ws, "sprint.burndown", { sprint: "active", project: "TM" });
    expect(burndown.days[0]?.date).toBe("2026-11-18");
    expect(burndown.days.at(-1)?.date).toBe("2026-11-23");
    expect(code(() => ws.call("sprint.burndown", { sprint: "Sprint 3", project: "TM" }))).toBe("conflict");
  });

  it("keeps a copy in its sprint, previews leaving it, and lets templates name only the active one", () => {
    const ws = workspace();
    const list = call<SprintList>(ws, "sprint.list", { project: "TM", include_tasks: true });
    const running = list.items[1]?.tasks?.[0]?.key ?? "";
    const finished = list.items[0]?.tasks?.[0]?.key ?? "";
    const check = call<MoveCheck>(ws, "task.move_check", { keys: [running], project: "PER" });
    expect(check.sprints).toEqual([{ name: "Sprint 2", tasks: expect.any(Number) as number }]);
    const copy = call<TaskChange>(ws, "task.duplicate", { key: running }).task;
    expect(copy.sprint?.name).toBe("Sprint 2");
    const old = call<TaskChange>(ws, "task.duplicate", { key: finished }).task;
    expect(old.sprint).toBeUndefined();
    const made = call<TemplateChange>(ws, "template.create", { name: "Sprint chore", from_task: running });
    expect((made.template.task as TaskBlueprint | undefined)?.sprint).toBe("active");
    const used = call<TemplateApplied>(ws, "template.apply", { template: "Sprint chore", project: "TM" });
    const task = call<{ items: Task[] }>(ws, "task.get", { keys: [used.task?.key] }).items[0];
    expect(task?.sprint?.name).toBe("Sprint 2");
    expect(code(() => ws.call("template.create", { name: "Planned", project: "TM", task: { title: "X", sprint: "Sprint 3" } }))).toBe("invalid");
  });
});
