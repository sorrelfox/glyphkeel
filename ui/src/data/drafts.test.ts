// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { afterEach, describe, expect, it, vi } from "vitest";

import { type Call, type DraftList, type TaskChange } from "../generated/ops";
import { flushDrafts, forgetDraftsInMemory, readDraft, SAVE_AFTER_MS, writeDraft } from "./drafts";
import { fakeTransport } from "./fake/transport";

afterEach(() => {
  forgetDraftsInMemory();
});

async function workspace() {
  const transport = fakeTransport();
  const task = ((await transport.call("task.create", { project: "PER", title: "Plan the trip" })) as TaskChange).task;
  const sent: string[] = [];
  const call = ((op: string, input: unknown) => {
    sent.push(op);
    return transport.call(op, input);
  }) as Call;
  const kept = async () => ((await transport.call("draft.list", { task: task.key })) as DraftList).items;
  return { task, call, sent, kept };
}

describe("drafts", () => {
  it("are saved in the engine once the typing pauses, as one save", async () => {
    const { task, call, sent, kept } = await workspace();
    const target = { kind: "new_comment" as const, id: task.id };
    const started = Date.now();
    writeDraft(call, target, { text: "H" });
    writeDraft(call, target, { text: "Hel" });
    writeDraft(call, target, { text: "Hello" });
    expect(readDraft(target, undefined)?.text).toBe("Hello");
    await vi.waitFor(() => {
      expect(sent).toEqual(["draft.save"]);
    });
    expect(Date.now() - started).toBeGreaterThanOrEqual(SAVE_AFTER_MS - 50);
    expect((await kept()).map((draft) => draft.body)).toEqual(["Hello"]);
  });

  it("come from the engine when this window has none, and a discarded one stays discarded", async () => {
    const { task, call, kept } = await workspace();
    const target = { kind: "task" as const, id: task.id };
    writeDraft(call, target, { text: "Unsaved", base: "r-1" });
    flushDrafts();
    await vi.waitFor(async () => {
      expect(await kept()).toHaveLength(1);
    });
    const fromEngine = await kept();
    forgetDraftsInMemory();
    expect(readDraft(target, fromEngine)).toEqual({ text: "Unsaved", base: "r-1" });
    expect(readDraft({ kind: "new_comment", id: task.id }, fromEngine)).toBeUndefined();

    writeDraft(call, target, null);
    expect(readDraft(target, fromEngine)).toBeUndefined();
    await vi.waitFor(async () => {
      expect(await kept()).toEqual([]);
    });
  });

  it("are given up after the save that was still on its way, never before it", async () => {
    const { task, call, sent, kept } = await workspace();
    const target = { kind: "new_comment" as const, id: task.id };
    writeDraft(call, target, { text: "Sent soon" });
    flushDrafts();
    writeDraft(call, target, null);
    await vi.waitFor(() => {
      expect(sent).toEqual(["draft.save", "draft.discard"]);
    });
    await vi.waitFor(async () => {
      expect(await kept()).toEqual([]);
    });
  });
});
