// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// CSV import in the in-memory engine. It reads no files: it knows one
// sample, "tasks.csv", shaped like a Jira export, and maps its columns as
// the engine does (the input's choices, over a preset's, over names).

import { type CsvColumn, type CsvColumnMapping, type CsvImportInput, type CsvPreset, type CsvProblem, type CsvRow } from "../../generated/ops";

/** The sample file's name in the exports folder. */
export const SAMPLE_CSV = "tasks.csv";

const HEADERS = ["Summary", "Issue key", "Issue Type", "Status", "Priority", "Due date", "Labels", "Original Estimate", "Assignee"];
const ROWS: readonly (readonly string[])[] = [
  ["Plan the import", "OLD-1", "Story", "In Progress", "High", "02/Feb/24 12:00 AM", "backend", "28800", "Alex"],
  ["Map the columns", "OLD-2", "Task", "To Do", "Medium", "soon", "", "7200", ""],
  ["", "OLD-3", "Bug", "Done", "Low", "", "", "", ""],
];

export const BUILT_IN_PRESETS: readonly CsvPreset[] = [
  {
    name: "jira",
    built_in: true,
    columns: [
      { column: "Summary", to: "title" },
      { column: "Issue key", to: "key" },
      { column: "Issue Type", to: "type" },
      { column: "Status", to: "status" },
      { column: "Priority", to: "priority" },
      { column: "Due date", to: "due_date" },
      { column: "Labels", to: "labels" },
      { column: "Fix Version/s", to: "fix_version" },
      { column: "Original Estimate", to: "estimate", unit: "seconds" },
    ],
  },
  { name: "redmine", built_in: true, columns: [{ column: "Subject", to: "title" }, { column: "#", to: "key" }, { column: "Tracker", to: "type" }, { column: "Status", to: "status" }] },
];

const BY_NAME: Readonly<Record<string, string>> = {
  summary: "title",
  title: "title",
  subject: "title",
  status: "status",
  priority: "priority",
  "due date": "due_date",
  labels: "labels",
  "fix version/s": "fix_version",
  "target version": "fix_version",
  "issue key": "key",
  "issue type": "type",
  "original estimate": "estimate",
};

const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/** A Jira date (`02/Feb/24 12:00 AM`) or `YYYY-MM-DD` as a day; null when it is neither. */
function dayOf(text: string): string | null {
  if (/^\d{4}-\d{2}-\d{2}$/.test(text)) {
    return text;
  }
  const match = /^(\d{1,2})\/([A-Za-z]{3})\/(\d{2})/.exec(text);
  const month = match ? MONTHS.indexOf((match[2] ?? "").toLowerCase()) : -1;
  if (!match || month < 0) {
    return null;
  }
  return `20${match[3] ?? ""}-${String(month + 1).padStart(2, "0")}-${(match[1] ?? "").padStart(2, "0")}`;
}

/** The planned rows of the sample, with the columns as mapped. */
export interface PlannedCsv {
  columns: CsvColumn[];
  preset: string | undefined;
  rows: { row: CsvRow; task: { title: string; status?: string; type?: string; due_date?: string; labels: string[]; estimate?: string } | null }[];
}

/** Reads the sample as `input` maps it; refuses any other file. */
export function planCsv(input: CsvImportInput, presets: readonly CsvPreset[], fail: (message: string) => never): PlannedCsv {
  if (input.path.split(/[/\\]/).at(-1) !== SAMPLE_CSV) {
    fail(`The sample workspace can import only its own file "${SAMPLE_CSV}".`);
  }
  const wanted = input.preset ?? "jira";
  const preset = presets.find((one) => one.name.toLowerCase() === wanted.toLowerCase());
  if (!preset) {
    fail(`There is no CSV import preset named "${wanted}".`);
  }
  const given = new Map((input.columns ?? []).map((choice: CsvColumnMapping) => [choice.column.toLowerCase(), choice]));
  const fromPreset = new Map((preset.columns ?? []).map((choice) => [choice.column.toLowerCase(), choice]));
  const columns: CsvColumn[] = HEADERS.map((name, index) => {
    const choice = given.get(name.toLowerCase()) ?? fromPreset.get(name.toLowerCase());
    const to = choice ? choice.to : (preset.columns ?? []).length === 0 ? BY_NAME[name.toLowerCase()] : undefined;
    const samples = ROWS.map((row) => row[index] ?? "").filter((value) => value !== "").slice(0, 3);
    return {
      index,
      name,
      ...(to === undefined || to === "ignore" ? {} : { to }),
      ...((to === "estimate" || to === "spent") ? { unit: choice?.unit ?? "text" } : {}),
      ...(samples.length > 0 ? { samples } : {}),
    };
  });
  const rows = ROWS.map((values, i) => {
    const problems: CsvProblem[] = [];
    const task: NonNullable<PlannedCsv["rows"][number]["task"]> = { title: "", labels: [] };
    let key: string | undefined;
    for (const column of columns) {
      const value = (values[column.index] ?? "").trim();
      if (value === "") {
        continue;
      }
      switch (column.to) {
        case "title":
          task.title = value;
          break;
        case "status":
          task.status = value;
          break;
        case "type":
          task.type = value;
          break;
        case "labels":
          task.labels.push(value);
          break;
        case "key":
          key = value;
          break;
        case "due_date": {
          const day = dayOf(value);
          if (day === null) {
            problems.push({ severity: "warning", column: column.name, code: "bad_date", message: `"${value}" is not a date; it is left out.` });
          } else {
            task.due_date = day;
          }
          break;
        }
        case "estimate": {
          const seconds = Number(value);
          const minutes = column.unit === "seconds" ? Math.round(seconds / 60) : column.unit === "hours" ? Math.round(seconds * 60) : Math.round(seconds);
          if (Number.isFinite(minutes) && minutes > 0) {
            task.estimate = `${String(minutes)}m`;
          }
          break;
        }
        default:
          break;
      }
    }
    if (task.title === "") {
      problems.push({ severity: "error", code: "no_title", message: "The row has no title." });
    }
    const failed = problems.some((problem) => problem.severity === "error");
    const row: CsvRow = { line: i + 2, outcome: failed ? "skipped" : "created", title: task.title, ...(key === undefined ? {} : { source_key: key }), ...(problems.length > 0 ? { problems } : {}) };
    return { row, task: failed ? null : task };
  });
  return { columns, preset: preset.name, rows };
}
