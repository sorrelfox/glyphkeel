// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test } from "@playwright/test";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

test("a CSV file is checked, mapped and imported from the project menu, then undone", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).first().click();
  await page.getByRole("button", { name: /Task Manager.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Import from CSV/ }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("heading", { name: /Import from CSV into/ })).toBeVisible();

  await dialog.getByRole("textbox", { name: "CSV file in the exports folder" }).fill("tasks.csv");
  await dialog.getByRole("button", { name: "Check" }).click();

  // The preset is detected; each column shows its examples and where it goes.
  await expect(dialog.getByRole("button", { name: "Preset", exact: true })).toContainText(/Detected: .?jira/);
  const summary = dialog.getByTestId("csv-summary");
  await expect.poll(async () => plain(await summary.innerText())).toContain("2 new tasks");
  await expect.poll(async () => plain(await summary.innerText())).toContain("One row is left out");
  const problems = dialog.getByTestId("csv-problems");
  await expect(problems.getByText("The row has no title.")).toBeVisible();
  await expect(problems.getByText(/"soon" is not a date/)).toBeVisible();
  await expect(dialog.getByRole("button", { name: /Import .?2.? tasks/ })).toBeDisabled();

  // Leaving out the row with errors allows the import.
  await dialog.getByText("Leave out rows with errors").click();
  const start = dialog.getByRole("button", { name: /Import .?2.? tasks/ });
  await expect(start).toBeEnabled();

  // A column is mapped by hand: the due dates are no longer read.
  await dialog.getByRole("button", { name: /Where “.?Due date.?” goes/ }).click();
  await page.getByRole("option", { name: "Not imported" }).click();
  await expect(problems.getByText(/"soon" is not a date/)).toBeHidden();

  await start.click();
  await expect(dialog).toBeHidden();
  const toast = page.getByRole("status").filter({ hasText: /Imported .?2.? tasks/ });
  await expect(toast).toBeVisible();
  await expect(page.locator("[data-card]").filter({ hasText: "Map the columns" })).toBeVisible();
  await toast.getByRole("button", { name: "Undo" }).click();
  await expect(page.locator("[data-card]").filter({ hasText: "Map the columns" })).toBeHidden();
});

test("the main menu opens the CSV import, with a project to choose, and keeps a preset", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Main menu" }).click();
  await page.getByRole("menuitem", { name: /Import tasks from a CSV file/ }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("heading", { name: "Import from CSV" })).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Project", exact: true })).toBeVisible();
  await dialog.getByRole("textbox", { name: "CSV file in the exports folder" }).fill("tasks.csv");
  await dialog.getByRole("button", { name: "Check" }).click();
  await expect.poll(async () => plain(await dialog.getByTestId("csv-summary").innerText())).toContain("2 new tasks");
  await dialog.getByRole("textbox", { name: "Preset name" }).fill("Our Jira");
  await dialog.getByRole("button", { name: "Save as preset" }).click();
  await expect(page.getByText(/Saved the preset .?Our Jira/)).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Preset", exact: true })).toContainText("Our Jira");
});
