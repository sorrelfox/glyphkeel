// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ActivityPage, type ListedTask, type Proposal, type ProposalApproveResult, type ProposalList } from "../../generated/ops";
import { FakeError } from "./engine";
import { SAMPLE_SESSIONS } from "./sampleProposals";
import { fakeTransport } from "./transport";

const today = new Date("2026-11-23T10:00:00Z");

function sample() {
  let now = today;
  const transport = fakeTransport({ sampleProposals: true, today: () => now });
  const ws = transport.workspace;
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
  const call = <T,>(op: string, input: unknown = {}) => ws.call(op, input).output as T;
  return {
    ws,
    call,
    later: (days: number) => {
      now = new Date(today.getTime() + days * 86_400_000);
    },
  };
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- the typed view of an untyped answer
const waiting = (call: <T>(op: string, input?: unknown) => T) => call<ProposalList>("proposal.list", { status: "pending" });

describe("proposals in the sample workspace", () => {
  it("wait with their preview, and one shows what changed since", () => {
    const { call } = sample();
    const list = waiting(call);
    expect(list.pending).toBe(4);
    const rename = list.items.find((item) => item.op === "task.update" && item.changes[0]?.fields.title !== undefined);
    expect(rename?.conflicts?.[0]?.field).toBe("title");
    const priority = list.items.find((item) => item.changes[0]?.fields.priority !== undefined);
    expect(["urgent", "high"]).toContain(priority?.changes[0]?.fields.priority?.after);
    expect(priority?.task?.key).toMatch(/^TM-/);
    expect(priority?.conflicts).toBeUndefined();
  });

  it("run once as the agent when approved, and history says the person approved them", () => {
    const { call, ws } = sample();
    const priority = waiting(call).items.find((item) => item.changes[0]?.fields.priority !== undefined) as Proposal;
    const result = call<ProposalApproveResult>("proposal.approve", { ids: [priority.id] });
    expect(result.approved).toBe(1);
    const task = call<{ items: ListedTask[] }>("task.search", { query: `key:${priority.task?.key ?? ""}` }).items[0];
    expect(task?.priority).toBe(priority.changes[0]?.fields.priority?.after);
    const group = result.items[0]?.proposal.group;
    const feed = call<ActivityPage>("history.feed", {});
    const change = feed.items.find((entry) => entry.group === group && entry.entity === "task");
    expect(change?.actor).toEqual({ kind: "agent", name: "Build agent" });
    expect(change?.approved_by_person).toBe(true);
    expect(waiting(call).pending).toBe(3);
    // Again: refused, nothing runs twice.
    const again = call<ProposalApproveResult>("proposal.approve", { ids: [priority.id] });
    expect(again.approved).toBe(0);
    expect(again.items[0]?.error?.code).toBe("conflict");
    // Undo takes the change back; the decision stays.
    ws.call("history.undo", {});
    expect(call<Proposal>("proposal.get", { id: priority.id }).status).toBe("approved");
  });

  it("need forcing when stale, keep a rejection's reason, and expire", () => {
    const { call, later } = sample();
    const stale = waiting(call).items.find((item) => (item.conflicts ?? []).length > 0) as Proposal;
    expect(call<ProposalApproveResult>("proposal.approve", { ids: [stale.id] }).approved).toBe(0);
    expect(call<ProposalApproveResult>("proposal.approve", { ids: [stale.id], force: true }).approved).toBe(1);

    const deletion = waiting(call).items.find((item) => item.session === SAMPLE_SESSIONS.review) as Proposal;
    call("proposal.reject", { ids: [deletion.id], reason: "Keep it for now." });
    expect(call<Proposal>("proposal.get", { id: deletion.id })).toMatchObject({ status: "rejected", reason: "Keep it for now." });

    later(15);
    expect(waiting(call).pending).toBe(0);
    expect(call<ProposalList>("proposal.list", { status: "expired" }).total).toBe(2);
  });

  it("approve a whole session oldest first, and refuse ids with a session", () => {
    const { call, ws } = sample();
    const result = call<ProposalApproveResult>("proposal.approve", { session: SAMPLE_SESSIONS.build, force: true });
    expect(result.approved).toBe(3);
    const created = result.items.map((item) => item.proposal.created_at);
    expect([...created].sort()).toEqual(created);
    expect(() => ws.call("proposal.approve", { ids: ["p-1"], session: SAMPLE_SESSIONS.build })).toThrow(FakeError);
  });

  it("being approved, refuse approving again; interrupted, can only be set aside", () => {
    const { call } = sample();
    const stuck = call<ProposalList>("proposal.list", { status: "approving" }).items[0] as Proposal;
    expect(stuck.session).toBe(SAMPLE_SESSIONS.docs);
    expect(stuck.interrupted).toBe(true);
    const again = call<ProposalApproveResult>("proposal.approve", { ids: [stuck.id] });
    expect(again.approved).toBe(0);
    expect(again.items[0]?.error?.details?.message_id).toBe("proposal-being-decided");
    call("proposal.reject", { ids: [stuck.id] });
    expect(call<Proposal>("proposal.get", { id: stuck.id }).status).toBe("rejected");
  });

  it("go back to waiting with the reason when an approval fails", () => {
    const { call } = sample();
    const deletion = waiting(call).items.find((item) => item.session === SAMPLE_SESSIONS.review) as Proposal;
    expect(deletion.last_error).toBeDefined();
    const key = (deletion.input as { keys: string[] }).keys[0] ?? "";
    call("task.delete", { keys: [key] });
    call("trash.purge", { all: true });
    const failed = call<ProposalApproveResult>("proposal.approve", { ids: [deletion.id] });
    expect(failed.approved).toBe(0);
    const now = call<Proposal>("proposal.get", { id: deletion.id });
    expect(now.status).toBe("pending");
    // eslint-disable-next-line no-restricted-syntax -- comparing the stored text with the error's
    expect(now.last_error).toBe(failed.items[0]?.error?.message);
  });

  it("follow the approvals setting", () => {
    const { call, ws } = sample();
    call("agent.policy.update", { max_profile: "standard", approvals: { read_only: "all_writes", standard: "destructive", full: "off", expire_days: 30 } });
    expect(call<{ agent_approvals: unknown }>("settings.get").agent_approvals).toEqual({ read_only: "all_writes", standard: "destructive", full: "off", expire_days: 30 });
    expect(() => ws.call("agent.policy.update", { max_profile: "standard", approvals: { read_only: "destructive", standard: "off", full: "off", expire_days: 14 } })).toThrow(FakeError);
  });
});
