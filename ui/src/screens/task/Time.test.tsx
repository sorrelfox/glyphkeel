// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type TaskChange, type TaskList, type TimerStatus } from "../../generated/ops";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");
const named = (pattern: RegExp) => (name: string) => pattern.test(plain(name));

async function renderPanel() {
  const transport = fakeTransport();
  const task = ((await transport.call("task.create", { project: "PER", title: "Plan the trip" })) as TaskChange).task;
  render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/projects/PER?task=${task.key}`} />);
  const panel = await screen.findByRole("complementary", { name: named(new RegExp(`Task ${task.key}`)) });
  await within(panel).findByRole("heading", { name: "Plan the trip" });
  return { transport, task, panel };
}

const entries = async (transport: ReturnType<typeof fakeTransport>, key: string) =>
  ((await transport.call("task.get", { keys: [key], include: ["time"] })) as TaskList).items[0]?.time_entries ?? [];

describe("time in the task panel", () => {
  it("logs time from the Time tab, with a decimal comma, and edits and deletes it", async () => {
    const { panel, transport, task } = await renderPanel();
    fireEvent.click(within(panel).getByRole("tab", { name: "Time" }));
    fireEvent.click(await within(panel).findByRole("button", { name: "Log time" }));
    const form = within(panel).getByRole("group", { name: "Log time" });
    const duration = within(form).getByRole("textbox", { name: "Duration" });
    fireEvent.change(duration, { target: { value: "soon" } });
    fireEvent.keyDown(duration, { key: "Enter" });
    expect(await within(form).findByRole("alert")).toBeDefined();
    fireEvent.change(duration, { target: { value: "1,5h" } });
    fireEvent.change(within(form).getByRole("textbox", { name: "Note" }), { target: { value: "Booked the hotel" } });
    fireEvent.keyDown(duration, { key: "Enter" });
    const list = await within(panel).findByRole("list", { name: "Time entries" });
    await within(list).findByText("Booked the hotel");
    expect(within(list).getByText("1h 30m")).toBeDefined();
    expect(within(panel).getByRole("tab", { name: named(/^Time1$/) })).toBeDefined();
    expect((await entries(transport, task.key)).map((entry) => entry.minutes)).toEqual([90]);

    fireEvent.click(within(list).getByRole("button", { name: "Time entry actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Edit" }));
    const edit = await within(panel).findByRole("textbox", { name: "Duration" });
    fireEvent.change(edit, { target: { value: "1d" } });
    fireEvent.keyDown(edit, { key: "Enter" });
    await waitFor(async () => {
      expect((await entries(transport, task.key)).map((entry) => entry.minutes)).toEqual([480]);
    });
    await within(list).findByText("1d");

    fireEvent.click(within(list).getByRole("button", { name: "Time entry actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
    await within(panel).findByText("No time logged yet.");
  });

  it("logs 30 minutes for today with +30m", async () => {
    const { panel, transport, task } = await renderPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Log 30 minutes for today" }));
    await waitFor(async () => {
      expect((await entries(transport, task.key)).map((entry) => entry.minutes)).toEqual([30]);
    });
    const spent = within(panel).getByText("Spent").closest("dl");
    await waitFor(() => {
      expect(spent?.textContent).toContain("30m");
    });
  });

  it("starts the timer, shows it in the sidebar's chip, and stops it there, logging the time", async () => {
    const { panel, transport, task } = await renderPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Start timer" }));
    await within(panel).findByRole("button", { name: named(/^Timer running, 0:00:0\d\. Stop and log the time$/) });
    const chip = await screen.findByRole("group", { name: "Timer" });
    expect(chip.dataset.timerChip).toBe(task.key);

    fireEvent.click(within(chip).getByRole("button", { name: "Pause the timer" }));
    await within(chip).findByRole("button", { name: "Go on with the timer" });
    expect(((await transport.call("timer.get", {})) as TimerStatus).timer?.state).toBe("paused");

    fireEvent.click(within(chip).getByRole("button", { name: "Stop the timer and log the time" }));
    await waitFor(() => {
      expect(screen.queryByRole("group", { name: "Timer" })).toBeNull();
    });
    await screen.findByText(/no time was logged/);
    await within(panel).findByRole("button", { name: "Start timer" });
  });

  it("follows a timer started elsewhere", async () => {
    const { transport, task } = await renderPanel();
    await transport.call("timer.start", { key: task.key });
    const chip = await screen.findByRole("group", { name: "Timer" });
    expect(within(chip).getByText(task.key)).toBeDefined();
    await transport.call("timer.stop", { discard: true });
    await waitFor(() => {
      expect(screen.queryByRole("group", { name: "Timer" })).toBeNull();
    });
  });

  it("sets a reminder relative to the due date and removes it", async () => {
    const { panel, transport, task } = await renderPanel();
    await transport.call("task.update", { key: task.key, due_date: "2026-12-10" });
    fireEvent.click(within(panel).getByRole("button", { name: named(/^Reminder: None$/) }));
    const dialog = await screen.findByRole("dialog", { name: "Reminder" });
    fireEvent.click(within(dialog).getByRole("radio", { name: "Before the due date" }));
    fireEvent.change(within(dialog).getByRole("spinbutton", { name: "Days before the due date" }), { target: { value: "2" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Set reminder" }));
    await within(panel).findByRole("button", { name: named(/^Reminder: .*2 days before due$/) });

    fireEvent.click(within(panel).getByRole("button", { name: named(/^Reminder: /) }));
    fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
    await within(panel).findByRole("button", { name: named(/^Reminder: None$/) });
  });
});

describe("the time report", () => {
  it("adds up the week's time by task and by day", async () => {
    const transport = fakeTransport();
    const task = ((await transport.call("task.create", { project: "PER", title: "Plan the trip" })) as TaskChange).task;
    await transport.call("time.log", { key: task.key, duration: "2h" });
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/time" />);
    const row = await screen.findByRole("row", { name: named(new RegExp(task.key)) });
    expect(plain(row.textContent)).toContain("2h");
    expect(within(row).getByRole("button", { name: named(new RegExp(task.key)) })).toBeDefined();
    fireEvent.click(screen.getByRole("radio", { name: "Day" }));
    await screen.findByRole("columnheader", { name: "Day" });
  });
});
