// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type Board } from "../../generated/ops";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

function firstOpenCard(transport: ReturnType<typeof fakeTransport>): string {
  const board = transport.workspace.call("board.get", { project: "TM" }).output as Board;
  const card = board.columns.find((column) => column.category === "todo")?.cards[0];
  if (!card) {
    throw new Error("no open card");
  }
  return card.key;
}

describe("marks on the board and in the sidebar", () => {
  it("a recurring task's card carries the recurring mark", async () => {
    const transport = fakeTransport();
    const key = firstOpenCard(transport);
    transport.workspace.call("recurrence.set", { key, text: "every week on mon" });
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/projects/TM" />);
    await screen.findByRole("group", { name: /Board of/ });
    const card = await waitFor(() => {
      const found = document.querySelector(`[data-card="${key}"]`);
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(card.querySelector("[data-recurring]")?.getAttribute("aria-label")).toBe("Recurring task");
    expect(document.querySelectorAll("[data-card] [data-recurring]")).toHaveLength(1);
  });

  it("the timer chip shows the kind of work the time is for", async () => {
    const transport = fakeTransport();
    const key = firstOpenCard(transport);
    transport.workspace.call("timer.start", { key, work_kind: "Design" });
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/projects/TM" />);
    const chip = await screen.findByRole("group", { name: "Timer" });
    expect(chip.querySelector("[data-timer-kind]")?.textContent).toBe("Design");
    const open = chip.querySelector("button");
    expect(plain(open?.getAttribute("aria-label"))).toMatch(new RegExp(`on ${key}: .*, as Design\\. Open the task$`));
  });
});
