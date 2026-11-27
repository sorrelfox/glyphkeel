// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

// Project folders in the sidebar on the sample data: making one, putting
// projects in by dragging and by the menu, ordering by keyboard, closing a
// folder (remembered), renaming, deleting with Undo, right to left, and no
// WCAG 2.2 AA violations.

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).options({ rules: { "target-size": { enabled: true } } }).withTags(TAGS).analyze();
  expect(results.violations.map((violation) => `${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`)).toEqual([]);
}

function projectTree(page: Page): Locator {
  return page.getByRole("treegrid", { name: "Projects" }).or(page.getByRole("tree", { name: "Projects" }));
}

const rows = async (tree: Locator) => (await tree.getByRole("row").allInnerTexts()).map((text) => text.trim().split("\n")[0]);

async function newFolder(page: Page, name: string) {
  await page.getByRole("button", { name: "New folder" }).click();
  const dialog = page.getByRole("dialog", { name: "New folder" });
  await dialog.getByRole("textbox", { name: "Name" }).fill(name);
  await dialog.getByRole("button", { name: "Create folder" }).click();
  await expect(dialog).toBeHidden();
}

test("folders group projects by dragging, by the menu and by keyboard, and closing one is remembered", async ({ page }) => {
  await page.goto("/");
  const tree = projectTree(page);
  await expect(tree.getByRole("row", { name: "Home lab" })).toBeVisible();
  await newFolder(page, "Clients");
  await expect(tree.getByRole("row", { name: "Clients" })).toHaveAttribute("aria-level", "1");
  expect(await rows(tree)).toEqual(["Clients", "Task Manager", "CLI and MCP", "Docs site", "Personal", "Home lab"]);
  await axe(page);

  // Dragged onto the folder, Home lab goes into it.
  await tree.getByRole("row", { name: "Home lab" }).dragTo(tree.getByRole("row", { name: "Clients" }));
  await expect(tree.getByRole("row", { name: "Home lab" })).toHaveAttribute("aria-level", "2");

  // Task Manager by the menu; its sub-projects come along.
  await tree.getByRole("row", { name: "Task Manager" }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Move to folder" }).click();
  await page.getByRole("menuitem", { name: "Clients" }).click();
  await expect(tree.getByRole("row", { name: "Task Manager" })).toHaveAttribute("aria-level", "2");
  await expect(tree.getByRole("row", { name: "Docs site" })).toHaveAttribute("aria-level", "3");
  expect(await rows(tree)).toEqual(["Clients", "Home lab", "Task Manager", "CLI and MCP", "Docs site", "Personal"]);

  // Alt+Up moves Task Manager above Home lab, inside the folder.
  await tree.getByRole("row", { name: "Task Manager" }).focus();
  await page.keyboard.press("Alt+ArrowUp");
  await expect.poll(() => rows(tree)).toEqual(["Clients", "Task Manager", "CLI and MCP", "Docs site", "Home lab", "Personal"]);

  // The folder's count adds up its projects' open tasks.
  await expect(tree.getByRole("row", { name: "Clients" }).locator(".count")).toHaveText(/^\d+$/);

  // Closed, the folder hides its projects, and the person's settings remember it.
  await tree.getByRole("row", { name: "Clients" }).getByRole("button", { name: /Clients/ }).click();
  await expect(tree.getByRole("row", { name: "Home lab" })).toHaveCount(0);
  await expect(tree.getByRole("row", { name: "Clients" })).not.toHaveAttribute("aria-expanded", "true");
  const remembered = await page.evaluate(() => JSON.parse(localStorage.getItem("glyphkeel.sidebar.collapsedFolders") ?? "[]") as unknown);
  expect(remembered).toHaveLength(1);
  await axe(page);
});

test("a folder is renamed and deleted, its projects going back to the top level, and Undo brings it back", async ({ page }) => {
  await page.goto("/");
  const tree = projectTree(page);
  await newFolder(page, "Clients");
  await tree.getByRole("row", { name: "Personal" }).dragTo(tree.getByRole("row", { name: "Clients" }));
  await expect(tree.getByRole("row", { name: "Personal" })).toHaveAttribute("aria-level", "2");

  // Renamed from its menu, opened with the keyboard.
  await tree.getByRole("row", { name: "Clients" }).focus();
  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Rename…" }).click();
  const dialog = page.getByRole("dialog", { name: "Rename folder" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Customers");
  await dialog.locator('[data-hue="teal"]').click();
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(tree.getByRole("row", { name: "Customers" })).toBeVisible();

  await tree.getByRole("row", { name: "Customers" }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Delete folder" }).click();
  await expect(tree.getByRole("row", { name: "Customers" })).toHaveCount(0);
  await expect(tree.getByRole("row", { name: "Personal" })).toHaveAttribute("aria-level", "1");
  await expect(page.getByText(/Folder deleted/)).toBeVisible();
  await page.getByRole("button", { name: "Undo" }).last().click();
  await expect(tree.getByRole("row", { name: "Personal" })).toHaveAttribute("aria-level", "2");
});

test("right to left: a project in a folder is indented from the right", async ({ browser }) => {
  const context = await browser.newContext({ locale: "he-IL", viewport: { width: 1400, height: 900 } });
  const page = await context.newPage();
  await page.goto("/");
  const tree = projectTree(page);
  await expect(tree.getByRole("row", { name: "Home lab" })).toBeVisible();
  await newFolder(page, "Clients");
  await tree.getByRole("row", { name: "Home lab" }).dragTo(tree.getByRole("row", { name: "Clients" }));
  const row = tree.getByRole("row", { name: "Home lab" });
  await expect(row).toHaveAttribute("aria-level", "2");
  const padding = await row.locator("div.nav").evaluate((element) => {
    const style = getComputedStyle(element);
    return { start: parseFloat(style.paddingRight), end: parseFloat(style.paddingLeft) };
  });
  expect(padding.start).toBeGreaterThan(padding.end);
  await axe(page);
  await context.close();
});
