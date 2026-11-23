// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The quick-add field's short syntax, read while the person types:
//
//   Renew the domain fri #Personal !high
//
// - `#Project`: the project, by key or by name without spaces (`#TM`,
//   `#Personal`, `#homelab` for "Home lab"); the last one counts.
// - `!urgent`, `!high`, `!normal`, `!low`, or `!1` (urgent) to `!4` (low).
// - A date at the end: `fri`, `tomorrow`, `next mon`, `in 2 weeks`,
//   `dec 1`, `end of month`, `2026-12-24` (the engine's friendly dates).
//
// Everything else is the title. The field shows what it recognised, and the
// person can tell it to keep a trailing date in the title instead.

import { parseNaturalDate } from "../../data/naturalDate";
import { type Priority, type WeekStart } from "../../generated/ops";

export interface ProjectChoice {
  key: string;
  name: string;
}

export interface QuickAddInput {
  title: string;
  /** The key of the project a `#` token names. */
  project?: string;
  /** A `#` token that names no project, as typed. */
  unknownProject?: string;
  /** The due date, `YYYY-MM-DD`. */
  due?: string;
  /** The words the due date was read from. */
  dueText?: string;
  priority?: Priority;
}

const PRIORITY_WORDS: Readonly<Record<string, Priority>> = {
  urgent: "urgent",
  high: "high",
  normal: "normal",
  low: "low",
  "1": "urgent",
  "2": "high",
  "3": "normal",
  "4": "low",
};

/** The longest date phrase the engine reads, in words (`end of the month`). */
const LONGEST_DATE = 4;

const compact = (text: string) => text.replace(/\s+/g, "").toLocaleLowerCase();

/** The project a `#` token names: by key, by name without spaces, or by the start of one name. */
export function projectFor(token: string, projects: readonly ProjectChoice[]): ProjectChoice | undefined {
  const wanted = compact(token);
  if (wanted === "") {
    return undefined;
  }
  const byKey = projects.find((project) => project.key.toLocaleLowerCase() === wanted);
  if (byKey) {
    return byKey;
  }
  const byName = projects.find((project) => compact(project.name) === wanted);
  if (byName) {
    return byName;
  }
  const starting = projects.filter((project) => compact(project.name).startsWith(wanted));
  return starting.length === 1 ? starting[0] : undefined;
}

interface Options {
  projects: readonly ProjectChoice[];
  /** Today's date in the person's time zone. */
  today: string;
  weekStart?: WeekStart;
  /** Keep a trailing date in the title. */
  keepDate?: boolean;
}

/** Reads the field. */
export function parseQuickAdd(text: string, { projects, today, weekStart = "monday", keepDate = false }: Options): QuickAddInput {
  const words: string[] = [];
  const result: QuickAddInput = { title: "" };
  for (const word of text.split(/\s+/).filter((each) => each !== "")) {
    if (word.length > 1 && word.startsWith("#")) {
      const project = projectFor(word.slice(1), projects);
      if (project) {
        result.project = project.key;
        delete result.unknownProject;
      } else {
        result.unknownProject = word.slice(1);
        delete result.project;
      }
      continue;
    }
    const priority = word.startsWith("!") ? PRIORITY_WORDS[word.slice(1).toLowerCase()] : undefined;
    if (priority) {
      result.priority = priority;
      continue;
    }
    words.push(word);
  }
  if (!keepDate) {
    // The longest date at the end, leaving at least one word of title.
    for (let count = Math.min(LONGEST_DATE, words.length - 1); count >= 1; count -= 1) {
      const phrase = words.slice(words.length - count).join(" ");
      const due = parseNaturalDate(phrase, today, weekStart);
      if (due !== null) {
        result.due = due;
        result.dueText = phrase;
        words.splice(words.length - count, count);
        break;
      }
    }
  }
  result.title = words.join(" ");
  return result;
}
