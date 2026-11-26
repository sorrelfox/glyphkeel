// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

async function openSettings(page: Page) {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).first().click();
  await page.getByRole("button", { name: /Task Manager.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Fields" }).click();
  return settings;
}

test("a field made in project settings shows in the task panel and as a list column", async ({ page }) => {
  const settings = await openSettings(page);
  await expect(settings.getByText(/No custom fields yet/)).toBeVisible();
  await settings.getByRole("textbox", { name: "New field name" }).fill("Story points");
  await settings.getByRole("button", { name: /Kind of the new field/ }).click();
  await page.getByRole("option", { name: "Whole number" }).click();
  await settings.getByRole("button", { name: "Add a field" }).click();
  const block = settings.getByTestId("field-settings-Story points");
  await expect(block).toBeVisible();
  await expect.poll(async () => plain(await block.innerText())).toContain("cf.story-points");
  await settings.getByRole("button", { name: "Close" }).click();
  await expect(settings).toBeHidden();

  // The task panel has the field; a value is typed in and kept.
  await page.locator("section[data-lane] [data-card]").first().dblclick();
  const panel = page.getByRole("complementary", { name: /^Task / });
  const fields = panel.getByRole("region", { name: "Fields" });
  const input = fields.getByRole("spinbutton", { name: /Change.*Story points/ });
  await input.fill("5");
  await input.press("Enter");
  await expect(input).toHaveValue("5");

  // The list shows it as a column and filters by it.
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  await page.getByRole("button", { name: "Columns" }).click();
  await page.getByRole("menuitemcheckbox", { name: "Story points" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("columnheader", { name: /Story points/ })).toBeVisible();
  const query = page.getByRole("textbox", { name: "Filter query" });
  await query.click();
  await page.keyboard.type("cf.story-points>=5");
  await page.keyboard.press("Enter");
  await expect.poll(async () => plain(await page.getByTestId("list-totals").innerText())).toMatch(/^1 task/);
});

test("a select field's options are renamed in settings and chosen in the panel", async ({ page }) => {
  const settings = await openSettings(page);
  await settings.getByRole("textbox", { name: "New field name" }).fill("Area");
  await settings.getByRole("button", { name: /Kind of the new field/ }).click();
  await page.getByRole("option", { name: "One option" }).click();
  await settings.getByRole("button", { name: "Add a field" }).click();
  const block = settings.getByTestId("field-settings-Area");
  // Exact: "Add an option" would match the name too.
  const options = block.getByRole("textbox", { name: "Option", exact: true });
  const first = options.first();
  await first.fill("Front end");
  await first.press("Enter");
  const add = block.getByRole("textbox", { name: "Add an option" });
  await add.fill("Back end");
  await add.press("Enter");
  await expect(options).toHaveCount(2);
  await expect(options.nth(1)).toHaveValue("Back end");
  await settings.getByRole("button", { name: "Close" }).click();

  await page.locator("section[data-lane] [data-card]").first().dblclick();
  const fields = page.getByRole("complementary", { name: /^Task / }).getByRole("region", { name: "Fields" });
  await fields.getByRole("button", { name: /Change.*Area/ }).click();
  await page.getByRole("option", { name: "Back end" }).click();
  await expect(fields.getByRole("button", { name: /Change.*Area/ })).toContainText("Back end");
});
