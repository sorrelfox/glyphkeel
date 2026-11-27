// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient, type Transport } from "../../data/transport";
import { type TaskSearchInput, type TaskSearchResult } from "../../generated/ops";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

// jsdom lays nothing out: the list's scroller gets a screen's height, so its rows are drawn.
const sizes = ["clientHeight", "clientWidth"] as const;
const original = sizes.map((name) => Object.getOwnPropertyDescriptor(HTMLElement.prototype, name));
beforeAll(() => {
  for (const name of sizes) {
    Object.defineProperty(HTMLElement.prototype, name, {
      configurable: true,
      get(this: HTMLElement) {
        return this.hasAttribute("data-list-scroll") ? (name === "clientHeight" ? 640 : 1400) : 0;
      },
    });
  }
});
afterAll(() => {
  sizes.forEach((name, index) => {
    const descriptor = original[index];
    if (descriptor) {
      Object.defineProperty(HTMLElement.prototype, name, descriptor);
    }
  });
});

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");
const named = (pattern: RegExp) => (name: string) => pattern.test(plain(name));

/** The sample workspace, with every call the list makes recorded. */
function recorded() {
  const transport = fakeTransport({ today: () => new Date("2026-11-23T10:00:00Z") });
  const calls: { op: string; input: unknown }[] = [];
  const recording: Transport & { workspace: typeof transport.workspace } = {
    ...transport,
    call: (op, input) => {
      calls.push({ op, input: structuredClone(input) });
      return transport.call(op, input);
    },
  };
  const searches = () => calls.filter((each) => each.op === "task.search").map((each) => each.input as TaskSearchInput);
  return { transport: recording, calls, searches };
}

async function renderList(transport: Transport, path = "/projects/TM/list") {
  const view = render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={path} />);
  const grid = await screen.findByRole("grid", { name: "Tasks" });
  return { grid, view };
}

/** What the engine answers for a query of the list's project. */
function search(transport: Transport, input: TaskSearchInput): Promise<TaskSearchResult> {
  return transport.call("task.search", { project: "TM", limit: 200, ...input }) as Promise<TaskSearchResult>;
}

describe("the list's views", () => {
  it("runs its first query with the default view, before any other", async () => {
    const { transport, searches } = recorded();
    await transport.call("view.create", { name: "Done work", project: "TM", query: "status:Done", is_default: true });
    await renderList(transport);
    await screen.findByRole("button", { name: named(/^Saved view: .*Done work/) });
    const listQueries = searches().filter((input) => input.limit === 200);
    expect(listQueries[0]?.query).toBe("status:Done");
    expect(listQueries.every((input) => input.query === "status:Done")).toBe(true);
  });

  it("shows Filtered: X of Y while a filter hides tasks, and Clear filter shows them all", async () => {
    const { transport } = recorded();
    await transport.call("view.create", { name: "Done work", project: "TM", query: "status:Done", is_default: true });
    const done = (await search(transport, { query: "status:Done" })).total;
    const all = (await search(transport, { query: "" })).total;
    await renderList(transport);
    const banner = await screen.findByTestId("list-banner");
    expect(plain(banner.textContent)).toBe(`Filtered: ${String(done)} of ${String(all)} tasksClear filter`);
    expect(banner.getAttribute("role")).toBe("status");
    fireEvent.click(within(banner).getByRole("button", { name: "Clear filter" }));
    await waitFor(() => {
      expect(screen.queryByTestId("list-banner")).toBeNull();
    });
    await waitFor(() => {
      expect(plain(screen.getByTestId("list-totals").textContent)).toContain(`${String(all)} tasks`);
    });
  });

  it("shows each view's description in the view menu", async () => {
    const { transport } = recorded();
    await transport.call("view.create", { name: "Due soon", project: "TM", query: "due<+7d", description: "What needs doing this week" });
    await transport.call("view.create", { name: "Bugs", project: "TM", query: "type:Bug" });
    await renderList(transport);
    fireEvent.click(screen.getByRole("button", { name: named(/^Saved view: All tasks/) }));
    const menu = await screen.findByRole("menu");
    const due = within(menu).getByRole("menuitemradio", { name: /Due soon/ });
    expect(due.textContent).toContain("What needs doing this week");
    const describedBy = due.getAttribute("aria-describedby") ?? "";
    expect(describedBy.split(" ").map((id) => document.getElementById(id)?.textContent)).toContain("What needs doing this week");
    expect(within(menu).getByRole("menuitemradio", { name: /Bugs/ }).getAttribute("aria-describedby")).toBeNull();
  });
});

describe("the list's table", () => {
  it("adds up estimates and time spent over every matching task, in the totals row", async () => {
    const { transport } = recorded();
    const answer = await search(transport, { query: "" });
    await renderList(transport);
    const totals = await screen.findByTestId("list-totals");
    await waitFor(() => {
      expect(plain(totals.textContent)).toContain(`${String(answer.totals.count)} tasks, ${String(answer.totals.open)} open`);
    });
    const cells = within(totals).getAllByRole("gridcell");
    const header = [...document.querySelectorAll('[role="columnheader"]')].slice(1).map((cell) => cell.textContent);
    const estimate = cells[header.findIndex((text) => text.startsWith("Estimate"))]?.textContent ?? "";
    const spent = cells[header.findIndex((text) => text.startsWith("Spent"))]?.textContent ?? "";
    expect(answer.totals.estimate_minutes).toBeGreaterThan(0);
    expect(estimate).toMatch(/\d/);
    expect(spent).toMatch(/\d/);
  });

  it("sorts by a second column with Shift-click and shows each key's place", async () => {
    const { transport, searches } = recorded();
    const { grid } = await renderList(transport);
    fireEvent.click(within(grid).getByRole("button", { name: named(/^Sort by Due$/) }));
    await waitFor(() => {
      expect(searches().at(-1)?.sort).toBe("due");
    });
    fireEvent.click(within(grid).getByRole("button", { name: named(/^Sort by Priority$/) }), { shiftKey: true });
    await waitFor(() => {
      expect(searches().at(-1)?.sort).toBe("due,priority");
    });
    const priority = within(grid).getByRole("button", { name: named(/^Sort by Priority$/) });
    await waitFor(() => {
      expect(plain(priority.getAttribute("aria-description"))).toBe("Sort key 2, ascending. Shift-click adds a sort key.");
    });
    expect(priority.closest('[role="columnheader"]')?.getAttribute("aria-sort")).toBe("none");
    expect(within(grid).getByRole("button", { name: named(/^Sort by Due$/) }).closest('[role="columnheader"]')?.getAttribute("aria-sort")).toBe("ascending");
    // Shift-click again turns the second key round, a plain click makes one key again.
    fireEvent.click(priority, { shiftKey: true });
    await waitFor(() => {
      expect(searches().at(-1)?.sort).toBe("due,-priority");
    });
    fireEvent.click(within(grid).getByRole("button", { name: named(/^Sort by Title$/) }));
    await waitFor(() => {
      expect(searches().at(-1)?.sort).toBe("title");
    });
  });

  it("keeps column widths and order for the project across a restart", async () => {
    const { transport, calls } = recorded();
    const first = await renderList(transport);
    const header = () => [...first.grid.querySelectorAll<HTMLElement>("[data-column]")].map((cell) => cell.dataset.column);
    const before = header();
    expect(before.slice(0, 2)).toEqual(["key", "title"]);
    // A wider due date column, with the keyboard on its edge.
    const edge = within(first.grid).getByRole("separator", { name: named(/^Width of Due$/) });
    fireEvent.keyDown(edge, { key: "ArrowRight" });
    // The key column moves one place on, with Ctrl+Shift+Arrow on its header.
    fireEvent.keyDown(within(first.grid).getByRole("button", { name: named(/^Sort by Key$/) }), { key: "ArrowRight", ctrlKey: true, shiftKey: true });
    await waitFor(() => {
      expect(header().slice(0, 2)).toEqual(["title", "key"]);
    });
    const updates = calls.filter((each) => each.op === "list.layout.update").map((each) => each.input);
    await waitFor(() => {
      expect(updates.length).toBeGreaterThanOrEqual(2);
    });
    expect(updates).toContainEqual({ project: "TM", widths: { due: 108 } });
    expect(updates).toContainEqual({ project: "TM", columns: ["title", "key", ...before.slice(2)] });

    // The app starts again on the same workspace.
    first.view.unmount();
    const again = await renderList(transport);
    await waitFor(() => {
      expect([...again.grid.querySelectorAll<HTMLElement>("[data-column]")].map((cell) => cell.dataset.column).slice(0, 2)).toEqual(["title", "key"]);
    });
    const row = again.grid.querySelector<HTMLElement>('[role="row"][aria-rowindex="1"]');
    expect(row?.style.gridTemplateColumns).toContain("108px");
    expect(within(again.grid).getByRole("separator", { name: named(/^Width of Due$/) }).getAttribute("aria-valuenow")).toBe("108");
  });

  it("groups rows by status under headers with counts, and collapses a group", async () => {
    const { transport, searches } = recorded();
    const { grid } = await renderList(transport);
    fireEvent.click(screen.getByRole("button", { name: named(/^Group by: None/) }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Status" }));
    await waitFor(() => {
      expect(searches().at(-1)?.group_by).toBe("status");
    });
    const answer = await search(transport, { query: "", group_by: "status" });
    const groups = answer.groups ?? [];
    expect(groups.map((group) => group.value)).toEqual(["Backlog", "To do", "In progress", "Review", "Done"]);
    const backlog = await within(grid).findByRole("button", { name: named(/^Backlog/) });
    expect(backlog.getAttribute("aria-expanded")).toBe("true");
    expect(plain(backlog.textContent)).toBe(`Backlog${String(groups[0]?.count)}`);
    expect(screen.getByTestId("list-changed")).toBeDefined();
    // Collapsed, the next group's header follows at once.
    fireEvent.click(backlog);
    await waitFor(() => {
      expect(backlog.getAttribute("aria-expanded")).toBe("false");
    });
    const lines = [...grid.querySelectorAll<HTMLElement>('[role="row"]')].slice(1, 3);
    expect(lines.map((line) => line.dataset.group)).toEqual(["Backlog", "To do"]);
    // By due week, the week is named by its first day.
    fireEvent.click(screen.getByRole("button", { name: named(/^Group by: Status/) }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Due week" }));
    await waitFor(() => {
      expect(searches().at(-1)?.group_by).toBe("due_week");
    });
    await within(grid).findAllByRole("button", { name: named(/^Week of /) });
  });
});

describe("the groups of the list", () => {
  it("keeps the groups the person folded, per grouping, across a restart", async () => {
    const { transport, calls } = recorded();
    const first = await renderList(transport);
    fireEvent.click(screen.getByRole("button", { name: named(/^Group by: None/) }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Status" }));
    fireEvent.click(await within(first.grid).findByRole("button", { name: named(/^Backlog/) }));
    await waitFor(() => {
      expect(calls.filter((each) => each.op === "list.layout.update").map((each) => each.input)).toContainEqual({ project: "TM", collapsed: { status: ["Backlog"] } });
    });
    // Another grouping has its own folded groups: none yet.
    fireEvent.click(screen.getByRole("button", { name: named(/^Group by: Status/) }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Priority" }));
    const high = await within(first.grid).findByRole("button", { name: named(/^High/) });
    expect(high.getAttribute("aria-expanded")).toBe("true");

    // The app starts again on the same workspace: grouped by status, Backlog is still folded.
    first.view.unmount();
    const again = await renderList(transport);
    fireEvent.click(screen.getByRole("button", { name: named(/^Group by: None/) }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Status" }));
    const backlog = await within(again.grid).findByRole("button", { name: named(/^Backlog/) });
    expect(backlog.getAttribute("aria-expanded")).toBe("false");
    expect((await within(again.grid).findByRole("button", { name: named(/^To do/) })).getAttribute("aria-expanded")).toBe("true");
    // Kept with the person's layout of the list, not in the workspace's settings.
    const settings = (await transport.call("settings.get", {})) as object;
    expect(JSON.stringify(settings)).not.toContain("Backlog");
  });

  it("adds a task with a group's value from the line at the group's end", async () => {
    const { transport, calls } = recorded();
    const { grid } = await renderList(transport);
    fireEvent.click(screen.getByRole("button", { name: named(/^Group by: None/) }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Priority" }));
    const before = (await search(transport, { query: "priority:high" })).total;
    fireEvent.click(await within(grid).findByRole("button", { name: named(/^Add a task to High$/) }));
    const field = await within(grid).findByRole("textbox", { name: named(/^Title of the new task in High$/) });
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: "Check the release notes" } });
    fireEvent.keyDown(field, { key: "Enter" });
    await waitFor(() => {
      expect(calls.filter((each) => each.op === "task.create").map((each) => each.input)).toEqual([{ project: "TM", title: "Check the release notes", priority: "high" }]);
    });
    await waitFor(async () => {
      expect((await search(transport, { query: "priority:high" })).total).toBe(before + 1);
    });
    // The field stays, empty, for the next one; Escape closes it.
    await waitFor(() => {
      expect((field as HTMLInputElement).value).toBe("");
    });
    fireEvent.keyDown(field, { key: "Escape" });
    expect(within(grid).queryByRole("textbox", { name: named(/^Title of the new task in High$/) })).toBeNull();
  });
});

describe("saving a grouped view", () => {
  it("saves the grouping with the view and opens it grouped", async () => {
    const { transport, calls } = recorded();
    await renderList(transport);
    fireEvent.click(screen.getByRole("button", { name: named(/^Group by: None/) }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Priority" }));
    fireEvent.click(await screen.findByRole("button", { name: "Save as…" }));
    const dialog = await screen.findByRole("dialog", { name: "Save view" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Name" }), { target: { value: "By priority" } });
    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Save view" }));
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(calls.find((each) => each.op === "view.create")?.input).toMatchObject({ name: "By priority", group_by: "priority" });
    });
  });
});
