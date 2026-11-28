// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Changes agents proposed for the person's approval: the waiting ones for
// the Approvals screen and the sidebar's count, grouped by agent session.

import { type Proposal, type ProposalStatus } from "../generated/ops";
import { type Translate } from "../i18n/I18n";
import { useOp } from "./hooks";

/** The most proposals the screen lists at once. */
export const PROPOSAL_PAGE = 200;

/** The input of the waiting proposals' list, shared by the screen and the sidebar's count. */
export const WAITING = { status: "pending", limit: PROPOSAL_PAGE } as const;

/** The proposals with `status`, newest first. */
export function useProposals(status: ProposalStatus) {
  return useOp("proposal.list", status === "pending" ? WAITING : { status, limit: PROPOSAL_PAGE });
}

/** How many proposals wait for the person, once known. */
export function usePendingProposals(): number | undefined {
  return useProposals("pending").data?.pending;
}

/** The proposals of one agent session, in the order the agent made them. */
export interface SessionGroup {
  /** The session's ID; none for proposals that came without one. */
  session: string | undefined;
  /** The agent's name. */
  agent: string;
  items: Proposal[];
}

/**
 * `proposals` (newest first) by agent session: the session with the newest
 * proposal first, and in each the proposals oldest first, as approving the
 * whole session runs them.
 */
export function bySession(proposals: readonly Proposal[]): SessionGroup[] {
  const groups = new Map<string, SessionGroup>();
  for (const proposal of proposals) {
    const id = proposal.session ?? `agent:${proposal.agent.name}`;
    let group = groups.get(id);
    if (!group) {
      group = { session: proposal.session, agent: proposal.agent.name, items: [] };
      groups.set(id, group);
    }
    group.items.unshift(proposal);
  }
  return [...groups.values()];
}

/** The message of an operation's name (`approvals-op-task-update`). */
export function opMessage(op: string): string {
  return `approvals-op-${op.replaceAll(".", "-").replaceAll("_", "-")}`;
}

/** What a proposal does, for people: its operation's name in the person's language, else the engine's summary. */
export function proposalTitle(t: Translate, proposal: Pick<Proposal, "op" | "summary">): string {
  const id = opMessage(proposal.op);
  return t.has?.(id) === true ? t(id) : proposal.summary;
}

/** The last characters of an ID (its random part), enough to tell sessions apart. */
export function shortId(id: string): string {
  return id.replaceAll("-", "").slice(-6);
}
