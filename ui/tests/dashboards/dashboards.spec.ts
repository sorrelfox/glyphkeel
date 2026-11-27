// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// Dashboards on the sample data, with the clock fixed: the empty state's
// starter dashboard, opening a widget's tasks as a list, arranging widgets
// by keyboard and by dragging, resizing, adding a note, a chart as a table,
// and no WCAG 2.2 AA violations.

const TODAY = "2026-11-11";

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

async function openDashboards(page: Page) {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /^Dashboards/ }).click();
  await expect(page.getByRole("region", { name: "Dashboards" })).toBeVisible();
}

/** A whole name, whatever isolation marks Fluent put around its placeables. */
const loose = (text: string) => new RegExp(`^[\u2068\u2069]*${Array.from(text).map((c) => c.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("[\u2068\u2069]*")}[\u2068\u2069]*$`);

const widgets = (page: Page) => page.getByRole("list", { name: "Widgets" }).locator(":scope > [data-widget]");
const kinds = (page: Page) => widgets(page).evaluateAll((items) => items.map((item) => item.getAttribute("data-widget")));

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).options({ rules: { "target-size": { enabled: true } } }).withTags(TAGS).analyze();
  expect(results.violations.map((violation) => `${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`)).toEqual([]);
}

test("the empty state makes a starter dashboard whose widgets show the sample data", async ({ page }) => {
  await openDashboards(page);
  await expect(page.getByText(/No dashboards yet/)).toBeVisible();
  await page.getByRole("button", { name: "Create a starter dashboard" }).click();
  await expect(widgets(page)).toHaveCount(6);
  expect(await kinds(page)).toEqual(["query_count", "overdue", "due_soon", "tasks_by_field", "created_closed", "activity"]);
  const count = page.getByRole("article", { name: "Open tasks" });
  await expect(count.locator("[data-widget-count]")).toHaveText(/^\d+$/);
  await expect(page.getByRole("table", { name: "Tasks by status" }).getByRole("row").first()).toBeVisible();
  await expect(page.getByRole("article", { name: "Created and closed" }).locator("[data-chart]")).toBeVisible();
  await axe(page);

  // A chart reads as a table too.
  const flow = page.getByRole("article", { name: "Created and closed" });
  await flow.getByText("Table", { exact: true }).click();
  await expect(flow.getByRole("table")).toBeVisible();

  // The count opens the list with its query.
  await count.getByRole("button", { name: "Open as a list" }).click();
  await expect(page.getByRole("grid").first()).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Filter query" })).toContainText("is:open");
});

test("widgets move and resize by keyboard and by dragging, and a note is added", async ({ page }) => {
  await openDashboards(page);
  await page.getByRole("button", { name: "Create a starter dashboard" }).click();
  await expect(widgets(page)).toHaveCount(6);

  // The keyboard: the handle's arrow keys move a widget a place.
  await page.getByRole("button", { name: loose("Move Overdue") }).focus();
  await page.keyboard.press("ArrowLeft");
  await expect.poll(() => kinds(page)).toEqual(["overdue", "query_count", "due_soon", "tasks_by_field", "created_closed", "activity"]);
  await expect(page.getByRole("button", { name: loose("Move Overdue") })).toBeFocused();

  // The width slider: the arrow keys make it wider.
  const slider = page.getByRole("slider", { name: loose("Width of Overdue") });
  await slider.focus();
  await page.keyboard.press("ArrowRight");
  await expect(widgets(page).first()).toHaveAttribute("data-width", "2");
  await expect(slider).toHaveAttribute("aria-valuenow", "2");

  // The menu does the same.
  await page.getByRole("button", { name: loose("Menu of Due soon") }).click();
  await page.getByRole("menuitem", { name: "Move earlier" }).click();
  await expect.poll(() => kinds(page)).toEqual(["overdue", "due_soon", "query_count", "tasks_by_field", "created_closed", "activity"]);

  // Dragging the handle of the third widget onto the first.
  await page.getByRole("list", { name: "Widgets" }).scrollIntoViewIfNeeded();
  const from = await page.getByRole("button", { name: loose("Move Open tasks") }).boundingBox();
  const to = await widgets(page).first().boundingBox();
  if (!from || !to) throw new Error("no boxes");
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(from.x + 20, from.y + 20, { steps: 4 });
  await page.mouse.move(to.x + 20, to.y + 20, { steps: 20 });
  await page.mouse.up();
  await expect.poll(async () => (await kinds(page))[0]).toBe("query_count");

  // A note, from the add dialog. The drag library swallows clicks for a
  // moment after a drop (so that the drop isn't also a click).
  await page.waitForTimeout(200);
  await page.getByRole("button", { name: "Add widget" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Add a widget" });
  await dialog.getByRole("button", { name: /^Shows/ }).click();
  await page.getByRole("option", { name: "Note" }).click();
  await dialog.getByLabel("Title").fill("Read me");
  await dialog.locator("[data-widget-text]").fill("Ship the **dashboards**.");
  await dialog.getByRole("button", { name: "Add" }).click();
  await expect(dialog).toHaveCount(0);
  const note = page.getByRole("article", { name: "Read me" });
  await expect(note.locator("strong")).toHaveText("dashboards");

  // Removing it, then Undo from the toast.
  await page.getByRole("button", { name: loose("Menu of Read me") }).click();
  await page.getByRole("menuitem", { name: "Remove" }).click();
  await expect(note).toHaveCount(0);
  await page.getByRole("button", { name: "Undo" }).last().click();
  await expect(page.getByRole("article", { name: "Read me" })).toBeVisible();
  await axe(page);
});

test("a dashboard pinned to a project shows its sprint's burndown", async ({ page }) => {
  await openDashboards(page);
  await page.getByRole("button", { name: "Create an empty one" }).click();
  await page.getByRole("button", { name: "Dashboard menu" }).click();
  await page.getByRole("menuitem", { name: "New dashboard" }).click();
  const dialog = page.getByRole("dialog", { name: "New dashboard" });
  await dialog.getByLabel("Name").fill("Team");
  await dialog.getByRole("button", { name: /^Pinned to/ }).click();
  await page.getByRole("option", { name: "Task Manager", exact: true }).click();
  await dialog.getByRole("button", { name: "Create" }).click();
  await expect(dialog).toHaveCount(0);
  await page.getByRole("button", { name: "Add widget" }).first().click();
  const add = page.getByRole("dialog", { name: "Add a widget" });
  await add.getByRole("button", { name: /^Shows/ }).click();
  await page.getByRole("option", { name: "Sprint burndown" }).click();
  await add.getByRole("button", { name: "Add" }).click();
  const burndown = page.getByRole("article", { name: "Sprint burndown" });
  await expect(burndown.locator("[data-chart]")).toBeVisible();
  await burndown.getByText("Table", { exact: true }).click();
  await expect(burndown.locator("[data-burndown-table] tbody tr").first()).toBeVisible();
  await axe(page);
});
