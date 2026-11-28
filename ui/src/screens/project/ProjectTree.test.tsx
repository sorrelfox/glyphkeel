// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const TODAY = new Date(2026, 10, 23, 9, 0);

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

async function renderTree() {
  const client = createClient(fakeTransport({ today: () => TODAY }));
  render(<App start={{ titleBar: "app", locale: "en-US" }} client={client} path="/" />);
  const tree = await screen.findByRole("treegrid", { name: "Projects" });
  await within(tree).findByRole("row", { name: "Personal" });
  return { client, tree };
}

/** The count on a project's row, with its accessible name and tooltip. */
function countOf(tree: HTMLElement, name: string): { value: number; label: string; title: string } {
  const element = within(tree).getByRole("row", { name }).querySelector<HTMLElement>("[data-count]");
  if (!element) {
    throw new Error(`no count on ${name}`);
  }
  return { value: Number(element.textContent), label: plain(element.getAttribute("aria-label")), title: plain(element.getAttribute("title")) };
}

describe("the sidebar's project counts", () => {
  it("count open tasks only, and follow a task that is closed, reopened, cancelled or trashed", async () => {
    const { client, tree } = await renderTree();
    const project = await client.call("project.get", { project: "PER" });
    const done = project.statuses.find((status) => status.category === "done");
    const todo = project.statuses.find((status) => status.category === "todo");
    expect(done).toBeDefined();
    expect(todo).toBeDefined();
    await client.call("status.create", { project: "PER", name: "Dropped", category: "cancelled" });
    const open = project.open_tasks;
    await waitFor(() => {
      expect(countOf(tree, "Personal").value).toBe(open);
    });
    expect(countOf(tree, "Personal").label).toBe(`${String(open)} open tasks`);
    expect(countOf(tree, "Personal").title).toBe(`${String(open)} open tasks`);

    const made = await client.call("task.create", { project: "PER", title: "Counted" });
    const key = made.task.key;
    await waitFor(() => {
      expect(countOf(tree, "Personal").value).toBe(open + 1);
    });

    // Closing lowers the count, reopening raises it.
    await client.call("task.move", { key, status: done?.id ?? "" });
    await waitFor(() => {
      expect(countOf(tree, "Personal").value).toBe(open);
    });
    await client.call("task.move", { key, status: todo?.id ?? "" });
    await waitFor(() => {
      expect(countOf(tree, "Personal").value).toBe(open + 1);
    });

    // Cancelled and trashed tasks are left out too.
    await client.call("task.move", { key, status: "Dropped" });
    await waitFor(() => {
      expect(countOf(tree, "Personal").value).toBe(open);
    });
    await client.call("task.move", { key, status: todo?.id ?? "" });
    await waitFor(() => {
      expect(countOf(tree, "Personal").value).toBe(open + 1);
    });
    await client.call("task.delete", { keys: [key] });
    await waitFor(() => {
      expect(countOf(tree, "Personal").value).toBe(open);
    });
  });

  it("names a single open task in the singular", async () => {
    const { client, tree } = await renderTree();
    await client.call("project.create", { name: "Garden", key: "GDN" });
    await client.call("task.create", { project: "GDN", title: "Plant the roses" });
    await waitFor(() => {
      expect(countOf(tree, "Garden")).toEqual({ value: 1, label: "1 open task", title: "1 open task" });
    });
  });
});
