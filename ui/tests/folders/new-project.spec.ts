// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

// A new project in the selected folder, on the sample data: a folder picked
// in the sidebar presets the new-project dialog's Folder field, as does an
// open project in a folder and the folder's own menu; with nothing picked
// the project goes to the top level. Also by keyboard alone, with no
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

async function newFolder(page: Page, name: string) {
  await page.getByRole("button", { name: "New folder" }).click();
  const dialog = page.getByRole("dialog", { name: "New folder" });
  await dialog.getByRole("textbox", { name: "Name" }).fill(name);
  await dialog.getByRole("button", { name: "Create folder" }).click();
  await expect(dialog).toBeHidden();
}

function newProjectDialog(page: Page): Locator {
  return page.getByRole("dialog", { name: "New project" });
}

/** The project's place in the tree: its level and the folder row it sits under, if any. */
async function folderOf(tree: Locator, name: string): Promise<string | null> {
  return tree.getByRole("row", { name, exact: true }).evaluate((row) => {
    let level = Number(row.getAttribute("aria-level"));
    let previous = row.previousElementSibling;
    while (previous !== null && level > 1) {
      const at = Number(previous.getAttribute("aria-level"));
      if (at < level) {
        level = at;
        if (level === 1) {
          return previous.hasAttribute("data-folder") ? previous.textContent.replace(/\d+$/, "").trim() : null;
        }
      }
      previous = previous.previousElementSibling;
    }
    return null;
  });
}

test("a new project goes into the folder picked in the sidebar, or the open project's, and can be changed", async ({ page }) => {
  await page.goto("/");
  const tree = projectTree(page);
  await expect(tree.getByRole("row", { name: "Home lab" })).toBeVisible();
  await newFolder(page, "Clients");

  // Clicking the folder's row selects it, shown like an open project.
  const clients = tree.getByRole("row", { name: "Clients" });
  await clients.click();
  await expect(clients.locator("div.nav")).toHaveClass(/\bon\b/);
  await page.getByRole("button", { name: "New project" }).click();
  let dialog = newProjectDialog(page);
  await expect(dialog.getByRole("button", { name: "Folder: Clients" })).toBeVisible();
  await dialog.getByRole("textbox", { name: "Name" }).fill("Orchard");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(dialog).toBeHidden();
  await expect(tree.getByRole("row", { name: "Orchard" })).toHaveAttribute("aria-level", "2");
  expect(await folderOf(tree, "Orchard")).toBe("Clients");
  // The new project is open; the folder row is no longer the selection.
  await expect(tree.getByRole("row", { name: "Orchard" }).locator("div.nav")).toHaveClass(/\bon\b/);
  await expect(clients.locator("div.nav")).not.toHaveClass(/\bon\b/);

  // The open project is in Clients, so the next one starts there too; changed to no folder, it goes to the top level.
  await page.getByRole("button", { name: "New project" }).click();
  dialog = newProjectDialog(page);
  await dialog.getByRole("button", { name: "Folder: Clients" }).click();
  await page.getByRole("option", { name: "No folder" }).click();
  await expect(dialog.getByRole("button", { name: "Folder: No folder" })).toBeVisible();
  await dialog.getByRole("textbox", { name: "Name" }).fill("Meadow");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(dialog).toBeHidden();
  await expect(tree.getByRole("row", { name: "Meadow" })).toHaveAttribute("aria-level", "1");
  expect(await folderOf(tree, "Meadow")).toBeNull();

  // Undo takes the project away in one step.
  await tree.getByRole("row", { name: "Meadow" }).focus();
  await page.keyboard.press("Control+z");
  await expect(tree.getByRole("row", { name: "Meadow" })).toHaveCount(0);
  await expect(tree.getByRole("row", { name: "Orchard" })).toHaveAttribute("aria-level", "2");
});

test("with no folder selected a new project goes to the top level; the folder's menu starts one in it", async ({ page }) => {
  await page.goto("/");
  const tree = projectTree(page);
  await expect(tree.getByRole("row", { name: "Home lab" })).toBeVisible();
  await newFolder(page, "Clients");

  await page.getByRole("button", { name: "New project" }).click();
  let dialog = newProjectDialog(page);
  await expect(dialog.getByRole("button", { name: "Folder: No folder" })).toBeVisible();
  await dialog.getByRole("textbox", { name: "Name" }).fill("Loose end");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(dialog).toBeHidden();
  await expect(tree.getByRole("row", { name: "Loose end" })).toHaveAttribute("aria-level", "1");
  expect(await folderOf(tree, "Loose end")).toBeNull();

  // A sub-project follows its parent: its dialog has no Folder field.
  await tree.getByRole("row", { name: "Home lab" }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "New sub-project…" }).click();
  dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("button", { name: /^Folder/ })).toHaveCount(0);
  await dialog.getByRole("button", { name: "Cancel" }).click();

  await tree.getByRole("row", { name: "Clients" }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "New project in this folder" }).click();
  dialog = newProjectDialog(page);
  await expect(dialog.getByRole("button", { name: "Folder: Clients" })).toBeVisible();
  await dialog.getByRole("textbox", { name: "Name" }).fill("Greenhouse");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(dialog).toBeHidden();
  expect(await folderOf(tree, "Greenhouse")).toBe("Clients");
});

test("by keyboard alone: focusing a folder selects it, and its menu and the + button start a project in it", async ({ page }) => {
  await page.goto("/");
  const tree = projectTree(page);
  await expect(tree.getByRole("row", { name: "Home lab" })).toBeVisible();
  await newFolder(page, "Clients");

  // From the + button, Tab goes into the tree, on its first row: the folder, now selected.
  const plus = page.getByRole("button", { name: "New project" });
  await plus.focus();
  await page.keyboard.press("Tab");
  const clients = tree.getByRole("row", { name: "Clients" });
  await expect(clients).toBeFocused();
  await expect(clients.locator("div.nav")).toHaveClass(/\bon\b/);
  await page.keyboard.press("Shift+Tab");
  await expect(plus).toBeFocused();
  await page.keyboard.press("Enter");
  const dialog = newProjectDialog(page);
  await expect(dialog.getByRole("button", { name: "Folder: Clients" })).toBeVisible();
  await axe(page);
  await page.keyboard.type("Keyboard farm");
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
  expect(await folderOf(tree, "Keyboard farm")).toBe("Clients");

  // The folder's menu by Shift+F10.
  await clients.focus();
  await page.keyboard.press("Shift+F10");
  await expect(page.getByRole("menuitem", { name: "New project in this folder" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog.getByRole("button", { name: "Folder: Clients" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  // The chevron still opens and closes the folder: the arrow keys on its row.
  await clients.focus();
  await page.keyboard.press("ArrowLeft");
  await expect(tree.getByRole("row", { name: "Keyboard farm" })).toHaveCount(0);
  await page.keyboard.press("ArrowRight");
  await expect(tree.getByRole("row", { name: "Keyboard farm" })).toBeVisible();
  await axe(page);
});
