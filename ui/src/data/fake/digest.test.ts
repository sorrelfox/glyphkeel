// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Digest, type DigestDismissed, type RevertResult } from "../../generated/ops";
import { FakeWorkspace } from "./engine";
import { seed } from "./seed";

const TODAY = new Date(2026, 10, 23, 9, 0);

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

describe("the sample workspace's digest", () => {
  it("groups made-up sessions, a rule and the command line, and dismissing empties it", () => {
    const ws = new FakeWorkspace(seed(TODAY), () => TODAY);
    const digest = call<Digest>(ws, "digest.get");
    expect(digest.since_from).toBe("last_day");
    expect(digest.agents.map((session) => session.name)).toEqual(["Planner", "Claude Code"]);
    expect(digest.automation[0]?.failures).toBe(1);
    expect(digest.cli.changes).toBe(2);
    expect(digest.counts.items).toBe(digest.counts.sessions + digest.counts.rules + digest.counts.reminders + digest.counts.became_due + 1);

    const dismissed = call<DigestDismissed>(ws, "digest.dismiss", { at: digest.until });
    expect(dismissed.marker).toBe(digest.until);
    const after = call<Digest>(ws, "digest.get");
    expect(after.since_from).toBe("marker");
    expect(after.counts.items).toBe(0);
    // An earlier moment doesn't move it back.
    expect(call<DigestDismissed>(ws, "digest.dismiss", { at: "2026-01-01T00:00:00Z" }).marker).toBe(digest.until);
  });

  it("reverts one session and leaves the other", () => {
    const ws = new FakeWorkspace(seed(TODAY), () => TODAY);
    const session = call<Digest>(ws, "digest.get").agents.find((each) => each.name === "Claude Code");
    const result = call<RevertResult>(ws, "history.revert", { session: session?.id });
    expect(result.reverted).toHaveLength(3);
    const after = call<Digest>(ws, "digest.get").agents;
    expect(after.find((each) => each.name === "Claude Code")?.reverted).toBe(7);
    expect(after.find((each) => each.name === "Planner")?.reverted).toBe(0);
  });

  it("keeps to a project", () => {
    const ws = new FakeWorkspace(seed(TODAY), () => TODAY);
    const all = call<Digest>(ws, "digest.get");
    const project = all.agents[0]?.tasks[0]?.key.split("-")[0] ?? "";
    const one = call<Digest>(ws, "digest.get", { project });
    for (const session of one.agents) {
      expect(session.tasks.every((task) => task.key.startsWith(`${project}-`))).toBe(true);
    }
  });
});
