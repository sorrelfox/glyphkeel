// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

// The lag of a dependency in the Gantt chart: clicking the arrow opens a
// dialog that sets it; the chart draws it as a dashed stretch after the
// blocking bar with its days, and Undo takes it back.

async function openGantt(page: Page) {
  await page.clock.setFixedTime(new Date("2026-11-11T10:00:00"));
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Gantt" }).click();
  const grid = page.getByRole("treegrid", { name: "Gantt chart" });
  await expect(grid.locator("[data-bar]").first()).toBeAttached();
  return grid;
}

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");

test("a dependency's lag is set in a dialog, drawn after the blocking bar and undone", async ({ page }) => {
  const grid = await openGantt(page);
  const link = grid.locator("[data-dependency]").first();
  await expect(link).toBeAttached();
  const name = (await link.getAttribute("data-dependency")) ?? "";
  const [from = "", to = ""] = name.split(">");
  // The arrow may lie outside the visible days; its line takes the click.
  await link.locator("path").nth(1).dispatchEvent("click");

  const dialog = page.getByRole("dialog", { name: new RegExp(`${from}\\W* blocks \\W*${to}`) });
  await expect(dialog).toBeVisible();
  const field = dialog.getByRole("textbox", { name: "Lag in days" });
  await field.fill("3");
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog).toBeHidden();

  const lagged = grid.locator(`[data-dependency="${name}"]`);
  await expect(lagged).toHaveAttribute("data-lag", "3");
  await expect(page.locator(`[data-lag-label="${name}"]`)).toHaveText(/^\+\W*3\W* days$/);
  await expect.poll(async () => plain(await page.getByTestId("gantt-live").textContent())).toBe(`${from} to ${to}: 3 days of lag`);

  await page.keyboard.press("Control+z");
  await expect(lagged).not.toHaveAttribute("data-lag", /.*/);
  await expect(page.locator(`[data-lag-label="${name}"]`)).toHaveCount(0);
});
