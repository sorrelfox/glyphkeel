// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// A column's WIP minimum, closing a task with its duplicates, and kinds of
// work on logged time, on the sample workspace.

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

function lane(page: Page, name: string): Locator {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

async function openBoard(page: Page, project: RegExp) {
  await page.goto("/");
  await page.getByRole("row", { name: project }).first().click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
}

/** Adds a task to a column and returns its key. */
async function addTask(page: Page, column: string, title: string): Promise<string> {
  await lane(page, column).getByRole("button", { name: /Add task to/ }).click();
  await page.keyboard.type(title);
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: title });
  await expect(card).toBeVisible();
  return (await card.getAttribute("data-card")) ?? "";
}

async function openPanel(page: Page, title: string): Promise<Locator> {
  await page.locator("section[data-lane] [data-card]", { hasText: title }).dblclick();
  const panel = page.getByRole("complementary", { name: /Task/ });
  await expect(panel.getByRole("heading", { name: title })).toBeVisible();
  return panel;
}

test("a column below its WIP minimum shows its count in amber", async ({ page }) => {
  await openBoard(page, /Home lab/);
  const count = lane(page, "Backlog").getByTestId("lane-count");
  await expect(count).not.toHaveClass(/under/);

  await page.getByRole("button", { name: /Home lab.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Statuses" }).click();
  const minimum = settings.getByTestId("status-Backlog").getByRole("textbox", { name: "WIP minimum" });
  await minimum.fill("999");
  await minimum.press("Enter");
  await page.keyboard.press("Escape");
  await expect(settings).toBeHidden();

  await expect(count).toHaveClass(/under/);
  await expect.poll(async () => plain(await count.innerText())).toMatch(/^\d+ \/ 999\+$/);
  await expect(lane(page, "Backlog")).toHaveAttribute("aria-label", /below its WIP minimum of \W*999/);
});

test("closing a task asks to close its duplicates too, and one Undo opens them again", async ({ page }) => {
  await openBoard(page, /Home lab/);
  const original = await addTask(page, "To do", "Crash on start");
  const copy = await addTask(page, "To do", "Crashes when it starts");

  // Mark the copy as a duplicate of the original, in its panel's links.
  const panel = await openPanel(page, "Crashes when it starts");
  await panel.getByRole("button", { name: "Add link" }).click();
  await panel.getByRole("button", { name: /Kind of link/ }).click();
  await page.getByRole("option", { name: "Duplicates" }).click();
  await panel.getByRole("textbox", { name: "Task key" }).fill(original);
  await panel.getByRole("textbox", { name: "Task key" }).press("Enter");
  await expect(panel.locator(`[data-link="duplicates ${original}"]`)).toBeVisible();
  await page.keyboard.press("Escape");

  // Done, with the keyboard: Alt and an arrow move the card a column on.
  const card = page.locator(`[data-card="${original}"]`);
  await card.focus();
  for (const column of ["In progress", "Review"]) {
    await page.keyboard.press("Alt+ArrowRight");
    await expect(lane(page, column).locator(`[data-card="${original}"]`)).toBeVisible();
  }
  await page.keyboard.press("Alt+ArrowRight");
  const ask = page.getByRole("dialog", { name: "Close the duplicate too?" });
  await expect(ask).toBeVisible();
  await expect(ask).toContainText(copy);
  await ask.getByRole("button", { name: "Close it too" }).click();
  await expect(ask).toBeHidden();
  await expect(lane(page, "Done").locator(`[data-card="${original}"]`)).toBeVisible();
  await expect(lane(page, "Done").locator(`[data-card="${copy}"]`)).toBeVisible();

  await page.keyboard.press("Control+z");
  await expect(lane(page, "Review").locator(`[data-card="${original}"]`)).toBeVisible();
  await expect(lane(page, "To do").locator(`[data-card="${copy}"]`)).toBeVisible();
});

test("time is marked with a kind of work, which the time report groups by", async ({ page }) => {
  await openBoard(page, /Home lab/);
  await addTask(page, "To do", "Sketch the logo");
  const panel = await openPanel(page, "Sketch the logo");
  await panel.getByRole("tab", { name: "Time" }).click();
  await panel.getByRole("button", { name: "Log time" }).click();
  const form = panel.getByRole("group", { name: "Log time" });
  await form.getByRole("textbox", { name: "Duration" }).fill("2h");
  await form.getByRole("button", { name: /Kind of work/ }).click();
  await page.getByRole("option", { name: "Design" }).click();
  await form.getByRole("button", { name: "Log", exact: true }).click();
  const entries = panel.getByRole("list", { name: "Time entries" });
  await expect(entries.locator('[data-work-kind="Design"]')).toBeVisible();

  await page.getByRole("button", { name: "Time report" }).click();
  await page.getByRole("button", { name: /Group by/ }).click();
  await page.getByRole("option", { name: "Kind of work" }).click();
  const table = page.getByRole("table", { name: /Time logged/ });
  await expect(table.getByRole("row", { name: /Design/ })).toContainText("2h");
  await page.getByRole("button", { name: /^Kind of work/ }).click();
  await page.getByRole("option", { name: "No kind of work" }).click();
  await expect(table.getByRole("row", { name: /Design/ })).toHaveCount(0);
});
