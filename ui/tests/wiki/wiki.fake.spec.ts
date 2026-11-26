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

test("the drag handle moves a page with the keyboard", async ({ page }) => {
  const tree = await openWiki(page);
  // The row's handle is reached with the Right arrow; Enter starts a drag.
  await tree.getByRole("row", { name: "Release plan" }).click();
  await page.keyboard.press("ArrowRight");
  await expect(tree.getByRole("button", { name: /Drag\W+Release plan/ })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("button", { name: /Insert after\W+Release plan/ })).toBeFocused();
  // The Up arrow walks through the places to drop it, onto pages and between them.
  const seen: string[] = [];
  for (let step = 0; step < 20; step += 1) {
    await page.keyboard.press("ArrowUp");
    const label = plain((await page.evaluate(() => document.activeElement?.getAttribute("aria-label"))) ?? "");
    seen.push(label);
    if (label === "Drop on Architecture") {
      break;
    }
  }
  expect(seen.at(-1), seen.join(" / ")).toBe("Drop on Architecture");
  await page.keyboard.press("Enter");
  await expect(tree.getByRole("row", { name: "Release plan" })).toHaveAttribute("aria-level", "2");
  expect(await rows(tree)).toEqual(["Home", "Architecture", "Query language", "Storage", "Release plan"]);
  // Escape gives a drag up: nothing moves.
  await tree.getByRole("row", { name: "Home" }).click();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Enter");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Escape");
  expect(await rows(tree)).toEqual(["Home", "Architecture", "Query language", "Storage", "Release plan"]);
});

test("a page shows its outline and links, marks a missing page, and follows links", async ({ page }) => {
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Query language" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await expect(view.getByRole("heading", { name: "Grammar" })).toBeVisible();
  await expect(view.getByRole("navigation", { name: "On this page" }).locator("button[data-level]")).toHaveText(["Query language", "Grammar", "Examples", "Progress"]);
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

test("a checkbox on a page is ticked in the reader and saved as a revision", async ({ page }) => {
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Query language" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  const box = view.locator("article").getByRole("checkbox", { name: /Formal grammar/ });
  await expect(box).not.toBeChecked();
  await box.click();
  await expect(box).toBeChecked();
  await view.getByRole("button", { name: "History" }).click();
  const diff = view.getByRole("group", { name: "Differences between the two revisions" });
  await expect(diff.locator("ins").filter({ hasText: "[x] Formal grammar" })).toBeVisible();
});

test("the formatting toolbar sits in the page bar while editing, and the page menu exports the page", async ({ page }) => {
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Storage" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await view.getByRole("button", { name: /^Edit/ }).click();
  const toolbar = view.locator("[data-toolbar-slot]").getByRole("toolbar", { name: "Formatting" });
  await expect(toolbar).toBeVisible();
  await expect(view.getByRole("toolbar")).toHaveCount(1);
  await expect(view.getByRole("textbox", { name: /Text of/ })).toBeFocused();
  await shot(page, "wiki-editor-toolbar");
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\nloud");
  await page.keyboard.press("Shift+Home");
  await toolbar.getByRole("button", { name: "Bold" }).click();
  await view.getByRole("button", { name: "Save" }).click();
  await expect(view.locator("article strong").filter({ hasText: "loud" })).toBeVisible();
  await expect(view.locator("[data-toolbar-slot]")).toHaveCount(0);

  await view.getByRole("button", { name: "Page menu" }).click();
  await page.getByRole("menuitem", { name: "Export…" }).click();
  let dialog = page.getByRole("dialog", { name: /Export\W+Storage/ });
  await expect(dialog.getByRole("radio", { name: "With the pages inside" })).toHaveCount(0);
  await dialog.getByRole("button", { name: "Export" }).click();
  await expect(page.getByText("Page exported as Markdown")).toBeVisible();
  // A page with pages inside offers them too.
  await tree.getByRole("row", { name: "Architecture" }).click();
  await view.getByRole("button", { name: "Page menu" }).click();
  await page.getByRole("menuitem", { name: "Export…" }).click();
  dialog = page.getByRole("dialog", { name: /Export\W+Architecture/ });
  await dialog.getByRole("radio", { name: "With the pages inside" }).click();
  await dialog.getByRole("button", { name: "Export" }).click();
  await expect(page.getByRole("region", { name: "Notifications" })).toContainText(/3\W+pages exported as Markdown/);
});

test("a page becomes the agent brief, a save carries a note, a heading's link is copied, and the whole wiki exports", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Storage" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await view.getByRole("button", { name: "Page menu" }).click();
  await page.getByRole("menuitem", { name: "Use as the agent brief" }).click();
  await expect(tree.getByRole("row", { name: "Storage" })).toContainText("Agent brief");
  await expect(view.getByRole("navigation", { name: "Place in the wiki" })).toContainText("Agent brief");

  await view.getByRole("button", { name: /^Edit/ }).click();
  await expect(view.getByRole("textbox", { name: /Text of/ })).toBeFocused();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\nMore.");
  await view.getByRole("textbox", { name: "Note about this change" }).fill("Explain the file");
  await view.getByRole("button", { name: "Save" }).click();
  await view.getByRole("button", { name: "History" }).click();
  await expect(view.getByRole("list", { name: "Revisions" })).toContainText("Explain the file");
  await view.getByRole("button", { name: "History" }).click();

  await view.getByRole("navigation", { name: "On this page" }).getByRole("button", { name: /Copy a link to\W+Storage/ }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("[[Storage#Storage]]");

  await view.getByRole("button", { name: "Page menu" }).click();
  await page.getByRole("menuitem", { name: "Export…" }).click();
  const dialog = page.getByRole("dialog", { name: /Export\W+Storage/ });
  await dialog.getByRole("radio", { name: "The whole wiki" }).click();
  await dialog.getByRole("radio", { name: "Web pages" }).click();
  await shot(page, "wiki-export-dialog");
  await dialog.getByRole("button", { name: "Export" }).click();
  await expect(page.getByRole("region", { name: "Notifications" })).toContainText(/5\W+pages exported as web pages/);
});

test("the wiki opens on its home page, a page takes files, and a folder imports as pages that Undo takes back", async ({ page }) => {
  const tree = await openWiki(page);
  await tree.getByRole("row", { name: "Storage" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await view.getByRole("button", { name: "Page menu" }).click();
  await page.getByRole("menuitem", { name: "Make this the home page" }).click();
  await expect(tree.getByRole("row", { name: "Storage" }).locator("[data-home]")).toHaveCount(1);
  await expect(view.getByRole("navigation", { name: "Place in the wiki" })).toContainText("Home page");
  // Back to the wiki from elsewhere: it opens on the home page.
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "Board" }).click();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "Wiki" }).click();
  await expect(page.getByRole("region", { name: /Wiki page\W+Storage/ })).toBeVisible();

  // A file chosen for the page is attached to it and listed beside it.
  await view.locator("input[data-attach-field]").setInputFiles({ name: "layout.txt", mimeType: "text/plain", buffer: Buffer.from("pages and files") });
  await expect(view.getByRole("button", { name: /Open\W+layout\.txt/ })).toBeVisible();
  await shot(page, "wiki-page-files");

  await page.getByRole("complementary", { name: "Pages" }).getByRole("button", { name: "Import a folder of Markdown files…" }).click();
  const dialog = page.getByRole("dialog", { name: "Import a folder of Markdown files" });
  await dialog.getByRole("textbox", { name: "Folder in the exports folder" }).fill("notes");
  await dialog.getByRole("button", { name: "Check" }).click();
  await expect(dialog.getByTestId("wiki-import-summary")).toContainText(/2\W+pages\W+and one attached file/);
  await shot(page, "wiki-import-dialog");
  await dialog.getByRole("button", { name: /Import\W+2\W+pages/ }).click();
  await expect(tree.getByRole("row", { name: "Meeting notes" })).toBeVisible();
  await expect(page.getByRole("region", { name: /Wiki page\W+Meeting notes/ })).toBeVisible();
  await page.getByRole("region", { name: "Notifications" }).getByRole("button", { name: "Undo" }).click();
  await expect(tree.getByRole("row", { name: "Meeting notes" })).toHaveCount(0);
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
    // The view switcher's last button: Wiki.
    await page.locator("header .seg button").last().click();
    // The second tree is the wiki's, after the sidebar's projects.
    await expect(page.getByRole("treegrid")).toHaveCount(2);
    await page.getByRole("treegrid").last().getByRole("row").nth(2).click();
    await expect(page.locator("section[data-wiki-page] article")).toBeVisible();
    await shot(page, "wiki-pseudo");
  });
});
