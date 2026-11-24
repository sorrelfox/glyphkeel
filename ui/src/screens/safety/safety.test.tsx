// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { daysLeft, declined, formatBytes } from "../../data/safety";
import { createClient, OpFailure } from "../../data/transport";
import { type ActivityEntry, type ActivityPage, type ActivityQuery, type TaskChange } from "../../generated/ops";
import { type Translate } from "../../i18n/I18n";
import { createBundle } from "../../i18n/i18n";
import { describeChange, feedRequest } from "./ActivityScreen";
import { keyChanges } from "./ImportDialog";

// jsdom lays nothing out; the editor measures text ranges.
Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
Range.prototype.getBoundingClientRect = () => new DOMRect();

afterEach(() => {
  cleanup();
  localStorage.clear();
});

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

const bundle = createBundle("en");
const t: Translate = (id, vars) => {
  const message = bundle.getMessage(id);
  return message?.value ? plain(bundle.formatPattern(message.value, vars)) : id;
};

function entry(fields: ActivityEntry["fields"], extra: Partial<ActivityEntry> = {}): ActivityEntry {
  return { actor: { kind: "user", name: "You" }, at: "2026-11-20T10:00:00Z", entity: "task", fields, group: "g-1", id: "t-1", op: "task.update", seq: 1, via: "app", ...extra };
}

describe("the activity feed's filters and words", () => {
  it("says what changed, on which item, and which fields", () => {
    const changed = describeChange(t, entry({ title: { before: "Old", after: "New" }, priority: { before: "low", after: "high" } }), "en-US");
    expect(changed.what).toBe("Task changed");
    expect(changed.item).toBe("New");
    expect(changed.fields).toBe("Title and Priority");
    const trashed = describeChange(t, entry({ trashed: { after: true }, key: { before: "PER-3" } }), "en-US");
    expect(trashed.what).toBe("Task moved to the trash");
    expect(trashed.item).toBe("PER-3");
    expect(describeChange(t, entry({ created: { after: true } }, { entity: "wiki_page" }), "en-US").what).toBe("Wiki page created");
    expect(describeChange(t, entry({}, { entity: "something_new" }), "en-US").what).toBe("Item changed");
    // Fields by their names; bookkeeping and fields the entry tells already are left out.
    expect(describeChange(t, entry({ closed_at: { after: 1 }, status: { before: "s-1", after: "s-2" } }), "en-US").fields).toBe("Status");
    expect(describeChange(t, entry({ claim: { after: { agent: "Helper" } }, status: { before: "s-1", after: "s-2" } }), "en-US").fields).toBe("Claim and Status");
    expect(describeChange(t, entry({ record: { before: {}, after: {} } }, { entity: "view" }), "en-US")).toEqual({ what: "Saved view changed", item: undefined, fields: undefined });
    expect(describeChange(t, entry({ timer: { after: {} } }, { entity: "timer" }), "en-US").fields).toBeUndefined();
    // A task the engine names stands for itself; the recorded title isn't repeated.
    const named = describeChange(t, entry({ title: { before: "Old", after: "New" } }, { task: { id: "t-1", key: "PER-1", title: "New" } }), "en-US");
    expect(named.item).toBeUndefined();
    expect(named.fields).toBe("Title");
    // A name the engine cut short says so.
    const long = "a".repeat(500);
    expect(describeChange(t, entry({ name: { after: long, truncated: true } }, { entity: "wiki_page" }), "en-US").item).toBe(`${long}…`);
  });
});

describe("helpers", () => {
  it("count the days left before a purge, today being the last", () => {
    const now = new Date("2026-11-24T12:00:00Z");
    expect(daysLeft("2026-11-24T18:00:00Z", now)).toBe(1);
    expect(daysLeft("2026-11-30T12:00:00Z", now)).toBe(6);
    expect(daysLeft("2026-11-20T12:00:00Z", now)).toBe(0);
  });

  it("format sizes for the locale", () => {
    expect(formatBytes(512, "en-US")).toBe("512 byte");
    expect(formatBytes(1_500_000, "en-US")).toBe("1.5 MB");
    expect(formatBytes(1_500_000, "de-DE")).toBe("1,5 MB");
  });

  it("tell a declined confirmation from a real refusal", () => {
    expect(declined(new OpFailure("trash.purge", { code: "forbidden", message: "not confirmed" }))).toBe(true);
    expect(declined(new OpFailure("trash.purge", { code: "forbidden", message: "x", details: { message_id: "trash-purge-all-agents" } }))).toBe(false);
    expect(declined(new OpFailure("trash.purge", { code: "invalid", message: "x" }))).toBe(false);
  });

  it("send new keys only for the taken keys that have one", () => {
    expect(keyChanges(["TM", "OPS"], { TM: " tm2 ", OPS: "" })).toEqual([{ from: "TM", to: "TM2" }]);
  });
});

/** The sample workspace with two fresh tasks in Personal. */
async function workspace() {
  const transport = fakeTransport();
  const create = (title: string) => transport.call("task.create", { project: "PER", title }) as Promise<TaskChange>;
  const first = (await create("Plan the trip")).task;
  const second = (await create("Book the train")).task;
  return { transport, first, second };
}

describe("the trash", () => {
  it("moves a task to the trash from the panel, with Undo", async () => {
    const { transport, first } = await workspace();
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/projects/PER?task=${first.key}`} />);
    const panel = await screen.findByRole("complementary", { name: (name) => plain(name).includes(`Task ${first.key}`) });
    await within(panel).findByRole("heading", { name: "Plan the trip" });
    fireEvent.click(within(panel).getByRole("button", { name: "Task menu" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Move to trash" }));
    const toast = await screen.findByText((text) => plain(text) === "Moved one task to the trash");
    expect(toast).toBeDefined();
    const trash = (await transport.call("trash.list", {})) as { items: { key?: string }[] };
    expect(trash.items.map((item) => item.key)).toEqual([first.key]);
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(async () => {
      expect(((await transport.call("trash.list", {})) as { total: number }).total).toBe(0);
    });
  });

  it("lists deleted tasks with days left, and restores the selected ones", async () => {
    const { transport, first, second } = await workspace();
    await transport.call("task.delete", { keys: [first.key, second.key] });
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/trash" />);
    const table = await screen.findByRole("table", { name: "Trash" });
    expect(within(table).getAllByText((text) => /^in \d+ days$/.test(plain(text))).length).toBe(2);
    fireEvent.click(within(table).getByRole("checkbox", { name: (name) => plain(name) === "Select Plan the trip" }));
    fireEvent.click(screen.getByRole("button", { name: "Restore" }));
    await screen.findByText((text) => plain(text) === "Restored one item");
    await waitFor(() => {
      expect(within(screen.getByRole("table", { name: "Trash" })).queryByText("Plan the trip")).toBeNull();
    });
    const tasks = (await transport.call("task.get", { keys: [first.key] })) as { items: unknown[] };
    expect(tasks.items.length).toBe(1);
  });
});

describe("backups in Preferences", () => {
  it("makes a backup now, lists it and checks it", async () => {
    const { transport } = await workspace();
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/" />);
    fireEvent.click(await screen.findByRole("button", { name: /Preferences/ }));
    fireEvent.click(await screen.findByRole("tab", { name: "Data and backups" }));
    expect((await screen.findByTestId("last-backup")).textContent).toBe("None yet");
    fireEvent.click(screen.getByRole("button", { name: "Back up now" }));
    const table = await screen.findByRole("table", { name: "Backups kept" });
    expect(within(table).getByText("Made by hand")).toBeDefined();
    fireEvent.click(within(table).getByRole("button", { name: "Check" }));
    await within(table).findByText((text) => plain(text).startsWith("Good"));
  });
});

describe("the activity screen", () => {
  // jsdom lays nothing out: the virtual list gets a window's height to fill.
  const height = Object.getOwnPropertyDescriptor(Element.prototype, "clientHeight");
  beforeAll(() => {
    Object.defineProperty(Element.prototype, "clientHeight", { configurable: true, get: () => 600 });
  });
  afterAll(() => {
    if (height) {
      Object.defineProperty(Element.prototype, "clientHeight", height);
    }
  });

  it("lists the workspace's changes and filters them to agents", async () => {
    const { transport, first } = await workspace();
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/activity" />);
    const table = await screen.findByRole("table", { name: "Activity" });
    expect((await within(table).findAllByText((text) => plain(text) === "Task created")).length).toBeGreaterThanOrEqual(2);
    // The task by its key and title, and no count of tasks in the status bar.
    expect(within(table).getAllByRole("button", { name: (name) => plain(name) === `${first.key}: Plan the trip` }).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByRole("contentinfo").textContent).not.toContain("tasks");
    fireEvent.click(screen.getByRole("radio", { name: "Agents" }));
    await screen.findByText("No changes match the filters.");
    expect(screen.queryByRole("table", { name: "Activity" })).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "People" }));
    expect((await within(await screen.findByRole("table", { name: "Activity" })).findAllByText((text) => plain(text) === "Task created")).length).toBeGreaterThanOrEqual(2);
  });
});

describe("the activity feed's request", () => {
  it("hands the filters to the engine, which counts sub-projects in and both days", async () => {
    const filter = { project: "p-1", actor: "agent" as const, from: "2026-11-20", to: "2026-11-21" };
    expect(feedRequest(filter)).toEqual({ actor_kind: "agent", project: "p-1", from: "2026-11-20", to: "2026-11-21" });
    expect(feedRequest({})).toEqual({});

    const transport = fakeTransport({ today: () => new Date(2026, 10, 20, 12) });
    for (const project of ["TM", "CLI", "PER"]) {
      await transport.call("task.create", { project, title: `In ${project}` });
    }
    const titles = async (query: ActivityQuery) =>
      ((await transport.call("history.feed", query)) as ActivityPage).items.map((item) => item.fields.title?.after);
    expect(await titles({ project: "TM" })).toEqual(["In CLI", "In TM"]);
    expect(await titles({ project: "CLI" })).toEqual(["In CLI"]);
    expect(await titles({ actor_kind: "agent" })).toEqual([]);
    expect((await titles({ actor_kind: "user", from: "2026-11-20", to: "2026-11-20" })).length).toBe(3);
    expect(await titles({ from: "2026-11-21" })).toEqual([]);
    expect(await titles({ to: "2026-11-19" })).toEqual([]);
    const page = (await transport.call("history.feed", { project: "TM", limit: 1 })) as ActivityPage;
    const next = (await transport.call("history.feed", { project: "TM", limit: 1, cursor: page.next_cursor ?? null })) as ActivityPage;
    expect([page.total, next.items[0]?.fields.title?.after, next.next_cursor]).toEqual([2, "In TM", undefined]);
    await expect(transport.call("history.feed", { from: "2026-11-21", to: "2026-11-20" })).rejects.toMatchObject({ code: "invalid", details: { path: "to" } });
  });
});
