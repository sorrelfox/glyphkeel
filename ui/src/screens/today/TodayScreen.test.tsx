// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type TaskChange, type Today } from "../../generated/ops";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

describe("Today", () => {
  it("lists an overdue task, counts it in the sidebar, and opens it beside the lists", async () => {
    const transport = fakeTransport();
    const task = ((await transport.call("task.create", { project: "PER", title: "Renew the passport" })) as TaskChange).task;
    await transport.call("task.update", { key: task.key, due_date: "yesterday" });
    const today = (await transport.call("today.get", {})) as Today;
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} />);

    const overdue = await screen.findByRole("list", { name: "Overdue" });
    const row = within(overdue).getByRole("button", { name: (name) => plain(name) === `${task.key} Renew the passport` });
    const sidebar = screen.getByRole("navigation", { name: "Workspace" });
    await waitFor(() => {
      expect(sidebar.querySelector("[data-count=today]")?.textContent).toBe(String(today.counts.today));
    });
    expect(sidebar.querySelector("[data-count=all]")?.textContent).toBe(String(today.counts.open));

    fireEvent.click(row);
    const panel = await screen.findByRole("complementary", { name: (name) => plain(name).startsWith(`Task ${task.key}`) });
    await within(panel).findByRole("heading", { name: "Renew the passport" });
  });

  it("reads dates typed in words in the task panel", async () => {
    const transport = fakeTransport();
    const task = ((await transport.call("task.create", { project: "PER", title: "Water the plants" })) as TaskChange).task;
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/?task=${task.key}`} />);
    const panel = await screen.findByRole("complementary", { name: (name) => plain(name).startsWith(`Task ${task.key}`) });
    fireEvent.click(await within(panel).findByRole("button", { name: (name) => plain(name).startsWith("Due") }));
    const words = within(panel).getByRole("textbox", { name: (name) => plain(name) === "Due in words" });
    fireEvent.change(words, { target: { value: "tomorrow" } });
    fireEvent.keyDown(words, { key: "Enter" });
    await waitFor(async () => {
      const after = (await transport.call("task.get", { keys: [task.key] })) as { items: { due_date?: string }[] };
      expect(after.items[0]?.due_date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    });
  });
});
