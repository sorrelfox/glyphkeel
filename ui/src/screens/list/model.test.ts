// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { changedFrom, clickRow, columnWidth, exportName, gridTemplate, listLines, moveColumn, nextSort, NO_SELECTION, removeRanges, sortState, viewColumns, withSort } from "./model";

describe("sorting by a column header", () => {
  it("goes ascending, descending, then back to the default order", () => {
    expect(nextSort([], "due")).toEqual(["due"]);
    expect(nextSort(["due"], "due")).toEqual(["-due"]);
    expect(nextSort(["-due"], "due")).toEqual([]);
    expect(nextSort(["-due", "priority"], "priority")).toEqual(["priority"]);
    expect(nextSort(["due"], "labels")).toEqual(["due"]);
  });

  it("adds a sort key with Shift, turns it round, then drops it, keeping the others", () => {
    expect(nextSort(["due"], "priority", true)).toEqual(["due", "priority"]);
    expect(nextSort(["due", "priority"], "priority", true)).toEqual(["due", "-priority"]);
    expect(nextSort(["due", "-priority"], "priority", true)).toEqual(["due"]);
    expect(nextSort(["due", "priority"], "due", true)).toEqual(["-due", "priority"]);
    expect(nextSort([], "due", true)).toEqual(["due"]);
    expect(nextSort(["due", "priority"], "title")).toEqual(["title"]);
  });

  it("shows every sort key on its header, with its place", () => {
    expect(sortState(["-due", "priority"], "due")).toEqual({ direction: "descending", position: 1 });
    expect(sortState(["-due", "priority"], "priority")).toEqual({ direction: "ascending", position: 2 });
    expect(sortState(["-due"], "priority")).toBeNull();
    expect(sortState(["status"], "labels")).toBeNull();
  });

  it("rewrites only the sort: of a query, keeping the rest as typed", () => {
    const text = "is:open  label:bug sort:due tail";
    const tokens = [{ kind: "sort" as const, range: { from: 19, to: 27 } }];
    expect(withSort(text, tokens, ["-priority"])).toBe("is:open  label:bug sort:-priority tail");
    expect(withSort(text, tokens, [])).toBe("is:open label:bug tail");
    expect(withSort("is:open", [], ["due"])).toBe("is:open sort:due");
    expect(withSort("", [], ["due"])).toBe("sort:due");
  });
});

describe("chips", () => {
  it("removing one takes out only its text", () => {
    const text = "status:Done -label:bug login";
    expect(removeRanges(text, [{ from: 12, to: 22 }])).toBe("status:Done login");
    expect(removeRanges(text, [{ from: 0, to: 11 }, { from: 23, to: 28 }])).toBe("-label:bug");
  });
});

describe("columns and views", () => {
  it("keeps known columns and always the title", () => {
    expect(viewColumns({ columns: ["due", "nonsense", "key"] }, false)).toEqual(["title", "due", "key"]);
    expect(viewColumns(undefined, true)).toContain("project");
    expect(viewColumns(undefined, false)).not.toContain("project");
    expect(gridTemplate(["key", "title"])).toBe("28px 76px minmax(240px, 1fr)");
  });

  it("opens a view without columns in the person's own columns", () => {
    expect(viewColumns({ columns: [] }, false, ["due", "key"])).toEqual(["title", "due", "key"]);
    expect(viewColumns({ columns: ["key"] }, false, ["due"])).toEqual(["title", "key"]);
    expect(viewColumns(undefined, false, ["nonsense"])).toContain("status");
  });

  it("keeps the widths the person chose and moves columns", () => {
    expect(gridTemplate(["key", "title", "due"], { title: 320, due: 140 })).toBe("28px 76px 320px 140px");
    expect(columnWidth("due", {})).toBe(92);
    expect(columnWidth("title", {})).toBe(240);
    expect(columnWidth("due", { due: 150 })).toBe(150);
    expect(moveColumn(["key", "title", "due"], "due", 0)).toEqual(["due", "key", "title"]);
    expect(moveColumn(["key", "title", "due"], "key", 9)).toEqual(["title", "due", "key"]);
    expect(moveColumn(["key", "title", "due"], "key", -1)).toEqual(["key", "title", "due"]);
  });

  it("knows when the list differs from its saved view", () => {
    const view = { id: "v", name: "Mine", kind: "list" as const, query: "is:open", is_default: false, rank: "1", created_at: "", updated_at: "", columns: ["key", "title"] };
    const base = ["key", "title"];
    expect(changedFrom(view, { query: " is:open ", sort: "", columns: ["key", "title"], groupBy: "" }, base)).toBe(false);
    expect(changedFrom(view, { query: "is:open", sort: "due", columns: ["key", "title"], groupBy: "" }, base)).toBe(true);
    expect(changedFrom(view, { query: "is:open", sort: "", columns: ["title"], groupBy: "" }, base)).toBe(true);
    expect(changedFrom(view, { query: "is:open", sort: "", columns: ["key", "title"], groupBy: "status" }, base)).toBe(true);
    expect(changedFrom({ ...view, group_by: "status" }, { query: "is:open", sort: "", columns: ["key", "title"], groupBy: "status" }, base)).toBe(false);
    expect(changedFrom(undefined, { query: "", sort: "", columns: [], groupBy: "" }, base)).toBe(false);
    expect(changedFrom(undefined, { query: "x", sort: "", columns: [], groupBy: "" }, base)).toBe(true);
    expect(changedFrom(undefined, { query: "", sort: "", columns: [], groupBy: "label" }, base)).toBe(true);
  });

  it("names exports after the view and the day", () => {
    expect(exportName("Open shed work", "2026-11-23", "csv")).toBe("open-shed-work-2026-11-23.csv");
    expect(exportName(undefined, "2026-11-23", "json")).toBe("tasks-2026-11-23.json");
    expect(exportName("///", "2026-11-23", "csv")).toBe("tasks-2026-11-23.csv");
  });
});

describe("groups of rows", () => {
  it("puts a header line before each group's rows", () => {
    const lines = listLines(5, [{ count: 2 }, { count: 3 }]);
    expect(lines.count).toBe(7);
    expect([0, 1, 2, 3, 4, 5, 6].map((line) => lines.at(line))).toEqual([
      { kind: "group", group: 0 },
      { kind: "row", row: 0 },
      { kind: "row", row: 1 },
      { kind: "group", group: 1 },
      { kind: "row", row: 2 },
      { kind: "row", row: 3 },
      { kind: "row", row: 4 },
    ]);
    expect(lines.lineOf(3)).toBe(5);
    expect(lines.rowsIn(3, 6)).toEqual({ first: 2, last: 4 });
    expect(lines.rowsIn(0, 0)).toEqual({ first: 0, last: 0 });
  });

  it("shows only the header of a collapsed group", () => {
    const lines = listLines(5, [{ count: 2 }, { count: 3 }], (group) => group === 0);
    expect(lines.count).toBe(5);
    expect(lines.at(1)).toEqual({ kind: "group", group: 1 });
    expect(lines.at(2)).toEqual({ kind: "row", row: 2 });
    expect(lines.lineOf(1)).toBeUndefined();
    expect(lines.lineOf(4)).toBe(4);
  });

  it("is one line per row without groups", () => {
    const lines = listLines(3, undefined);
    expect(lines.count).toBe(3);
    expect(lines.at(2)).toEqual({ kind: "row", row: 2 });
    expect(lines.at(3)).toBeUndefined();
    expect(lines.rowsIn(1, 2)).toEqual({ first: 1, last: 2 });
  });
});

describe("selection", () => {
  const keys = ["A-1", "A-2", "A-3", "A-4"];
  const keyAt = (index: number) => keys[index];

  it("selects one row, toggles with Ctrl and extends with Shift", () => {
    let selection = clickRow(NO_SELECTION, 1, "A-2", { toggle: false, range: false }, keyAt);
    expect([...selection.keys]).toEqual(["A-2"]);
    selection = clickRow(selection, 3, "A-4", { toggle: true, range: false }, keyAt);
    expect([...selection.keys].sort()).toEqual(["A-2", "A-4"]);
    selection = clickRow(selection, 3, "A-4", { toggle: true, range: false }, keyAt);
    expect([...selection.keys]).toEqual(["A-2"]);
    selection = clickRow(clickRow(NO_SELECTION, 0, "A-1", { toggle: false, range: false }, keyAt), 2, "A-3", { toggle: false, range: true }, keyAt);
    expect([...selection.keys]).toEqual(["A-1", "A-2", "A-3"]);
  });
});
