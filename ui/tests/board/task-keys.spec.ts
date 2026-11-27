// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// The keys of an open task (E, S, P, L, D, Ctrl+Enter) in its panel, and
// their place in the shortcut sheet, on the sample workspace.

function lane(page: Page, name: string): Locator {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

/** A new task in "To do", open in its panel, with the focus on the panel. */
async function openNewTask(page: Page, title: string): Promise<Locator> {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await lane(page, "To do").getByRole("button", { name: /Add task to/ }).click();
  await page.keyboard.type(title);
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: title });
  const key = (await card.getAttribute("data-card")) ?? "";
  await card.dblclick();
  const panel = page.getByRole("complementary", { name: new RegExp(`^Task \\W*${key}\\W*$`) });
  await expect(panel).toBeVisible();
  // The panel's own copy-key button: inside the panel, not a text field.
  await panel.getByRole("button", { name: "Copy key" }).focus();
  return panel;
}

test("S, P, L and D open the status, priority, labels and due date of the open task; E edits its description", async ({ page }) => {
  const panel = await openNewTask(page, "Book the ferry");

  await page.keyboard.press("s");
  const statuses = page.getByRole("menu", { name: "Status" });
  await expect(statuses).toBeVisible();
  await statuses.getByRole("menuitemradio", { name: "In progress" }).click();
  await expect(panel.getByRole("button", { name: /^Status: \W*In progress/ })).toBeVisible();

  await panel.getByRole("button", { name: "Copy key" }).focus();
  await page.keyboard.press("p");
  const priorities = page.getByRole("menu", { name: "Priority" });
  await expect(priorities).toBeVisible();
  await priorities.getByRole("menuitemradio", { name: "High" }).click();
  await expect(panel.getByRole("button", { name: /^Priority: \W*High/ })).toBeVisible();

  await panel.getByRole("button", { name: "Copy key" }).focus();
  await page.keyboard.press("l");
  await expect(page.getByRole("searchbox", { name: "Find or create a label" })).toBeFocused();
  await page.keyboard.press("Escape");

  await panel.getByRole("button", { name: "Copy key" }).focus();
  await page.keyboard.press("d");
  await expect(panel.getByRole("textbox", { name: /Due\W* in words/ })).toBeFocused();
  await page.keyboard.press("Escape");

  await panel.getByRole("button", { name: "Copy key" }).focus();
  await page.keyboard.press("e");
  await expect(panel.getByRole("textbox", { name: /Description/ })).toBeVisible();

  // Typed into a text field, the letters are text.
  const composer = panel.getByRole("textbox", { name: "Write a comment" });
  await composer.click();
  await page.keyboard.type("sp");
  await expect(page.getByRole("menu")).toHaveCount(0);
  await expect(composer).toContainText("sp");
});

test("Ctrl+Enter outside the editors opens the open task in its own window", async ({ page }) => {
  const panel = await openNewTask(page, "Pack the tent");
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.keyboard.press("Control+Enter");
  // Without the desktop app, the task's window is this page.
  await expect(page.getByRole("group", { name: /Board of/ })).toBeHidden();
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("button", { name: "Open in its own window" })).toHaveCount(0);
});

test("the shortcut sheet lists the keys of an open task", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
  await page.keyboard.press("?");
  const sheet = page.getByRole("dialog", { name: "Keyboard shortcuts" });
  await expect(sheet).toBeVisible();
  const group = sheet.locator("section").filter({ has: page.getByRole("heading", { name: "In an open task" }) });
  for (const name of ["Edit the description", "Change the status", "Change the priority", "Change the labels", "Change the due date", "In the description or a comment: save or send"]) {
    await expect(group.getByText(name)).toBeVisible();
  }
});
