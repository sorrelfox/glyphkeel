// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { changedFrom, clickRow, exportName, gridTemplate, nextSort, NO_SELECTION, removeRanges, sortState, viewColumns, withSort } from "./model";

describe("sorting by a column header", () => {
  it("goes ascending, descending, then back to the default order", () => {
    expect(nextSort([], "due")).toEqual(["due"]);
    expect(nextSort(["due"], "due")).toEqual(["-due"]);
    expect(nextSort(["-due"], "due")).toEqual([]);
    expect(nextSort(["-due", "priority"], "priority")).toEqual(["priority"]);
    expect(nextSort(["due"], "labels")).toEqual(["due"]);
  });

  it("shows the first sort key on its header", () => {
    expect(sortState(["-due", "priority"], "due")).toBe("descending");
    expect(sortState(["-due", "priority"], "priority")).toBeNull();
    expect(sortState(["status"], "status")).toBe("ascending");
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

  it("knows when the list differs from its saved view", () => {
    const view = { id: "v", name: "Mine", kind: "list" as const, query: "is:open", is_default: false, rank: "1", created_at: "", updated_at: "", columns: ["key", "title"] };
    expect(changedFrom(view, { query: " is:open ", sort: "", columns: ["key", "title"] }, false)).toBe(false);
    expect(changedFrom(view, { query: "is:open", sort: "due", columns: ["key", "title"] }, false)).toBe(true);
    expect(changedFrom(view, { query: "is:open", sort: "", columns: ["title"] }, false)).toBe(true);
    expect(changedFrom(undefined, { query: "", sort: "", columns: [] }, false)).toBe(false);
    expect(changedFrom(undefined, { query: "x", sort: "", columns: [] }, false)).toBe(true);
  });

  it("names exports after the view and the day", () => {
    expect(exportName("Open shed work", "2026-11-23", "csv")).toBe("open-shed-work-2026-11-23.csv");
    expect(exportName(undefined, "2026-11-23", "json")).toBe("tasks-2026-11-23.json");
    expect(exportName("///", "2026-11-23", "csv")).toBe("tasks-2026-11-23.csv");
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
