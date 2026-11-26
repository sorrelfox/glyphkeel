// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { mkdirSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

// The wiki on sample data: the page tree (dragging and the keyboard), a
// page with its outline and links, the editor with `[[` completion, the
// history, and the task panel's "Linked from". With GK_SCREENSHOT_DIR set,
// the flows save screenshots there.

const SHOTS = process.env.GK_SCREENSHOT_DIR;

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

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

const rows = async (tree: Locator) => (await tree.getByRole("row").allInnerTexts()).map((text) => text.trim().split("\n")[0]);

test("the tree moves pages by dragging and with the keyboard", async ({ page }) => {
  const tree = await openWiki(page);
  expect(await rows(tree)).toEqual(["Home", "Architecture", "Query language", "Storage", "Release plan"]);

  // Release plan dragged onto Home goes inside it.
  await tree.getByRole("row", { name: "Release plan" }).dragTo(tree.getByRole("row", { name: "Home" }));
  await expect(tree.getByRole("row", { name: "Release plan" })).toHaveAttribute("aria-level", "2");
  expect(await rows(tree)).toEqual(["Home", "Release plan", "Architecture", "Query language", "Storage"]);

  // Alt+Up moves Storage above Query language; Alt+Left takes it out of Architecture.
  await tree.getByRole("row", { name: "Storage" }).click();
  await page.keyboard.press("Alt+ArrowUp");
  await expect.poll(() => rows(tree)).toEqual(["Home", "Release plan", "Architecture", "Storage", "Query language"]);
  await expect(tree.getByRole("row", { name: "Storage" })).toBeFocused();
  await page.keyboard.press("Alt+ArrowLeft");
  await expect(tree.getByRole("row", { name: "Storage" })).toHaveAttribute("aria-level", "1");
  await shot(page, "wiki-tree-moved");
});

test("a page shows its outline and links, marks a missing page, and follows links", async ({ page }) => {
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Query language" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await expect(view.getByRole("heading", { name: "Grammar" })).toBeVisible();
  await expect(view.getByRole("navigation", { name: "On this page" }).getByRole("button")).toHaveText(["Query language", "Grammar", "Examples", "Progress"]);
  await expect(view.getByRole("region", { name: "Linked from" }).getByRole("button", { name: "Architecture" })).toBeVisible();
  await expect(view.getByRole("region", { name: "Tasks that link here" })).toContainText("TM-3");
  // Saved views has no page yet: the link is marked.
  await expect(view.locator('a[data-wiki="Saved views"]')).toHaveAttribute("data-missing", "");
  await shot(page, "wiki-page");

  // A link with a heading opens the other page.
  await view.locator('a[data-wiki="Release plan#Dates"]').click();
  await expect(page.getByRole("region", { name: /Wiki page/ }).getByRole("heading", { name: "Release plan", level: 2 })).toBeVisible();
});

test("the editor saves a page with a [[ link chosen from the completion, and the history shows the change", async ({ page }) => {
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Storage" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await expect(view.getByRole("heading", { name: "Storage", level: 1 })).toBeVisible();
  await view.getByRole("heading", { name: "Storage", level: 1 }).click();
  await page.keyboard.press("e");
  const editor = view.getByRole("textbox", { name: /Text of/ });
  await expect(editor).toBeFocused();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\nSee [[Query language#Gram");
  const option = page.getByRole("option", { name: /Query language#Grammar/ });
  await expect(option).toBeVisible();
  await shot(page, "wiki-editor-completion");
  await page.keyboard.press("Enter");
  await view.getByRole("button", { name: "Save" }).click();
  await expect(view.locator('a[data-wiki="Query language#Grammar"]')).toBeVisible();

  await view.getByRole("button", { name: "History" }).click();
  const diff = view.getByRole("group", { name: "Differences between the two revisions" });
  await expect(diff.locator("ins").filter({ hasText: "Query language#Grammar" })).toBeVisible();
  await shot(page, "wiki-history");
});

test("the task panel lists the pages that link to the task", async ({ page }) => {
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Release plan" }).click();
  // The task the page links to opens beside it.
  await page.getByRole("region", { name: "Links on this page" }).getByRole("button", { name: /TM-1/ }).click();
  const panel = page.getByRole("complementary", { name: /^Task\W+TM-1\W*$/ });
  const section = panel.getByRole("region", { name: "Linked from" });
  await section.scrollIntoViewIfNeeded();
  await expect(section.getByRole("button")).toHaveCount(1);
  expect(plain(await section.getByRole("button").innerText())).toContain("Release plan");
  await shot(page, "task-linked-from");
  // From the panel, the page opens in the wiki.
  await tree.getByRole("row", { name: "Home" }).click();
  await section.getByRole("button").click();
  await expect(page.getByRole("region", { name: /Wiki page/ }).getByRole("heading", { name: "Release plan", level: 2 })).toBeVisible();
});

test.describe("in the pseudo-locale", () => {
  test.use({ locale: "en-XA" });

  test("the wiki screen, for the screenshot", async ({ page }) => {
    await page.goto("/?wiki");
    // Project names are sample data, not translated.
    await page.getByRole("row", { name: "Task Manager" }).click();
    await expect(page.getByRole("heading", { name: "Task Manager", level: 1 })).toBeVisible();
    // The view switcher's fifth button: Board, List, Calendar, Gantt, Wiki.
    await page.locator("header .seg button").nth(4).click();
    // The second tree is the wiki's, after the sidebar's projects.
    await expect(page.getByRole("treegrid")).toHaveCount(2);
    await page.getByRole("treegrid").last().getByRole("row").nth(2).click();
    await expect(page.locator("section[data-wiki-page] article")).toBeVisible();
    await shot(page, "wiki-pseudo");
  });
});
