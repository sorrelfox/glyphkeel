// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

// The calendar on the sample data, whose dates lie around today. The clock
// is fixed, so the sample data and the grid are the same on every run. With
// GK_SCREENSHOT_DIR set, the flows save screenshots there.

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

const TODAY = "2026-11-11";

async function openCalendar(page: Page) {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Calendar" }).click();
  const grid = page.getByRole("grid", { name: /^Calendar/ });
  await expect(grid).toBeVisible();
  await expect(grid.locator("[data-task-bar]").first()).toBeVisible();
  return grid;
}

async function shot(page: Page, name: string) {
  const dir = process.env.GK_SCREENSHOT_DIR;
  if (dir) {
    await page.screenshot({ path: join(dir, name) });
  }
}

const panel = (page: Page, key: string) => page.getByRole("complementary", { name: new RegExp(`^Task\\W*${key}\\W*$`) });
const day = (grid: Locator, date: string) => grid.locator(`[data-day="${date}"]`);
const bar = (grid: Locator, key: string) => grid.locator(`[data-task-bar="${key}"]`);
const daysOf = async (grid: Locator, key: string) => (await bar(grid, key).first().getAttribute("data-days")) ?? "";
const focusedDay = (page: Page) => page.evaluate(() => (document.activeElement as HTMLElement | null)?.dataset.day);

function addDays(date: string, days: number): string {
  const at = new Date(`${date}T12:00:00Z`);
  at.setUTCDate(at.getUTCDate() + days);
  return at.toISOString().slice(0, 10);
}

/** Shows the week of today, where every task of a day is drawn. */
async function showWeek(page: Page) {
  await page.getByRole("radiogroup", { name: "Calendar range" }).getByRole("radio", { name: "Week" }).click();
  await expect(page.locator("[data-week]")).toHaveCount(1);
}

/** A one-day task near today. */
async function oneDayTask(grid: Locator): Promise<{ key: string; date: string }> {
  for (const element of await grid.locator("[data-task-bar]").all()) {
    const days = (await element.getAttribute("data-days")) ?? "";
    const key = (await element.getAttribute("data-task-bar")) ?? "";
    if (!days.includes(" ") && days >= addDays(TODAY, -3) && days <= addDays(TODAY, 3) && (await element.isVisible())) {
      return { key, date: days };
    }
  }
  throw new Error("no one-day task near today in the sample data");
}

test("a month of tasks: today, the locale's first day, spans and more, paging", async ({ page }) => {
  const grid = await openCalendar(page);
  await expect(page.getByTestId("calendar-title")).toHaveText("November 2026");
  // The browser's locale is American English: weeks start on Sunday.
  await expect(grid.getByRole("columnheader").first()).toHaveAttribute("aria-label", "Sunday");
  await expect(grid.locator("[data-day]").first()).toHaveAttribute("data-day", "2026-11-01");
  await expect(day(grid, TODAY)).toHaveAttribute("aria-current", "date");
  expect(plain((await day(grid, TODAY).getAttribute("aria-label")) ?? "")).toMatch(/^Wednesday, November 11, 2026, .*, today$/);

  // A task with a start and a due date runs over its days.
  await expect(grid.locator("[data-task-bar][data-days*=' ']").first()).toBeVisible();
  // A day with more tasks than fit says how many more, and shows its week.
  const more = grid.locator("[data-more]").first();
  await expect(more).toBeVisible();
  const count = Number(await more.getAttribute("data-more"));
  expect(plain(await more.innerText())).toBe(`+${String(count)} more`);
  await shot(page, "calendar-month.png");
  await more.click();
  await expect(grid.locator("[data-week]")).toHaveCount(1);
  await expect(grid.locator("[data-more]")).toHaveCount(0);
  await shot(page, "calendar-week.png");

  // Back to the month; the arrows page, Today comes back.
  await page.getByRole("radiogroup", { name: "Calendar range" }).getByRole("radio", { name: "Month" }).click();
  await page.getByRole("button", { name: "Next month" }).click();
  await expect(page.getByTestId("calendar-title")).toHaveText("December 2026");
  await page.getByRole("button", { name: "Today", exact: true }).click();
  await expect(page.getByTestId("calendar-title")).toHaveText("November 2026");
});

test.describe("in British English", () => {
  test.use({ locale: "en-GB" });

  test("weeks start on Monday", async ({ page }) => {
    const grid = await openCalendar(page);
    await expect(grid.getByRole("columnheader").first()).toHaveAttribute("aria-label", "Monday");
    await expect(grid.locator("[data-day]").first()).toHaveAttribute("data-day", "2026-10-26");
  });
});

test("drag a task to another day, then Undo; a click opens it beside the calendar", async ({ page }) => {
  const grid = await openCalendar(page);
  await showWeek(page);
  const { key, date } = await oneDayTask(grid);
  const column = await day(grid, date).evaluate((cell) => [...(cell.parentElement?.children ?? [])].indexOf(cell));
  const target = addDays(date, column <= 4 ? 2 : -2);

  const from = await bar(grid, key).first().boundingBox();
  const to = await day(grid, target).boundingBox();
  if (!from || !to) {
    throw new Error("the task or the day isn't on screen");
  }
  // Taken at the day it lies on, dropped on the other.
  await page.mouse.move(from.x + 12, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(from.x + 22, from.y + from.height / 2, { steps: 3 });
  await page.mouse.move(to.x + to.width / 2, to.y + to.height - 8, { steps: 12 });
  await expect(day(grid, target)).toHaveAttribute("data-drop", "");
  await page.mouse.up();
  await expect.poll(() => daysOf(grid, key)).toBe(target);
  const note = page.getByRole("status").filter({ hasText: new RegExp(`${key}\\W* moved to`) });
  await expect(note).toBeVisible();

  // Undo takes it back, as one step.
  await note.getByRole("button", { name: "Undo" }).click();
  await expect.poll(() => daysOf(grid, key)).toBe(date);

  // A click opens the task's panel, as on the board.
  await bar(grid, key).first().click();
  await expect(panel(page, key)).toBeVisible();
  await panel(page, key).getByRole("button", { name: "Close panel" }).click();
  await expect(panel(page, key)).toHaveCount(0);
  // The focus goes back to the task on the calendar.
  await expect.poll(() => page.evaluate(() => (document.activeElement as HTMLElement | null)?.dataset.taskBar)).toBe(key);
});

test("the keyboard moves between days and pages, opens a day's tasks and moves a task", async ({ page }) => {
  const grid = await openCalendar(page);
  await day(grid, TODAY).click({ position: { x: 4, y: 4 } });
  await expect.poll(() => focusedDay(page)).toBe(TODAY);
  await page.keyboard.press("ArrowRight");
  await expect.poll(() => focusedDay(page)).toBe(addDays(TODAY, 1));
  await page.keyboard.press("ArrowDown");
  await expect.poll(() => focusedDay(page)).toBe(addDays(TODAY, 8));
  await page.keyboard.press("PageDown");
  await expect(page.getByTestId("calendar-title")).toHaveText("December 2026");
  await expect.poll(() => focusedDay(page)).toBe("2026-12-19");
  await page.keyboard.press("PageUp");
  await expect(page.getByTestId("calendar-title")).toHaveText("November 2026");

  // Enter goes into a day's tasks, the arrow keys go through them; Alt and
  // an arrow key moves one a day on, and the focus stays on it.
  await page.getByRole("button", { name: "Today", exact: true }).click();
  await showWeek(page);
  const { key, date } = await oneDayTask(grid);
  await day(grid, date).click({ position: { x: 4, y: 4 } });
  await page.keyboard.press("Enter");
  const focusedTask = () => page.evaluate(() => (document.activeElement as HTMLElement | null)?.dataset.taskBar);
  for (let tries = 0; tries < 30 && (await focusedTask()) !== key; tries += 1) {
    await page.keyboard.press("ArrowDown");
  }
  expect(await focusedTask()).toBe(key);
  await page.keyboard.press("Alt+ArrowRight");
  await expect.poll(() => daysOf(grid, key)).toBe(addDays(date, 1));
  await expect.poll(focusedTask).toBe(key);

  // Enter opens it beside the calendar.
  await page.keyboard.press("Enter");
  await expect(panel(page, key)).toBeVisible();
});

test("the query field and sub-projects choose the tasks", async ({ page }) => {
  const grid = await openCalendar(page);
  await expect(grid.locator("[data-task-bar^='CLI-']")).toHaveCount(0);
  await page.getByText("Include sub-projects", { exact: true }).click();
  await expect(page.getByRole("switch", { name: "Include sub-projects" })).toBeChecked();
  await expect(grid.locator("[data-task-bar^='CLI-'], [data-task-bar^='DOC-']").first()).toBeAttached();

  await showWeek(page);
  const { key } = await oneDayTask(grid);
  const field = page.getByRole("textbox", { name: "Filter query" });
  await field.click();
  await page.keyboard.type(`key:${key}`);
  await page.keyboard.press("Escape");
  await expect.poll(() => grid.locator("[data-task-bar]").evaluateAll((bars) => [...new Set(bars.map((each) => (each as HTMLElement).dataset.taskBar))])).toEqual([key]);
});
