// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { highlightPieces } from "../../data/search";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type TaskChange } from "../../generated/ops";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

describe("search", () => {
  it("cuts a text into marked matches", () => {
    expect(highlightPieces("Fix the sync bug", [{ from: 8, to: 12 }])).toEqual([
      { text: "Fix the ", hit: false },
      { text: "sync", hit: true },
      { text: " bug", hit: false },
    ]);
  });

  it("finds a comment from the sidebar's field, marks the match, and opens its task", async () => {
    const transport = fakeTransport();
    const task = ((await transport.call("task.create", { project: "PER", title: "Plan the trip" })) as TaskChange).task;
    await transport.call("comment.add", { key: task.key, body: "The ferry leaves at noon from pier four." });
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} />);

    const field = await screen.findByRole("searchbox", { name: "Search the workspace" });
    fireEvent.change(field, { target: { value: "ferry" } });
    fireEvent.keyDown(field, { key: "Enter" });

    const results = await screen.findByRole("list", { name: "Search results" });
    const hit = within(results).getByRole("button", { name: /ferry/ });
    expect(hit.dataset.hit).toBe("comment");
    expect(hit.querySelector("mark")?.textContent).toBe("ferry");
    expect(screen.getByRole("searchbox", { name: "Search" })).toHaveProperty("value", "ferry");

    fireEvent.click(hit);
    const panel = await screen.findByRole("complementary", { name: (name) => plain(name).startsWith(`Task ${task.key}`) });
    await within(panel).findByRole("heading", { name: "Plan the trip" });
  });

  it("filters by kind, and opens a wiki page read only", async () => {
    const transport = fakeTransport();
    await transport.call("wiki.create", { project: "PER", title: "Packing list", body: "# Packing list\n\n## Ferry\n\nTickets for the ferry." });
    const task = ((await transport.call("task.create", { project: "PER", title: "Ferry tickets" })) as TaskChange).task;
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/search?q=ferry" />);

    const results = await screen.findByRole("list", { name: "Search results" });
    expect(within(results).getByRole("button", { name: new RegExp(task.key) })).toBeDefined();
    fireEvent.click(screen.getByRole("radio", { name: "Wiki pages" }));
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: new RegExp(task.key) })).toBeNull();
    });
    const wikiOnly = await screen.findByRole("list", { name: "Search results" });

    fireEvent.click(within(wikiOnly).getByRole("button", { name: /Packing list/ }));
    const dialog = await screen.findByRole("dialog", { name: "Packing list" });
    await within(dialog).findByRole("heading", { name: "Ferry" });
  });
});
