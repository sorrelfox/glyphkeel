// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The sample workspace's task details: what a stored file holds, sample
// comments and files, and the activity log's record of a change.

import { type EntityChange, type FieldChange, type FileKind } from "../../generated/ops";
import { nextFileId } from "./seed";
import { type FakeState, type FakeTask } from "./state";

/** What the bytes of a file are, from their first bytes, the way the engine sniffs them. */
export function sniff(bytes: Uint8Array, name: string): { kind: FileKind; mime: string; width?: number; height?: number } {
  const starts = (...prefix: number[]) => prefix.every((byte, index) => bytes[index] === byte);
  const u32 = (at: number) => (((bytes[at] ?? 0) << 24) | ((bytes[at + 1] ?? 0) << 16) | ((bytes[at + 2] ?? 0) << 8) | (bytes[at + 3] ?? 0)) >>> 0;
  if (starts(0x89, 0x50, 0x4e, 0x47)) {
    return { kind: "image", mime: "image/png", width: u32(16), height: u32(20) };
  }
  if (starts(0xff, 0xd8, 0xff)) {
    return { kind: "image", mime: "image/jpeg" };
  }
  if (starts(0x47, 0x49, 0x46, 0x38)) {
    return { kind: "image", mime: "image/gif", width: (bytes[6] ?? 0) | ((bytes[7] ?? 0) << 8), height: (bytes[8] ?? 0) | ((bytes[9] ?? 0) << 8) };
  }
  if (starts(0x52, 0x49, 0x46, 0x46) && bytes[8] === 0x57 && bytes[9] === 0x45) {
    return { kind: "image", mime: "image/webp" };
  }
  if (/\.(html?|sh|bat|cmd|exe|ps1|js|py)$/i.test(name) || starts(0x7f, 0x45, 0x4c, 0x46) || starts(0x4d, 0x5a)) {
    return { kind: "executable", mime: "application/octet-stream" };
  }
  try {
    new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    return { kind: "text", mime: /\.md$/i.test(name) ? "text/markdown" : "text/plain" };
  } catch {
    return { kind: "other", mime: "application/octet-stream" };
  }
}

export function decodeBase64(text: string): Uint8Array | null {
  try {
    return Uint8Array.from(atob(text), (char) => char.charCodeAt(0));
  } catch {
    return null;
  }
}

export function encodeText(text: string): string {
  return btoa(String.fromCharCode(...new TextEncoder().encode(text)));
}

const SAMPLE_COMMENTS = [
  "Looked into this today; the first part is straightforward.",
  "Blocked on the review of the layout change. Picking it up again tomorrow.",
  "Done with the **first pass**. Notes are in the attached file.",
  "Could this wait until the next milestone?",
  "Checked on Fedora and KDE: works as expected.",
];

/** Sample comments and files for the counts the sample data starts with. */
export function materialize(state: FakeState, agent: string, nextId: (prefix: string) => string): void {
  for (const task of state.tasks) {
    for (let index = 0; index < task.comments; index += 1) {
      const byAgent = (task.number + index) % 4 === 1;
      state.comments.push({
        id: nextId("c"),
        taskId: task.id,
        body: SAMPLE_COMMENTS[(task.number + index) % SAMPLE_COMMENTS.length] ?? "",
        author: byAgent ? { kind: "agent", name: agent } : { kind: "user", name: "You" },
        via: byAgent ? "mcp" : "app",
        createdAt: new Date(Date.parse(task.createdAt) + (index + 1) * 3_600_000).toISOString(),
        editedAt: null,
        version: 1,
        revisions: [],
        replyTo: null,
        deletedAt: null,
      });
    }
    for (let index = 0; index < task.files; index += 1) {
      const text = `Notes ${String(index + 1)} for ${task.title}.\n`;
      state.files.push({
        id: nextFileId(),
        taskId: task.id,
        commentId: null,
        name: `notes-${String(index + 1)}.txt`,
        mime: "text/plain",
        kind: "text",
        size: new TextEncoder().encode(text).length,
        base64: encodeText(text),
        createdAt: task.createdAt,
        deletedAt: null,
      });
    }
    task.comments = 0;
    task.files = 0;
  }
}

export const TASK_FIELDS: Record<string, (task: FakeTask) => unknown> = {
  title: (task) => task.title,
  description: (task) => task.description,
  status: (task) => task.statusId,
  type: (task) => task.typeId,
  priority: (task) => task.priority,
  labels: (task) => task.labelIds,
  parent: (task) => task.parentId,
  subtask_rank: (task) => task.subtaskRank,
  start_date: (task) => task.startDate,
  due_date: (task) => task.dueDate,
  estimate: (task) => task.estimateMin,
  sprint: (task) => task.sprintId ?? null,
};

const side = (value: unknown) => (value === null || value === undefined ? undefined : value);

function pair(before: unknown, after: unknown): FieldChange {
  const change: FieldChange = {};
  if (side(before) !== undefined) {
    change.before = before;
  }
  if (side(after) !== undefined) {
    change.after = after;
  }
  return change;
}

/** The fields of one change as the activity log records them: each with its value before and after. */
export function recordedFields(before: FakeState, after: FakeState, change: EntityChange, keyOf: (task: FakeTask) => string): Record<string, FieldChange> {
  const fields: Record<string, FieldChange> = {};
  switch (change.entity) {
    case "task": {
      const old = before.tasks.find((task) => task.id === change.id);
      const now = after.tasks.find((task) => task.id === change.id);
      const trashed = (state: FakeState) => state.trash?.some((deletion) => deletion.tasks.some((task) => task.id === change.id)) === true;
      if (!old && now && trashed(before)) {
        return { trashed: { before: true }, key: { after: keyOf(now) }, title: { after: now.title } };
      }
      if (old && !now) {
        return trashed(after) ? { trashed: { after: true }, key: { before: keyOf(old) }, title: { before: old.title } } : { purged: { before: true } };
      }
      if (!old && now) {
        return { created: { after: true }, key: { after: keyOf(now) }, title: { after: now.title }, status: { after: now.statusId } };
      }
      for (const [field, read] of Object.entries(TASK_FIELDS)) {
        const a = old ? read(old) : undefined;
        const b = now ? read(now) : undefined;
        if (JSON.stringify(a) !== JSON.stringify(b)) {
          fields[field] = pair(a, b);
        }
      }
      const claimBefore = before.claims.find((claim) => claim.taskId === change.id);
      const claimAfter = after.claims.find((claim) => claim.taskId === change.id);
      if (claimBefore?.session !== claimAfter?.session) {
        fields.claim = pair(claimBefore && { session: claimBefore.session, agent: claimBefore.agent }, claimAfter && { session: claimAfter.session, agent: claimAfter.agent });
      }
      return fields;
    }
    case "comment": {
      const old = before.comments.find((comment) => comment.id === change.id);
      const now = after.comments.find((comment) => comment.id === change.id);
      const task = now && after.tasks.find((candidate) => candidate.id === now.taskId);
      if (!old && now) {
        return { body: { after: now.body }, created: { after: true }, ...(task ? { task: { after: keyOf(task) } } : {}) };
      }
      if (old && now && old.body !== now.body) {
        fields.body = pair(old.body, now.body);
      }
      if (old && now && old.deletedAt !== now.deletedAt) {
        fields.trashed = pair(old.deletedAt !== null || undefined, now.deletedAt !== null || undefined);
      }
      return fields;
    }
    case "attachment": {
      const old = before.files.find((file) => file.id === change.id);
      const now = after.files.find((file) => file.id === change.id);
      if (!old && now) {
        return { created: { after: true }, name: { after: now.name } };
      }
      if (old && now && old.deletedAt !== now.deletedAt) {
        fields.trashed = pair(old.deletedAt !== null || undefined, now.deletedAt !== null || undefined);
      }
      return fields;
    }
    case "relation": {
      const old = before.relations.find((relation) => relation.id === change.id);
      const now = after.relations.find((relation) => relation.id === change.id);
      const link = (relation: typeof old) => relation && { from: relation.from, to: relation.to, kind: relation.kind };
      return { link: pair(link(old), link(now)) };
    }
    case "proposal": {
      // A proposal is recorded by its status: proposed, approved or rejected.
      const old = before.proposals?.find((proposal) => proposal.id === change.id);
      const now = after.proposals?.find((proposal) => proposal.id === change.id);
      return { status: pair(old?.status, now?.status), ...(now?.reason !== undefined && old?.reason === undefined ? { reason: pair(undefined, now.reason) } : {}) };
    }
    default:
      return Object.fromEntries(change.fields.map((field) => [field, {}]));
  }
}
