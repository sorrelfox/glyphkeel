// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ActivityPage, type CommentChange, type CommentList, type FileChange, type FileContent, type MoveCheck, type ProjectChange, type ProjectList, type RevertResult, type TaskList } from "../../generated/ops";
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

describe("task details", () => {
  const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
  const get = (ws: FakeWorkspace, key: string) => call<TaskList>(ws, "task.get", { keys: [key], include: ["subtasks", "links", "files"] }).items[0];

  it("links tasks both ways, marks the blocked one, and refuses a loop", () => {
    const ws = workspace();
    const [a, b] = [call<TaskChange>(ws, "task.create", { project: "PER", title: "A" }).task, call<TaskChange>(ws, "task.create", { project: "PER", title: "B" }).task];
    call(ws, "relation.add", { key: a.key, kind: "blocks", other: b.key });
    expect(get(ws, a.key)?.links).toMatchObject([{ kind: "blocks", task: { key: b.key } }]);
    expect(get(ws, b.key)?.links).toMatchObject([{ kind: "blocked_by", task: { key: a.key } }]);
    expect(get(ws, b.key)?.blocked).toBe(true);
    expect(failure(() => call(ws, "relation.add", { key: b.key, kind: "blocks", other: a.key })).code).toBe("invalid");
    call(ws, "relation.remove", { key: b.key, kind: "blocked_by", other: a.key });
    expect(get(ws, b.key)?.blocked).toBe(false);
  });

  it("makes subtasks through links and with a parent, in board order", () => {
    const ws = workspace();
    const parent = call<TaskChange>(ws, "task.create", { project: "PER", title: "Parent" }).task;
    const first = call<TaskChange>(ws, "task.create", { project: "PER", title: "First", parent: parent.key }).task;
    const second = call<TaskChange>(ws, "task.create", { project: "PER", title: "Second" }).task;
    call(ws, "relation.add", { key: second.key, kind: "subtask_of", other: parent.key });
    expect(get(ws, parent.key)?.subtasks?.map((sub) => sub.key)).toEqual([first.key, second.key]);
    expect(get(ws, parent.key)?.subtask_count).toEqual({ done: 0, total: 2 });
    expect(failure(() => call(ws, "relation.add", { key: parent.key, kind: "subtask_of", other: first.key })).code).toBe("invalid");
  });

  it("keeps subtasks in their own order across statuses and reorders them", () => {
    const ws = workspace();
    const parent = call<TaskChange>(ws, "task.create", { project: "PER", title: "Parent" }).task;
    const [a, b, c] = ["A", "B", "C"].map((title) => call<TaskChange>(ws, "task.create", { project: "PER", title, parent: parent.key }).task);
    if (!a || !b || !c) {
      throw new Error("no subtasks");
    }
    const order = () => get(ws, parent.key)?.subtasks?.map((sub) => sub.key);
    // A status doesn't reorder them.
    call(ws, "task.move", { key: a.key, status: "Done" });
    expect(order()).toEqual([a.key, b.key, c.key]);
    // Past a subtask of another status.
    const moved = call<TaskChange>(ws, "task.reorder_subtask", { key: a.key, after: b.key });
    expect(moved.task.key).toBe(parent.key);
    expect(moved.task.subtasks?.map((sub) => sub.key)).toEqual([b.key, a.key, c.key]);
    call(ws, "task.reorder_subtask", { key: c.key, at: "top" });
    expect(order()).toEqual([c.key, b.key, a.key]);
    call(ws, "task.reorder_subtask", { key: a.key, before: b.key });
    expect(order()).toEqual([c.key, a.key, b.key]);
    // Where it already is: no change.
    expect(call<TaskChange>(ws, "task.reorder_subtask", { key: a.key, after: c.key }).group).toBeUndefined();
    expect(failure(() => call(ws, "task.reorder_subtask", { key: parent.key, at: "top" })).code).toBe("invalid");
    const stranger = call<TaskChange>(ws, "task.create", { project: "PER", title: "Elsewhere" }).task;
    expect(failure(() => call(ws, "task.reorder_subtask", { key: a.key, before: stranger.key })).code).toBe("invalid");
    // The move is recorded on the subtask.
    const history = call<ActivityPage>(ws, "history.feed", { task: a.key });
    expect(history.items[0]?.fields.subtask_rank).toMatchObject({ before: expect.any(Number) as number, after: expect.any(Number) as number });
  });

  it("makes thumbnails of images only, at most 256 pixels a side", () => {
    const ws = workspace();
    const task = call<TaskChange>(ws, "task.create", { project: "PER", title: "Pictures" }).task;
    const image = call<FileChange>(ws, "file.attach", { task: task.key, name: "dot.png", data_base64: PNG }).file;
    expect(call(ws, "file.thumbnail", { attachment: image.id })).toMatchObject({ attachment: image.id, mime: "image/png", width: 1, height: 1 });
    const text = call<FileChange>(ws, "file.attach", { task: task.key, name: "a.txt", data_base64: btoa("hello") }).file;
    expect(failure(() => call(ws, "file.thumbnail", { attachment: text.id })).code).toBe("invalid");
  });

  it("keeps a comment's earlier texts, refuses a stale version and trashes it", () => {
    const ws = workspace();
    const task = call<TaskChange>(ws, "task.create", { project: "PER", title: "Talk" }).task;
    const added = call<CommentChange>(ws, "comment.add", { key: task.key, body: "first" }).comment;
    expect(failure(() => call(ws, "comment.add", { key: task.key, body: "  " })).code).toBe("invalid");
    call(ws, "comment.update", { comment: added.id, body: "second", expected_version: 1 });
    expect(failure(() => call(ws, "comment.update", { comment: added.id, body: "third", expected_version: 1 })).code).toBe("conflict");
    const [listed] = call<CommentList>(ws, "comment.list", { key: task.key, revisions: true }).items;
    expect(listed).toMatchObject({ body: "second", version: 2, revision_count: 1, revisions: [{ body: "first" }] });
    expect(listed?.edited_at).toBeDefined();
    call(ws, "comment.delete", { comment: added.id });
    expect(call<CommentList>(ws, "comment.list", { key: task.key }).total).toBe(0);
    expect(get(ws, task.key)?.comment_count).toBe(0);
  });

  it("sniffs attached files, counts them on the card and trashes them", () => {
    const ws = workspace();
    const task = call<TaskChange>(ws, "task.create", { project: "PER", title: "Files" }).task;
    const image = call<FileChange>(ws, "file.attach", { task: task.key, name: "dot.txt", data_base64: PNG }).file;
    expect(image).toMatchObject({ kind: "image", mime: "image/png", width: 1, height: 1, task: task.key });
    const text = call<FileChange>(ws, "file.attach", { task: task.key, name: "a.txt", data_base64: btoa("hello") }).file;
    expect(call<FileContent>(ws, "file.read", { attachment: text.id }).text).toBe("hello");
    expect(call<FileChange>(ws, "file.attach", { task: task.key, name: "run.sh", data_base64: btoa("echo") }).file.kind).toBe("executable");
    expect(failure(() => call(ws, "file.attach", { task: task.key, path: "/etc/passwd" })).code).toBe("invalid");
    expect(get(ws, task.key)?.file_count).toBe(3);
    call(ws, "file.delete", { attachment: text.id });
    expect(get(ws, task.key)?.files?.map((file) => file.name)).toEqual(["dot.txt", "run.sh"]);
  });

  it("duplicates a task with its subtasks and links the copy as a clone", () => {
    const ws = workspace();
    const parent = call<TaskChange>(ws, "task.create", { project: "PER", title: "Original" }).task;
    call(ws, "task.create", { project: "PER", title: "Child", parent: parent.key });
    const copy = call<TaskChange>(ws, "task.duplicate", { key: parent.key }).task;
    expect(copy.title).toBe("Original");
    expect(get(ws, copy.key)?.subtasks?.map((sub) => sub.title)).toEqual(["Child"]);
    expect(get(ws, copy.key)?.links).toMatchObject([{ kind: "clones", task: { key: parent.key } }]);
  });

  it("records changes in the activity log and reverts the latest", () => {
    const ws = workspace();
    const task = call<TaskChange>(ws, "task.create", { project: "PER", title: "Log" }).task;
    const change = ws.call("task.update", { key: task.key, priority: "high" });
    const feed = call<ActivityPage>(ws, "history.feed", { limit: 1 });
    expect(feed.items[0]).toMatchObject({ entity: "task", id: task.id, op: "task.update", fields: { priority: { before: "none", after: "high" } } });
    expect(feed.next_cursor).toBe("1");
    const group = change.changes?.group ?? "";
    const result = call<RevertResult>(ws, "history.revert", { groups: [group] });
    expect(result.reverted).toEqual([group]);
    expect(get(ws, task.key)?.priority).toBe("none");
    expect(call<ActivityPage>(ws, "history.feed", {}).items.find((entry) => entry.group === group)?.reverted_by).toBeDefined();
    expect(call<RevertResult>(ws, "history.revert", { groups: ["g-1"] }).skipped).toHaveLength(1);
  });

  it("turns the sample counts into comments and files, and has one agent's claim", () => {
    const ws = workspace();
    const board = call<Board>(ws, "board.get", { project: "TM" });
    const withComments = cards(board).find((card) => card.comment_count > 0);
    expect(withComments).toBeDefined();
    expect(call<CommentList>(ws, "comment.list", { key: withComments?.key }).total).toBe(withComments?.comment_count);
    const claimed = ["TM", "PER", "LAB", "CLI", "DOC"].flatMap((key) => cards(call<Board>(ws, "board.get", { project: key }))).filter((card) => card.claim);
    expect(claimed).toHaveLength(1);
    expect(failure(() => call(ws, "task.claim", { key: claimed[0]?.key })).code).toBe("forbidden");
    call(ws, "task.release", { key: claimed[0]?.key });
    expect(get(ws, claimed[0]?.key ?? "")?.claim).toBeUndefined();
  });
});

describe("moving tasks between projects and new project keys", () => {
  it("moves a task with its subtasks, keeps the old keys and maps what the target lacks", () => {
    const ws = workspace();
    const created = call<TaskChange>(ws, "task.create", { project: "TM", title: "Parent" }).task;
    const parent = call<TaskChange>(ws, "task.update", { key: created.key, labels: ["board"] }).task;
    const child = call<TaskChange>(ws, "task.create", { project: "TM", title: "Child", parent: parent.key }).task;
    const check = call<MoveCheck>(ws, "task.move_check", { keys: [parent.key], project: "PER" });
    expect(check.keys.map((k) => k.from)).toEqual([parent.key, child.key]);
    expect(check.labels).toEqual([{ name: "board", tasks: 1 }]);
    // The check changes nothing.
    expect(call<TaskList>(ws, "task.get", { keys: [parent.key] }).items[0]?.project).toBe("TM");

    const label = call<ProjectChange>(ws, "label.create", { project: "PER", name: "screens" });
    expect(label.project.key).toBe("PER");
    const moved = call<TaskChange>(ws, "task.move", { key: parent.key, project: "PER", mapping: { labels: { board: "screens" } } });
    expect(moved.task.key).toBe(check.keys[0]?.to);
    expect(moved.task.labels.map((l) => l.name)).toEqual(["screens"]);
    expect(moved.task.old_keys).toEqual([parent.key]);
    const sub = call<TaskList>(ws, "task.get", { keys: [child.key] }).items[0];
    expect(sub?.key).toBe(check.keys[1]?.to);
    expect(sub?.parent).toBe(moved.task.key);

    // Undo takes it home.
    ws.call("history.undo", {});
    expect(call<TaskList>(ws, "task.get", { keys: [parent.key] }).items[0]?.key).toBe(parent.key);
  });

  it("refuses a mapping to something the target doesn't have", () => {
    const ws = workspace();
    const task = call<TaskChange>(ws, "task.create", { project: "TM", title: "One" }).task;
    expect(failure(() => ws.call("task.move", { key: task.key, project: "PER", mapping: { statuses: { Backlog: "Nope" } } })).code).toBe("not_found");
  });

  it("gives a project a new key that the old keys keep finding", () => {
    const ws = workspace();
    const task = call<TaskChange>(ws, "task.create", { project: "PER", title: "One" }).task;
    const changed = call<ProjectChange>(ws, "project.change_key", { project: "PER", key: "ME" });
    expect(changed.project.key).toBe("ME");
    expect(changed.project.old_keys).toEqual(["PER"]);
    const found = call<TaskList>(ws, "task.get", { keys: [task.key] }).items[0];
    expect(found?.key).toBe(task.key.replace("PER-", "ME-"));
    expect(failure(() => ws.call("project.change_key", { project: "TM", key: "PER" })).code).toBe("conflict");
  });
});
