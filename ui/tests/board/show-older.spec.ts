// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test } from "@playwright/test";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

// Older closed cards stay in the engine until "Show older" asks for them;
// then they join the column, and the button goes.
test("Show older brings the column's older closed cards", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();

  const button = page.getByRole("button", { name: /Show older/ }).first();
  await expect(button).toBeVisible();
  const count = Number(/\((\d+)\)/.exec(plain(await button.innerText()))?.[1]);
  expect(count).toBeGreaterThan(0);
  const id = await page.locator("section[data-lane]").filter({ has: button }).getAttribute("data-lane");
  const lane = page.locator(`section[data-lane="${id ?? ""}"]`);
  const before = await lane.locator("[data-card]").count();

  await button.click();
  await expect(lane.locator("[data-card]")).toHaveCount(before + count);
  await expect(page.getByRole("button", { name: /Show older/ })).toHaveCount(0);
});
