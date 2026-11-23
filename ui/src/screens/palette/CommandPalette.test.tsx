// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it } from "vitest";

import { App } from "../../App";
import { settings, workspaceInfo } from "../../data/fixtures";
import { fakeEngine } from "../../data/testing";
import { type ListedTask, type ProjectSummary, type TaskSearchResult } from "../../generated/ops";

beforeAll(() => {
  // jsdom has no layout; cmdk scrolls the chosen row into view and measures its list.
  Element.prototype.scrollIntoView = () => undefined;
  globalThis.ResizeObserver = class {
    observe() {
      return undefined;
    }
    unobserve() {
      return undefined;
    }
    disconnect() {
      return undefined;
    }
  };
});

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

function project(key: string, name: string): ProjectSummary {
  return { depth: 0, hue: "blue", id: `p-${key}`, key, name, open_tasks: 1, open_tasks_with_subprojects: 1, path: name, state: "active" };
}

function task(key: string, title: string): ListedTask {
  return {
    key,
    title,
    status: { id: "s1", name: "To do", category: "todo" },
    project: key.split("-")[0] ?? "",
  } as ListedTask;
}

function results(items: ListedTask[]): TaskSearchResult {
  return { items, query: "", sort: [], total: items.length, totals: { count: items.length, open: items.length, estimate_minutes: 0, spent_minutes: 0, remaining_minutes: 0 } };
}

async function renderApp() {
  const engine = fakeEngine({
    "workspace.get": () => workspaceInfo(),
    "settings.get": () => settings(),
    "project.list": () => ({ items: [project("TM", "Task Manager"), project("PER", "Personal")], total: 2 }),
    "task.search": ({ query }) => results(/timber|tm/i.test(query ?? "") ? [task("TM-3", "Buy timber for the shed")] : []),
    "task.get": ({ keys }) => ({ items: keys[0] === "TM-7" ? [task("TM-7", "Fix the login")] : [] }) as never,
  });
  render(<App start={{ titleBar: "app", locale: "en-US" }} client={engine.client} path="/" />);
  await screen.findByRole("navigation", { name: "Workspace" });
  return engine;
}

function openPalette() {
  act(() => {
    fireEvent.keyDown(document.body, { key: "k", ctrlKey: true });
  });
  return screen.findByRole("dialog", { name: "Command palette" });
}

function type(text: string) {
  const field = screen.getByRole("combobox", { name: "Search tasks, projects and actions" });
  act(() => {
    fireEvent.change(field, { target: { value: text } });
  });
  return field;
}

describe("the command palette", () => {
  it("opens with Ctrl+K and lists the common actions with their shortcuts", async () => {
    await renderApp();
    const palette = await openPalette();
    const actions = within(palette).getByRole("group", { name: "Actions" });
    expect(within(actions).getByRole("option", { name: /New task/ })).toBeDefined();
    expect(within(actions).getByRole("option", { name: /Show or hide the sidebar/ }).textContent).toContain("F9");
    // It never lists itself.
    expect(within(palette).queryByRole("option", { name: /Search or run a command/ })).toBeNull();
  });

  it("runs an action from the keyboard", async () => {
    await renderApp();
    await openPalette();
    const field = type("sidebar");
    act(() => {
      fireEvent.keyDown(field, { key: "Enter" });
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull();
    });
    await waitFor(() => {
      expect(screen.queryByRole("navigation", { name: "Workspace" })).toBeNull();
    });
  });

  it("searches tasks by words through the engine", async () => {
    const engine = await renderApp();
    const palette = await openPalette();
    type("timber");
    const tasks = await within(palette).findByRole("group", { name: "Tasks" });
    expect(within(tasks).getByRole("option", { name: /TM-3/ }).textContent).toContain("Buy timber for the shed");
    expect(engine.calls.some((call) => call.op === "task.search" && (call.input as { query: string }).query === "timber")).toBe(true);
  });

  it("puts the task of a typed key first, and a project's key offers the project", async () => {
    await renderApp();
    const palette = await openPalette();
    type("tm-7");
    const tasks = await within(palette).findByRole("group", { name: "Tasks" });
    await within(tasks).findByRole("option", { name: /TM-3/ });
    expect(within(tasks).getAllByRole("option").map((option) => option.textContent.slice(0, 4))).toEqual(["TM-7", "TM-3"]);
    type("per");
    const projects = await within(palette).findByRole("group", { name: "Projects" });
    expect(within(projects).getByRole("option", { name: /Go to \W*Personal/ })).toBeDefined();
  });

  it("cycles what it lists with Tab, and closes with Escape", async () => {
    await renderApp();
    const palette = await openPalette();
    const field = type("o");
    act(() => {
      fireEvent.keyDown(field, { key: "Tab" });
    });
    expect(within(palette).getByRole("button", { name: "Tasks" }).getAttribute("aria-pressed")).toBe("true");
    expect(within(palette).queryByRole("group", { name: "Actions" })).toBeNull();
    act(() => {
      fireEvent.keyDown(field, { key: "Escape" });
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull();
    });
  });
});
