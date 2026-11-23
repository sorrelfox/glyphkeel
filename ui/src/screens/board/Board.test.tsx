// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

async function renderBoard(path = "/projects/TM") {
  const transport = fakeTransport();
  render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={path} />);
  await screen.findByRole("group", { name: /Board of/ });
  return transport;
}

function lane(name: string): HTMLElement {
  const found = document.querySelector<HTMLElement>(`section[data-lane][aria-label^="⁨${name}⁩"]`);
  if (!found) {
    throw new Error(`no lane ${name}`);
  }
  return found;
}

const cardsIn = (name: string) => [...lane(name).querySelectorAll("[data-card]")].map((card) => card.getAttribute("data-card"));
const status = () => plain(screen.getByRole("contentinfo").textContent);

describe("the board", () => {
  it("shows one column per status with its count, and the WIP limit over its limit", async () => {
    await renderBoard();
    const names = [...document.querySelectorAll("section[data-lane] h2")].map((h) => h.textContent);
    expect(names).toEqual(["Backlog", "To do", "In progress", "Review", "Done"]);
    const count = within(lane("In progress")).getByTestId("lane-count");
    expect(plain(count.textContent)).toMatch(/^\d+ \/ 3$/);
    expect(count.className).toContain("over");
    expect(status()).toMatch(/80 tasks · \d+ open/);
  });

  it("filters with a quick filter and shows the banner until cleared", async () => {
    await renderBoard();
    fireEvent.click(screen.getByRole("button", { name: "Recently updated" }));
    const banner = await screen.findByText(/Filtered:/);
    expect(plain(banner.textContent)).toMatch(/Filtered: \d+ of 80 tasks/);
    fireEvent.click(screen.getByRole("button", { name: "Clear filter" }));
    await waitFor(() => {
      expect(screen.queryByText(/Filtered:/)).toBeNull();
    });
  });

  it("hides a column into the chip and brings it back with Reset to project layout", async () => {
    await renderBoard();
    fireEvent.click(within(lane("Review")).getByRole("button", { name: /column menu/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Hide" }));
    const chip = await screen.findByRole("button", { name: (name) => /^1 hidden column · \d+ tasks/.test(plain(name)) });
    expect(document.querySelector('section[data-lane][aria-label^="⁨Review"]')).toBeNull();
    fireEvent.click(chip);
    fireEvent.click(await screen.findByRole("menuitem", { name: "Reset to project layout" }));
    await waitFor(() => {
      expect(lane("Review")).toBeDefined();
    });
  });

  it("adds a task from a column's Add task field", async () => {
    await renderBoard();
    fireEvent.click(within(lane("Backlog")).getByRole("button", { name: /Add task to/ }));
    const field = await within(lane("Backlog")).findByRole("textbox");
    fireEvent.change(field, { target: { value: "Write the release notes" } });
    fireEvent.keyDown(field, { key: "Enter" });
    await waitFor(() => {
      expect(cardsIn("Backlog")).toContain("TM-81");
    });
    expect(status()).toContain("TM-81 added to Backlog");
  });

  it("moves the selected card with Alt and the arrow keys, and says so in the status bar", async () => {
    await renderBoard();
    const key = cardsIn("To do")[0];
    if (!key) {
      throw new Error("no card in To do");
    }
    const card = document.querySelector<HTMLElement>(`[data-card="${key}"]`);
    act(() => {
      card?.focus();
    });
    fireEvent.keyDown(card ?? document.body, { key: "ArrowRight", altKey: true });
    await waitFor(() => {
      expect(cardsIn("In progress")).toContain(key);
    });
    expect(status()).toContain(`${key} moved to`);
    expect(status()).toContain("In progress");
  });

  it("starts and stops the timer with T on the selected card, and not from a text field", async () => {
    const transport = await renderBoard();
    const key = cardsIn("To do")[0] ?? "";
    const card = document.querySelector<HTMLElement>(`[data-card="${key}"]`);
    act(() => {
      card?.focus();
    });
    const field = document.body.appendChild(document.createElement("input"));
    fireEvent.keyDown(field, { key: "t" });
    field.remove();
    expect(transport.workspace.call("timer.get", {}).output).toEqual({});
    fireEvent.keyDown(card ?? document.body, { key: "t" });
    const chip = await screen.findByRole("group", { name: "Timer" });
    expect(chip.getAttribute("data-timer-chip")).toBe(key);
    fireEvent.keyDown(card ?? document.body, { key: "t" });
    await waitFor(() => {
      expect(screen.queryByRole("group", { name: "Timer" })).toBeNull();
    });
    expect(transport.workspace.call("timer.get", {}).output).toEqual({});
  });

  it("opens the task panel with Enter and changes the priority there", async () => {
    await renderBoard();
    const key = cardsIn("To do")[0] ?? "";
    const card = document.querySelector<HTMLElement>(`[data-card="${key}"]`);
    fireEvent.keyDown(card ?? document.body, { key: "Enter" });
    const panel = await screen.findByRole("complementary", { name: new RegExp(`Task.*${key}`) });
    fireEvent.click(await within(panel).findByRole("button", { name: /^Priority:/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /Urgent/ }));
    await waitFor(() => {
      expect(plain(within(panel).getByRole("button", { name: /^Priority:/ }).getAttribute("aria-label"))).toBe("Priority: Urgent");
    });
    await waitFor(() => {
      expect(document.querySelector(`[data-card="${key}"] svg[aria-label="Urgent"]`)).not.toBeNull();
    });
    fireEvent.click(within(panel).getByRole("button", { name: "Close panel" }));
    await waitFor(() => {
      expect(screen.queryByRole("complementary")).toBeNull();
    });
  });

  it("includes sub-projects: their cards, and a column for a status only a sub-project has", async () => {
    await renderBoard();
    fireEvent.click(screen.getByRole("switch", { name: "Include sub-projects" }));
    await waitFor(() => {
      expect([...document.querySelectorAll("section[data-lane] h2")].map((h) => h.textContent)).toContain("Waiting");
    });
    expect(document.querySelector('[data-card^="CLI-"]')).not.toBeNull();
  });
});

describe("the project tree", () => {
  it("lists projects as a tree and rolls a closed parent's count up", async () => {
    await renderBoard();
    const tree = screen.getByRole("treegrid", { name: "Projects" });
    const tm = within(tree).getByRole("row", { name: /Task Manager/ });
    const own = plain(tm.querySelector(".count")?.textContent);
    expect(within(tree).getByRole("row", { name: /CLI and MCP/ })).toBeDefined();
    fireEvent.click(within(tm).getByRole("button", { name: /Collapse/ }));
    await waitFor(() => {
      expect(within(tree).queryByRole("row", { name: /CLI and MCP/ })).toBeNull();
    });
    const rolled = plain(within(tree).getByRole("row", { name: /Task Manager/ }).querySelector(".count")?.textContent);
    expect(Number(rolled)).toBeGreaterThan(Number(own));
    expect(localStorage.getItem("glyphkeel.sidebar.collapsed")).toBe('["TM"]');
  });
});
