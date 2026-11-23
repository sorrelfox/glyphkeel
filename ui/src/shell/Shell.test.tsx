// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../App";
import { settings, workspaceInfo } from "../data/fixtures";
import { fakeEngine } from "../data/testing";
import { SIDEBAR_WIDTH } from "./Shell";

afterEach(cleanup);

async function renderShell() {
  const engine = fakeEngine({ "workspace.get": () => workspaceInfo(), "settings.get": () => settings() });
  const view = render(<App start={{ titleBar: "app", locale: "en-US" }} client={engine.client} path="/" />);
  await screen.findByRole("navigation", { name: "Workspace" });
  return view;
}

function press(key: string, init: KeyboardEventInit = {}, target: Element = document.body) {
  act(() => {
    fireEvent.keyDown(target, { key, ...init });
  });
}

describe("Shell", () => {
  it("hides and shows the sidebar with F9", async () => {
    await renderShell();
    expect(screen.queryByRole("navigation", { name: "Workspace" })).not.toBeNull();
    press("F9");
    expect(screen.queryByRole("navigation", { name: "Workspace" })).toBeNull();
    press("F9");
    expect(screen.queryByRole("navigation", { name: "Workspace" })).not.toBeNull();
  });

  it("switches between Today and All tasks from the sidebar", async () => {
    await renderShell();
    const sidebar = screen.getByRole("navigation", { name: "Workspace" });
    fireEvent.click(within(sidebar).getByRole("button", { name: "All tasks" }));
    expect(await screen.findByText("No tasks yet.")).toBeDefined();
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("All tasks");
    expect(within(sidebar).getByRole("button", { name: "All tasks" }).getAttribute("aria-current")).toBe("page");
  });

  it("opens the shortcut sheet with ?, listing the actions and their keys", async () => {
    await renderShell();
    press("?", { shiftKey: true });
    const sheet = await screen.findByRole("dialog");
    expect(within(sheet).getByRole("heading", { name: "Keyboard shortcuts" })).toBeDefined();
    expect(within(sheet).getByText("Show or hide the sidebar")).toBeDefined();
    expect(within(sheet).getByText("F9")).toBeDefined();
    expect(within(sheet).getByText("Ctrl K")).toBeDefined();
    expect(within(sheet).getByText("Start or stop the timer")).toBeDefined();
  });

  it("doesn't take single-key shortcuts from text fields", async () => {
    await renderShell();
    const field = document.body.appendChild(document.createElement("input"));
    press("?", { shiftKey: true }, field);
    expect(screen.queryByRole("dialog")).toBeNull();
    field.remove();
  });

  it("resizes the sidebar from the keyboard within its limits", async () => {
    await renderShell();
    const splitter = screen.getByRole("separator", { name: "Sidebar width" });
    expect(splitter.getAttribute("aria-valuenow")).toBe(String(SIDEBAR_WIDTH.regular));
    fireEvent.keyDown(splitter, { key: "ArrowRight" });
    expect(splitter.getAttribute("aria-valuenow")).toBe(String(SIDEBAR_WIDTH.regular + 8));
    fireEvent.keyDown(splitter, { key: "End" });
    expect(splitter.getAttribute("aria-valuenow")).toBe(String(SIDEBAR_WIDTH.max));
    fireEvent.keyDown(splitter, { key: "Home" });
    expect(splitter.getAttribute("aria-valuenow")).toBe(String(SIDEBAR_WIDTH.min));
  });

  it("moves the focus between the regions with F6", async () => {
    await renderShell();
    press("F6");
    expect(screen.getByRole("navigation", { name: "Workspace" }).contains(document.activeElement)).toBe(true);
    press("F6", {}, document.activeElement ?? document.body);
    expect(screen.getByRole("banner").contains(document.activeElement)).toBe(true);
    press("F6", { shiftKey: true }, document.activeElement ?? document.body);
    expect(screen.getByRole("navigation", { name: "Workspace" }).contains(document.activeElement)).toBe(true);
  });

  it("shows the registry's actions in the main menu, unavailable ones dimmed", async () => {
    await renderShell();
    fireEvent.click(screen.getByRole("button", { name: "Main menu" }));
    const menu = await screen.findByRole("menu");
    expect(within(menu).getByText("Keyboard shortcuts")).toBeDefined();
    expect(within(menu).getByRole("menuitem", { name: /Back up now/ }).getAttribute("aria-disabled")).toBe("true");
    expect(within(menu).getByRole("menuitem", { name: /Undo/ }).getAttribute("aria-disabled")).toBeNull();
  });
});
