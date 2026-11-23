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
//
// The page only sees that the last words have the shape of a date; which
// day they mean is the engine's answer, when the task is created, so that
// quick add, the task panel, the CLI and agents read dates the same way.

import { type Priority } from "../../generated/ops";

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
  /** The words at the end that look like a due date, for the engine to read. */
  dueText?: string;
  /**
   * The title and date for a shorter date, longest first, should the
   * engine not read `dueText` (`week end of month` → `end of month`).
   */
  shorterDates?: { title: string; dueText: string }[];
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

/** A name or its start from three letters on, as the engine reads them: `fri`, `frid`, `friday`. */
const shortened = (names: readonly string[]) => `(?:${names.map((name) => name.slice(0, 3) + Array.from(name.slice(3)).reduceRight((rest, letter) => `(?:${letter}${rest})?`, "")).join("|")})`;

const WEEKDAY = shortened(["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]);
const MONTH = `${shortened(["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"])}\\.?`;
const UNIT = "(?:days?|d|weeks?|w|months?|m|years?|y)";
const PERIOD = "(?:week|month|year)";

/** The shapes of the engine's friendly dates; which day they mean is the engine's to say. */
const DATE_SHAPES = [
  "today|tomorrow|yesterday",
  WEEKDAY,
  "\\d{4}-\\d{1,2}-\\d{1,2}",
  `[+-]\\d+${UNIT}`,
  `(?:this|last|next)-${PERIOD}`,
  `(?:this|next) (?:${WEEKDAY}|${PERIOD})`,
  `in \\d+${UNIT}`,
  `in (?:\\d+|a|an|one) ${UNIT}`,
  `end of (?:the )?${PERIOD}`,
  `${MONTH} \\d{1,2}(?:st|nd|rd|th)?,?(?: \\d{4})?`,
  `\\d{1,2}(?:st|nd|rd|th)? ${MONTH},?(?: \\d{4})?`,
].map((shape) => new RegExp(`^(?:${shape})$`, "i"));

/** Whether `phrase` has the shape of a friendly date. */
export function looksLikeDate(phrase: string): boolean {
  return DATE_SHAPES.some((shape) => shape.test(phrase));
}

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
  /** Keep a trailing date in the title. */
  keepDate?: boolean;
}

/** Reads the field. */
export function parseQuickAdd(text: string, { projects, keepDate = false }: Options): QuickAddInput {
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
    const dates: { title: string; dueText: string }[] = [];
    for (let count = Math.min(LONGEST_DATE, words.length - 1); count >= 1; count -= 1) {
      const phrase = words.slice(words.length - count).join(" ");
      if (looksLikeDate(phrase)) {
        dates.push({ title: words.slice(0, words.length - count).join(" "), dueText: phrase });
      }
    }
    const [longest, ...shorter] = dates;
    if (longest) {
      result.title = longest.title;
      result.dueText = longest.dueText;
      if (shorter.length > 0) {
        result.shorterDates = shorter;
      }
      return result;
    }
  }
  result.title = words.join(" ");
  return result;
}
