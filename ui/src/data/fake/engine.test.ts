// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Board, type Counts, type Project, type ProjectList, type Task, type TaskChange } from "../catalog";
import { moveOnBoard } from "../board";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";
import { fakeTransport } from "./transport";

const TODAY = new Date(2026, 10, 23, 9, 0);

function workspace() {
  return new FakeWorkspace(seed(TODAY), () => TODAY);
}

// Answers come back untyped; the tests read them as the catalog types them.
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

describe("the sample workspace", () => {
  it("has data set S: three top-level projects, one with two sub-projects, an archived one and 200 tasks", () => {
    const ws = workspace();
    const list = call<ProjectList>(ws, "project.list");
    expect(list.items.map((p) => [p.key, p.parent, p.state])).toEqual([
      ["TM", null, "active"],
      ["CLI", "TM", "active"],
      ["DOC", "TM", "active"],
      ["PER", null, "active"],
      ["LAB", null, "active"],
      ["BLOG", null, "archived"],
    ]);
    const counts = call<Counts>(ws, "workspace.counts");
    const boards = ["TM", "CLI", "DOC", "PER", "LAB"].map((key) => call<Board>(ws, "board.get", { project: key }).cards.length);
    expect(boards.reduce((a, b) => a + b, 0)).toBe(200);
    expect(counts.all).toBeGreaterThan(0);
  });

  it("is the same on every start for the same day", () => {
    expect(JSON.stringify(seed(TODAY))).toBe(JSON.stringify(seed(TODAY)));
  });

  it("puts Task Manager's In progress over its WIP limit", () => {
    const board = call<Board>(workspace(), "board.get", { project: "TM" });
    const column = board.columns.find((c) => c.name === "In progress");
    const count = board.cards.filter((card) => board.column_of[card.key] === column?.id).length;
    expect(column?.wip_limit).toBe(3);
    expect(count).toBeGreaterThan(3);
  });
});

describe("tasks", () => {
  it("creates a task from a title alone, with the next key, in the first to-do status", () => {
    const ws = workspace();
    const project = call<Project>(ws, "project.get", { project: "TM" });
    const change = call<TaskChange>(ws, "task.create", { project: "TM", title: "  New one  " });
    expect(change.task.key).toBe("TM-81");
    expect(change.task.title).toBe("New one");
    expect(change.task.status.id).toBe(project.statuses[0]?.id);
    expect(change.task.type.name).toBe("Task");
    expect(change.task.priority).toBe("none");
    expect(change.group).toMatch(/^g-/);
    expect(failure(() => ws.call("task.create", { project: "TM", title: "   " })).code).toBe("invalid");
  });

  it("closes a task entering a done status and reopens it leaving", () => {
    const ws = workspace();
    const project = call<Project>(ws, "project.get", { project: "PER" });
    const done = project.statuses.find((s) => s.category === "done");
    const todo = project.statuses.find((s) => s.name === "To do");
    const key = call<TaskChange>(ws, "task.create", { project: "PER", title: "Close me" }).task.key;
    expect(call<TaskChange>(ws, "task.move", { key, status: done?.id }).task.closed_at).not.toBeNull();
    expect(call<TaskChange>(ws, "task.move", { key, status: todo?.id }).task.closed_at).toBeNull();
  });

  it("orders a moved task before or after its neighbour", () => {
    const ws = workspace();
    const board = call<Board>(ws, "board.get", { project: "PER" });
    const column = board.columns[1];
    const inColumn = board.cards.filter((c) => board.column_of[c.key] === column?.id).map((c) => c.key);
    const [first, second, third] = inColumn;
    call(ws, "task.move", { key: third, position: { before: first } });
    const after = call<Board>(ws, "board.get", { project: "PER" });
    expect(after.cards.filter((c) => after.column_of[c.key] === column?.id).map((c) => c.key).slice(0, 3)).toEqual([third, first, second]);
  });

  it("rejects a start after the due date and a stale version", () => {
    const ws = workspace();
    const key = call<TaskChange>(ws, "task.create", { project: "PER", title: "Dates" }).task.key;
    expect(failure(() => ws.call("task.update", { key, patch: { start_date: "2026-12-02", due_date: "2026-12-01" } })).code).toBe("invalid");
    expect(failure(() => ws.call("task.update", { key, expected_version: 99, patch: { title: "x" } })).code).toBe("conflict");
    const task = call<TaskChange>(ws, "task.update", { key, patch: { labels: { add: ["home"] }, estimate_min: 90 } }).task;
    expect(task.labels.map((l) => l.name)).toEqual(["home"]);
    expect(task.estimate_min).toBe(90);
  });

  it("counts checklist items from the description", () => {
    const ws = workspace();
    const key = call<TaskChange>(ws, "task.create", { project: "PER", title: "List" }).task.key;
    const task = call<TaskChange>(ws, "task.update", { key, patch: { description: "- [x] one\n- [ ] two\n* [X] three" } }).task;
    expect(task.checklist).toEqual({ done: 2, total: 3 });
  });
});

describe("projects", () => {
  it("checks keys, copies the parent's settings into a sub-project and limits the depth", () => {
    const ws = workspace();
    expect(failure(() => ws.call("project.create", { name: "Bad", key: "1A" })).code).toBe("invalid");
    expect(failure(() => ws.call("project.create", { name: "Taken", key: "TM" })).code).toBe("conflict");
    const sub = call<{ project: Project }>(ws, "project.create", { name: "Sub", key: "SUB", parent: "CLI" }).project;
    const cli = call<Project>(ws, "project.get", { project: "CLI" });
    expect(sub.statuses.map((s) => s.name)).toEqual(cli.statuses.map((s) => s.name));
    expect(sub.statuses[0]?.id).not.toBe(cli.statuses[0]?.id);
    expect(failure(() => ws.call("project.create", { name: "Deep", key: "DEEP", parent: "SUB" })).code).toBe("limit");
    expect(failure(() => ws.call("project.move", { project: "TM", parent: "CLI" })).code).toBe("invalid");
  });

  it("refuses changes in an archived project", () => {
    const ws = workspace();
    expect(failure(() => ws.call("task.create", { project: "BLOG", title: "No" })).code).toBe("forbidden");
    call(ws, "project.archive", { project: "TM" });
    expect(call<Project>(ws, "project.get", { project: "CLI" }).state).toBe("archived");
  });

  it("moves a deleted status's tasks to the chosen one", () => {
    const ws = workspace();
    const project = call<Project>(ws, "project.get", { project: "LAB" });
    const [backlog, todo] = project.statuses;
    const before = call<Board>(ws, "board.get", { project: "LAB" });
    const moving = before.cards.filter((c) => c.status === backlog?.id).length;
    const staying = before.cards.filter((c) => c.status === todo?.id).length;
    call(ws, "status.delete", { project: "LAB", status: backlog?.id, move_tasks_to: todo?.id });
    const after = call<Board>(ws, "board.get", { project: "LAB" });
    expect(after.columns.map((c) => c.name)).not.toContain("Backlog");
    expect(after.cards.filter((c) => c.status === todo?.id).length).toBe(moving + staying);
  });
});

describe("the board with sub-projects", () => {
  it("matches columns by status name and adds a sub-project's own status at the end", () => {
    const ws = workspace();
    const board = call<Board>(ws, "board.get", { project: "TM", include_subprojects: true });
    expect(board.columns.map((c) => c.name)).toEqual(["Backlog", "To do", "In progress", "Review", "Done", "Waiting"]);
    const waiting = board.columns.at(-1);
    expect(Object.keys(waiting?.statuses ?? {})).toEqual(["CLI"]);
    expect(Object.keys(board.columns[0]?.statuses ?? {})).toEqual(["TM", "CLI", "DOC"]);
    expect(board.cards.some((c) => c.project === "DOC")).toBe(true);
    expect(failure(() => ws.call("task.move", { key: board.cards.find((c) => c.project === "TM")?.key, status: "Waiting" })).code).toBe("invalid");
  });
});

describe("Undo", () => {
  it("takes back the last change and redoes it", () => {
    const ws = workspace();
    const key = call<TaskChange>(ws, "task.create", { project: "PER", title: "Undo me" }).task.key;
    call(ws, "history.undo");
    expect(failure(() => ws.call("task.get", { key })).code).toBe("not_found");
    call(ws, "history.redo");
    expect(call<Task>(ws, "task.get", { key }).title).toBe("Undo me");
  });
});

describe("the transport", () => {
  it("answers with plain data, rejects with the engine's error objects and announces changes", async () => {
    const transport = fakeTransport({ today: () => TODAY });
    const events: unknown[] = [];
    await transport.subscribe((event) => events.push(event));
    await expect(transport.call("task.get", { key: "NOPE-1" })).rejects.toMatchObject({ code: "not_found" });
    await transport.call("task.create", { project: "PER", title: "Hello" });
    await new Promise((resolve) => setTimeout(resolve, 5));
    expect(events).toMatchObject([{ changes: [{ entity: "task", project: "PER" }] }]);
  });
});

describe("moveOnBoard", () => {
  it("shows a moved card in its new column and place at once", () => {
    const ws = workspace();
    const board = call<Board>(ws, "board.get", { project: "PER" });
    const [todo, doing] = [board.columns[1], board.columns[2]];
    const card = board.cards.find((c) => board.column_of[c.key] === todo?.id);
    const anchor = board.cards.find((c) => board.column_of[c.key] === doing?.id);
    if (!card || !anchor || !doing) {
      throw new Error("sample data changed");
    }
    const moved = moveOnBoard(board, { key: card.key, status: doing.id, column: doing.id, position: { before: anchor.key } });
    const keys = moved.cards.filter((c) => moved.column_of[c.key] === doing.id).map((c) => c.key);
    expect(keys[0]).toBe(card.key);
    expect(keys[1]).toBe(anchor.key);
    expect(moved.cards.find((c) => c.key === card.key)?.status).toBe(doing.id);
  });
});
