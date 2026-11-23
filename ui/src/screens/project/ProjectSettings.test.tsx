// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { suggestKey } from "./NewProjectDialog";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

async function renderApp() {
  render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(fakeTransport())} path="/projects/PER" />);
  await screen.findByRole("group", { name: /Board of/ });
}

describe("project settings", () => {
  it("sets a WIP limit that the board's column shows", async () => {
    await renderApp();
    fireEvent.click(screen.getByRole("button", { name: /project menu/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /Project settings/ }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("tab", { name: "Statuses" }));
    const row = await within(dialog).findByTestId("status-To do");
    const limit = within(row).getByRole("textbox", { name: "WIP limit" });
    fireEvent.change(limit, { target: { value: "2" } });
    fireEvent.blur(limit);
    await waitFor(() => {
      const lane = document.querySelector('section[data-lane][aria-label^="⁨To do"]');
      expect(plain(lane?.querySelector('[data-testid="lane-count"]')?.textContent)).toMatch(/^\d+ \/ 2$/);
    });
  });

  it("adds a label and a status", async () => {
    await renderApp();
    fireEvent.click(screen.getByRole("button", { name: /project menu/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /Project settings/ }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("tab", { name: "Statuses" }));
    const name = await within(dialog).findByRole("textbox", { name: "New status" });
    fireEvent.change(name, { target: { value: "Waiting" } });
    fireEvent.submit(name);
    await within(dialog).findByTestId("status-Waiting");
    await waitFor(() => {
      expect([...document.querySelectorAll("section[data-lane] h2")].map((h) => h.textContent)).toContain("Waiting");
    });
    fireEvent.click(within(dialog).getByRole("tab", { name: "Labels" }));
    const label = await within(dialog).findByRole("textbox", { name: "New label" });
    fireEvent.change(label, { target: { value: "garden" } });
    fireEvent.submit(label);
    expect(await within(dialog).findByText("garden", { selector: ".chip" })).toBeDefined();
  });
});

describe("suggestKey", () => {
  it("suggests a key from a name", () => {
    expect(suggestKey("Task Manager")).toBe("TM");
    expect(suggestKey("Personal")).toBe("PER");
    expect(suggestKey("Écoles du monde")).toBe("EDM");
    expect(suggestKey("2 cats")).toBe("CAT");
    expect(suggestKey("")).toBe("");
  });
});
