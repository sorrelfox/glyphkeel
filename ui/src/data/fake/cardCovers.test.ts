// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Board, type Card, type FileChange, type FileCoverChange, type LayoutChange, type Project, type TaskChange } from "../../generated/ops";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";

const TODAY = new Date(2026, 10, 23, 9, 0);
/** A 1×1 PNG. */
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

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

function card(ws: FakeWorkspace, key: string): Card | undefined {
  return call<Board>(ws, "board.get", { project: "TM" })
    .columns.flatMap((column) => column.cards)
    .find((each) => each.key === key);
}

describe("card colours and covers in the sample workspace", () => {
  it("colours the edge by priority, label, type or the first matching rule", () => {
    const ws = workspace();
    const key = call<TaskChange>(ws, "task.create", { project: "TM", title: "Crash", priority: "urgent" }).task.key;
    expect(card(ws, key)?.edge).toBeUndefined();
    call(ws, "layout.update", { project: "TM", colour_by: "priority" });
    expect(card(ws, key)?.edge).toEqual({ hue: "red", by: "priority", name: "urgent" });
    call(ws, "layout.update", { project: "TM", colour_by: "type" });
    expect(card(ws, key)?.edge?.by).toBe("type");
    call(ws, "layout.update", { project: "TM", colour_by: "label" });
    expect(card(ws, key)?.edge).toBeUndefined();

    const changed = call<LayoutChange>(ws, "layout.update", {
      project: "TM",
      colour_by: "query",
      colour_rules: [
        { query: "priority:low", hue: "grey" },
        { query: " priority:urgent ", hue: "pink", name: "Fire" },
        { query: "priority:urgent", hue: "green" },
      ],
    });
    expect(changed.layout.colour_rules[1]).toEqual({ query: "priority:urgent", hue: "pink", name: "Fire" });
    expect(card(ws, key)?.edge).toEqual({ hue: "pink", by: "query", name: "Fire" });

    const wrong = failure(() => call(ws, "layout.update", { project: "TM", colour_rules: [{ query: "priority:", hue: "red" }, { query: "nosuchfield:1", hue: "red" }] }));
    expect(wrong.code).toBe("invalid");
    expect(wrong.details?.path).toMatch(/^colour_rules\[\d\]\.query$/);
  });

  it("shows the chosen cover, or the first image, only while the layout asks", () => {
    const ws = workspace();
    const key = call<TaskChange>(ws, "task.create", { project: "TM", title: "Mock-up" }).task.key;
    const attach = (name: string, data: string) => call<FileChange>(ws, "file.attach", { task: key, name, data_base64: data }).file;
    const notes = attach("notes.txt", "aGVsbG8=");
    const first = attach("first.png", PNG);
    const second = attach("second.png", PNG);
    expect(card(ws, key)?.cover).toBeUndefined();
    call(ws, "layout.update", { project: "TM", covers: "first_image" });
    expect(card(ws, key)?.cover).toMatchObject({ attachment: first.id, name: "first.png", mime: "image/png" });
    call(ws, "layout.update", { project: "TM", covers: "chosen" });
    expect(card(ws, key)?.cover).toBeUndefined();

    expect(failure(() => call(ws, "file.cover", { task: key, attachment: notes.id })).details?.message_id).toBe("file-cover-not-image");
    const chosen = call<FileCoverChange>(ws, "file.cover", { task: key, attachment: second.id });
    expect(chosen.cover?.cover).toBe(true);
    expect(card(ws, key)?.cover?.name).toBe("second.png");

    call(ws, "history.undo");
    expect(card(ws, key)?.cover).toBeUndefined();
  });

  it("rewrites the rules' queries when a name they use changes, as the engine does", () => {
    const ws = workspace();
    const project = call<Project>(ws, "project.get", { project: "TM" });
    const status = project.statuses[0]?.name ?? "";
    const label = project.labels[0]?.name ?? "";
    call(ws, "layout.update", {
      project: "TM",
      colour_by: "query",
      colour_rules: [
        { query: `status:"${status}"`, hue: "grey" },
        { query: `label:${label},docs -priority:low`, hue: "pink" },
      ],
    });
    call(ws, "status.update", { project: "TM", status, name: "Ice box" });
    call(ws, "label.update", { project: "TM", label, name: "front" });
    call(ws, "project.change_key", { project: "TM", key: "TK" });
    const layout = call<LayoutChange>(ws, "layout.get", { project: "TK" }).layout;
    expect(layout.colour_rules.map((rule) => rule.query)).toEqual(['status:"Ice box"', "label:front,docs -priority:low"]);
  });
});
