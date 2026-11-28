// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Changes agents proposed for the person's approval, in the sample
// workspace: listed with the preview kept when the agent proposed them,
// the fields changed since, expiry after the setting's days, and rejection
// with a reason. Approving runs the operation as the agent (in the engine
// stand-in itself).

import {
  type EntityChange,
  type Proposal,
  type ProposalConflict,
  type ProposalListInput,
  type ProposalList,
  type ProposalRejectInput,
  type ProposalRejectResult,
  type ProposalStatus,
} from "../../generated/ops";
import { TASK_FIELDS } from "./details";
import { type FakeState } from "./state";

const DAY_MS = 86_400_000;
/** An approval running longer than this was interrupted: the program running it stopped. */
const INTERRUPTED_AFTER_MS = 10 * 60_000;

/** A proposal that is no longer waiting, or none: the engine's error. */
export class ProposalRefusal extends Error {
  constructor(
    readonly code: "invalid" | "not_found" | "conflict",
    message: string,
    readonly details: Record<string, unknown> = {},
  ) {
    super(message);
  }
}

/** Days a proposal waits, as the settings say. */
export function expireDays(state: FakeState): number {
  return state.settings.agent_approvals.expire_days;
}

/** When a proposal expires while nobody decides. */
function expiresAt(proposal: Proposal, days: number): number {
  return new Date(proposal.created_at).getTime() + days * DAY_MS;
}

/** Where a proposal stands at `now`. */
export function statusAt(proposal: Proposal, now: Date, days: number): ProposalStatus {
  if (proposal.status === "pending" && now.getTime() >= expiresAt(proposal, days)) {
    return "expired";
  }
  return proposal.status;
}

/** Whether a proposal being approved was interrupted at `now`. */
export function interruptedAt(proposal: Proposal, now: Date): boolean {
  return proposal.status === "approving" && proposal.claimed_at !== undefined && now.getTime() - new Date(proposal.claimed_at).getTime() > INTERRUPTED_AFTER_MS;
}

/** The task fields a waiting proposal changes whose value differs now from the one its preview started from. */
export function conflictsOf(state: FakeState, proposal: Proposal): ProposalConflict[] {
  const out: ProposalConflict[] = [];
  for (const change of proposal.changes) {
    if (change.entity !== "task" || "created" in change.fields) {
      continue;
    }
    const task = state.tasks.find((candidate) => candidate.id === change.id);
    if (!task) {
      continue;
    }
    for (const [field, recorded] of Object.entries(change.fields)) {
      const read = TASK_FIELDS[field];
      if (!read) {
        continue;
      }
      const now = read(task) ?? undefined;
      if (JSON.stringify(now) !== JSON.stringify(recorded.before)) {
        out.push({ entity: "task", id: change.id, field, ...(recorded.before === undefined ? {} : { then: recorded.before }), ...(now === undefined ? {} : { now }) });
      }
    }
  }
  return out;
}

/** The proposal as callers see it at `now`: its status, expiry, the task as it is now and what changed since. */
export function shownProposal(state: FakeState, proposal: Proposal, now: Date, keyOf: (taskId: string) => string | undefined): Proposal {
  const days = expireDays(state);
  const status = statusAt(proposal, now, days);
  const expires = new Date(expiresAt(proposal, days)).toISOString();
  const taskId = proposal.task?.id;
  const task = taskId === undefined ? undefined : state.tasks.find((candidate) => candidate.id === taskId);
  const key = task && keyOf(task.id);
  const conflicts = status === "pending" ? conflictsOf(state, proposal) : [];
  const rest: Proposal = { ...proposal };
  delete rest.task;
  delete rest.conflicts;
  return {
    ...rest,
    status,
    expires_at: expires,
    ...(status === "expired" ? { decided_at: expires } : {}),
    ...(interruptedAt(proposal, now) ? { interrupted: true } : {}),
    ...(task && key !== undefined ? { task: { id: task.id, key, title: task.title } } : {}),
    ...(conflicts.length > 0 ? { conflicts } : {}),
  };
}

/** `proposal.list`: newest first, filtered by status, session or IDs. */
export function listProposals(state: FakeState, input: ProposalListInput, now: Date, keyOf: (taskId: string) => string | undefined): ProposalList {
  const all = [...(state.proposals ?? [])].reverse().map((proposal) => shownProposal(state, proposal, now, keyOf));
  const pending = all.filter((proposal) => proposal.status === "pending").length;
  const ids = input.ids ?? [];
  const matching = all.filter(
    (proposal) =>
      (ids.length === 0 || ids.includes(proposal.id)) &&
      (input.status === null || input.status === undefined || proposal.status === input.status) &&
      (input.session === null || input.session === undefined || proposal.session === input.session),
  );
  return { items: matching.slice(0, input.limit ?? 50), total: matching.length, pending };
}

/** The proposal with `id`, as stored. */
export function storedProposal(state: FakeState, id: string): Proposal {
  const found = (state.proposals ?? []).find((proposal) => proposal.id === id);
  if (!found) {
    throw new ProposalRefusal("not_found", `There is no proposal ${id}.`, { message_id: "proposal-not-found", args: { id } });
  }
  return found;
}

/** The waiting proposals `ids` or `session` name, oldest first. */
export function chosenProposals(state: FakeState, input: { ids?: string[] | undefined; session?: string | null | undefined }, now: Date): Proposal[] {
  const ids = input.ids ?? [];
  const session = input.session ?? undefined;
  if ((ids.length === 0) === (session === undefined)) {
    throw new ProposalRefusal("invalid", "Name the proposals by ID, or give a session, not both.", { path: "ids", message_id: "proposal-ids-or-session" });
  }
  const days = expireDays(state);
  const rows = ids.length > 0 ? ids.map((id) => storedProposal(state, id)) : (state.proposals ?? []).filter((proposal) => proposal.session === session && statusAt(proposal, now, days) === "pending");
  if (rows.length === 0) {
    throw new ProposalRefusal("not_found", "No proposal waits for a decision there.", { message_id: "proposal-none-waiting" });
  }
  return [...rows].sort((a, b) => a.created_at.localeCompare(b.created_at));
}

/** The refusal for a proposal that no longer waits. */
export function decided(proposal: Proposal, status: ProposalStatus): ProposalRefusal {
  if (status === "approving") {
    return new ProposalRefusal("conflict", `Proposal ${proposal.id} is being approved right now, by this or another program.`, { proposal: proposal.id, status, message_id: "proposal-being-decided", args: { id: proposal.id } });
  }
  return status === "expired"
    ? new ProposalRefusal("conflict", `Proposal ${proposal.id} expired before anyone decided on it.`, { proposal: proposal.id, status, message_id: "proposal-expired", args: { id: proposal.id } })
    : new ProposalRefusal("conflict", `Proposal ${proposal.id} is no longer waiting: it is ${status}.`, { proposal: proposal.id, status, message_id: "proposal-decided", args: { id: proposal.id, status } });
}

/** `proposal.reject`: each chosen proposal rejected, with the reason. */
export function rejectProposals(state: FakeState, input: ProposalRejectInput, now: Date, keyOf: (taskId: string) => string | undefined): { output: ProposalRejectResult; changes: EntityChange[] } {
  const reason = input.reason?.trim() ?? "";
  if (Array.from(reason).length > 2000) {
    throw new ProposalRefusal("invalid", "A reason is at most 2000 characters long.", { path: "reason", message_id: "proposal-reason-too-long", args: { max: "2000" } });
  }
  const rows = chosenProposals(state, input, now);
  const days = expireDays(state);
  for (const proposal of rows) {
    const status = statusAt(proposal, now, days);
    // An interrupted approval can only be set aside.
    if (status !== "pending" && !interruptedAt(proposal, now)) {
      throw decided(proposal, status);
    }
  }
  for (const proposal of rows) {
    proposal.status = "rejected";
    delete proposal.claimed_at;
    proposal.decided_at = now.toISOString();
    proposal.decided_by = { kind: "user", name: "You" };
    if (reason !== "") {
      proposal.reason = reason;
    }
  }
  return {
    output: { items: rows.map((proposal) => shownProposal(state, proposal, now, keyOf)) },
    changes: rows.map((proposal) => ({ entity: "proposal", id: proposal.id, fields: ["status"], project: "" })),
  };
}
