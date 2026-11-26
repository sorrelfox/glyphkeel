// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator } from "@playwright/test";

import { datesOf, day, gk, openView, project } from "./engine";

// The Gantt chart on the engine: a roadmap made from the command line
// shows as bars, a milestone and a dependency; a keyboard move with "Also
// move dependents" moves the blocked task in the engine as one change for
// Undo; a date changed by another process shows as a problem.

const bar = (grid: Locator, key: string) => grid.locator(`[data-bar="${key}"]`);
const row = (grid: Locator, key: string) => grid.locator(`[data-row="${key}"]`);

test("a roadmap from the command line: bars, a milestone, a dependency, a move with its dependents, Undo, and an outside change", async ({ page }) => {
  project("Roadmap", "RD");
  gk("task", "create", "--project", "RD", "--title", "Engine", "--start-date", day(1), "--due-date", day(3));
  gk("task", "create", "--project", "RD", "--title", "Screens", "--start-date", day(4), "--due-date", day(6));
  gk("task", "create", "--project", "RD", "--title", "Ship", "--type", "Milestone", "--due-date", day(10));
  gk("task", "create", "--project", "RD", "--title", "Someday");
  gk("task", "link", "RD-1", "--kind", "blocks", "--other", "RD-2");

  await openView(page, "Roadmap", "Gantt");
  const grid = page.getByRole("treegrid", { name: "Gantt chart" });
  await expect(grid).toBeVisible();
  await expect(bar(grid, "RD-1")).toHaveAttribute("data-shape", "bar");
  await expect(bar(grid, "RD-2")).toHaveAttribute("data-shape", "bar");
  await expect(bar(grid, "RD-3")).toHaveAttribute("data-shape", "milestone");
  await expect(grid.locator('[data-dependency="RD-1>RD-2"]')).toHaveCount(1);
  await expect(row(grid, "RD-4")).toBeAttached();
  await expect(bar(grid, "RD-4")).toHaveCount(0);

  // The switch is kept per person in the engine.
  const alsoMove = page.getByRole("switch", { name: "Also move dependents" });
  await page.getByText("Also move dependents").click();
  await expect(alsoMove).toBeChecked();

  // Alt+Right moves the blocker a day; the blocked task follows.
  await row(grid, "RD-1").getByRole("gridcell").first().click();
  await expect(grid).toBeFocused();
  await page.keyboard.press("Alt+ArrowRight");
  await expect.poll(() => datesOf("RD-1")).toEqual([day(2), day(4)]);
  await expect.poll(() => datesOf("RD-2")).toEqual([day(5), day(7)]);
  // One Undo takes both back.
  await page.keyboard.press("Control+z");
  await expect.poll(() => datesOf("RD-1")).toEqual([day(1), day(3)]);
  expect(datesOf("RD-2")).toEqual([day(4), day(6)]);

  // Another process makes the blocker end later: the chart marks the
  // blocked task without a reload.
  await expect(row(grid, "RD-2").locator("[data-problems]")).toHaveCount(0);
  gk("task", "edit", "RD-1", "--due-date", day(5));
  await expect(row(grid, "RD-2").locator("[data-problems]")).toHaveAttribute("data-problems", /starts_before_blocker_ends/);

  // A fresh page: the switch is still on, read back from the engine.
  await openView(page, "Roadmap", "Gantt");
  await expect(page.getByRole("treegrid", { name: "Gantt chart" })).toBeVisible();
  await expect(page.getByRole("switch", { name: "Also move dependents" })).toBeChecked();
});
