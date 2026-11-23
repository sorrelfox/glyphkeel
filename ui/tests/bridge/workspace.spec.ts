// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

const token = process.env.GK_BRIDGE_TOKEN ?? "";
const PAGE = `/?bridge=5321&token=${token}`;

async function openPreferences(page: Page) {
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /Preferences/ }).click();
  await expect(page.getByRole("dialog", { name: "Preferences" })).toBeVisible();
}

function day(page: Page, name: string) {
  return page.getByRole("radiogroup", { name: "First day of the week" }).getByRole("radio", { name });
}

test("two windows work on one workspace and follow each other's changes", async ({ browser }) => {
  const a = await browser.newPage();
  const b = await browser.newPage();
  await a.goto(PAGE);
  await b.goto(PAGE);

  // Data from the engine: the workspace's folder name.
  await expect(a.getByTestId("status-workspace")).toHaveText("Personal");
  await expect(b.getByTestId("status-workspace")).toHaveText("Personal");

  // An engine error, typed, as a toast.
  await a.keyboard.press("Control+z");
  await expect(a.getByRole("alert")).toContainText("Nothing to undo.");

  await openPreferences(a);
  await openPreferences(b);
  await expect(day(a, "Monday")).toHaveAttribute("aria-checked", "true");

  // B changes a setting; A shows it through the change event alone.
  await day(b, "Sunday").click();
  await expect(day(b, "Sunday")).toHaveAttribute("aria-checked", "true");
  await expect(day(a, "Sunday")).toHaveAttribute("aria-checked", "true");

  // Undo from B's toast takes it back in both.
  await b.getByRole("status").filter({ hasText: "Settings changed" }).getByRole("button", { name: "Undo" }).click();
  await expect(day(a, "Monday")).toHaveAttribute("aria-checked", "true");
  await expect(day(b, "Monday")).toHaveAttribute("aria-checked", "true");
});

test("a page with the wrong token gets no workspace", async ({ page }) => {
  await page.goto("/?bridge=5321&token=wrong");
  await expect(page.getByTestId("status-workspace")).toHaveText("Workspace unavailable");
});

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

function lane(page: Page, name: string) {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

// Runs after the flows above: it leaves changes on the Undo stack.
test("a project's board on the engine: create a project and a task, drag the task, Undo", async ({ page }) => {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");

  // A new project opens on its board, with the first column's title field ready.
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Garden shed");
  await expect(dialog.getByRole("textbox", { name: "Key" })).toHaveValue(/^[A-Z][A-Z0-9]{1,9}$/);
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  const names = page.locator("section[data-lane] h2");
  await expect(names).toHaveText(["Backlog", "To do", "In progress", "Review", "Done"]);

  await page.keyboard.type("Buy timber");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Buy timber" });
  await expect(lane(page, "Backlog").locator("[data-card]")).toHaveCount(1);
  const key = (await card.getAttribute("data-card")) ?? "";
  expect(key).toMatch(/^[A-Z][A-Z0-9]+-1$/);

  // Drag it into the empty In progress column.
  const from = await card.boundingBox();
  const target = await lane(page, "In progress").boundingBox();
  if (!from || !target) {
    throw new Error("the card or the target column isn't on screen");
  }
  await page.mouse.move(from.x + from.width / 2, from.y + 12);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2 + 10, from.y + 20, { steps: 4 });
  await page.mouse.move(target.x + target.width / 2, target.y + 120, { steps: 20 });
  await page.mouse.up();
  await expect(lane(page, "In progress").locator(`[data-card="${key}"]`)).toBeVisible();
  await expect.poll(async () => plain(await page.getByTestId("status-note").innerText())).toBe(`${key} moved to In progress`);

  // The engine has it: a fresh page shows the card where it was dropped.
  await page.reload();
  await page.getByRole("row", { name: /Garden shed/ }).click();
  await expect(lane(page, "In progress").locator(`[data-card="${key}"]`)).toBeVisible();

  // Undo takes the move back, through the engine's change event.
  await page.locator(`[data-card="${key}"]`).click();
  await page.keyboard.press("Control+z");
  await expect(lane(page, "Backlog").locator(`[data-card="${key}"]`)).toBeVisible();
  await expect(lane(page, "In progress").locator("[data-card]")).toHaveCount(0);

  // The personal layout: hide a column, then reset it.
  await lane(page, "Review").getByRole("button", { name: /column menu/ }).click();
  await page.getByRole("menuitem", { name: "Hide" }).click();
  await expect(names).toHaveText(["Backlog", "To do", "In progress", "Done"]);
  await page.getByRole("button", { name: /hidden column/ }).click();
  await page.getByRole("menuitem", { name: "Reset to project layout" }).click();
  await expect(names).toHaveText(["Backlog", "To do", "In progress", "Review", "Done"]);

  // The task panel edits through the engine.
  await page.locator(`[data-card="${key}"]`).dblclick();
  const panel = page.getByRole("complementary", { name: /^Task / });
  await expect(panel).toHaveAccessibleName(new RegExp(key));
  await panel.getByRole("button", { name: /^Priority:/ }).click();
  await page.getByRole("menuitem", { name: "High" }).click();
  await expect(panel.getByRole("button", { name: /^Priority:/ })).toHaveAccessibleName(/High/);
  await panel.getByRole("button", { name: /Estimate\u2069?:/ }).click();
  await page.keyboard.type("1:30");
  await page.keyboard.press("Enter");
  await expect(panel.getByRole("button", { name: /Estimate\u2069?:/ })).toHaveAccessibleName(/1h 30m/);
  await panel.getByRole("button", { name: "Close panel" }).click();

  // Project settings: a WIP limit shows on the column, a new status becomes a column.
  await page.getByRole("button", { name: /project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Statuses" }).click();
  const limit = settings.getByTestId("status-In progress").getByRole("textbox", { name: "WIP limit" });
  await limit.fill("2");
  await limit.press("Tab");
  await expect.poll(async () => plain(await lane(page, "In progress").getByTestId("lane-count").innerText())).toBe("0 / 2");
  const newStatus = settings.getByRole("textbox", { name: "New status" });
  await newStatus.fill("Waiting");
  await newStatus.press("Enter");
  await expect(names).toHaveText(["Backlog", "To do", "In progress", "Review", "Done", "Waiting"]);
});
