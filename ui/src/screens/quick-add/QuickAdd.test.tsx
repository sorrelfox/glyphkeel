// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { settings, workspaceInfo } from "../../data/fixtures";
import { fakeEngine, rejectWith } from "../../data/testing";
import { type ProjectSummary, type TaskChange, type TaskCreateInput, type TemplateApplied, type TemplateApplyInput } from "../../generated/ops";

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

function project(key: string, name: string): ProjectSummary {
  return { depth: 0, hue: "green", id: `p-${key}`, key, name, open_tasks: 0, open_tasks_with_subprojects: 0, path: name, state: "active" };
}

async function renderQuickAdd() {
  const created: TaskCreateInput[] = [];
  const applied: TemplateApplyInput[] = [];
  const engine = fakeEngine({
    "template.list": (input) =>
      Promise.resolve(input.project === "TM" ? { items: [{ id: "tpl-1", name: "Weekly report", kind: "task" as const, project: "TM", title: "Report", items: 1 }], total: 1 } : { items: [], total: 0 }),
    "template.apply": (input) => {
      if (input.due_date !== undefined && input.due_date !== null && !["fri", "friday"].includes(input.due_date)) {
        const text = input.due_date;
        return rejectWith({ code: "invalid", message: `"${text}" is not a date.`, details: { path: "due_date", message_id: "date-invalid", args: { text } } });
      }
      applied.push(input);
      return Promise.resolve({ task: { key: "TM-2", title: input.title ?? "Report" }, tasks: 1 } as TemplateApplied);
    },
    // The engine reads the date words for the chip: here only "fri".
    "date.parse": (input) => (input.text === "fri" ? Promise.resolve({ date: "2026-11-27" }) : rejectWith({ code: "invalid", message: "not a date", details: { path: "text", message_id: "date-invalid", args: { text: input.text } } })),
    "workspace.get": () => workspaceInfo(),
    "settings.get": () => settings(),
    "project.list": () => ({ items: [project("TM", "Task Manager"), project("PER", "Personal")], total: 2 }),
    // Like the engine, it reads only some dates.
    "task.create": (input) => {
      if (input.due_date !== undefined && input.due_date !== null && !["fri", "friday"].includes(input.due_date)) {
        const text = input.due_date;
        return rejectWith({ code: "invalid", message: `"${text}" is not a date.`, details: { path: "due_date", message_id: "date-invalid", args: { text } } });
      }
      created.push(input);
      return Promise.resolve({ task: { key: `${input.project}-1`, title: input.title } } as TaskChange);
    },
  });
  render(<App start={{ titleBar: "system", locale: "en-GB", quickAdd: true }} client={engine.client} />);
  const field = await screen.findByRole("textbox", { name: "New task" });
  await waitFor(() => {
    expect(projectField().textContent).toContain("Task Manager");
  });
  return { field, created, applied };
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
    // The day the engine reads in the words shows beside them.
    await waitFor(() => {
      expect(screen.getByTestId("quick-add-due").textContent).toMatch(/^Due \u2068?fri\u2069?: \u2068?Fri,? 27 Nov/);
    });
    expect(screen.getByTestId("quick-add-priority").textContent).toBe("High");
    expect(projectField().textContent).toContain("Personal");
    enter(field);
    await waitFor(() => {
      expect(created).toHaveLength(1);
    });
    expect(created[0]).toEqual({ project: "PER", title: "Renew domain", due_date: "fri", priority: "high" });
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

  it("leaves the date to the engine: a shorter one when it can't read the words, else its message", async () => {
    const { field, created } = await renderQuickAdd();
    type(field, "Plan the week end of the month");
    expect(screen.getByTestId("quick-add-due").textContent).toContain("end of the month");
    enter(field);
    // Refused here, and no shorter date is left, so nothing is added.
    expect((await screen.findByRole("alert")).textContent).toMatch(/end of the month\W* is not a date/);
    expect(created).toHaveLength(0);
    expect((field as HTMLInputElement).value).toBe("Plan the week end of the month");

    type(field, "Call the shop next friday");
    enter(field);
    await waitFor(() => {
      expect(created).toHaveLength(1);
    });
    expect(created[0]).toMatchObject({ title: "Call the shop next", due_date: "friday" });
  });

  it("keeps the typed due date and priority when a template is chosen", async () => {
    const { field, created, applied } = await renderQuickAdd();
    const picker = await screen.findByRole("button", { name: /Template/ });
    act(() => {
      fireEvent.click(picker);
    });
    act(() => {
      fireEvent.click(screen.getByRole("option", { name: "Weekly report" }));
    });
    type(field, "Report for the board next friday !urgent");
    expect(screen.getByTestId("quick-add-due").textContent).toContain("next friday");
    expect(screen.getByTestId("quick-add-priority").textContent).toBe("Urgent");
    enter(field);
    await waitFor(() => {
      expect(applied).toHaveLength(1);
    });
    expect(created).toHaveLength(0);
    // "next friday" isn't read here; the shorter date is, with the rest in the title.
    expect(applied[0]).toMatchObject({ template: "tpl-1", project: "TM", title: "Report for the board next", due_date: "friday", priority: "urgent" });
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
