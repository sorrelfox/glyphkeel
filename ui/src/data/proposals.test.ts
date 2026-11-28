// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Proposal } from "../generated/ops";
import { type Translate } from "../i18n/I18n";
import { createBundle } from "../i18n/i18n";
import { bySession, opMessage, proposalTitle, shortId } from "./proposals";

function proposal(id: string, session: string | undefined, agent = "Build agent"): Proposal {
  return { id, op: "task.update", summary: "Change a task.", input: {}, status: "pending", created_at: "", expires_at: "", agent: { kind: "agent", name: agent }, ...(session === undefined ? {} : { session }), profile: "standard", changes: [], previewed: true };
}

const bundle = createBundle("en");
const t: Translate = Object.assign(
  (id: string) => {
    const message = bundle.getMessage(id);
    return message?.value ? bundle.formatPattern(message.value) : id;
  },
  { has: (id: string) => bundle.hasMessage(id) },
);

describe("proposals", () => {
  it("group by session, newest session first and each oldest first", () => {
    const groups = bySession([proposal("p-4", "s-2"), proposal("p-3", "s-1"), proposal("p-2", "s-2"), proposal("p-1", undefined, "Other")]);
    expect(groups.map((group) => [group.session, group.items.map((item) => item.id)])).toEqual([
      ["s-2", ["p-2", "p-4"]],
      ["s-1", ["p-3"]],
      [undefined, ["p-1"]],
    ]);
  });

  it("name what they do in the person's language, else by the engine's summary", () => {
    expect(opMessage("task.bulk_update")).toBe("approvals-op-task-bulk-update");
    expect(proposalTitle(t, { op: "task.update", summary: "x" })).toBe("Change a task");
    expect(proposalTitle(t, { op: "dashboard.create", summary: "Create a dashboard." })).toBe("Create a dashboard.");
    expect(shortId("0190c0de-0000-7000-8000-000000000001")).toBe("000001");
  });
});
