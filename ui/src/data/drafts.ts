// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Unsaved texts of the editors, kept while the window is open: closing the
// panel, opening another task or switching tabs doesn't lose them. They are
// held in memory only, so they don't survive a restart.

export interface Draft {
  text: string;
  /** The description revision the text was edited from. */
  base?: string | undefined;
  /** The conflict copy this text resolves. */
  resolving?: string | undefined;
}

const drafts = new Map<string, Draft>();
const MAX_DRAFTS = 50;

/** The draft kept under `id` ("description:TM-41", "comment:…", "composer:TM-41"). */
export function readDraft(id: string): Draft | undefined {
  return drafts.get(id);
}

/** Keeps a draft, or forgets it with `null`. */
export function writeDraft(id: string, draft: Draft | null): void {
  drafts.delete(id);
  if (draft === null) {
    return;
  }
  if (drafts.size >= MAX_DRAFTS) {
    const oldest = drafts.keys().next();
    if (!oldest.done) {
      drafts.delete(oldest.value);
    }
  }
  drafts.set(id, draft);
}
