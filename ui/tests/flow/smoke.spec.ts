// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test } from "@playwright/test";

test("the window shows its header bar", async ({ page }) => {
  await page.goto("/");
  const header = page.getByRole("banner");
  await expect(header).toContainText("Glyphkeel");
  await expect(page.getByRole("button", { name: "Close" })).toBeVisible();
});
