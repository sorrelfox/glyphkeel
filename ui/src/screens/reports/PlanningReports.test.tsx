// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { setEstimateHint } from "../../data/planning";
import { createClient } from "../../data/transport";
import { type TaskChange, type VelocityReport } from "../../generated/ops";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

async function closedTasks(transport: ReturnType<typeof fakeTransport>, count: number, type: string) {
  const keys: string[] = [];
  for (let i = 0; i < count; i += 1) {
    const task = ((await transport.call("task.create", { project: "PER", title: `Fix ${String(i)}`, type, estimate: "1h" })) as TaskChange).task;
    await transport.call("time.log", { key: task.key, duration: "90m" });
    await transport.call("task.move", { key: task.key, status: "Done" });
    keys.push(task.key);
  }
  return keys;
}

describe("the planning reports", () => {
  it("opens velocity from the address, as a chart and as a table matching the engine", async () => {
    const transport = fakeTransport();
    const report = (await transport.call("report.velocity", { project: "TM" })) as VelocityReport;
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/reports?report=velocity&project=TM" />);
    await screen.findByRole("tab", { name: "Velocity", selected: true });
    const chart = await screen.findByRole("group", { name: /Work committed and completed per sprint of/ });
    fireEvent.keyDown(chart, { key: "Home" });
    const first = report.sprints[0];
    await waitFor(() => {
      expect(plain(document.querySelector("[data-chart-readout]")?.textContent)).toBe(`${first?.sprint.name ?? ""}: ${String(first?.committed)} committed, ${String(first?.completed)} completed, average ${String(first?.average)}`);
    });
    fireEvent.click(screen.getByRole("switch", { name: "Show as a table" }));
    const table = await screen.findByRole("table", { name: /Work committed and completed/ });
    expect(within(table).getAllByRole("row")).toHaveLength(report.sprints.length + 1);
    expect(within(table).getByRole("rowheader", { name: first?.sprint.name ?? "" })).toBeDefined();
  });

  it("shows estimate accuracy with the outliers and the groups", async () => {
    const transport = fakeTransport();
    const keys = await closedTasks(transport, 2, "Bug");
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/reports?report=estimates&project=PER" />);
    const outliers = await screen.findByRole("table", { name: "Furthest off" });
    expect(within(outliers).getByRole("button", { name: new RegExp(keys[0] ?? "") })).toBeDefined();
    const groups = screen.getByRole("table", { name: /Estimate accuracy by/ });
    expect(within(groups).getByRole("rowheader", { name: "Bug" })).toBeDefined();
    expect(within(groups).getAllByText(/1\.5.×/).length).toBeGreaterThan(0);
  });

  it("hints in the task panel how a type's estimates turn out, once switched on", async () => {
    const transport = fakeTransport();
    await closedTasks(transport, 5, "Bug");
    const open = ((await transport.call("task.create", { project: "PER", title: "Another", type: "Bug", estimate: "1h" })) as TaskChange).task;
    setEstimateHint(true);
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/open/${open.key}`} />);
    await waitFor(() => {
      expect(plain(document.querySelector("[data-estimate-hint]")?.textContent)).toMatch(/^Your Bug (estimates usually run [\d.]+× over|tasks usually take [\d.]+× of their estimate) \(\d+ closed tasks\)\.$/);
    });
  });
});
