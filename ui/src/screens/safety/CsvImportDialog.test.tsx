// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { FakeWorkspace } from "../../data/fake/engine";
import { seed } from "../../data/fake/seed";
import { type CsvImportResult, type CsvPresetChange } from "../../generated/ops";
import { createBundle } from "../../i18n/i18n";
import { presetColumns, requestOf, type CsvSettings } from "./CsvImportDialog";

const settings: CsvSettings = {
  project: "PER",
  preset: "",
  encoding: "auto",
  separator: "auto",
  dateOrder: "auto",
  columns: {},
  createMissing: true,
  skipInvalid: false,
};

describe("the CSV import's input", () => {
  it("leaves out what is detected and keeps what the person chose", () => {
    expect(requestOf(settings, true)).toEqual({ project: "PER", create_missing: true, skip_invalid: false, only_problems: true, dry_run: true });
    const chosen = requestOf(
      { ...settings, preset: "jira", encoding: "windows1252", separator: ";", dateOrder: "dmy", columns: { Points: { to: "field.Points" }, Spent: { to: "spent", unit: "hours" } } },
      false,
    );
    expect(chosen).toEqual({
      project: "PER",
      preset: "jira",
      format: { encoding: "windows1252", separator: ";", date_order: "dmy" },
      columns: [
        { column: "Points", to: "field.Points" },
        { column: "Spent", to: "spent", unit: "hours" },
      ],
      create_missing: true,
      skip_invalid: false,
      only_problems: true,
      dry_run: false,
    });
  });

  it("keeps a checked file's mapping as a preset, each column once", () => {
    expect(
      presetColumns([
        { index: 0, name: "Summary", to: "title" },
        { index: 1, name: "Labels", to: "labels" },
        { index: 2, name: "Labels", to: "labels" },
        { index: 3, name: "Assignee" },
        { index: 4, name: "Time Spent", to: "spent", unit: "seconds" },
      ]),
    ).toEqual([
      { column: "Summary", to: "title" },
      { column: "Labels", to: "labels" },
      { column: "Time Spent", to: "spent", unit: "seconds" },
    ]);
  });

  it("has every message in English", () => {
    const bundle = createBundle("en");
    for (const id of ["csv-title", "csv-target-blocked_by", "csv-unit-seconds", "csv-preset-jira", "csv-summary-links", "action-import-csv", "project-import-csv"]) {
      expect(bundle.hasMessage(id), id).toBe(true);
    }
  });
});

describe("the sample engine's CSV import", () => {
  it("checks the sample file, refuses other files, and imports with Undo", () => {
    const today = new Date(2026, 10, 23, 9, 0);
    const engine = new FakeWorkspace(seed(today), () => today);
    const project = (engine.call("project.list", {}).output as { items: { key: string }[] }).items[0]?.key ?? "";
    const check = engine.call("transfer.import_csv", { project, path: "tasks.csv", dry_run: true }).output as CsvImportResult;
    expect(check.preset).toBe("jira");
    expect(check.counts).toMatchObject({ rows: 3, created: 2, skipped: 1 });
    expect(check.blocked).toBe(true);
    expect(() => engine.call("transfer.import_csv", { project, path: "other.csv", dry_run: true })).toThrow();
    expect(() => engine.call("transfer.import_csv", { project, path: "tasks.csv" })).toThrow();
    const done = engine.call("transfer.import_csv", { project, path: "tasks.csv", skip_invalid: true });
    expect((done.output as CsvImportResult).counts.created).toBe(2);
    expect(done.changes?.group).toBeDefined();
    const saved = engine.call("csv_preset.save", { name: "Ours", columns: [{ column: "Summary", to: "title" }] }).output as CsvPresetChange;
    expect(saved.items.map((preset) => preset.name)).toEqual(["jira", "redmine", "Ours"]);
  });
});
