// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { settings, workspaceInfo } from "../../data/fixtures";
import { parseNaturalDate } from "../../data/naturalDate";
import { fakeEngine } from "../../data/testing";
import { type ProjectSummary, type TaskChange, type TaskCreateInput } from "../../generated/ops";

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

function project(key: string, name: string): ProjectSummary {
  return { depth: 0, hue: "green", id: `p-${key}`, key, name, open_tasks: 0, open_tasks_with_subprojects: 0, path: name, state: "active" };
}

/** Today on this computer, as the page reads it. */
function today(): string {
  const now = new Date();
  return `${String(now.getFullYear())}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
}

async function renderQuickAdd() {
  const created: TaskCreateInput[] = [];
  const engine = fakeEngine({
    "workspace.get": () => workspaceInfo(),
    "settings.get": () => settings(),
    "project.list": () => ({ items: [project("TM", "Task Manager"), project("PER", "Personal")], total: 2 }),
    "task.create": (input) => {
      created.push(input);
      return { task: { key: `${input.project}-1`, title: input.title } } as TaskChange;
    },
  });
  render(<App start={{ titleBar: "system", locale: "en-GB", quickAdd: true }} client={engine.client} />);
  const field = await screen.findByRole("textbox", { name: "New task" });
  await waitFor(() => {
    expect(projectField().textContent).toContain("Task Manager");
  });
  return { field, created };
}

/** The project picker's button. */
function projectField(): HTMLElement {
  const button = document.querySelector<HTMLElement>("button[aria-haspopup='listbox']");
  if (!button) {
    throw new Error("no project picker yet");
  }
  return button;
}

function type(field: HTMLElement, text: string) {
  act(() => {
    fireEvent.change(field, { target: { value: text } });
  });
}

function enter(field: HTMLElement) {
  act(() => {
    fireEvent.keyDown(field, { key: "Enter" });
  });
}

describe("the quick-add window", () => {
  it("opens on its field, ready to type", async () => {
    const { field } = await renderQuickAdd();
    expect(document.activeElement).toBe(field);
  });

  it("reads the project, the priority and a due date from the line, and creates the task on Enter", async () => {
    const { field, created } = await renderQuickAdd();
    type(field, "Renew domain fri #Personal !high");
    expect(screen.getByTestId("quick-add-due").textContent).toMatch(/^Due /);
    expect(screen.getByTestId("quick-add-priority").textContent).toBe("High");
    expect(projectField().textContent).toContain("Personal");
    enter(field);
    await waitFor(() => {
      expect(created).toHaveLength(1);
    });
    expect(created[0]).toEqual({ project: "PER", title: "Renew domain", due_date: parseNaturalDate("fri", today()), priority: "high" });
    expect((await screen.findByRole("status")).textContent).toMatch(/PER-1.*added/);
    expect((field as HTMLInputElement).value).toBe("");
  });

  it("goes into the chosen project, and keeps a trailing date in the title when told to", async () => {
    const { field, created } = await renderQuickAdd();
    type(field, "Plan the trip next week");
    fireEvent.click(screen.getByRole("button", { name: /Keep .*next week.* in the title/ }));
    expect(screen.queryByTestId("quick-add-due")).toBeNull();
    enter(field);
    await waitFor(() => {
      expect(created).toHaveLength(1);
    });
    expect(created[0]).toEqual({ project: "TM", title: "Plan the trip next week" });
  });

  it("asks for a title instead of creating an empty task, and empties the field on Escape", async () => {
    const { field, created } = await renderQuickAdd();
    type(field, "#Personal");
    enter(field);
    expect((await screen.findByRole("alert")).textContent).toBe("Type a title first.");
    expect(created).toHaveLength(0);
    act(() => {
      fireEvent.keyDown(field, { key: "Escape" });
    });
    expect((field as HTMLInputElement).value).toBe("");
  });
});
