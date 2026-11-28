// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

// The critical path and baselines on the Gantt chart, on the sample data:
// the toggle highlights the critical bars (stripes and colour) and adds the
// slack column; a saved baseline draws ghost bars and the slip column, and
// a drag shows up in both. Also right to left, in the pseudo-locale and
// against axe.

const TODAY = "2026-11-11";
/** Pixels per day at the day zoom. */
const DAY_PX = 32;
/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];
/** Accented letters inside brackets: the pseudo-locale's mark on a message. */
const PSEUDO_TEXT = /\[[^\]]*[áéíóúýƀçđĝĥĵķĺñþŕšţṽŵẋž]/u;

/**
 * Opens the sample project's Gantt chart, by structure rather than by the
 * interface's words, so that it works in every locale: the project views
 * are board, list, calendar, Gantt…, the board's button the current one.
 */
async function toGantt(page: Page) {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
  const views = page.locator("[role=group]").filter({ has: page.locator("button[aria-current=page]") }).first();
  await views.getByRole("button").nth(3).click();
  const grid = page.getByRole("treegrid");
  await expect(grid).toBeVisible();
  await expect(grid.locator("[data-bar]").first()).toBeAttached();
  return grid;
}

async function dragBy(page: Page, handle: Locator, dx: number) {
  const box = await handle.boundingBox();
  if (!box) {
    throw new Error("nothing to drag");
  }
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx / 2, y, { steps: 4 });
  await page.mouse.move(x + dx, y, { steps: 4 });
  await page.mouse.up();
}

/** A plain bar on screen, not closed, wide enough to grab. */
async function plainBar(grid: Locator, left: number, right: number): Promise<string> {
  for (const element of await grid.locator("[data-bar][data-shape=bar]").all()) {
    const key = (await element.getAttribute("data-bar")) ?? "";
    const box = await element.locator("[data-handle=move]").boundingBox();
    const cls = (await element.getAttribute("class")) ?? "";
    if (box && box.width >= 3 * DAY_PX && box.x > left && box.x + box.width < right && !cls.includes("closed")) {
      return key;
    }
  }
  throw new Error("no plain bar on screen in the sample data");
}

/** The Critical path switch's label, which takes the click. */
const criticalToggle = (page: Page) => page.locator("label").filter({ has: page.getByRole("switch", { name: "Critical path" }) });

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[\u2068\u2069]/g, "");

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).options({ rules: { "target-size": { enabled: true } } }).withTags(TAGS).analyze();
  expect(results.violations.map((violation) => `${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`)).toEqual([]);
}

/** Saves a baseline from the toolbar's menu; the chart then compares with it. */
async function saveBaseline(page: Page) {
  await page.getByTestId("gantt-baseline-menu").click();
  await page.getByRole("menuitem").first().click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.locator("input")).toBeFocused();
  await dialog.locator("input").fill("Kick-off");
  await dialog.locator("input").press("Enter");
  await expect(dialog).toBeHidden();
}

test("the critical path toggle highlights the critical bars, with stripes, and shows slack", async ({ page }) => {
  const grid = await toGantt(page);
  await expect(grid.locator("[data-bar][data-critical]")).toHaveCount(0);
  await criticalToggle(page).click();
  await expect(grid.getByRole("columnheader", { name: "Slack" })).toBeVisible();
  await expect(page.getByTestId("gantt-critical-note")).toContainText("critical");
  // Every row drawn has its slack; the ones without any are the critical ones.
  for (const cell of await grid.locator("[data-row] [data-slack]").all()) {
    const slack = Number(await cell.getAttribute("data-slack"));
    const key = await cell.evaluate((element) => element.closest("[data-row]")?.getAttribute("data-row") ?? "");
    const drawn = grid.locator(`[data-bar="${key}"]`);
    if ((await drawn.count()) > 0) {
      if (slack > 0) {
        await expect(drawn).not.toHaveAttribute("data-critical");
      } else {
        await expect(drawn).toHaveAttribute("data-critical", "");
      }
    }
  }
  // Only the critical tasks: each one highlighted, its plain bar striped, so not by colour alone.
  const field = page.getByRole("textbox", { name: "Filter query" });
  await field.click();
  await page.keyboard.type("is:critical");
  await page.keyboard.press("Escape");
  const rows = grid.locator("[data-row] [data-slack]");
  await expect.poll(async () => (await rows.evaluateAll((cells) => cells.every((cell) => Number(cell.getAttribute("data-slack")) <= 0))) && (await rows.count()) > 0).toBe(true);
  const first = (await grid.locator("[data-row]").first().getAttribute("data-row")) ?? "";
  await grid.locator(`[data-row="${first}"]`).getByRole("gridcell").first().click();
  const bar = grid.locator(`[data-bar="${first}"]`);
  await expect(bar).toHaveAttribute("data-critical", "");
  if ((await bar.getAttribute("data-shape")) === "bar") {
    await expect(bar.locator("rect[fill^='url(']")).toHaveCount(1);
  }
  // The bar's tooltip tells its slack.
  expect(plain(await bar.locator("title").textContent())).toMatch(/Critical/);
  await axe(page);
  await criticalToggle(page).click();
  await expect(grid.locator("[data-bar][data-critical]")).toHaveCount(0);
  await expect(grid.getByRole("columnheader", { name: "Slack" })).toHaveCount(0);
});

test("a saved baseline draws ghost bars and the slip column; a drag shows its slip", async ({ page }) => {
  const grid = await toGantt(page);
  await saveBaseline(page);
  await expect(page.getByTestId("gantt-baseline-menu")).toContainText("Kick-off");
  await expect(grid.getByRole("columnheader", { name: "Slip" })).toBeVisible();
  await expect(grid.locator("[data-baseline-ghost]").first()).toBeAttached();
  const key = await plainBar(grid, 520, 1250);
  const slip = grid.locator(`[data-row="${key}"] [data-slip]`);
  await expect(slip).toHaveAttribute("data-slip", "0");
  const ghost = grid.locator(`[data-baseline-ghost="${key}"]`);
  const before = await ghost.boundingBox();

  await dragBy(page, grid.locator(`[data-bar="${key}"] [data-handle=move]`), 3 * DAY_PX);
  await expect(slip).toHaveAttribute("data-slip", "3");
  await expect(slip).toHaveText("+3d");
  expect(plain(await grid.locator(`[data-row="${key}"]`).getAttribute("aria-label"))).toContain("3 days later than the baseline");
  // The ghost stays where the baseline had the task; the bar moved on.
  const after = await ghost.boundingBox();
  expect(after?.x).toBeCloseTo(before?.x ?? 0, 0);
  const moved = await grid.locator(`[data-bar="${key}"] [data-handle=move]`).boundingBox();
  expect((moved?.x ?? 0) - (after?.x ?? 0)).toBeGreaterThan(2 * DAY_PX);
  expect(plain(await grid.locator(`[data-bar="${key}"] title`).textContent())).toContain("3 days later");
  await axe(page);

  // Show none: the ghosts and the column go.
  await page.getByTestId("gantt-baseline-menu").click();
  await page.getByRole("menuitemradio").first().click();
  await expect(grid.locator("[data-baseline-ghost]")).toHaveCount(0);
  await expect(grid.getByRole("columnheader", { name: "Slip" })).toHaveCount(0);
});

test("the critical path and the shown baseline are remembered; a baseline is renamed and deleted from the menu", async ({ page }) => {
  let grid = await toGantt(page);
  await criticalToggle(page).click();
  await saveBaseline(page);
  await expect(grid.locator("[data-baseline-ghost]").first()).toBeAttached();
  // Back on the chart from the board: the same switch and baseline.
  const views = page.locator("[role=group]").filter({ has: page.locator("button[aria-current=page]") }).first();
  await views.getByRole("button").nth(0).click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
  await views.getByRole("button").nth(3).click();
  grid = page.getByRole("treegrid");
  await expect(page.getByRole("switch", { name: "Critical path" })).toBeChecked();
  await expect(page.getByTestId("gantt-baseline-menu")).toContainText("Kick-off");
  await expect(grid.locator("[data-baseline-ghost]").first()).toBeAttached();

  // Rename it.
  await page.getByTestId("gantt-baseline-menu").click();
  await page.getByRole("menuitem", { name: "Rename shown baseline…" }).click();
  const rename = page.getByRole("dialog", { name: "Rename the baseline" });
  await expect(rename.locator("input")).toHaveValue("Kick-off");
  await rename.locator("input").fill("First plan");
  await rename.getByRole("button", { name: "Rename" }).click();
  await expect(rename).toBeHidden();
  await expect(page.getByTestId("gantt-baseline-menu")).toContainText("First plan");

  // Delete it, after the confirmation; Cancel keeps it.
  await page.getByTestId("gantt-baseline-menu").click();
  await page.getByRole("menuitem", { name: "Delete shown baseline…" }).click();
  let confirm = page.getByRole("dialog", { name: /Delete the baseline .*First plan/ });
  await confirm.getByRole("button", { name: "Cancel" }).click();
  await expect(confirm).toBeHidden();
  await expect(page.getByTestId("gantt-baseline-menu")).toContainText("First plan");
  await page.getByTestId("gantt-baseline-menu").click();
  await page.getByRole("menuitem", { name: "Delete shown baseline…" }).click();
  confirm = page.getByRole("dialog", { name: /Delete the baseline .*First plan/ });
  await axe(page);
  await confirm.getByRole("button", { name: "Delete baseline" }).click();
  await expect(confirm).toBeHidden();
  await expect(page.getByTestId("gantt-baseline-menu")).toContainText("None");
  await expect(grid.locator("[data-baseline-ghost]")).toHaveCount(0);
  await page.getByTestId("gantt-baseline-menu").click();
  await expect(page.getByRole("menuitemradio")).toHaveCount(1);
  await expect(page.getByRole("menuitem", { name: "Delete shown baseline…" })).toBeDisabled();
  await page.keyboard.press("Escape");

  // The switch outlasts a reload too (the sample data starts afresh, so it has no baselines then).
  await toGantt(page);
  await expect(page.getByRole("switch", { name: "Critical path" })).toBeChecked();
});

test("right to left: ghost bars sit under their bars and slip reads the same", async ({ browser }) => {
  const context = await browser.newContext({ locale: "he-IL", viewport: { width: 1400, height: 900 } });
  const page = await context.newPage();
  const grid = await toGantt(page);
  await expect(page.locator("[data-gantt]")).toHaveAttribute("dir", "rtl");
  await saveBaseline(page);
  const ghost = grid.locator("[data-baseline-ghost]").first();
  await expect(ghost).toBeAttached();
  const key = (await ghost.getAttribute("data-baseline-ghost")) ?? "";
  const g = await ghost.boundingBox();
  const b = await grid.locator(`[data-bar="${key}"]`).boundingBox();
  // Under its own bar: the same stretch of the timeline, a little lower.
  expect(g && b && Math.abs(g.x + g.width / 2 - (b.x + b.width / 2)) < DAY_PX).toBe(true);
  expect(g && b && g.y > b.y).toBe(true);
  await context.close();
});

test("in the pseudo-locale, the new controls and columns come from the message files", async ({ browser }) => {
  const context = await browser.newContext({ locale: "en-XA", viewport: { width: 1400, height: 900 } });
  const page = await context.newPage();
  const grid = await toGantt(page);
  const toggle = page.locator("[data-gantt-view] label").filter({ has: page.getByRole("switch") }).nth(2);
  test.skip(!PSEUDO_TEXT.test(await page.locator("[data-gantt-view]").innerText()), "the build doesn't show the pseudo-locale");
  await toggle.click();
  const heads = grid.locator("[data-column-head]");
  await expect(heads).toHaveCount(1);
  expect(await heads.first().innerText()).toMatch(PSEUDO_TEXT);
  expect(await toggle.innerText()).toMatch(PSEUDO_TEXT);
  expect(await page.getByTestId("gantt-baseline-menu").innerText()).toMatch(PSEUDO_TEXT);
  expect(await page.getByTestId("gantt-critical-note").innerText()).toMatch(PSEUDO_TEXT);
  await context.close();
});

test("5,000 dated tasks: showing a baseline and the critical path stays within the chart's budget", async ({ page }) => {
  test.setTimeout(120_000);
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  // GK_CPU_SLOWDOWN=4 measures on a CPU four times slower (Chromium only).
  const slowdown = Number(process.env.GK_CPU_SLOWDOWN ?? 1);
  if (slowdown > 1) {
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: slowdown });
  }
  await page.goto("/?tasks=5000&links=300");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  const views = page.getByRole("group", { name: "Project views" });
  await views.getByRole("button", { name: "Gantt" }).click();
  const grid = page.getByRole("treegrid", { name: "Gantt chart" });
  await expect(grid.locator("[data-bar]").first()).toBeAttached({ timeout: 30_000 });
  expect(Number(await grid.getAttribute("aria-rowcount"))).toBeGreaterThan(5000);
  const frame = () => page.evaluate(() => new Promise<number>((resolve) => requestAnimationFrame(() => { resolve(performance.now()); })));
  const now = () => page.evaluate(() => performance.now());

  await saveBaseline(page);
  await expect(grid.locator("[data-baseline-ghost]").first()).toBeAttached({ timeout: 30_000 });
  // Hide it, then measure showing it again: reading the schedule with the
  // baseline (in the sample engine) and drawing the ghost bars and slip.
  await page.getByTestId("gantt-baseline-menu").click();
  await page.getByRole("menuitemradio").first().click();
  await expect(grid.locator("[data-baseline-ghost]")).toHaveCount(0);
  let started = await now();
  await page.getByTestId("gantt-baseline-menu").click();
  await page.getByRole("menuitemradio").nth(1).click();
  await expect(grid.locator("[data-baseline-ghost]").first()).toBeAttached();
  const show = (await frame()) - started;

  started = await now();
  await criticalToggle(page).click();
  await expect(grid.getByRole("columnheader", { name: "Slack" })).toBeVisible();
  const critical = (await frame()) - started;

  // A zoom draws everything on screen anew, ghosts and stripes too.
  started = await now();
  await page.getByRole("radiogroup", { name: "Zoom" }).getByRole("radio", { name: "Week" }).click();
  await expect(page.locator("[data-gantt]")).toHaveAttribute("data-zoom", "week");
  const zoom = (await frame()) - started;
  console.log(`gantt with 5,000 tasks: baseline shown in ${String(Math.round(show))} ms, critical path in ${String(Math.round(critical))} ms, zoom in ${String(Math.round(zoom))} ms`);
  // The same budget as the chart's own: reading and drawing within a second, a zoom within half.
  expect(show).toBeLessThan(1000);
  expect(critical).toBeLessThan(1000);
  expect(zoom).toBeLessThan(500);
  // Only what is on screen is drawn.
  expect(await grid.locator("[data-baseline-ghost]").count()).toBeLessThan(120);
});
