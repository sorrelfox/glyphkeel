// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator } from "@playwright/test";

import { datesOf, day, gk, openView, project } from "./engine";

// The calendar on the engine: the engine's `during:` search picks the
// tasks of the week on screen; a task moved with the keyboard moves in the
// engine and Undo takes it back; tasks made or moved by another process
// show without a reload.

const bar = (grid: Locator, key: string) => grid.locator(`[data-task-bar="${key}"]`);
const cell = (grid: Locator, date: string) => grid.locator(`[data-day="${date}"]`);

test("the week from the engine: a keyboard move with Undo, and changes from another process", async ({ page }) => {
  project("Events", "EV");
  gk("task", "create", "--project", "EV", "--title", "Today's task", "--start-date", day(0), "--due-date", day(0));
  gk("task", "create", "--project", "EV", "--title", "Far away", "--due-date", day(60));
  gk("task", "create", "--project", "EV", "--title", "Moved later", "--due-date", day(45));

  await openView(page, "Events", "Calendar");
  const grid = page.getByRole("grid", { name: /^Calendar/ });
  await expect(grid).toBeVisible();
  await page.getByRole("radiogroup", { name: "Calendar range" }).getByRole("radio", { name: "Week" }).click();
  await expect(grid.locator("[data-week]")).toHaveCount(1);
  await expect(bar(grid, "EV-1").first()).toHaveAttribute("data-days", day(0));
  await expect(bar(grid, "EV-2")).toHaveCount(0);

  // Into today's tasks, then Alt+Right: a day later in the engine.
  await cell(grid, day(0)).click({ position: { x: 4, y: 4 } });
  await page.keyboard.press("Enter");
  const focusedTask = () => page.evaluate(() => (document.activeElement as HTMLElement | null)?.dataset.taskBar);
  await expect.poll(focusedTask).toBe("EV-1");
  await page.keyboard.press("Alt+ArrowRight");
  await expect.poll(() => datesOf("EV-1")).toEqual([day(1), day(1)]);
  await page.keyboard.press("Control+z");
  await expect.poll(() => datesOf("EV-1")).toEqual([day(0), day(0)]);
  await expect(bar(grid, "EV-1").first()).toHaveAttribute("data-days", day(0));

  // Another process adds a task for today and moves one into the week.
  gk("task", "create", "--project", "EV", "--title", "From elsewhere", "--due-date", day(0));
  gk("task", "edit", "EV-3", "--due-date", day(0));
  await expect(bar(grid, "EV-4").first()).toBeVisible();
  await expect(bar(grid, "EV-3").first()).toHaveAttribute("data-days", day(0));
  await expect(bar(grid, "EV-2")).toHaveCount(0);
});
