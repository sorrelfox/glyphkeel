// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type TaskChange } from "../../generated/ops";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

/** A 1×1 PNG. */
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

describe("card colours and covers on the board", () => {
  it("names the coloured edge in words and gives the cover its file name", async () => {
    const transport = fakeTransport();
    const ws = transport.workspace;
    const key = (ws.call("task.create", { project: "TM", title: "Mock-up", priority: "urgent" }).output as TaskChange).task.key;
    ws.call("file.attach", { task: key, name: "mock-up.png", data_base64: PNG });
    ws.call("layout.update", { project: "TM", colour_by: "priority", covers: "first_image" });
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/projects/TM" />);
    await screen.findByRole("group", { name: /Board of/ });
    const card = await waitFor(() => {
      const found = document.querySelector<HTMLElement>(`[data-card="${key}"]`);
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(card.dataset.edge).toBe("red");
    expect(plain(card.querySelector("[title]")?.getAttribute("title"))).toBe("Colour: priority Urgent");
    // The words are part of the card's text, so screen readers hear them too.
    expect(plain(card.textContent)).toContain("Colour: priority Urgent");
    const image = await waitFor(() => {
      const found = card.querySelector("img");
      expect(found).not.toBeNull();
      return found as HTMLImageElement;
    });
    expect(image.alt).toBe("mock-up.png");
    expect(image.getAttribute("loading")).toBe("lazy");
    expect(image.src.startsWith("data:image/png;base64,")).toBe(true);
  });
});
