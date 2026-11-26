// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test } from "@playwright/test";

test("a status that needs an estimate is closed to a task without one, and read-only fields say why", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("row", { name: /Personal/ }).first().click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();

  // The rule: Done needs an estimate, and its title can't change there.
  await page.getByRole("button", { name: /Personal.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Rules" }).click();
  const done = settings.getByTestId("rules-Done");
  await done.getByRole("button", { name: /Done/ }).click();
  // The box shows the change once it is saved, so it is clicked rather than checked.
  const estimate = done.getByRole("group", { name: /Must be filled in/ }).getByRole("checkbox", { name: "Estimate" });
  await estimate.click();
  await expect(estimate).toBeChecked();
  await expect(done.getByRole("button", { name: /Done/ })).toContainText("One rule");
  // And in Backlog the title can't change.
  const backlog = settings.getByTestId("rules-Backlog");
  await backlog.getByRole("button", { name: /Backlog/ }).click();
  const title = backlog.getByRole("group", { name: /Can't change/ }).getByRole("checkbox", { name: "Title" });
  await title.click();
  await expect(title).toBeChecked();
  await page.keyboard.press("Escape");
  await expect(settings).toBeHidden();

  // A new task has no estimate.
  const lane = page.locator("section[data-lane]").first();
  await lane.getByRole("button", { name: "Add task", exact: true }).click();
  await lane.getByRole("textbox", { name: /Title of the new task/ }).fill("Check the rules");
  await page.keyboard.press("Enter");
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Check the rules" });
  await expect(card).toBeVisible();

  await card.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Move to", exact: true }).click();
  const target = page.getByRole("menuitem", { name: /^Done/ });
  await expect(target).toBeDisabled();
  await expect(target).toContainText(/Fill in first: .?Estimate/);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");

  // The task panel's status menu says the same.
  await card.dblclick();
  const panel = page.getByRole("complementary", { name: /^Task / });
  await panel.getByRole("button", { name: /^Status:/ }).click();
  const option = page.getByRole("menuitem", { name: /^Done/ });
  await expect(option).toBeDisabled();
  await expect(option).toContainText(/Fill in first: .?Estimate/);
  await page.keyboard.press("Escape");
  await expect(panel.getByRole("note").filter({ hasText: /this can't change: .?Title/ })).toBeVisible();
});
