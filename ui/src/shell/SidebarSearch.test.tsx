// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../App";
import { fakeTransport } from "../data/fake/transport";
import { createClient } from "../data/transport";
import { type TaskChange } from "../generated/ops";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

describe("the sidebar's search field", () => {
  it("suggests tasks while typing, searches everything first, and opens a chosen task", async () => {
    const transport = fakeTransport();
    const task = ((await transport.call("task.create", { project: "PER", title: "Book the ferry" })) as TaskChange).task;
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} />);

    const field = await screen.findByRole("combobox", { name: "Search the workspace" });
    fireEvent.focus(field);
    fireEvent.change(field, { target: { value: "ferry" } });
    const list = await screen.findByRole("listbox", { name: "Suggestions" });
    const first = within(list).getAllByRole("option")[0];
    expect(plain(first?.textContent)).toContain("Search everything for “ferry”");
    expect(first?.getAttribute("aria-selected")).toBe("true");

    const suggested = await within(list).findByRole("option", { name: new RegExp(task.key) });
    expect(suggested.querySelector("mark")?.textContent).toBe("ferry");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(field.getAttribute("aria-activedescendant")).toBe(suggested.id);
    fireEvent.keyDown(field, { key: "Enter" });

    const panel = await screen.findByRole("complementary", { name: (name) => plain(name).startsWith(`Task ${task.key}`) });
    await within(panel).findByRole("heading", { name: "Book the ferry" });
    expect(field).toHaveProperty("value", "");
  });

  it("empties itself on Escape", async () => {
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(fakeTransport())} />);
    const field = await screen.findByRole("combobox", { name: "Search the workspace" });
    fireEvent.focus(field);
    fireEvent.change(field, { target: { value: "anything" } });
    expect(screen.getByRole("listbox", { name: "Suggestions" })).toBeTruthy();
    fireEvent.keyDown(field, { key: "Escape" });
    expect(field).toHaveProperty("value", "");
    expect(screen.queryByRole("listbox", { name: "Suggestions" })).toBeNull();
  });
});
