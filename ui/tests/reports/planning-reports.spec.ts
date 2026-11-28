// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// The planning reports on the sample data, with the clock fixed: velocity
// from the sprints page, the burn-up from the roadmap, estimate accuracy,
// each as a chart that reads by keyboard and as a table with the same
// figures, without accessibility findings, and right to left.

const TODAY = "2026-11-11";

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");

const readout = (page: Page) => page.locator("[data-chart-readout]");

async function noFindings(page: Page) {
  const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`)).toEqual([]);
}

async function openProject(page: Page, view: "Sprints" | "Roadmap") {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: view }).click();
}

test("velocity opens from the sprints page; its chart, table and planning hint agree", async ({ page }) => {
  await openProject(page, "Sprints");
  // Complete the running sprint, so there are two completed ones.
  await page.getByRole("article", { name: "Sprint 2", exact: true }).getByRole("button", { name: /Complete/ }).click();
  await page.getByRole("dialog", { name: /Complete/ }).getByRole("button", { name: "Complete sprint" }).click();
  await expect(page.getByRole("article", { name: "Sprint 2", exact: true })).toHaveCount(0);
  await expect(page.locator("[data-plan-hint]")).toContainText(/Planned: .+ · average velocity: /);

  await page.getByRole("button", { name: "Velocity", exact: true }).click();
  await expect(page.getByRole("tab", { name: "Velocity", selected: true })).toBeVisible();
  const chart = page.getByRole("group", { name: /Work committed and completed per sprint of/ });
  await expect(chart.getByRole("img")).toBeVisible();
  await expect(page.locator("[data-bar=committed]")).toHaveCount(2);
  await expect(page.locator("[data-bar=completed]")).toHaveCount(2);
  await expect(page.locator("[data-line=average]")).toHaveCount(1);

  // By keyboard: each sprint's figures.
  await chart.focus();
  await page.keyboard.press("Home");
  const first = plain(await readout(page).textContent());
  expect(first).toMatch(/^Sprint 1: \d+ committed, \d+ completed, average [\d.]+$/);
  await page.keyboard.press("ArrowRight");
  await expect.poll(async () => plain(await readout(page).textContent())).toMatch(/^Sprint 2: /);
  const second = plain(await readout(page).textContent());
  await noFindings(page);

  // The table says the same.
  await page.getByText("Show as a table", { exact: true }).click();
  const table = page.getByRole("table", { name: /Work committed and completed/ });
  const rows = table.locator("tbody tr");
  await expect(rows).toHaveCount(2);
  const cells = async (i: number) => (await rows.nth(i).locator("th, td").allTextContents()).map(plain);
  const [name, committed, completed, average] = await cells(0);
  expect(first).toBe(`${name ?? ""}: ${committed ?? ""} committed, ${completed ?? ""} completed, average ${average ?? ""}`);
  const [name2, committed2, completed2] = await cells(1);
  expect(second.startsWith(`${name2 ?? ""}: ${committed2 ?? ""} committed, ${completed2 ?? ""} completed`)).toBe(true);
  await expect(page.locator("[data-plan-hint]")).toContainText(/Sprint 3/);

  // Estimated hours instead of tasks.
  await page.getByRole("radio", { name: "Estimated hours" }).click();
  await expect(rows.first().locator("td").first()).toHaveText(/h|hr/);
});

test("the burn-up opens from the roadmap, with its table of the same days", async ({ page }) => {
  await openProject(page, "Roadmap");
  await page.getByRole("button", { name: /Burn-up of .?1\.0/ }).click();
  await expect(page.getByRole("tab", { name: "Burn-up", selected: true })).toBeVisible();
  const chart = page.getByRole("group", { name: /Scope and completed work of/ });
  await expect(chart.getByRole("img")).toBeVisible();
  await expect(page.locator("[data-line=scope]")).toHaveCount(1);
  await expect(page.locator("[data-line=completed]")).toHaveCount(1);
  await chart.focus();
  await page.keyboard.press("Home");
  await expect.poll(async () => plain(await readout(page).textContent())).toMatch(/: [\d.]+ of [\d.]+ completed$/);
  const firstDay = plain(await readout(page).textContent());
  await noFindings(page);

  await page.getByText("Show as a table", { exact: true }).click();
  const table = page.getByRole("table", { name: /Scope and completed work of/ });
  const [date, scope, completed] = (await table.locator("tbody tr").first().locator("th, td").allTextContents()).map(plain);
  expect(firstDay).toBe(`${date ?? ""}: ${completed ?? ""} of ${scope ?? ""} completed`);

  // The running sprint's burn-up from the version-or-sprint choice.
  await page.getByRole("button", { name: /^Version or sprint/ }).click();
  await page.getByRole("listbox").getByRole("option", { name: /^Sprint .?Sprint 2.?$/ }).click();
  await expect(page.getByRole("table", { name: /Scope and completed work of .?Sprint 2/ })).toBeVisible();
});

test("estimate accuracy: the histogram, its table, the groups and the outliers", async ({ page }) => {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("navigation").getByText("Reports", { exact: true }).click();
  await page.getByRole("tab", { name: "Estimate accuracy" }).click();
  await page.getByRole("button", { name: /^Last .?365.? days$/ }).click();
  const chart = page.getByRole("group", { name: /Closed tasks by time spent against their estimate/ });
  await expect(chart.getByRole("img")).toBeVisible();
  await expect(page.locator("[data-bar=on-target]")).toHaveCount(1);
  await expect(page.locator("[data-bar=off-target]")).toHaveCount(5);
  const count = Number(plain(await page.locator("[data-stat=plan-stat-tasks] dd").textContent()).replace(/\D/g, ""));
  expect(count).toBeGreaterThan(0);
  await noFindings(page);

  await page.getByText("Show as a table", { exact: true }).click();
  const bins = page.getByRole("table", { name: /Closed tasks by time spent/ });
  const numbers = (await bins.locator("tbody td").allTextContents()).map((text) => Number(plain(text).replace(/\D/g, "")));
  expect(numbers.reduce((sum, n) => sum + n, 0)).toBe(count);
  await expect(page.getByRole("table", { name: /Estimate accuracy by/ }).locator("tbody tr").first()).toBeVisible();

  // An outlier opens its task.
  const outlier = page.getByRole("table", { name: "Furthest off" }).getByRole("button").first();
  const key = plain(await outlier.locator(".key").textContent());
  await outlier.click();
  await expect(page.getByText(key, { exact: true }).first()).toBeVisible();
});

test("right to left: the sprints run from the right", async ({ browser }) => {
  const context = await browser.newContext({ locale: "he-IL", viewport: { width: 1400, height: 900 } });
  const page = await context.newPage();
  await openProject(page, "Sprints");
  await page.getByRole("article", { name: "Sprint 2", exact: true }).getByRole("button", { name: /Complete/ }).click();
  await page.getByRole("dialog", { name: /Complete/ }).getByRole("button", { name: "Complete sprint" }).click();
  await page.getByRole("button", { name: "Velocity", exact: true }).click();
  const bars = page.locator("[data-bar=completed]");
  await expect(bars).toHaveCount(2);
  const xs = await bars.evaluateAll((all) => all.map((bar) => Number(bar.getAttribute("x"))));
  expect(xs[0]).toBeGreaterThan(xs[1] ?? 0);
  await context.close();
});

test("planning widgets on a dashboard show the reports", async ({ page }) => {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /^Dashboards/ }).click();
  await page.getByRole("button", { name: "Create a starter dashboard" }).click();
  for (const kind of ["Sprint velocity", "Burn-up", "Estimate accuracy"]) {
    await page.waitForTimeout(200);
    await page.getByRole("button", { name: "Add widget" }).first().click();
    const dialog = page.getByRole("dialog", { name: "Add a widget" });
    await dialog.getByRole("button", { name: /^Shows/ }).click();
    await page.getByRole("option", { name: kind, exact: true }).click();
    if (kind !== "Estimate accuracy") {
      await dialog.getByRole("button", { name: /^Project/ }).click();
      await page.getByRole("option", { name: "Task Manager", exact: true }).click();
    }
    await dialog.locator("[data-widget-save]").click();
    await expect(dialog).toHaveCount(0);
  }
  const velocity = page.getByRole("article", { name: "Sprint velocity" });
  await expect(velocity.getByRole("group", { name: /Work committed and completed/ })).toBeVisible();
  await expect(page.getByRole("article", { name: "Burn-up" }).getByRole("group", { name: /Scope and completed work of .?Sprint 2/ })).toBeVisible();
  await expect(page.getByRole("article", { name: "Estimate accuracy" }).locator("[data-plan-figures]")).toBeVisible();
  await noFindings(page);
});

test.describe("dark", () => {
  test.use({ colorScheme: "dark" });

  test("the planning reports take the theme's colours and keep without findings", async ({ page }) => {
    await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
    await page.goto("/");
    await expect(page.locator("html")).toHaveAttribute("data-theme", /dark|slate/);
    await page.getByRole("navigation").getByText("Reports", { exact: true }).click();
    for (const [tab, mark] of [
      ["Velocity", "[data-bar=completed]"],
      ["Burn-up", "[data-line=completed]"],
      ["Estimate accuracy", "[data-bar=on-target]"],
    ] as const) {
      await page.getByRole("tab", { name: tab }).click();
      await expect(page.locator(mark).first()).toBeVisible();
      // The series are drawn in the theme's own hues.
      const fill = await page.locator(mark).first().evaluate((el) => getComputedStyle(el).getPropertyValue(el.tagName === "path" ? "stroke" : "fill"));
      expect(fill).not.toBe("rgb(0, 0, 0)");
      await noFindings(page);
    }
  });
});

test("Preferences › Time switches the estimate hint on and off", async ({ page }) => {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  // A bug: the sample's closed bugs ran over their estimates.
  await page.locator('[data-card="TM-13"]').click();
  await page.keyboard.press("Enter");
  await expect(page.getByText(/^Estimate$/).first()).toBeVisible();
  const hint = page.locator("[data-estimate-hint]");
  await expect(hint).toHaveCount(0);

  const toggle = async () => {
    await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /Preferences/ }).click();
    const dialog = page.getByRole("dialog", { name: "Preferences" });
    await dialog.getByRole("tab", { name: "Time" }).click();
    // The switch is the row's control: by keyboard, as its box is drawn over the hidden input.
    await dialog.getByRole("switch", { name: "Show how estimates usually turn out" }).focus();
    await page.keyboard.press("Space");
    await dialog.getByRole("button", { name: "Close" }).click();
    await expect(dialog).toHaveCount(0);
  };
  await toggle();
  await expect.poll(async () => plain(await hint.textContent())).toMatch(/^Your Bug estimates usually run [\d.]+× over \(\d+ closed tasks\)\.$/);
  await toggle();
  await expect(hint).toHaveCount(0);
});
