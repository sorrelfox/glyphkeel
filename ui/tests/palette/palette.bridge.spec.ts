// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

const BASE = process.env.GK_PALETTE_UI_URL ?? "";
const QUERY = `?bridge=${process.env.GK_PALETTE_BRIDGE_PORT ?? ""}&token=${process.env.GK_BRIDGE_TOKEN ?? ""}`;
const PAGE = `${BASE}/${QUERY}`;
const QUICK_ADD = `${BASE}/quick-add${QUERY}`;
const SHOTS = process.env.GK_SCREENSHOT_DIR;

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

/** The key of the project made below, as the engine chose it. */
let projectKey = "";

async function openPalette(page: Page) {
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await expect(palette).toBeVisible();
  await expect(palette.getByRole("combobox", { name: "Search tasks, projects and actions" })).toBeFocused();
  return palette;
}

test("the quick-add page creates tasks on the engine from one line", async ({ page }) => {
  // A project to add to, made in the main window.
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Garden shed");
  projectKey = await dialog.getByRole("textbox", { name: "Key" }).inputValue();
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();

  // The quick-add window's page, at the window's size.
  await page.setViewportSize({ width: 600, height: 136 });
  await page.goto(QUICK_ADD);
  const field = page.getByRole("textbox", { name: "New task" });
  await expect(field).toBeFocused();
  await expect(page.getByRole("button", { name: /Project/ })).toContainText("Garden shed");

  await field.pressSequentially("Buy timber for the floor fri !high #gardenshed");
  await expect(page.getByTestId("quick-add-due")).toContainText(/^Due /);
  await expect(page.getByTestId("quick-add-priority")).toHaveText("High");
  if (SHOTS !== undefined) {
    await page.screenshot({ path: join(SHOTS, "quick-add.png") });
  }
  await field.press("Enter");
  await expect.poll(async () => plain(await page.getByRole("status").innerText())).toBe(`${projectKey}-1 added`);
  await expect(field).toHaveValue("");

  await field.pressSequentially("Paint the door tomorrow");
  await field.press("Enter");
  await expect.poll(async () => plain(await page.getByRole("status").innerText())).toBe(`${projectKey}-2 added`);

  // Escape empties the line without adding anything.
  await field.pressSequentially("Never mind");
  await field.press("Escape");
  await expect(field).toHaveValue("");

  // The engine has them, with the date and priority read from the line.
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto(PAGE);
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("row", { name: /Garden shed/ }).click();
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Buy timber for the floor" });
  await expect(card).toBeVisible();
  await expect(page.locator("section[data-lane] [data-card]")).toHaveCount(2);
});

test("the command palette: open, search a task, jump by key, run an action", async ({ page }) => {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");

  // Search by words: the engine's search finds the task; Enter opens it.
  let palette = await openPalette(page);
  await page.keyboard.type("timber");
  const found = palette.getByRole("option", { name: new RegExp(`${projectKey}-1`) });
  await expect(found).toContainText("Buy timber for the floor");
  await page.keyboard.press("ArrowDown");
  await expect(found).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("Enter");
  await expect(palette).toBeHidden();
  await expect(page.getByRole("heading", { level: 1, name: "Buy timber for the floor" })).toBeVisible();

  // A key puts its task first.
  palette = await openPalette(page);
  await page.keyboard.type(`${projectKey.toLowerCase()}-2`);
  await expect(palette.getByRole("option").first()).toContainText("Paint the door");
  // Recently opened, with an empty field.
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Backspace");
  await expect(palette.getByRole("group", { name: "Recent" }).getByRole("option").first()).toContainText(`${projectKey}-1`);

  // For the screenshot: actions, tasks and a project for one word.
  await page.keyboard.type("the");
  await expect(palette.getByRole("group", { name: "Tasks" })).toBeVisible();
  if (SHOTS !== undefined) {
    await page.screenshot({ path: join(SHOTS, "palette.png") });
  }

  // Run an action from the registry, by keyboard only.
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Backspace");
  await page.keyboard.type("sidebar");
  await expect(palette.getByRole("option", { name: /Show or hide the sidebar/ })).toContainText("F9");
  await page.keyboard.press("Enter");
  await expect(palette).toBeHidden();
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeHidden();

  // Escape closes without doing anything.
  palette = await openPalette(page);
  await page.keyboard.press("Escape");
  await expect(palette).toBeHidden();
});
