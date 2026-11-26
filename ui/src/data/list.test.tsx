// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { type ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { type ListedTask, type SavedView } from "../generated/ops";
import { DataProvider, createQueryClient } from "./DataProvider";
import { fakeTransport } from "./fake/transport";
import { bulkInput, defaultView, LIST_PAGE, pagesFor, searchInput, toCsv, toJson, useListRows, useParsedQuery } from "./list";
import { createClient } from "./transport";

const TODAY = new Date("2026-11-23T10:00:00Z");

function wrapper(extraTasks = 0) {
  const transport = fakeTransport({ today: () => TODAY, extraTasks });
  const client = createClient(transport);
  const queryClient = createQueryClient();
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <DataProvider client={client} queryClient={queryClient}>
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </DataProvider>
  );
  return { transport, Wrapper };
}

describe("list paging", () => {
  it("asks for 200-row pages by offset, with the scope", () => {
    expect(searchInput({ query: "is:open", includeSubprojects: true, project: "TM", sort: "due" }, 2)).toEqual({
      query: "is:open",
      limit: LIST_PAGE,
      cursor: "400",
      project: "TM",
      include_subprojects: true,
      sort: "due",
    });
    expect(searchInput({ query: "", includeSubprojects: false }, 0)).toEqual({ query: "", limit: LIST_PAGE });
  });

  it("reads the first page always, and the pages under the rows in view", () => {
    expect(pagesFor(0, 30, 20_000)).toEqual([0]);
    expect(pagesFor(190, 230, 20_000)).toEqual([0, 1]);
    expect(pagesFor(10_010, 10_040, 20_000)).toEqual([0, 50]);
    expect(pagesFor(19_990, 20_050, 20_000)).toEqual([0, 99]);
    expect(pagesFor(0, 10, 0)).toEqual([0]);
  });

  it("loads a far page of a 20,000-task list when its rows come into view", async () => {
    const { Wrapper } = wrapper(20_000);
    const scope = { query: "", includeSubprojects: false };
    const { result, rerender } = renderHook(({ first, last }) => useListRows(scope, { first, last }), {
      wrapper: Wrapper,
      initialProps: { first: 0, last: 30 },
    });
    await waitFor(() => {
      expect(result.current.total).toBeGreaterThan(20_000);
    });
    expect(result.current.rowAt(0)).toBeDefined();
    expect(result.current.rowAt(15_000)).toBeUndefined();
    rerender({ first: 15_000, last: 15_030 });
    await waitFor(() => {
      expect(result.current.rowAt(15_000)).toBeDefined();
    });
    // Only the first page and the page in view are held.
    expect(result.current.loaded()).toHaveLength(2 * LIST_PAGE);
    expect(result.current.totals?.count).toBe(result.current.total);
  });

  it("keeps the last rows while a new query runs", async () => {
    const { Wrapper } = wrapper();
    const { result, rerender } = renderHook(({ query }) => useListRows({ query, includeSubprojects: false }, { first: 0, last: 30 }), {
      wrapper: Wrapper,
      initialProps: { query: "" },
    });
    await waitFor(() => {
      expect(result.current.total).toBeGreaterThan(0);
    });
    const before = result.current.total;
    rerender({ query: "is:closed" });
    expect(result.current.total).toBe(before);
    await waitFor(() => {
      expect(result.current.total).toBeLessThan(before ?? 0);
    });
  });
});

describe("the query's reading", () => {
  it("gives tokens, chips and the mistake with UTF-16 ranges", async () => {
    const { Wrapper } = wrapper();
    const { result, rerender } = renderHook(({ query }) => useParsedQuery(query), { wrapper: Wrapper, initialProps: { query: "😀 status:Doing" } });
    await waitFor(() => {
      expect(result.current.data?.valid).toBe(false);
    });
    // The emoji takes two UTF-16 units: "Doing" starts at 10.
    expect(result.current.data?.error).toMatchObject({ message_id: "query-unknown-status", range: { from: 10, to: 15 }, args: { value: "Doing" } });
    act(() => {
      rerender({ query: "status:Done -label:board" });
    });
    await waitFor(() => {
      expect(result.current.data?.valid).toBe(true);
    });
    expect(result.current.data?.chips?.map((chip) => [chip.field, chip.negated, chip.values])).toEqual([
      ["status", false, ["Done"]],
      ["label", true, ["board"]],
    ]);
  });
});

describe("saved views and bulk changes", () => {
  const view = (fields: Partial<SavedView>): SavedView => ({ id: "v", name: "v", kind: "list", query: "", is_default: false, rank: "1", created_at: "", updated_at: "", ...fields });

  it("opens with the project's default view, else the workspace's", () => {
    const views = [view({ id: "a", is_default: true }), view({ id: "b", project: "TM", is_default: true }), view({ id: "c", project: "TM" })];
    expect(defaultView(views, "TM")?.id).toBe("b");
    expect(defaultView(views, "PER")?.id).toBe("a");
    expect(defaultView(views, undefined)?.id).toBe("a");
    expect(defaultView([view({ id: "c" })], "TM")).toBeUndefined();
  });

  it("previews first, then changes with the previewed count", () => {
    expect(bulkInput(["TM-1"], { priority: "high" }, null)).toEqual({ keys: ["TM-1"], changes: { priority: "high" }, dry_run: true });
    expect(bulkInput(["TM-1"], { priority: "high" }, { count: 1 })).toEqual({ keys: ["TM-1"], changes: { priority: "high" }, dry_run: false, expected_count: 1 });
  });

  it("changes three tasks as one Undo step in the sample workspace", async () => {
    const transport = fakeTransport({ today: () => TODAY });
    const client = createClient(transport);
    const keys = ["TM-1", "TM-3", "TM-8"];
    const preview = await client.call("task.bulk_update", bulkInput(keys, { priority: "urgent" }, null));
    expect(preview.dry_run).toBe(true);
    const before = await client.call("task.get", { keys });
    expect(before.items.every((task) => task.priority === "urgent")).toBe(keys.length === preview.count - preview.changed);
    const done = await client.call("task.bulk_update", bulkInput(keys, { priority: "urgent" }, { count: preview.count }));
    expect(done.changed).toBe(preview.changed);
    expect((await client.call("task.get", { keys })).items.map((task) => task.priority)).toEqual(["urgent", "urgent", "urgent"]);
    await client.call("history.undo", {});
    expect((await client.call("task.get", { keys })).items.map((task) => task.priority)).toEqual(before.items.map((task) => task.priority));
  });
});

describe("export", () => {
  const task = (fields: Partial<ListedTask>): ListedTask => ({
    blocked: false,
    checklist: { done: 0, total: 0 },
    comment_count: 0,
    created_at: "2026-11-01T09:00:00Z",
    created_by: { kind: "user", name: "You" },
    file_count: 0,
    id: "t-1",
    key: "TM-1",
    labels: [],
    priority: "normal",
    progress_percent: 0,
    project: "TM",
    rank: "1",
    spent_minutes: 90,
    status: { id: "s", name: "To do", category: "todo" },
    subtask_count: { done: 0, total: 0 },
    title: "Plain",
    type: { id: "y", name: "Task", glyph: "task" },
    updated_at: "2026-11-02T09:00:00Z",
    version: 1,
    ...fields,
  });

  it("writes CSV with a header, quoting and minutes, in the workspace's units", () => {
    const csv = toCsv(
      [task({ title: 'Say "hi", then go', estimate_minutes: 600, labels: [{ id: "l", name: "a", hue: "red" }, { id: "m", name: "b", hue: "red" }] }), task({ key: "TM-2", title: "=SUM(A1)" })],
      { hoursPerDay: 8, daysPerWeek: 5 },
    ).split("\r\n");
    expect(csv[0]).toBe("key,title,project,status,type,priority,labels,parent,start,due,estimate,spent,remaining,estimate_minutes,spent_minutes,progress,created,updated,closed");
    expect(csv[1]).toBe('TM-1,"Say ""hi"", then go",TM,To do,Task,normal,"a, b",,,,1d 2h,1h 30m,,600,90,0,2026-11-01T09:00:00Z,2026-11-02T09:00:00Z,');
    // A formula is never handed to a spreadsheet as one.
    expect(csv[2]?.split(",")[1]).toBe("'=SUM(A1)");
    expect(csv[3]).toBe("");
  });

  it("adds the custom fields as columns of their own", () => {
    const csv = toCsv(
      [task({ fields: { points: 3, platforms: ["Linux", "macOS"], approved: true } }), task({ key: "TM-2", fields: { note: "=1+1" } })],
      { hoursPerDay: 8, daysPerWeek: 5 },
      ["cf.points", "cf.platforms", "cf.approved"],
    ).split("\r\n");
    expect(csv[0]?.endsWith(",closed,cf.points,cf.platforms,cf.approved,cf.note")).toBe(true);
    expect(csv[1]?.endsWith(",3,Linux; macOS,true,")).toBe(true);
    expect(csv[2]?.endsWith(",,,,'=1+1")).toBe(true);
  });

  it("writes JSON with the query and the engine's records", () => {
    const json = JSON.parse(toJson([task({})], { query: "is:open", includeSubprojects: false, project: "TM" })) as { query: string; project: string; total: number; items: ListedTask[] };
    expect(json).toMatchObject({ query: "is:open", project: "TM", total: 1 });
    expect(json.items[0]?.key).toBe("TM-1");
  });
});
