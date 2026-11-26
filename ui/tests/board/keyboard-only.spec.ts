// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// The list, the trash and Preferences without a mouse: every step below is a
// key press, on the sample workspace.

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

/** The focused element's accessible name, roughly: its label or its text. */
function focusedName(page: Page): Promise<string> {
  return page.evaluate(() => {
    const element = document.activeElement as HTMLElement | null;
    const labelled = element?.getAttribute("aria-labelledby");
    const label = labelled ? document.getElementById(labelled)?.textContent : null;
    return (element?.getAttribute("aria-label") ?? label ?? element?.textContent ?? "").replace(/[⁨⁩]/g, "");
  });
}

/** Presses Tab (or Shift+Tab) until the focus is on an element whose name matches, as a person would. */
async function tabTo(page: Page, name: RegExp, key: "Tab" | "Shift+Tab" = "Tab"): Promise<void> {
  for (let step = 0; step < 40; step += 1) {
    if (name.test(await focusedName(page))) {
      return;
    }
    await page.keyboard.press(key);
  }
  throw new Error(`nothing named ${String(name)} within 40 presses of ${key}`);
}

/** Presses Tab until the focus is inside `target`. */
async function tabInto(page: Page, target: Locator): Promise<void> {
  for (let step = 0; step < 40; step += 1) {
    if (await target.evaluate((element) => element.contains(document.activeElement))) {
      return;
    }
    await page.keyboard.press("Tab");
  }
  throw new Error("the focus never reached the element within 40 presses of Tab");
}

/** The row that has the focus: its place and its task's key. */
function focusedRow(page: Page): Promise<{ index: number; key: string }> {
  return page.evaluate(() => {
    const row = document.activeElement?.closest<HTMLElement>("[data-list-scroll] [data-index]");
    return { index: Number(row?.dataset.index ?? -1), key: row?.querySelector(".key")?.textContent ?? "" };
  });
}

/** Opens a project's list from the command palette: Ctrl+K, its key, Enter, then Ctrl+2. */
async function openList(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
  await page.keyboard.press("Control+k");
  await page.keyboard.type("TM");
  await expect(page.getByRole("option").first()).toContainText("Task Manager");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.keyboard.press("Control+2");
  await expect(page.getByRole("grid")).toBeVisible();
}

test("the list without a mouse: filter, walk the rows, select, open, trash and undo", async ({ page }) => {
  await openList(page);

  // Ctrl+F puts the cursor in the query; Enter runs it and goes to the rows.
  await page.keyboard.press("Control+f");
  await expect(page.getByRole("textbox", { name: "Filter query" })).toBeFocused();
  await page.keyboard.type("priority:high");
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("list-banner")).toBeVisible();
  // The rows the query found replace the others; the focus stays on the first row.
  await expect.poll(() => focusedRow(page)).toMatchObject({ index: 0 });
  const shown = Number(/^(\d+)/.exec(plain(await page.getByRole("contentinfo").innerText()))?.[1] ?? 0);
  expect(shown).toBeGreaterThan(3);

  // Arrows walk the rows; Shift+Down adds the next row to the selection.
  await page.keyboard.press("ArrowDown");
  await expect.poll(async () => (await focusedRow(page)).index).toBe(1);
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(async () => (await focusedRow(page)).index).toBe(2);
  const status = async () => plain(await page.getByRole("contentinfo").innerText());
  await expect.poll(status).toContain("2 selected");
  await page.keyboard.press("Escape");
  await expect.poll(status).not.toContain("selected");
  await page.keyboard.press("Home");
  await expect.poll(async () => (await focusedRow(page)).index).toBe(0);
  await page.keyboard.press("End");
  await expect.poll(async () => (await focusedRow(page)).index).toBe(shown - 1);
  await page.keyboard.press("Home");
  await expect.poll(async () => (await focusedRow(page)).index).toBe(0);

  // Enter opens the task; F6 goes into it; Escape closes it and gives the
  // focus back to its row.
  const { key } = await focusedRow(page);
  await page.keyboard.press("Enter");
  const panel = page.getByRole("complementary", { name: new RegExp(key) });
  await expect(panel).toBeVisible();
  await page.keyboard.press("F6");
  await expect.poll(() => panel.evaluate((element) => element.contains(document.activeElement))).toBe(true);
  await page.keyboard.press("Escape");
  await expect(panel).toBeHidden();
  await expect.poll(async () => (await focusedRow(page)).key).toBe(key);

  // Delete moves it to the trash; Ctrl+Z brings it back.
  await page.keyboard.press("Delete");
  await expect(page.locator(".toasts")).toContainText("trash");
  await expect(page.locator("[data-list-scroll] .key", { hasText: new RegExp(`^${key}$`) })).toHaveCount(0);
  await page.keyboard.press("Control+z");
  await expect(page.locator("[data-list-scroll] .key", { hasText: new RegExp(`^${key}$`) })).toHaveCount(1);
});

test("the trash without a mouse: a task goes in from the list and comes back", async ({ page }) => {
  await openList(page);
  await page.keyboard.press("Control+f");
  await page.keyboard.press("Enter");
  await expect.poll(() => focusedRow(page)).toMatchObject({ index: 0 });
  await page.keyboard.press("ArrowDown");
  const { key } = await focusedRow(page);
  await page.keyboard.press("Delete");
  await expect(page.locator(".toasts")).toContainText("trash");

  // The trash, from the command palette.
  await page.keyboard.press("Control+k");
  await page.keyboard.type("Trash");
  await expect(page.getByRole("option", { name: /Trash/ })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { name: "Trash" })).toBeVisible();

  // F6 goes to the screen; Tab to the task's box (the only item in the
  // trash), Space ticks it, then back to Restore.
  const item = page.getByRole("checkbox", { name: /^Select (?!everything shown)/ });
  await expect(item).toHaveCount(1);
  await page.keyboard.press("F6");
  await tabTo(page, /^Select (?!everything shown)/);
  await page.keyboard.press("Space");
  await expect(item).toBeChecked();
  await tabTo(page, /^Restore$/, "Shift+Tab");
  await page.keyboard.press("Enter");
  await expect(item).toHaveCount(0);

  // Back in the list.
  await page.keyboard.press("Control+k");
  await page.keyboard.type(key);
  await expect(page.getByRole("option").first()).toContainText(key);
  await page.keyboard.press("Enter");
  await expect(page.getByRole("complementary", { name: new RegExp(key) })).toBeVisible();
});

test("Preferences without a mouse: pages, a setting, the shortcut search, and back", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
  const accent = () => page.evaluate(() => document.documentElement.dataset.accent);
  const before = await accent();

  await page.keyboard.press("Control+,");
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await expect(dialog).toBeVisible();

  // The pages are tabs: arrows move between them.
  await tabTo(page, /^Appearance$/);
  await page.keyboard.press("ArrowDown");
  await expect(dialog.getByRole("tab", { name: "Language" })).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("ArrowUp");
  await expect(dialog.getByRole("tab", { name: "Appearance" })).toHaveAttribute("aria-selected", "true");

  // An accent colour, chosen with the arrows, applies at once.
  // The first choice is the desktop's own colour, the next ones are the swatches.
  await tabInto(page, dialog.getByRole("radiogroup", { name: "Accent colour" }));
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await expect.poll(accent).not.toBe(before);

  // The shortcuts page: its search field narrows the list as one types.
  await tabTo(page, /^Appearance$/, "Shift+Tab");
  for (let step = 0; step < 10 && !/^Keyboard shortcuts$/.test(await focusedName(page)); step += 1) {
    await page.keyboard.press("ArrowDown");
  }
  await expect(dialog.getByRole("tab", { name: "Keyboard shortcuts" })).toHaveAttribute("aria-selected", "true");
  await tabTo(page, /^Search shortcuts/);
  await expect(dialog.getByText("Redo", { exact: true })).toBeVisible();
  await page.keyboard.type("command");
  await expect(dialog.getByText("Search or run a command")).toBeVisible();
  await expect(dialog.getByText("Redo", { exact: true })).toHaveCount(0);

  // Escape closes Preferences; the choice stays.
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  expect(await accent()).not.toBe(before);
});
