// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { mkdirSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

// Wiki macros, page templates and word-level history diffs, on sample data.
// With GK_SCREENSHOT_DIR set, the flows save screenshots there.

const SHOTS = process.env.GK_SCREENSHOT_DIR;

async function shot(page: Page, name: string): Promise<void> {
  if (!SHOTS) {
    return;
  }
  mkdirSync(SHOTS, { recursive: true });
  await page.waitForTimeout(300);
  await page.screenshot({ path: join(SHOTS, `${name}.png`) });
}

/** Opens Task Manager's wiki on the sample data. */
async function openWiki(page: Page): Promise<Locator> {
  await page.goto("/?wiki");
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "Wiki" }).click();
  const tree = page.getByRole("treegrid", { name: "Pages" });
  await expect(tree.getByRole("row")).toHaveCount(5);
  return tree;
}

/** Replaces the open page's text through the editor and saves it. */
async function writePage(page: Page, view: Locator, text: string): Promise<void> {
  await view.getByRole("button", { name: /^Edit/ }).click();
  const editor = view.getByRole("textbox", { name: /Text of/ });
  await expect(editor).toBeFocused();
  await page.keyboard.press("Control+A");
  await page.keyboard.press("Delete");
  await page.keyboard.insertText(text);
  await view.getByRole("button", { name: "Save" }).click();
  await expect(view.getByRole("button", { name: /^Edit/ })).toBeVisible();
}

test("a page's macros show its outline, subpages, an included page and a live task list", async ({ page }) => {
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Architecture" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await writePage(page, view, "# Overview\n\n{{toc}}\n\n## Parts\n\n{{children}}\n\n{{include Release plan}}\n\n{{tasks is:open}}\n\n{{include Nowhere}}\n\n{{tasks colour:red}}\n");
  const article = view.locator("article");
  await expect(article.locator('[data-macro="toc"] a[data-wiki="#Parts"]')).toBeVisible();
  await expect(article.locator('[data-macro="children"] a[data-wiki="Query language"]')).toBeVisible();
  await expect(article.locator('[data-macro="include"]').first().getByRole("heading", { name: "Dates" })).toBeVisible();
  const tasks = article.locator('[data-macro="tasks"]');
  await expect(tasks.locator("a[data-ref]").first()).toBeVisible();
  await expect(article.getByText(/There is no page\W+Nowhere\W+to include/)).toBeVisible();
  // A query's mistake is worded from its message, inside the sentence about the list.
  await expect(article.getByText(/The task list can.t be shown: \W*Unknown field\W+colour/)).toBeVisible();
  await shot(page, "wiki-macros");

  // A subpage listed by {{children}} opens on a click.
  await article.locator('[data-macro="children"] a[data-wiki="Storage"]').click();
  await expect(page.getByRole("region", { name: /Wiki page\W+Storage/ })).toBeVisible();
});

test("the editor offers the macros after {{ at the start of a line", async ({ page }) => {
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Storage" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await view.getByRole("button", { name: /^Edit/ }).click();
  await expect(view.getByRole("textbox", { name: /Text of/ })).toBeFocused();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\n\n{{");
  await expect(page.getByRole("option", { name: /\{\{toc\}\}/ })).toBeVisible();
  await page.keyboard.press("Enter");
  await view.getByRole("button", { name: "Save" }).click();
  await expect(view.locator('article [data-macro="toc"]')).toBeVisible();
});

test("a page marked as a template starts a new page, and the history marks the changed words", async ({ page }) => {
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Storage" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await view.getByRole("button", { name: "Page menu" }).click();
  await page.getByRole("menuitem", { name: "Use as a template" }).click();
  await expect(tree.getByRole("row", { name: "Storage" })).toContainText("Template");
  await expect(view.getByRole("navigation", { name: "Place in the wiki" })).toContainText("Template");

  await page.getByRole("complementary", { name: "Pages" }).getByRole("button", { name: "New page" }).click();
  const dialog = page.getByRole("dialog", { name: "New page" });
  await dialog.getByRole("textbox", { name: "Title" }).fill("Storage notes");
  await dialog.getByRole("button", { name: "Start from" }).click();
  await page.getByRole("option", { name: "Storage" }).click();
  await shot(page, "wiki-new-from-template");
  await dialog.getByRole("button", { name: "Create" }).click();
  // The new page opens in the editor with the template's text.
  const made = page.getByRole("region", { name: /Wiki page\W+Storage notes/ });
  const editor = made.getByRole("textbox", { name: /Text of/ });
  await expect(editor).toContainText("One SQLite file per workspace.");
  await expect(tree.getByRole("row", { name: "Storage notes" })).not.toContainText("Template");
  await made.getByRole("button", { name: "Save" }).click();
  await expect(made.getByRole("heading", { name: "Storage", level: 1 })).toBeVisible();

  // One word changed in a line: the history marks that word.
  await made.getByRole("button", { name: /^Edit/ }).click();
  await expect(editor).toBeFocused();
  await page.keyboard.press("Control+Home");
  await page.keyboard.press("End");
  await page.keyboard.type(" layer");
  await made.getByRole("button", { name: "Save" }).click();
  await made.getByRole("button", { name: "History" }).click();
  const diff = made.getByRole("group", { name: "Differences between the two revisions" });
  await expect(diff.locator("ins mark[data-diff-word]")).toHaveText([/layer/]);
  await shot(page, "wiki-word-diff");
});
