// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// The sprints page on the sample data: Task Manager has Sprint 1
// (completed), Sprint 2 (running, started five days ago) and Sprint 3
// (planned). The clock is fixed, so the sample data is the same on every
// run.

const TODAY = "2026-11-11";

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");

async function openSprints(page: Page) {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "Sprints" }).click();
  await expect(page.getByRole("region", { name: /Sprints of/ })).toBeVisible();
}

/** A whole name, whatever isolation marks Fluent put around its placeables. */
const loose = (text: string, whole = true) => {
  const body = [...text].map((c) => c.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("[⁨⁩]*");
  return new RegExp(whole ? `^[⁨⁩]*${body}[⁨⁩]*$` : `^[⁨⁩]*${body}`);
};

const card = (page: Page, name: string) => page.getByRole("article", { name, exact: true });
const list = (page: Page, name: string) => page.getByRole("grid", { name: loose(name) });

test("the active sprint shows its progress and burndown; completed sprints on request", async ({ page }) => {
  await openSprints(page);
  await expect(card(page, "Sprint 2")).toBeVisible();
  await expect(card(page, "Sprint 3")).toBeVisible();
  await expect(card(page, "Sprint 1")).toHaveCount(0);
  await expect(card(page, "Sprint 2").getByRole("progressbar")).toBeVisible();
  const chart = card(page, "Sprint 2").locator("[data-chart]");
  await expect(chart).toBeVisible();
  // The arrow keys read the days one by one.
  await chart.focus();
  await page.keyboard.press("Home");
  await expect(card(page, "Sprint 2").locator("[data-chart-readout]")).toContainText(/open/);
  await card(page, "Sprint 2").getByRole("radio", { name: "Estimate" }).click();
  await expect(card(page, "Sprint 2").getByRole("radio", { name: "Estimate" })).toBeChecked();
  // Sprint 3 can't start while Sprint 2 runs.
  await expect(card(page, "Sprint 3").getByRole("button", { name: loose("Start Sprint 3", false) })).toBeDisabled();
  await page.getByText(/Show completed sprints/).click();
  await expect(card(page, "Sprint 1")).toBeVisible();
});

test("tasks move from the backlog to a sprint by choice and by dragging, and back", async ({ page }) => {
  await openSprints(page);
  const backlog = list(page, "Backlog");
  const first = backlog.getByRole("row").first();
  const key = plain(await first.locator(".key").textContent());
  // Choose it and move it to Sprint 3.
  await first.locator("label").first().click();
  await expect(first.getByRole("checkbox")).toBeChecked();
  await page.getByRole("button", { name: "Move to" }).click();
  await page.getByRole("menuitem", { name: "Sprint 3" }).click();
  await expect(list(page, "Tasks of Sprint 3").getByRole("row", { name: loose(`${key} `, false) })).toBeVisible();
  // Drag another one onto the planned sprint, just above.
  const second = backlog.getByRole("row").first();
  const other = plain(await second.locator(".key").textContent());
  await second.dragTo(list(page, "Tasks of Sprint 3").getByRole("row").last());
  await expect(list(page, "Tasks of Sprint 3").getByRole("row", { name: loose(`${other} `, false) })).toBeVisible();
  // Undo takes it back to the backlog.
  await page.getByRole("button", { name: "Undo" }).last().click();
  await expect(backlog.getByRole("row", { name: loose(`${other} `, false) })).toBeVisible();
  // The task panel shows the sprint, and the list filters by it.
  await list(page, "Tasks of Sprint 3").getByRole("row", { name: loose(`${key} `, false) }).click();
  await expect(page.getByRole("button", { name: loose("Sprint: Sprint 3") })).toBeVisible();
});

test("a sprint is planned, the running one completed with its open tasks moved, and the next one started", async ({ page }) => {
  await openSprints(page);
  await page.getByRole("button", { name: "New sprint" }).click();
  const dialog = page.getByRole("dialog", { name: "New sprint" });
  await expect(dialog.getByLabel("Name")).toHaveValue("Sprint 4");
  await dialog.getByLabel("Goal").fill("Polish");
  await dialog.getByRole("button", { name: "Create sprint" }).click();
  await expect(card(page, "Sprint 4")).toBeVisible();

  // Complete Sprint 2: its open tasks go to Sprint 3.
  const before = await list(page, "Tasks of Sprint 3").getByRole("row").count();
  await card(page, "Sprint 2").getByRole("button", { name: loose("Complete Sprint 2") }).click();
  const complete = page.getByRole("dialog", { name: /Complete/ });
  await expect(complete.getByText(/still open/)).toBeVisible();
  await complete.getByRole("button", { name: "Complete sprint" }).click();
  await expect(card(page, "Sprint 2")).toHaveCount(0);
  expect(await list(page, "Tasks of Sprint 3").getByRole("row").count()).toBeGreaterThan(before);

  // Start Sprint 3 now that none runs.
  await card(page, "Sprint 3").getByRole("button", { name: loose("Start Sprint 3") }).click();
  const start = page.getByRole("dialog", { name: loose("Start Sprint 3") });
  await start.getByRole("button", { name: "Start sprint" }).click();
  await expect(card(page, "Sprint 3").getByText("Active", { exact: true })).toBeVisible();
  await expect(card(page, "Sprint 3").locator("[data-chart]")).toBeVisible();
});

test("the board filters by sprint, and the list by its sprint menu", async ({ page }) => {
  await openSprints(page);
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "Board" }).click();
  const cards = page.locator("[data-card]");
  await expect(cards.first()).toBeVisible();
  const all = await cards.count();
  await page.getByRole("button", { name: "Sprint", exact: true }).click();
  await page.getByRole("menuitemcheckbox", { name: /Sprint 3/ }).click();
  await page.keyboard.press("Escape");
  await expect.poll(() => cards.count()).toBeLessThan(all);
  await expect(cards.first()).toBeVisible();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  await page.getByRole("button", { name: loose("Sprint: Any sprint") }).click();
  await page.getByRole("menuitemradio", { name: "Active sprint" }).click();
  await expect(page.getByRole("textbox", { name: "Filter query" })).toContainText("sprint:active");
  await expect(page.getByRole("button", { name: loose("Sprint: Active sprint") })).toBeVisible();
});

test("the sprints page has no accessibility findings", async ({ page }) => {
  await openSprints(page);
  await page.getByText(/Show completed sprints/).click();
  await expect(card(page, "Sprint 1")).toBeVisible();
  const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`)).toEqual([]);
});
