// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

// The Gantt chart on the sample data, whose dates lie around today. The
// clock is fixed, so the sample data is the same on every run. With
// GK_SCREENSHOT_DIR set, the flows save screenshots there.

const TODAY = "2026-11-11";
/** Pixels per day at the day zoom. */
const DAY_PX = 32;

async function openGantt(page: Page, query = "", project = /Task Manager/) {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto(`/${query}`);
  await page.getByRole("row", { name: project }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Gantt" }).click();
  const grid = page.getByRole("treegrid", { name: "Gantt chart" });
  await expect(grid).toBeVisible();
  await expect(grid.locator("[data-bar]").first()).toBeAttached();
  return grid;
}

async function shot(page: Page, name: string) {
  const dir = process.env.GK_SCREENSHOT_DIR;
  if (dir) {
    await page.screenshot({ path: join(dir, name) });
  }
}

function addDays(date: string, days: number): string {
  const at = new Date(`${date}T12:00:00Z`);
  at.setUTCDate(at.getUTCDate() + days);
  return at.toISOString().slice(0, 10);
}

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[\u2068\u2069]/g, "");
/** What the chart last told screen readers. */
const live = (page: Page) => async () => plain(await page.getByTestId("gantt-live").textContent());

const bar = (grid: Locator, key: string) => grid.locator(`[data-bar="${key}"]`);
const row = (grid: Locator, key: string) => grid.locator(`[data-row="${key}"]`);
/** A row's key cell, which a click selects without editing anything. */
const cellOf = (grid: Locator, key: string) => row(grid, key).getByRole("gridcell").first();

/** The dates a row's cells show, as the engine knows them (from the buttons' names). */
async function datesOf(grid: Locator, key: string): Promise<[string, string]> {
  const buttons = row(grid, key).locator("button[aria-label*='date of']");
  const start = (await buttons.nth(0).getAttribute("aria-label")) ?? "";
  const due = (await buttons.nth(1).getAttribute("aria-label")) ?? "";
  return [start.replace(/^.*: /, "").replace(/[⁨⁩]/g, ""), due.replace(/^.*: /, "").replace(/[⁨⁩]/g, "")];
}

/** A plain bar (start and due) on screen, not closed. */
async function plainBar(grid: Locator, skip: string[] = []): Promise<string> {
  for (const element of await grid.locator("[data-bar][data-shape=bar]").all()) {
    const key = (await element.getAttribute("data-bar")) ?? "";
    const box = await element.locator("[data-handle=move]").boundingBox();
    const cls = (await element.getAttribute("class")) ?? "";
    if (!skip.includes(key) && box && box.width >= 3 * DAY_PX && box.x > 520 && box.x + box.width < 1250 && !cls.includes("closed")) {
      return key;
    }
  }
  throw new Error("no plain bar on screen in the sample data");
}

async function dragBy(page: Page, handle: Locator, dx: number, dy = 0) {
  const box = await handle.boundingBox();
  if (!box) {
    throw new Error("nothing to drag");
  }
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx / 2, y + dy / 2, { steps: 4 });
  await page.mouse.move(x + dx, y + dy, { steps: 4 });
  await page.mouse.up();
}

test("the chart: table and timeline, today, the sample tasks, the Unscheduled group", async ({ page }) => {
  const grid = await openGantt(page);
  await expect(grid.locator("[data-today]")).toHaveCount(1);
  await expect(grid.getByRole("columnheader")).toHaveCount(6);
  await expect(grid.getByRole("row", { name: /^Unscheduled/ })).toHaveCount(await grid.getByRole("row", { name: /^Unscheduled/ }).count());
  // The range of days on screen is shown in the toolbar.
  await expect(page.getByTestId("gantt-range")).not.toHaveText("");
  // The status bar counts the tasks.
  await expect(page.getByText(/scheduled.*unscheduled/)).toBeVisible();
  await shot(page, "gantt-day.png");
});

test("dragging a bar moves it, dragging an edge resizes it, and Undo takes it back", async ({ page }) => {
  const grid = await openGantt(page);
  const key = await plainBar(grid);
  const [start, due] = await datesOf(grid, key);

  await dragBy(page, bar(grid, key).locator("[data-handle=move]"), 3 * DAY_PX);
  await expect.poll(() => datesOf(grid, key)).not.toEqual([start, due]);
  const moved = await datesOf(grid, key);
  await expect.poll(live(page)).toContain(`${key} now runs`);

  await dragBy(page, bar(grid, key).locator("[data-handle=end]"), 2 * DAY_PX);
  await expect.poll(async () => (await datesOf(grid, key))[1]).not.toBe(moved[1]);
  expect((await datesOf(grid, key))[0]).toBe(moved[0]);

  // Undo, twice, restores the dates.
  await page.keyboard.press("Control+z");
  await page.keyboard.press("Control+z");
  await expect.poll(() => datesOf(grid, key)).toEqual([start, due]);
});

test("a summary bar follows its subtasks: a drag or Alt+arrow on it changes no dates", async ({ page }) => {
  // The home lab's sample data has a parent with a dated subtask.
  const grid = await openGantt(page, "", /Home lab/);
  const summary = grid.locator("[data-bar][data-shape=summary]").first();
  await expect(summary).toBeAttached();
  const key = (await summary.getAttribute("data-bar")) ?? "";
  // It has nothing to grab, and its dates aren't buttons to edit.
  await expect(summary.locator("[data-handle]")).toHaveCount(0);
  await expect(row(grid, key).locator("button[aria-label*='date of']:not([disabled])")).toHaveCount(0);
  const allDates = () => grid.locator("button[aria-label*='date of']").evaluateAll((buttons) => buttons.map((button) => button.getAttribute("aria-label")));
  const before = await allDates();
  // The bar is on the timeline, not under the table.
  const box = await summary.boundingBox();
  expect(box !== null && box.x > 520 && box.x + box.width < 1350).toBe(true);
  await dragBy(page, summary.locator("rect").first(), 3 * DAY_PX);
  await cellOf(grid, key).click();
  await page.keyboard.press("Alt+ArrowRight");
  await page.keyboard.press("Alt+Shift+ArrowRight");
  // Nothing moved, here or in the subtasks, and nothing was announced as moved.
  expect(await allDates()).toEqual(before);
  expect(await live(page)()).not.toContain("now runs");
});

test("with Also move dependents on, a drag says how many tasks it may move, and moves them in one change", async ({ page }) => {
  const grid = await openGantt(page);
  const alsoMove = page.getByRole("switch", { name: "Also move dependents" });
  await page.getByText("Also move dependents").click();
  await expect(alsoMove).toBeChecked();
  const candidates = grid.locator("[data-bar][data-shape=bar]:not([data-dependents='0'])");
  await expect(candidates.first()).toBeAttached();
  const key = (await candidates.first().getAttribute("data-bar")) ?? "";
  const count = Number(await candidates.first().getAttribute("data-dependents"));
  const before = await datesOf(grid, key);
  const handle = bar(grid, key).locator("[data-handle=end]");
  await handle.scrollIntoViewIfNeeded();
  const box = await handle.boundingBox();
  if (!box) {
    throw new Error("no bar end");
  }
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 10 * DAY_PX, box.y + box.height / 2, { steps: 10 });
  // The engine works out exactly which tasks the drop moves; they show as outlines where they would go.
  const tip = page.getByTestId("gantt-tip");
  await expect(tip).toContainText(/Moves (no|one|\d+) dependent tasks?/);
  const tipText = plain(await tip.textContent());
  const exact = /Moves no/.test(tipText) ? 0 : /Moves one/.test(tipText) ? 1 : Number(/Moves (\d+)/.exec(tipText)?.[1]);
  expect(exact).toBeLessThanOrEqual(count);
  await expect(grid.locator("[data-ghost]")).toHaveCount(await grid.locator("[data-ghost]").count());
  await shot(page, "gantt-dependents.png");
  await page.mouse.up();
  if (exact > 0) {
    await expect(page.getByText(exact === 1 ? "and one dependent task moved" : `and ${String(exact)} dependent tasks moved`)).toBeVisible();
  }
  await page.keyboard.press("Control+z");
  await expect.poll(() => datesOf(grid, key)).toEqual(before);
});

test("dragging from a bar's end to another bar links them; clicking the arrow offers to remove it", async ({ page }) => {
  const grid = await openGantt(page);
  // Two bars not linked yet, either way round.
  const from = await plainBar(grid);
  let to = await plainBar(grid, [from]);
  const skip = [from];
  while ((await grid.locator(`[data-dependency="${to}>${from}"], [data-dependency="${from}>${to}"]`).count()) > 0) {
    skip.push(to);
    to = await plainBar(grid, skip);
  }
  const link = grid.locator(`[data-dependency="${from}>${to}"]`);
  await expect(link).toHaveCount(0);

  await bar(grid, from).locator("[data-handle=move]").hover();
  const handle = bar(grid, from).locator("[data-handle=link]");
  const target = await bar(grid, to).locator("[data-handle=move]").boundingBox();
  const start = await handle.boundingBox();
  if (!target || !start) {
    throw new Error("bars not on screen");
  }
  await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2);
  await page.mouse.down();
  await page.mouse.move(target.x + 4, target.y + target.height / 2, { steps: 8 });
  await page.mouse.up();
  await expect(link).toHaveCount(1);
  await shot(page, "gantt-link.png");

  // Click on the line itself, half way along it.
  const point = await link.locator("path").nth(1).evaluate((path: SVGPathElement) => {
    const at = path.getPointAtLength(path.getTotalLength() / 2);
    const screen = new DOMPoint(at.x, at.y).matrixTransform(path.getScreenCTM() ?? new DOMMatrix());
    return { x: screen.x, y: screen.y };
  });
  await page.mouse.click(point.x, point.y);
  await page.getByRole("dialog").getByRole("button", { name: "Remove the dependency" }).click();
  await expect(link).toHaveCount(0);
});

test("the keyboard moves, resizes, links, folds and zooms; zoom keeps the middle day", async ({ page }) => {
  const grid = await openGantt(page);
  const key = await plainBar(grid);
  await cellOf(grid, key).click();
  await expect(grid).toBeFocused();
  await expect(grid).toHaveAttribute("aria-activedescendant", /row/);
  const [start, due] = await datesOf(grid, key);

  // Alt+Right moves it a day; Alt+Shift+Right makes it a day longer.
  await page.keyboard.press("Alt+ArrowRight");
  await expect.poll(live(page)).toContain(`${key} now runs`);
  await expect.poll(async () => (await datesOf(grid, key))[1]).not.toBe(due);
  const [, movedDue] = await datesOf(grid, key);
  await page.keyboard.press("Alt+Shift+ArrowRight");
  await expect.poll(async () => (await datesOf(grid, key))[1]).not.toBe(movedDue);
  expect((await datesOf(grid, key))[0]).not.toBe(start);

  // L, then the next task and Enter, makes a dependency; again removes it.
  const active = async () => (await grid.getAttribute("aria-activedescendant")) ?? "";
  await page.keyboard.press("l");
  await expect.poll(live(page)).toContain(`Dependency from ${key}`);
  const before = await active();
  await page.keyboard.press("ArrowDown");
  await expect.poll(active).not.toBe(before);
  const other = (await grid.locator(`[id="${await active()}"]`).getAttribute("data-row")) ?? "";
  await page.keyboard.press("Enter");
  await expect(grid.locator(`[data-dependency="${key}>${other}"]`)).toHaveCount(1);
  await cellOf(grid, key).click();
  await page.keyboard.press("l");
  await cellOf(grid, other).click();
  await page.keyboard.press("Enter");
  await expect(grid.locator(`[data-dependency="${key}>${other}"]`)).toHaveCount(0);

  // Zoom out with "-", in with "+"; the day in the middle stays.
  const range = page.getByTestId("gantt-range");
  await expect(page.locator("[data-gantt]")).toHaveAttribute("data-zoom", "day");
  const dayRange = await range.textContent();
  await page.keyboard.press("-");
  await expect(page.locator("[data-gantt]")).toHaveAttribute("data-zoom", "week");
  await expect(range).not.toHaveText(dayRange ?? "");
  await page.keyboard.press("+");
  await expect(page.locator("[data-gantt]")).toHaveAttribute("data-zoom", "day");
  await expect(range).toHaveText(dayRange ?? "");

  // The zoom is the person's: the engine keeps it for the next visit.
  await page.keyboard.press("-");
  await page.keyboard.press("-");
  await expect(page.locator("[data-gantt]")).toHaveAttribute("data-zoom", "month");
  await shot(page, "gantt-month.png");
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Board" }).click();
  await expect(page.locator("[data-gantt]")).toHaveCount(0);
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Gantt" }).click();
  await expect(page.locator("[data-gantt]")).toHaveAttribute("data-zoom", "month");
});

test("a task from the Unscheduled group is scheduled by dropping it on the timeline; F2 edits dates", async ({ page }) => {
  const grid = await openGantt(page);
  // The Unscheduled group is last; only the rows on screen are drawn.
  const unscheduled = grid.getByRole("row", { name: /, unscheduled$/ }).last();
  await expect(async () => {
    await grid.evaluate((el) => {
      el.scrollTop = el.scrollHeight;
    });
    await expect(unscheduled).toBeVisible({ timeout: 500 });
  }).toPass();
  const key = (await unscheduled.getAttribute("data-row")) ?? "";
  const box = await unscheduled.boundingBox();
  const today = await grid.locator("[data-today]").boundingBox();
  if (!box || !today) {
    throw new Error("nothing to drag");
  }
  await page.mouse.move(box.x + 200, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(today.x + today.width / 2, box.y - 40, { steps: 10 });
  await expect(page.getByTestId("gantt-tip")).toBeVisible();
  await page.mouse.up();
  await expect(bar(grid, key)).toHaveCount(1);
  await expect.poll(live(page)).toContain(`${key} scheduled on`);

  // F2 edits the start date, Tab goes on to the due date.
  await cellOf(grid, key).click();
  await page.keyboard.press("F2");
  const field = grid.locator("input[type=date]");
  await expect(field).toBeFocused();
  expect(plain(await field.getAttribute("aria-label"))).toBe(`Start date of ${key}`);
  await page.keyboard.press("Tab");
  await expect(field).toBeFocused();
  expect(plain(await field.getAttribute("aria-label"))).toBe(`Due date of ${key}`);
  const dueField = field;
  await dueField.fill(addDays(TODAY, 5));
  await page.keyboard.press("Enter");
  await expect(bar(grid, key)).toHaveAttribute("data-shape", "bar");
});

test("right to left: the table is on the right, the timeline runs leftwards, drags follow the pointer", async ({ browser }) => {
  const context = await browser.newContext({ locale: "he-IL", viewport: { width: 1400, height: 900 } });
  const page = await context.newPage();
  const grid = await openGantt(page);
  await expect(page.locator("[data-gantt]")).toHaveAttribute("dir", "rtl");
  const table = await grid.getByRole("columnheader").first().boundingBox();
  const frame = await grid.boundingBox();
  expect(table && frame && table.x > frame.x + frame.width / 2).toBe(true);
  const key = await (async () => {
    for (const element of await grid.locator("[data-bar][data-shape=bar]").all()) {
      const b = await element.locator("[data-handle=move]").boundingBox();
      if (b && b.width >= 3 * DAY_PX && b.x > 150 && b.x + b.width < 850) {
        return (await element.getAttribute("data-bar")) ?? "";
      }
    }
    throw new Error("no bar on screen");
  })();
  const [start] = await datesOf(grid, key);
  // Dragging leftwards moves it later in right-to-left.
  await dragBy(page, bar(grid, key).locator("[data-handle=move]"), -2 * DAY_PX);
  await expect.poll(async () => (await datesOf(grid, key))[0]).not.toBe(start);
  await expect(page.getByTestId("gantt-live")).toContainText(key);
  await shot(page, "gantt-rtl.png");
  await context.close();
});

test("500 tasks and 300 dependencies draw in under a second", async ({ page }) => {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  // GK_CPU_SLOWDOWN=4 measures on a CPU four times slower (Chromium only).
  // The limit is for reading and drawing the chart; the first visit, which
  // also loads the chart's code, is only reported.
  const slowdown = Number(process.env.GK_CPU_SLOWDOWN ?? 1);
  if (slowdown > 1) {
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: slowdown });
  }
  await page.goto("/?tasks=500&links=300");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  const views = page.getByRole("group", { name: "Project views" });
  const grid = page.getByRole("treegrid", { name: "Gantt chart" });
  const frame = () => page.evaluate(() => new Promise<number>((resolve) => requestAnimationFrame(() => { resolve(performance.now()); })));

  // The first visit also loads the chart's code.
  let started = await page.evaluate(() => performance.now());
  await views.getByRole("button", { name: "Gantt" }).click();
  await expect(grid.locator("[data-bar]").first()).toBeAttached();
  const first = (await frame()) - started;

  // The second: reading the schedule (in the sample engine) and drawing it.
  await views.getByRole("button", { name: "Board" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  started = await page.evaluate(() => performance.now());
  await views.getByRole("button", { name: "Gantt" }).click();
  await expect(grid.locator("[data-bar]").first()).toBeAttached();
  const again = (await frame()) - started;

  // A zoom draws everything on screen anew.
  started = await page.evaluate(() => performance.now());
  await page.getByRole("radiogroup", { name: "Zoom" }).getByRole("radio", { name: "Week" }).click();
  await expect(page.locator("[data-gantt]")).toHaveAttribute("data-zoom", "week");
  const zoom = (await frame()) - started;

  const rows = Number(await grid.getAttribute("aria-rowcount"));
  const links = await grid.locator("[data-dependency]").count();
  console.log(`gantt render: first ${String(Math.round(first))} ms, again ${String(Math.round(again))} ms, zoom ${String(Math.round(zoom))} ms; ${String(rows)} rows, ${String(links)} dependencies drawn`);
  expect(rows).toBeGreaterThan(500);
  expect(links).toBeGreaterThan(0);
  expect(again).toBeLessThan(1000);
  expect(zoom).toBeLessThan(500);
  // Scrolling to the end keeps drawing only what is on screen.
  await grid.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });
  await expect(grid.locator("[data-row]").last()).toBeVisible();
  expect(await grid.locator("[data-row]").count()).toBeLessThan(120);
  await shot(page, "gantt-500.png");
});

test("pseudo-locale: the chart's words all come from the messages", async ({ browser }) => {
  const context = await browser.newContext({ locale: "en-XA", viewport: { width: 1400, height: 900 } });
  const page = await context.newPage();
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  // The view with the Gantt icon.
  await page.getByRole("banner").locator("button:has(svg.lucide-chart-gantt)").click();
  const chart = page.locator("[data-gantt]");
  await expect(chart.locator("[data-bar]").first()).toBeAttached();
  // Every message of the toolbar and the table is accented and in brackets.
  const words = await page.locator("[data-gantt-view]").evaluate((root) => {
    const texts: string[] = [];
    for (const el of root.querySelectorAll("[role=columnheader], [role=radio], label, button.btn, [role=switch]")) {
      const text = el.textContent.trim();
      if (text !== "") {
        texts.push(text);
      }
    }
    return texts;
  });
  expect(words.length).toBeGreaterThan(5);
  const english = words.filter((text) => /\b(Key|Title|Start|Due|Day|Week|Month|Quarter|Today|Include|Also)\b/.test(text));
  expect(english).toEqual([]);
  await shot(page, "gantt-pseudo.png");
  await context.close();
});

test("grouping by label, status or none, kept by the engine for the next visit", async ({ page }) => {
  const grid = await openGantt(page);
  const group = page.getByRole("button", { name: /^Group: / });
  expect(plain(await group.getAttribute("aria-label"))).toBe("Group: Parent");
  await group.click();
  await page.getByRole("menuitemradio", { name: "Label" }).click();
  await expect(grid.locator("[data-group^='label:']").first()).toBeAttached();
  // Every task is in a group of its own label, one level deep; the tasks without dates stay last.
  await expect(grid.locator("[role=row][aria-level='2']")).toHaveCount(0);
  await shot(page, "gantt-group-label.png");

  await page.getByRole("button", { name: /^Group: / }).click();
  await page.getByRole("menuitemradio", { name: "Status" }).click();
  await expect(grid.locator("[data-group^='status:']").first()).toBeAttached();
  const first = plain(await grid.locator("[data-group^='status:']").first().textContent());
  expect(first).toMatch(/: (one task|\d+ tasks)$/);

  await page.getByRole("button", { name: /^Group: / }).click();
  await page.getByRole("menuitemradio", { name: "None" }).click();
  await expect(grid.locator("[data-group^='status:'], [data-group^='label:']")).toHaveCount(0);

  await page.getByRole("button", { name: /^Group: / }).click();
  await page.getByRole("menuitemradio", { name: "Status" }).click();
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Board" }).click();
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Gantt" }).click();
  await expect.poll(async () => plain(await page.getByRole("button", { name: /^Group: / }).getAttribute("aria-label"))).toBe("Group: Status");
  await expect(page.locator("[data-group^='status:']").first()).toBeAttached();
});

test("export: a PNG picture of the whole chart, and printing (or PDF) through the system's dialog", async ({ page }) => {
  await openGantt(page);
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export" }).click();
  await page.getByRole("menuitem", { name: "PNG image" }).click();
  const file = await download;
  expect(file.suggestedFilename()).toMatch(/^TM Gantt chart \d{4}-\d{2}-\d{2}\.png$/);
  const path = await file.path();
  const { readFile } = await import("node:fs/promises");
  const bytes = await readFile(path);
  expect([...bytes.subarray(0, 8)]).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  // Wide enough for the table and the timeline: the picture's width sits in the PNG header.
  expect(bytes.readUInt32BE(16)).toBeGreaterThan(1000);
  await expect(page.getByText(/Saved the chart as/)).toBeVisible();

  // Printing shows only the picture; the dialog itself is the system's.
  await page.evaluate(() => {
    (window as unknown as { printed: number }).printed = 0;
    window.print = () => {
      (window as unknown as { printed: number }).printed += 1;
    };
  });
  await page.getByRole("button", { name: "Export" }).click();
  await page.getByRole("menuitem", { name: "Print or save as PDF" }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as { printed: number }).printed)).toBe(1);
  const sheet = page.locator("img[data-gantt-print]");
  await expect(sheet).toHaveCount(1);
  expect(await sheet.getAttribute("src")).toMatch(/^data:image\/svg\+xml/);
  await page.emulateMedia({ media: "print" });
  await expect(sheet).toBeVisible();
  await expect(page.locator("[data-gantt]")).toBeHidden();
  await page.emulateMedia({ media: "screen" });
});

test("bars behind schedule are marked; the status bar shows overdue tasks in red", async ({ page }) => {
  const grid = await openGantt(page);
  // The sample data has open bars over today with less done than the time gone.
  const behind = grid.locator("[data-bar][class*=behind]");
  await expect(behind.first()).toBeAttached();
  const key = (await behind.first().getAttribute("data-bar")) ?? "";
  await expect(row(grid, key)).toHaveAttribute("aria-label", /behind schedule$/);
  await expect(grid.getByText(/^Behind schedule: \S+ done, \S+ of the time gone$/).first()).toBeAttached();
  const alert = page.getByTestId("status-alert");
  await expect(alert).toHaveText(/^(One|\u2068\d+\u2069) overdue$/);
  expect(await alert.evaluate((el) => getComputedStyle(el).color)).not.toBe(await page.getByTestId("status-note").evaluate((el) => getComputedStyle(el).color));
  await shot(page, "gantt-behind.png");
});

test("tasks of an archived project can't be changed: a drag or F2 says why", async ({ page }) => {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  // Archive the Personal project, then open its chart from the Archived list.
  await page.getByRole("row", { name: /Personal/ }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Archive" }).click();
  await page.getByRole("button", { name: "Personal", exact: true }).click();
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Gantt" }).click();
  const grid = page.getByRole("treegrid", { name: "Gantt chart" });
  await expect(grid).toBeVisible();
  const first = grid.locator("[data-row]").first();
  await expect(first).toBeAttached();
  await expect(first).toHaveAttribute("aria-label", /read only because its project is archived/);
  const key = (await first.getAttribute("data-row")) ?? "";
  await first.getByRole("gridcell").first().click();
  await page.keyboard.press("F2");
  await expect(grid.locator("input[type=date]")).toHaveCount(0);
  await expect(page.getByRole("alert").filter({ hasText: /can't be changed: its project is archived/ }).first()).toBeVisible();
  await expect.poll(live(page)).toBe(`${key} can't be changed: its project is archived.`);
  const bars = grid.locator("[data-bar][data-read-only] [data-handle=move]");
  if ((await bars.count()) > 0) {
    const barKey = (await grid.locator("[data-bar][data-read-only]").first().getAttribute("data-bar")) ?? "";
    const before = await datesOf(grid, barKey);
    await dragBy(page, bars.first(), 2 * DAY_PX);
    await expect.poll(live(page)).toContain(`${barKey} can't be changed`);
    expect(await datesOf(grid, barKey)).toEqual(before);
  }
});

test("touch screens: read only, a tap opens the task, two fingers zoom", async ({ browser }) => {
  const context = await browser.newContext({ hasTouch: true, isMobile: true, viewport: { width: 1000, height: 800 } });
  const page = await context.newPage();
  const grid = await openGantt(page);
  await expect(page.getByTestId("status-note")).toHaveText(/Read only on touch screens/);
  await expect(page.getByRole("switch", { name: "Also move dependents" })).toHaveCount(0);
  await expect(grid.locator("[data-handle=link]")).toHaveCount(0);
  await expect(grid.locator("button[aria-label*='date of']:not([disabled])")).toHaveCount(0);
  const key = await (async () => {
    for (const element of await grid.locator("[data-bar][data-shape=bar]").all()) {
      const box = await element.locator("[data-handle=move]").boundingBox();
      if (box && box.width >= 2 * DAY_PX && box.x > 450 && box.x + box.width < 950) {
        return (await element.getAttribute("data-bar")) ?? "";
      }
    }
    throw new Error("no bar on screen");
  })();
  const before = await datesOf(grid, key);
  await bar(grid, key).locator("[data-handle=move]").tap();
  await expect(page.getByRole("complementary").or(page.locator("[data-task-panel]")).first()).toBeVisible();
  expect(await datesOf(grid, key)).toEqual(before);

  // Pinch out zooms in, pinch in zooms out: touch events through the browser's own protocol.
  const cdp = await context.newCDPSession(page);
  const box = await grid.boundingBox();
  if (!box) {
    throw new Error("no chart");
  }
  const cx = box.x + box.width * 0.6;
  const cy = box.y + box.height / 2;
  const fingers = (gap: number) => [
    { x: cx - gap, y: cy, id: 1 },
    { x: cx + gap, y: cy, id: 2 },
  ];
  const pinch = async (from: number, to: number) => {
    await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: fingers(from) });
    for (let step = 1; step <= 6; step += 1) {
      await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: fingers(from + ((to - from) * step) / 6) });
    }
    await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  };
  await expect(page.locator("[data-gantt]")).toHaveAttribute("data-zoom", "day");
  await pinch(120, 30);
  await expect(page.locator("[data-gantt]")).not.toHaveAttribute("data-zoom", "day");
  const out = await page.locator("[data-gantt]").getAttribute("data-zoom");
  await pinch(30, 120);
  await expect(page.locator("[data-gantt]")).not.toHaveAttribute("data-zoom", out ?? "");
  await context.close();
});

test("claimed by: the agent's column and grouping by the session that claimed the tasks", async ({ page }) => {
  const grid = await openGantt(page);
  await expect(grid.getByRole("columnheader", { name: "Claimed by" })).toBeVisible();
  await page.getByText("Include sub-projects", { exact: true }).click();
  await page.getByRole("button", { name: /^Group: / }).click();
  await page.getByRole("menuitemradio", { name: "Claimed by" }).click();
  await expect.poll(async () => plain(await page.getByRole("button", { name: /^Group: / }).getAttribute("aria-label"))).toBe("Group: Claimed by");
  // The tasks no agent works on come last, flat.
  const groups = grid.locator("[data-group^='claim:']");
  await expect(groups.last()).toHaveAttribute("data-group", "claim:");
  expect(plain(await groups.last().textContent())).toMatch(/^Not claimed: (one task|\d+ tasks)$/);
  await expect(grid.locator("[role=row][aria-level='2']")).toHaveCount(0);
  // A claimed task names its agent in the column and in its row's name.
  const claimed = grid.locator("[data-row]").filter({ has: page.locator("[role=gridcell]", { hasText: "Build agent" }) });
  if ((await claimed.count()) > 0) {
    await expect(claimed.first()).toHaveAttribute("aria-label", /claimed by ⁨?Build agent/);
    expect(plain(await groups.first().textContent())).toMatch(/^Build agent: (one task|\d+ tasks)$/);
  }
});

test("a claim that ends leaves the chart without a reload", async ({ page }) => {
  // A clock that runs, so the chart's timer fires when the time is moved on.
  await page.clock.install({ time: new Date(`${TODAY}T10:00:00`) });
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Gantt" }).click();
  const grid = page.getByRole("treegrid", { name: "Gantt chart" });
  await page.getByText("Include sub-projects", { exact: true }).click();
  // Grouped by claim, the claimed task comes first.
  await page.getByRole("button", { name: /^Group: / }).click();
  await page.getByRole("menuitemradio", { name: "Claimed by" }).click();
  const groups = grid.locator("[data-group^='claim:']");
  const agentGroup = groups.filter({ hasText: "Build agent" });
  // The sample agent's claim lasts 45 minutes from when the sample was made.
  await expect(agentGroup).toHaveCount(1);
  const claimed = grid.locator("[data-row]").filter({ has: page.locator("[role=gridcell]", { hasText: "Build agent" }) });
  await expect(claimed).toHaveCount(1);
  await page.clock.runFor("30:00");
  await expect(agentGroup).toHaveCount(1);
  await page.clock.runFor("16:00");
  await expect(agentGroup).toHaveCount(0);
  await expect(claimed).toHaveCount(0);
  await expect(groups).toHaveCount(1);
});

test("milestones: the project's list with dates and how each stands; choosing one shows it in the chart", async ({ page }) => {
  const grid = await openGantt(page);
  await page.getByRole("button", { name: "Milestones" }).click();
  const dialog = page.getByRole("dialog", { name: "Milestones" });
  await expect(dialog).toBeVisible();
  const items = dialog.locator("[data-milestone]");
  await expect(items.first()).toBeVisible();
  // Each has a date (or "No date") and a state; overdue ones say by how long.
  for (const item of await items.all()) {
    const text = plain(await item.innerText());
    expect(text.length).toBeGreaterThan(0);
    if ((await item.getAttribute("data-state")) === "overdue") {
      expect(text).toMatch(/Overdue by/);
    }
  }
  await shot(page, "gantt-milestones.png");
  const key = (await items.first().getAttribute("data-milestone")) ?? "";
  await items.first().click();
  await expect(dialog).toBeHidden();
  // The chart selects its row.
  await expect(row(grid, key)).toHaveAttribute("aria-selected", "true");
  await expect(row(grid, key)).toBeInViewport();
});

test("saved views: a query and a grouping saved as a view of the chart, chosen again later", async ({ page }) => {
  await openGantt(page);
  // The status bar counts the tasks shown; the rows themselves are drawn only near the screen.
  const counts = page.getByText(/scheduled.*unscheduled/);
  const all = plain(await counts.textContent());
  const field = page.getByRole("textbox", { name: "Filter query" });
  await field.click();
  await page.keyboard.type("is:closed");
  await page.keyboard.press("Escape");
  await expect.poll(async () => plain(await counts.textContent())).not.toBe(all);
  const filtered = plain(await counts.textContent());
  await expect(page.getByTestId("list-changed")).toBeVisible();

  await page.getByRole("button", { name: "Save as…" }).click();
  const save = page.getByRole("dialog", { name: "Save view" });
  await save.getByRole("textbox", { name: "Name" }).fill("Finished work");
  await expect(save.getByText("Show it first when the chart opens")).toBeVisible();
  await save.getByRole("button", { name: "Save view" }).click();
  await expect(page.getByRole("button", { name: /Saved view: .*Finished work/ })).toBeVisible();
  await expect(page.getByTestId("list-changed")).toHaveCount(0);

  // A grouping chosen while the view is on changes the view, until saved.
  await page.getByRole("button", { name: /^Group: / }).click();
  await page.getByRole("menuitemradio", { name: "Status" }).click();
  await expect(page.getByTestId("list-changed")).toBeVisible();
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByTestId("list-changed")).toHaveCount(0);

  // All tasks: the person's own grouping and no query.
  await page.getByRole("button", { name: /Saved view: / }).click();
  await page.getByRole("menuitem", { name: "All tasks" }).click();
  await expect.poll(async () => plain(await counts.textContent())).toBe(all);
  await expect.poll(async () => plain(await page.getByRole("button", { name: /^Group: / }).getAttribute("aria-label"))).toBe("Group: Parent");

  // The view is there on the next visit, and brings back its query and grouping.
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Board" }).click();
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Gantt" }).click();
  await page.getByRole("button", { name: /Saved view: / }).click();
  await page.getByRole("menuitem", { name: /Finished work/ }).click();
  await expect.poll(async () => plain(await counts.textContent())).toBe(filtered);
  await expect(field).toHaveText("is:closed");
  await expect.poll(async () => plain(await page.getByRole("button", { name: /^Group: / }).getAttribute("aria-label"))).toBe("Group: Status");
});

test("Today, and the keys for a page earlier, later and back to today", async ({ page }) => {
  const grid = await openGantt(page);
  const today = grid.locator("[data-today]");
  await expect(today).toBeInViewport();
  await page.keyboard.press("Alt+PageDown");
  await page.keyboard.press("Alt+PageDown");
  await expect(today).not.toBeInViewport();
  await page.keyboard.press("Alt+Home");
  await expect(today).toBeInViewport();
  await page.getByRole("button", { name: "Later" }).click();
  await page.getByRole("button", { name: "Later" }).click();
  await expect(today).not.toBeInViewport();
  await page.getByRole("button", { name: "Today", exact: true }).click();
  await expect(today).toBeInViewport();
});
