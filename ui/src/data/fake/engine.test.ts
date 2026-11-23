// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ProjectList, type TaskList } from "../../generated/ops";
import { cardsOf, countsOf, moveOnBoard } from "../board";
import { type Board, type Project, type TaskChange } from "../catalog";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";
import { fakeTransport } from "./transport";

const TODAY = new Date(2026, 10, 23, 9, 0);

function workspace() {
  return new FakeWorkspace(seed(TODAY), () => TODAY);
}

// Answers come back untyped; the tests read them as the generated client types them.
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

const cards = (board: Board) => [...cardsOf(board).values()];
const keysIn = (board: Board, column: string | undefined) => board.columns.find((c) => c.id === column)?.cards.map((c) => c.key) ?? [];

describe("the sample workspace", () => {
  it("has data set S: three top-level projects, one with two sub-projects, an archived one and 200 tasks", () => {
    const ws = workspace();
    const list = call<ProjectList>(ws, "project.list", { include_archived: true });
    expect(list.items.map((p) => [p.key, p.parent ?? null, p.state, p.depth])).toEqual([
      ["TM", null, "active", 0],
      ["CLI", "TM", "active", 1],
      ["DOC", "TM", "active", 1],
      ["PER", null, "active", 0],
      ["LAB", null, "active", 0],
      ["BLOG", null, "archived", 0],
    ]);
    expect(call<ProjectList>(ws, "project.list").items.map((p) => p.key)).not.toContain("BLOG");
    const counts = countsOf(list);
    const tm = list.items[0];
    expect(tm?.open_tasks_with_subprojects).toBe((counts.own.TM ?? 0) + (counts.own.CLI ?? 0) + (counts.own.DOC ?? 0));
    const boards = ["TM", "CLI", "DOC", "PER", "LAB"].map((key) => cards(call<Board>(ws, "board.get", { project: key, show_older: true })).length);
    expect(boards.reduce((a, b) => a + b, 0)).toBe(200);
    expect(counts.all).toBeGreaterThan(0);
  });

  it("is the same on every start for the same day", () => {
    expect(JSON.stringify(seed(TODAY))).toBe(JSON.stringify(seed(TODAY)));
  });

  it("puts Task Manager's In progress over its WIP limit", () => {
    const board = call<Board>(workspace(), "board.get", { project: "TM" });
    const column = board.columns.find((c) => c.name === "In progress");
    expect(column?.wip_limit).toBe(3);
    expect(column?.count).toBeGreaterThan(3);
    expect(column?.over_limit).toBe(true);
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
    expect(call<TaskChange>(ws, "task.move", { key, status: done?.id }).task.closed_at).toBeDefined();
    expect(call<TaskChange>(ws, "task.move", { key, status: todo?.id }).task.closed_at).toBeUndefined();
  });

  it("orders a moved task before or after its neighbour", () => {
    const ws = workspace();
    const board = call<Board>(ws, "board.get", { project: "PER" });
    const column = board.columns[1]?.id;
    const [first, second, third] = keysIn(board, column);
    call(ws, "task.move", { key: third, before: first });
    const after = call<Board>(ws, "board.get", { project: "PER" });
    expect(keysIn(after, column).slice(0, 3)).toEqual([third, first, second]);
    expect(failure(() => ws.call("task.move", { key: third, before: first, at: "top" })).code).toBe("invalid");
  });

  it("rejects a start after the due date and a stale version", () => {
    const ws = workspace();
    const key = call<TaskChange>(ws, "task.create", { project: "PER", title: "Dates" }).task.key;
    expect(failure(() => ws.call("task.update", { key, start_date: "2026-12-02", due_date: "2026-12-01" })).code).toBe("invalid");
    expect(failure(() => ws.call("task.update", { key, expected_version: 99, title: "x" })).code).toBe("conflict");
    const task = call<TaskChange>(ws, "task.update", { key, add_labels: ["home"], estimate: "1h 30m", due_date: "2026-12-01" }).task;
    expect(task.labels.map((l) => l.name)).toEqual(["home"]);
    expect(task.estimate_minutes).toBe(90);
    const cleared = call<TaskChange>(ws, "task.update", { key, clear: ["estimate", "due_date", "labels"] }).task;
    expect([cleared.estimate_minutes, cleared.due_date, cleared.labels]).toEqual([undefined, undefined, []]);
  });

  it("counts checklist items from the description", () => {
    const ws = workspace();
    const key = call<TaskChange>(ws, "task.create", { project: "PER", title: "List" }).task.key;
    const task = call<TaskChange>(ws, "task.update", { key, description: "- [x] one\n- [ ] two\n* [X] three" }).task;
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
    const moving = keysIn(before, backlog?.id).length;
    const staying = keysIn(before, todo?.id).length;
    expect(failure(() => ws.call("status.delete", { project: "LAB", status: backlog?.id })).code).toBe("invalid");
    call(ws, "status.delete", { project: "LAB", status: backlog?.id, move_tasks_to: todo?.id });
    const after = call<Board>(ws, "board.get", { project: "LAB" });
    expect(after.columns.map((c) => c.name)).not.toContain("Backlog");
    expect(keysIn(after, todo?.id).length).toBe(moving + staying);
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
    expect(cards(board).some((c) => c.project === "DOC")).toBe(true);
    expect(failure(() => ws.call("task.move", { key: cards(board).find((c) => c.project === "TM")?.key, status: "Waiting" })).code).toBe("invalid");
  });
});

describe("Undo", () => {
  it("takes back the last change and redoes it", () => {
    const ws = workspace();
    const key = call<TaskChange>(ws, "task.create", { project: "PER", title: "Undo me" }).task.key;
    call(ws, "history.undo");
    expect(failure(() => ws.call("task.get", { keys: [key] })).code).toBe("not_found");
    call(ws, "history.redo");
    expect(call<TaskList>(ws, "task.get", { keys: [key] }).items[0]?.title).toBe("Undo me");
  });
});

describe("the transport", () => {
  it("answers with plain data, rejects with the engine's error objects and announces changes", async () => {
    const transport = fakeTransport({ today: () => TODAY });
    const events: unknown[] = [];
    await transport.subscribe((event) => events.push(event));
    await expect(transport.call("task.get", { keys: ["NOPE-1"] })).rejects.toMatchObject({ code: "not_found" });
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
    const card = todo?.cards[0];
    const anchor = doing?.cards[0];
    if (!todo || !doing || !card || !anchor) {
      throw new Error("sample data changed");
    }
    const moved = moveOnBoard(board, { key: card.key, status: doing.id, column: doing.id, place: { before: anchor.key } });
    expect(keysIn(moved, doing.id).slice(0, 2)).toEqual([card.key, anchor.key]);
    expect(keysIn(moved, todo.id)).not.toContain(card.key);
    expect(moved.columns[2]?.count).toBe(doing.count + 1);
  });
});
