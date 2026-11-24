// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

const BASE = process.env.GK_SAFETY_UI_URL ?? "";
const PAGE = `${BASE}/?bridge=${process.env.GK_SAFETY_BRIDGE_PORT ?? ""}&token=${process.env.GK_BRIDGE_TOKEN ?? ""}`;

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

/** The key of the project made below, as the engine chose it. */
let projectKey = "";

async function newProject(page: Page, name: string): Promise<string> {
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill(name);
  const key = await dialog.getByRole("textbox", { name: "Key" }).inputValue();
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  return key;
}

const card = (page: Page, title: string) => page.locator("section[data-lane] [data-card]", { hasText: title });

async function addTask(page: Page, title: string) {
  const field = page.getByRole("textbox", { name: /Title of the new task in/ }).first();
  if (!(await field.isVisible())) {
    await page.locator("section[data-lane]").first().getByRole("button", { name: /Add task to/ }).click();
  }
  await field.fill(title);
  await field.press("Enter");
  await expect(card(page, title)).toBeVisible();
}

test("a task goes to the trash with Undo, comes back from the trash, and is purged", async ({ page }) => {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  projectKey = await newProject(page, "Attic");
  await addTask(page, "Sort the boxes");
  await addTask(page, "Fix the hatch");
  await page.keyboard.press("Escape");

  // Delete from the card's menu; Undo in the toast takes it back.
  await card(page, "Sort the boxes").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Move to trash" }).click();
  const toast = page.getByRole("status").filter({ hasText: "Moved one task to the trash" });
  await expect(toast).toBeVisible();
  await expect(card(page, "Sort the boxes")).toHaveCount(0);
  await toast.getByRole("button", { name: "Undo" }).click();
  await expect(card(page, "Sort the boxes")).toBeVisible();

  // Delete both with the Delete key on the focused card.
  for (const title of ["Sort the boxes", "Fix the hatch"]) {
    await card(page, title).focus();
    await page.keyboard.press("Delete");
    await expect(card(page, title)).toHaveCount(0);
  }

  // The trash lists both, with who deleted them and the days left.
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "Trash" }).click();
  const table = page.getByRole("table", { name: "Trash" });
  await expect(table.getByRole("row")).toHaveCount(3);
  await expect(table.getByText(/^in .*30.* days$/)).toHaveCount(2);
  await table.getByRole("checkbox", { name: /Sort the boxes/ }).check();
  await page.getByRole("button", { name: "Restore", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "Restored one item" })).toBeVisible();
  await expect(table.getByRole("row")).toHaveCount(2);

  // Deleting for good is confirmed by the app (the bridge confirms here).
  await table.getByRole("checkbox", { name: /Fix the hatch/ }).check();
  await page.getByRole("button", { name: "Delete for good" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Deleted one item for good" })).toBeVisible();
  await expect(page.getByText("The trash is empty.")).toBeVisible();
});

test("the activity feed lists the changes and filters them", async ({ page }) => {
  await page.goto(PAGE);
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "Activity" }).click();
  const table = page.getByRole("table", { name: "Activity" });
  // Fluent's isolation marks sit around the item's kind.
  await expect(table.getByText(/Task\W* moved to the trash/).first()).toBeVisible();
  await expect(table.getByText(/Task\W* created/).first()).toBeVisible();
  await page.getByRole("radio", { name: "Agents" }).click();
  await expect(page.getByText("No changes match the filters.")).toBeVisible();
  await page.getByRole("button", { name: "Clear filters" }).click();
  await expect(table.getByText(/Project\W* created/).first()).toBeVisible();
});

test("backups are made, checked and deleted in Preferences", async ({ page }) => {
  await page.goto(PAGE);
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /Preferences/ }).click();
  const prefs = page.getByRole("dialog", { name: "Preferences" });
  await prefs.getByRole("tab", { name: "Data and backups" }).click();
  await prefs.getByRole("button", { name: "Back up now" }).click();
  await expect(page.getByRole("status").filter({ hasText: /Backup made/ })).toBeVisible();
  const table = prefs.getByRole("table", { name: "Backups kept" });
  const row = table.getByRole("row").filter({ hasText: "Made by hand" }).first();
  await row.getByRole("button", { name: "Check" }).click();
  await expect(row.getByTestId("backup-check")).toContainText("Good");
  await row.getByRole("button", { name: "Delete…" }).click();
  await expect(page.getByRole("status").filter({ hasText: /Backup deleted/ })).toBeVisible();
  await expect(prefs.getByRole("row").filter({ hasText: "Made by hand" })).toHaveCount(0);
});

test("a project is exported, and imported again under a new key", async ({ page }) => {
  await page.goto(PAGE);
  await page.getByRole("button", { name: "Main menu" }).click();
  await page.getByRole("menuitem", { name: /Export/ }).click();
  const exporting = page.getByRole("dialog", { name: "Export" });
  await exporting.getByRole("radio", { name: "Chosen projects" }).click();
  await exporting.getByRole("checkbox", { name: /Attic/ }).check();
  await exporting.getByRole("button", { name: "Export" }).click();
  const path = plain((await exporting.getByTestId("export-path").textContent()) ?? "");
  expect(path).toMatch(/\.zip$/);
  await exporting.getByRole("button", { name: "Done" }).click();

  await page.getByRole("button", { name: "Main menu" }).click();
  await page.getByRole("menuitem", { name: /Import/ }).click();
  const importing = page.getByRole("dialog", { name: "Import" });
  await importing.getByRole("textbox", { name: "Archive in the exports folder" }).fill(path);
  await importing.getByRole("button", { name: "Check" }).click();
  // The key is taken here: the dialog offers a new one.
  const newKey = importing.getByRole("textbox", { name: /New key for/ });
  const taken = plain((await newKey.getAttribute("aria-label")) ?? "").replace("New key for ", "");
  if (projectKey !== "") {
    expect(taken).toBe(projectKey);
  }
  await expect(newKey).toHaveValue(`${taken}2`);
  await importing.getByRole("button", { name: "Import one project" }).click();
  await expect(importing.getByText(`${taken}2`)).toBeVisible();
  await expect(page.getByRole("status").filter({ hasText: "Imported one project" })).toBeVisible();
  await importing.getByRole("button", { name: "Done" }).click();
  await expect(page.getByRole("navigation", { name: "Workspace" }).getByText("Attic").nth(1)).toBeVisible();
});
