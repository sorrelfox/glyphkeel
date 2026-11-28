// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Proposals in the sample workspace (`?proposals`): agent sessions whose
// changes wait for the person, one changed since by the person, one whose
// last approval failed and one whose approval was interrupted, so the
// Approvals screen has something to show.

import { type ListedTask } from "../../generated/ops";
import { type FakeWorkspace } from "./engine";

/** The agent sessions of the sample proposals. */
export const SAMPLE_SESSIONS = { build: "0190c0de-0000-7000-8000-000000000001", review: "0190c0de-0000-7000-8000-000000000002", docs: "0190c0de-0000-7000-8000-000000000003" } as const;

export function addSampleProposals(workspace: FakeWorkspace, today: Date): void {
  const found = workspace.call("task.search", { query: "project:TM is:open", limit: 10 }).output as { items: ListedTask[] };
  const [first, second, third, fourth, fifth] = found.items;
  if (!first || !second || !third || !fourth || !fifth) {
    return;
  }
  const ago = (minutes: number) => new Date(today.getTime() - minutes * 60_000);
  const build = { name: "Build agent", session: SAMPLE_SESSIONS.build };
  const review = { name: "Review agent", session: SAMPLE_SESSIONS.review };
  workspace.propose("task.update", { key: first.key, priority: first.priority === "urgent" ? "high" : "urgent" }, build, ago(90));
  workspace.propose("comment.add", { key: second.key, body: "The nightly build passed; ready for review." }, build, ago(80));
  workspace.propose("task.update", { key: third.key, title: `${third.title} (agent's wording)` }, build, ago(70));
  // The person renamed it since: approving would overwrite that.
  workspace.call("task.update", { key: third.key, title: `${third.title} (your wording)` });
  const deletion = workspace.propose("task.delete", { keys: [fourth.key] }, review, ago(30));
  // The last approval of it failed: it waits again, saying why.
  workspace.setProposal(deletion, { last_error: "The task changed while it was being approved." });
  // An approval that began an hour ago and never ended: the program running it stopped.
  const docs = { name: "Docs agent", session: SAMPLE_SESSIONS.docs };
  const stuck = workspace.propose("comment.add", { key: fifth.key, body: "Docs updated for the new columns." }, docs, ago(120));
  workspace.setProposal(stuck, { status: "approving", claimed_at: ago(60).toISOString() });
}
