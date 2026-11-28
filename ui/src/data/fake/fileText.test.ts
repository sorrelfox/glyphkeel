// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type FileChange, type FileIndexStatus, type SearchResult, type TaskChange } from "../../generated/ops";
import { encodeText } from "./details";
import { FakeWorkspace } from "./engine";
import { seed } from "./seed";

const TODAY = new Date(2026, 10, 23, 9, 0);

function workspace() {
  return new FakeWorkspace(seed(TODAY), () => TODAY);
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

/** A one-page PDF whose page shows `text`. */
function pdf(text: string): string {
  return encodeText(`%PDF-1.4\n1 0 obj\n<< /Length 40 >>\nstream\nBT /F1 18 Tf 72 720 Td (${text}) Tj ET\nendstream\nendobj\n%%EOF\n`);
}

describe("the text inside attached files", () => {
  it("is found by search with the words around the match", () => {
    const ws = workspace();
    const task = call<TaskChange>(ws, "task.create", { project: "TM", title: "Supplier contract" }).task;
    const file = call<FileChange>(ws, "file.attach", { task: task.key, name: "contract.pdf", data_base64: pdf("The quokka clause applies") }).file;
    expect(file.mime).toBe("application/pdf");

    const found = call<SearchResult>(ws, "search.query", { text: "quokka" });
    expect(found.items).toHaveLength(1);
    const hit = found.items[0];
    expect(hit?.kind).toBe("file");
    expect(hit?.matched_in).toBe("text");
    expect(hit?.title).toBe("contract.pdf");
    expect(hit?.task).toBe(task.key);
    expect(hit?.snippet).toContain("quokka clause");
    expect(hit?.snippet_matches?.length).toBe(1);

    // A trashed file is not found.
    call(ws, "file.delete", { attachment: file.id });
    expect(call<SearchResult>(ws, "search.query", { text: "quokka" }).items).toHaveLength(0);
  });

  it("is counted for Preferences, Data", () => {
    const ws = workspace();
    const task = call<TaskChange>(ws, "task.create", { project: "TM", title: "Files" }).task;
    const before = call<FileIndexStatus>(ws, "file.index_status");
    call(ws, "file.attach", { task: task.key, name: "a.pdf", data_base64: pdf("words") });
    call(ws, "file.attach", { task: task.key, name: "scan.pdf", data_base64: encodeText("%PDF-1.4\n%%EOF\n") });
    const after = call<FileIndexStatus>(ws, "file.reindex");
    expect(after.files).toBe(before.files + 2);
    expect(after.waiting).toBe(0);
    expect(after.with_text).toBe(before.with_text + 1);
    expect(after.skipped).toBe(before.skipped + 1);
  });
});
