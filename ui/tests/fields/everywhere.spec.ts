// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

/** Makes the select field "Area" (Front end, Back end) in Task Manager's settings. */
async function makeArea(page: Page) {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).first().click();
  await page.getByRole("button", { name: /Task Manager.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Fields" }).click();
  await settings.getByRole("textbox", { name: "New field name" }).fill("Area");
  await settings.getByRole("button", { name: /Kind of the new field/ }).click();
  await page.getByRole("option", { name: "One option" }).click();
  await settings.getByRole("button", { name: "Add a field" }).click();
  const block = settings.getByTestId("field-settings-Area");
  const options = block.getByRole("textbox", { name: "Option", exact: true });
  await options.first().fill("Front end");
  await options.first().press("Enter");
  const add = block.getByRole("textbox", { name: "Add an option" });
  await add.fill("Back end");
  await add.press("Enter");
  await expect(options).toHaveCount(2);
  await settings.getByRole("button", { name: "Close" }).click();
  await expect(settings).toBeHidden();
}

test("a board card shows a custom field the board's options choose", async ({ page }) => {
  await makeArea(page);
  const card = page.locator("section[data-lane] [data-card]").first();
  const key = await card.getAttribute("data-card");
  await card.dblclick();
  const fields = page.getByRole("complementary", { name: /^Task / }).getByRole("region", { name: "Fields" });
  await fields.getByRole("button", { name: /Change.*Area/ }).click();
  await page.getByRole("option", { name: "Back end" }).click();
  await expect(fields.getByRole("button", { name: /Change.*Area/ })).toContainText("Back end");

  const shown = page.locator(`section[data-lane] [data-card="${key ?? ""}"]`);
  await expect(shown).not.toContainText("Back end");
  await page.getByRole("button", { name: /^Board options/ }).click();
  await page.getByRole("menuitemcheckbox", { name: "Area" }).click();
  await expect(shown.getByLabel(/Area.*Back end/)).toBeVisible();
  await expect.poll(async () => plain(await shown.innerText())).toContain("Back end");
});

test("the list groups by a custom field and a bulk change sets it on every selected task", async ({ page }) => {
  await makeArea(page);
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  // Two tasks, each by its own checkbox.
  const boxes = page.getByRole("checkbox", { name: /^Select (?!all)/ });
  await boxes.nth(0).click();
  await boxes.nth(1).click({ modifiers: ["ControlOrMeta"] });
  await expect(page.getByTestId("bulk-bar")).toContainText(/2.*selected/);
  const bar = page.getByTestId("bulk-bar");
  await bar.getByRole("button", { name: /Set a field/ }).click();
  await page.getByRole("option", { name: "Area" }).click();
  await bar.getByTestId("bulk-field-editor").getByRole("button", { name: /Change.*Area/ }).click();
  await page.getByRole("option", { name: "Front end" }).click();
  await bar.getByRole("button", { name: "Preview" }).click();
  await expect.poll(async () => plain(await bar.getByRole("status").innerText())).toMatch(/^Area: Front end · 2 of 2 tasks change/);
  await bar.getByRole("button", { name: /^Change \W*2\W* tasks$/ }).click();
  await expect(page.getByText(/^\W*2\W* tasks changed$/).first()).toBeVisible();

  await page.getByRole("button", { name: /^Group by:/ }).click();
  await page.getByRole("menuitemradio", { name: "Area" }).click();
  await expect(page.getByRole("button", { name: /^Group by: \W*Area/ })).toBeVisible();
  // The two tasks make the first group; the others have no value.
  const groups = page.getByRole("button", { expanded: true });
  await expect.poll(async () => plain(await groups.first().innerText())).toMatch(/^Front end\s*2$/);
  await expect(page.getByRole("button", { name: /No \W*Area/ })).toBeVisible();
});
