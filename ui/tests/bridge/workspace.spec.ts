// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

const token = process.env.GK_BRIDGE_TOKEN ?? "";
const PAGE = `/?bridge=5321&token=${token}`;

async function openPreferences(page: Page) {
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /Preferences/ }).click();
  await expect(page.getByRole("dialog", { name: "Preferences" })).toBeVisible();
}

function day(page: Page, name: string) {
  return page.getByRole("radiogroup", { name: "First day of the week" }).getByRole("radio", { name });
}

test("two windows work on one workspace and follow each other's changes", async ({ browser }) => {
  const a = await browser.newPage();
  const b = await browser.newPage();
  await a.goto(PAGE);
  await b.goto(PAGE);

  // Data from the engine: the workspace's folder name.
  await expect(a.getByTestId("status-workspace")).toHaveText("Personal");
  await expect(b.getByTestId("status-workspace")).toHaveText("Personal");

  // An engine error, typed, as a toast.
  await a.keyboard.press("Control+z");
  await expect(a.getByRole("alert")).toContainText("Nothing to undo.");

  await openPreferences(a);
  await openPreferences(b);
  await expect(day(a, "Monday")).toHaveAttribute("aria-checked", "true");

  // B changes a setting; A shows it through the change event alone.
  await day(b, "Sunday").click();
  await expect(day(b, "Sunday")).toHaveAttribute("aria-checked", "true");
  await expect(day(a, "Sunday")).toHaveAttribute("aria-checked", "true");

  // Undo from B's toast takes it back in both.
  await b.getByRole("status").filter({ hasText: "Settings changed" }).getByRole("button", { name: "Undo" }).click();
  await expect(day(a, "Monday")).toHaveAttribute("aria-checked", "true");
  await expect(day(b, "Monday")).toHaveAttribute("aria-checked", "true");
});

test("a page with the wrong token gets no workspace", async ({ page }) => {
  await page.goto("/?bridge=5321&token=wrong");
  await expect(page.getByTestId("status-workspace")).toHaveText("Workspace unavailable");
});
