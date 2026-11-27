// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { readFile } from "node:fs/promises";
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

// The flow reports on the sample data, with the clock fixed: the charts, the
// keyboard read-out, the tables, the filters, CSV, and right-to-left.

const TODAY = "2026-11-11";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");

async function openReports(page: Page) {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("navigation").getByText("Reports", { exact: true }).click();
  await expect(page.getByRole("region", { name: "Reports" })).toBeVisible();
}

const readout = (page: Page) => page.locator("[data-chart-readout]");

/** With GK_SCREENSHOT_DIR set, saves a picture of the screen there. */
async function shot(page: Page, name: string) {
  const dir = process.env.GK_SCREENSHOT_DIR;
  if (dir) {
    await page.screenshot({ path: join(dir, name) });
  }
}

test("created against closed: chart, keyboard read-out, table and CSV", async ({ page }) => {
  await openReports(page);
  await page.getByRole("button", { name: "Last 12 weeks" }).click();
  await expect(page.getByLabel("First day")).toHaveValue("2026-08-24");
  const chart = page.getByRole("group", { name: /Tasks created and closed/ });
  await expect(chart.getByRole("img")).toBeVisible();
  await expect(page.locator("[data-bar=created]")).toHaveCount(12);

  // Only the keyboard: the chart takes the focus and reads out each period.
  await chart.focus();
  await page.keyboard.press("Home");
  const first = plain(await readout(page).textContent());
  expect(first).toMatch(/^Mon, Aug 24, 2026|Aug 24 – 30/);
  await page.keyboard.press("ArrowRight");
  await expect(readout(page)).not.toHaveText(first);
  await expect.poll(async () => plain(await readout(page).textContent())).toMatch(/\d+ created, \d+ closed, \d+ reopened, \d+ open/);
  await shot(page, "reports-created-closed.png");
  await page.keyboard.press("Escape");
  await expect(readout(page)).toContainText(/tasks? created/);

  await page.getByText("Show as a table", { exact: true }).click();
  const table = page.getByRole("table", { name: /Tasks created and closed/ });
  await expect(table.locator("tbody tr")).toHaveCount(12);

  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export as CSV" }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe("created-closed-2026-08-24-2026-11-11.csv");
  const text = await readFile(await file.path(), "utf8");
  expect(text.split("\r\n")[0]).toBe("start,end,created,closed,reopened,open");
  expect(text.split("\r\n").filter(Boolean)).toHaveLength(13);
});

test("cumulative flow by status and by category; cycle time with its tasks", async ({ page }) => {
  await openReports(page);
  await page.getByRole("button", { name: "Last 30 days" }).click();
  await page.getByRole("tab", { name: "Cumulative flow" }).click();
  await expect(page.getByRole("group", { name: /Tasks in each status/ })).toBeVisible();
  const bands = page.locator("[data-band]");
  expect(await bands.count()).toBeGreaterThan(1);
  await shot(page, "reports-cumulative.png");
  await page.getByRole("radio", { name: "By category" }).click();
  await expect(bands).toHaveCount(4);
  await expect(page.locator("figure ul li")).toHaveText(["To do", "In progress", "Done", "Cancelled"]);

  await page.getByRole("tab", { name: "Cycle time" }).click();
  await page.getByRole("button", { name: "This year" }).click();
  await expect(page.getByRole("region", { name: "Lead time" })).toBeVisible();
  await page.getByRole("radio", { name: "Lead time" }).click();
  expect(await page.locator("[data-dot]").count()).toBeGreaterThan(0);
  await shot(page, "reports-cycle.png");
  await page.getByText("Show as a table", { exact: true }).click();
  const tasks = page.locator("[data-flow-table=cycle-tasks] tbody tr");
  expect(await tasks.count()).toBeGreaterThan(0);
  await tasks.first().getByRole("button").click();
  await expect(page.getByRole("complementary", { name: /Task / })).toBeVisible();
});

test("a project, a version and a query narrow the report, and a wrong query says why", async ({ page }) => {
  await openReports(page);
  await page.getByRole("button", { name: "This year" }).click();
  await page.getByText("Show as a table", { exact: true }).click();
  const total = page.locator("[data-flow-table=created-closed] tfoot td").first();
  const all = Number(plain(await total.textContent()).replace(/\D/g, ""));
  await page.getByRole("button", { name: /^Project/ }).click();
  await page.getByRole("listbox").getByRole("option", { name: "Task Manager", exact: true }).click();
  await expect(page.getByRole("switch", { name: "Include sub-projects" })).toBeChecked();
  await expect.poll(async () => Number(plain(await total.textContent()).replace(/\D/g, ""))).toBeLessThan(all);
  const inProject = Number(plain(await total.textContent()).replace(/\D/g, ""));
  await page.getByRole("button", { name: /^Version/ }).click();
  await page.getByRole("listbox").getByRole("option", { name: "1.0", exact: true }).click();
  await expect.poll(async () => Number(plain(await total.textContent()).replace(/\D/g, ""))).toBeLessThan(inProject);
  await page.getByRole("button", { name: /^Version/ }).click();
  await page.getByRole("listbox").getByRole("option", { name: "Any version", exact: true }).click();
  await expect.poll(async () => Number(plain(await total.textContent()).replace(/\D/g, ""))).toBe(inProject);
  const query = page.getByRole("textbox", { name: "Query" });
  await query.fill("priority:");
  // The first Escape closes the list of values, if it opened; Enter then runs the query.
  await query.press("Escape");
  await query.press("Enter");
  await expect(page.locator("[data-flow-error]")).not.toBeEmpty();
  await query.fill("");
  await expect(page.locator("[data-flow-error]")).toHaveCount(0);
  await expect(total).toBeVisible();
});

test("right to left: time runs from the right", async ({ browser }) => {
  const context = await browser.newContext({ locale: "he-IL", viewport: { width: 1400, height: 900 } });
  const page = await context.newPage();
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.locator("nav button").filter({ has: page.locator("svg.lucide-chart-area") }).click();
  const bars = page.locator("[data-bar=created]");
  await expect(bars.first()).toBeVisible();
  const xs = await bars.evaluateAll((all) => all.map((bar) => Number(bar.getAttribute("x"))));
  expect(xs.length).toBeGreaterThan(2);
  expect(xs[0]).toBeGreaterThan(xs[xs.length - 1] ?? 0);
  // Arrow left moves forward in time.
  await page.locator("[data-chart]").focus();
  await page.keyboard.press("ArrowLeft");
  const first = plain(await readout(page).textContent());
  await page.keyboard.press("ArrowLeft");
  await expect(readout(page)).not.toHaveText(first);
  await shot(page, "reports-rtl.png");
  await context.close();
});

test("dark: the charts take the theme's colours", async ({ browser }) => {
  const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1400, height: 900 } });
  const page = await context.newPage();
  await openReports(page);
  await page.getByRole("tab", { name: "Cumulative flow" }).click();
  const band = page.locator("[data-band]").first();
  await expect(band).toBeVisible();
  const colours = await page.evaluate(() => {
    const chart = document.querySelector("[data-chart]");
    const bandFill = document.querySelector("[data-band]");
    return {
      surface: chart ? getComputedStyle(chart).backgroundColor : "",
      body: getComputedStyle(document.body).backgroundColor,
      fill: bandFill ? getComputedStyle(bandFill).fill : "",
    };
  });
  // The dark surface is dark, and the band is painted from a hue token.
  const [r = 255, g = 255, b = 255] = (colours.surface.match(/\d+/g) ?? []).map(Number);
  expect(r + g + b).toBeLessThan(300);
  expect(colours.fill).toMatch(/^rgb/);
  await shot(page, "reports-dark.png");
  await context.close();
});
