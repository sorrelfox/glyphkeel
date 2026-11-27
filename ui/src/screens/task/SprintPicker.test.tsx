// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { forgetDraftsInMemory } from "../../data/drafts";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type TaskChange, type TaskList } from "../../generated/ops";

// jsdom lays nothing out; the description editor measures text ranges.
Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
Range.prototype.getBoundingClientRect = () => new DOMRect();
Element.prototype.scrollIntoView = () => undefined;

afterEach(() => {
  cleanup();
  forgetDraftsInMemory();
  localStorage.clear();
});

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");
const named = (pattern: RegExp) => (name: string) => pattern.test(plain(name));

/**
 * The sample workspace's Task Manager (Sprint 1 completed, Sprint 2 running,
 * Sprint 3 planned) with a new task in its backlog, open in the panel.
 */
async function renderPanel(title = "Sort the sprint") {
  const transport = fakeTransport();
  const task = ((await transport.call("task.create", { project: "TM", title })) as TaskChange).task;
  render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/projects/TM?task=${task.key}`} />);
  const panel = await screen.findByRole("complementary", { name: named(new RegExp(`Task ${task.key}`)) });
  await within(panel).findByRole("heading", { name: title });
  const sprintOf = async () => ((await transport.call("task.get", { keys: [task.key] })) as TaskList).items[0]?.sprint?.name;
  return { transport, task, panel, sprintOf };
}

/** Whether a menu entry shows the check mark of the current choice. */
const isChecked = (item: HTMLElement) => item.querySelector(".menuCheck")?.textContent === "✓";

const picker = (panel: HTMLElement, value: string) => within(panel).getByRole("button", { name: named(new RegExp(`^Sprint: ${value}$`)) });

describe("the task panel's sprint picker", () => {
  it("offers the running and planned sprints, puts the task in one and back in the backlog", async () => {
    const { panel, sprintOf } = await renderPanel();
    fireEvent.click(picker(panel, "Backlog"));
    const menu = await screen.findByRole("menu");
    // The running sprint says so; the completed one isn't offered.
    await within(menu).findByRole("menuitem", { name: named(/^Sprint 2 \(active\)$/) });
    const names = within(menu)
      .getAllByRole("menuitem")
      .map((item) => plain(item.textContent));
    expect(names.some((name) => name.includes("Sprint 3"))).toBe(true);
    expect(names.some((name) => name.includes("Sprint 1"))).toBe(false);
    // Already in the backlog: that choice is checked and can't be chosen.
    const backlog = within(menu).getByRole("menuitem", { name: "Backlog" });
    expect(isChecked(backlog)).toBe(true);
    expect(backlog.getAttribute("aria-disabled")).toBe("true");

    fireEvent.click(within(menu).getByRole("menuitem", { name: named(/^Sprint 3$/) }));
    await waitFor(() => {
      expect(picker(panel, "Sprint 3")).toBeTruthy();
    });
    expect(await sprintOf()).toBe("Sprint 3");

    // Moved to the running sprint, the value says it is active.
    fireEvent.click(picker(panel, "Sprint 3"));
    const again = await screen.findByRole("menu");
    const current = await within(again).findByRole("menuitem", { name: named(/^Sprint 3$/) });
    expect(isChecked(current)).toBe(true);
    fireEvent.click(within(again).getByRole("menuitem", { name: named(/^Sprint 2 \(active\)$/) }));
    await waitFor(() => {
      expect(within(picker(panel, "Sprint 2")).getByText("Active")).toBeTruthy();
    });
    expect(await sprintOf()).toBe("Sprint 2");

    // And back to the backlog.
    fireEvent.click(picker(panel, "Sprint 2"));
    const last = await screen.findByRole("menu");
    fireEvent.click(within(last).getByRole("menuitem", { name: "Backlog" }));
    await waitFor(() => {
      expect(picker(panel, "Backlog")).toBeTruthy();
    });
    expect(await sprintOf()).toBeUndefined();
  });

  it("says when the project has no running or planned sprint", async () => {
    const transport = fakeTransport();
    const task = ((await transport.call("task.create", { project: "PER", title: "No sprints here" })) as TaskChange).task;
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/projects/PER?task=${task.key}`} />);
    const panel = await screen.findByRole("complementary", { name: named(new RegExp(`Task ${task.key}`)) });
    await within(panel).findByRole("heading", { name: "No sprints here" });
    fireEvent.click(picker(panel, "Backlog"));
    const menu = await screen.findByRole("menu");
    const none = await within(menu).findByRole("menuitem", { name: "No planned or active sprints" });
    expect(none.getAttribute("aria-disabled")).toBe("true");
  });
});
