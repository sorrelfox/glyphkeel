// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type TaskChange } from "../../generated/ops";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

async function renderReports() {
  const transport = fakeTransport();
  const task = ((await transport.call("task.create", { project: "PER", title: "Plan the trip" })) as TaskChange).task;
  await transport.call("task.move", { key: task.key, status: "Done" });
  render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/reports" />);
  const chart = await screen.findByRole("group", { name: /Tasks created and closed/ });
  return { transport, task, chart };
}

describe("the flow reports", () => {
  it("reads the chart period by period with the keyboard and shows its table", async () => {
    const { chart } = await renderReports();
    expect(within(chart).getByRole("img", { name: /Tasks created and closed/ })).toBeDefined();
    const readout = document.querySelector("[data-chart-readout]");
    expect(plain(readout?.textContent)).toMatch(/created/);
    fireEvent.keyDown(chart, { key: "End" });
    await waitFor(() => {
      expect(plain(readout?.textContent)).toMatch(/: \d+ created, \d+ closed, \d+ reopened, \d+ open/);
    });
    fireEvent.click(screen.getByRole("switch", { name: "Show as a table" }));
    const table = await screen.findByRole("table", { name: /Tasks created and closed/ });
    expect(within(table).getByRole("columnheader", { name: "Reopened" })).toBeDefined();
    expect(within(table).getByRole("rowheader", { name: "Total" })).toBeDefined();
  });

  it("switches to the cumulative flow and to cycle time", async () => {
    const { task } = await renderReports();
    fireEvent.click(screen.getByRole("tab", { name: "Cumulative flow" }));
    await screen.findByRole("group", { name: /Tasks in each status/ });
    expect(screen.getByRole("radio", { name: "By category" })).toBeDefined();
    fireEvent.click(screen.getByRole("tab", { name: "Cycle time" }));
    await screen.findByRole("region", { name: "Lead time" });
    fireEvent.click(screen.getByRole("switch", { name: "Show as a table" }));
    expect(await screen.findByRole("button", { name: new RegExp(task.key) })).toBeDefined();
  });
});
