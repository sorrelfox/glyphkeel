// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/** Sample texts and completion data for the editor's development page. */

import crlf from "../corpus/crlf.md?raw";
import mixed from "../corpus/mixed-endings.md?raw";
import sample from "../corpus/sp4-sample.md?raw";
import typical from "../corpus/sp4-typical.md?raw";

/** About 100 KB of typical descriptions, for typing on a long text. */
function long(): string {
  let text = "";
  for (let part = 1; text.length < 100_000; part++) {
    text += `## Part ${String(part)}\n\n${typical}\n${sample}\n`;
  }
  return text;
}

export const SAMPLES = { sample, crlf, mixed, long: long() } as const;

export type SampleId = keyof typeof SAMPLES;

export const TASKS: readonly { key: string; title: string }[] = [
  { key: "TM-7", title: "Check the sync logs" },
  { key: "TM-9", title: "Write the release notes" },
  { key: "TM-12", title: "Board drag and drop" },
  { key: "TM-41", title: "Fix the sync bug" },
  { key: "TM-42", title: "Fix the export dialog" },
  { key: "TM-108", title: "Wiki page history" },
];

export const PAGES: readonly string[] = ["Release plan", "Roadmap", "Meeting notes 2026-11", "Onboarding", "Rust style guide"];
