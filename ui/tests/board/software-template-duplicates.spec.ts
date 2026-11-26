// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// The built-in "Software project" template and what project settings show
// of a template; closing a task with open duplicates from the list's status
// cell and from a bulk change, on the sample workspace.

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

async function addTask(page: Page, column: string, title: string): Promise<string> {
  await lane(page, column).getByRole("button", { name: /Add task to/ }).click();
  await page.keyboard.type(title);
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: title });
  await expect(card).toBeVisible();
  return (await card.getAttribute("data-card")) ?? "";
}

/** Marks task `copy` (by title) as a duplicate of `original` (by key), in its panel. */
async function markDuplicate(page: Page, copy: string, original: string) {
  await page.locator("section[data-lane] [data-card]", { hasText: copy }).dblclick();
  const panel = page.getByRole("complementary", { name: /Task/ });
  await expect(panel.getByRole("heading", { name: copy })).toBeVisible();
  await panel.getByRole("button", { name: "Add link" }).click();
  await panel.getByRole("button", { name: /Kind of link/ }).click();
  await page.getByRole("option", { name: "Duplicates" }).click();
  await panel.getByRole("textbox", { name: "Task key" }).fill(original);
  await panel.getByRole("textbox", { name: "Task key" }).press("Enter");
  await expect(panel.locator(`[data-link="duplicates ${original}"]`)).toBeVisible();
  await page.keyboard.press("Escape");
}

function row(page: Page, key: string): Locator {
  return page.locator("[data-list-scroll] [data-row]").filter({ has: page.locator(".key", { hasText: new RegExp(`^${key}$`) }) });
}

async function statusOf(page: Page, key: string): Promise<string> {
  return plain((await row(page, key).getByRole("button", { name: /^Status of / }).getAttribute("aria-label")) ?? "");
}

test("a project made from the built-in Software project template has epics, stories and a review column", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /New project/ }).first().click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("textbox").first().fill("Web shop");
  await dialog.getByRole("button", { name: /Start from/ }).click();
  await page.getByRole("option", { name: "Software project" }).click();
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(dialog).toBeHidden();

  await page.getByRole("row", { name: /Web shop/ }).first().click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  const lanes = page.locator("section[data-lane] h2");
  await expect(lanes).toHaveText(["Backlog", "To do", "In progress", "In review", "Done", "Won't do"]);

  await page.getByRole("button", { name: /Web shop.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Task types" }).click();
  await expect(settings.locator("svg.t-epic").first()).toBeVisible();
  await expect(settings.locator("svg.t-story").first()).toBeVisible();

  // The template itself: it comes with the program, and shows what it holds.
  await settings.getByRole("tab", { name: "Templates" }).click();
  const templates = settings.getByRole("region", { name: "Project templates" });
  const builtIn = templates.locator('[data-template="Software project"]');
  await expect(builtIn).toContainText("Comes with Glyphkeel");
  await expect(builtIn.getByRole("button", { name: /^Delete/ })).toHaveCount(0);
  await builtIn.getByRole("button", { name: /^Contents of \W*Software project/ }).click();
  const contents = templates.locator('[data-template-contents="Software project"]');
  await expect(contents.locator("[data-template-status]")).toHaveText([/Backlog/, /To do/, /In progress/, /In review/, /Done/, /Won't do/]);
  await expect(contents.locator('[data-template-status="Done"] [data-template-rules]')).toHaveText(/3\W* rules/);
  await expect(contents).toContainText("Bug report");

  // A task template of the new project shows its contents too.
  const own = settings.getByRole("region", { name: "Task templates of this project" });
  await own.getByRole("button", { name: /^Contents of \W*Bug report/ }).click();
  await expect(own.locator('[data-template-contents="Bug report"]')).toContainText("High");
});

test("closing from the list's status cell and from the bulk bar asks about the duplicates", async ({ page }) => {
  await openBoard(page, /Home lab/);
  const original = await addTask(page, "To do", "Router drops the line");
  const copy = await addTask(page, "To do", "No internet at night");
  await markDuplicate(page, "No internet at night", original);

  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  // Only the two, so closing them doesn't move them out of sight.
  await page.getByRole("textbox", { name: "Filter query" }).click();
  await page.keyboard.type(`${original} OR ${copy}`);
  await page.keyboard.press("Enter");
  await expect(page.locator("[data-list-scroll] [data-row]")).toHaveCount(2);
  await expect(row(page, original)).toBeVisible();

  // The status cell: Done, and the question.
  await row(page, original).getByRole("button", { name: /^Status of / }).click();
  await page.getByRole("menuitem", { name: "Done" }).click();
  const ask = page.getByRole("dialog", { name: "Close the duplicate too?" });
  await expect(ask).toBeVisible();
  await expect(ask).toContainText(copy);
  await ask.getByRole("button", { name: "Close it too" }).click();
  await expect(ask).toBeHidden();
  await expect.poll(() => statusOf(page, copy)).toContain("Done");
  await expect.poll(() => statusOf(page, original)).toContain("Done");

  // One Undo opens both again.
  await page.keyboard.press("Control+z");
  await expect.poll(() => statusOf(page, copy)).toContain("To do");
  await expect.poll(() => statusOf(page, original)).toContain("To do");

  // A bulk change asks the same, here to leave the duplicate open.
  await row(page, original).locator(".key").click();
  const bar = page.getByTestId("bulk-bar");
  await expect.poll(async () => plain(await bar.innerText())).toContain("1 selected");
  await bar.getByRole("button", { name: /Status…/ }).click();
  await page.getByRole("option", { name: "Done", exact: true }).click();
  await bar.getByRole("button", { name: /^Change / }).click();
  await expect(ask).toBeVisible();
  await ask.getByRole("button", { name: "Leave it open" }).click();
  await expect.poll(() => statusOf(page, original)).toContain("Done");
  await expect.poll(() => statusOf(page, copy)).toContain("To do");
});
