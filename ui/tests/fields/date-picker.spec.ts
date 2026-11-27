// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The task panel's day picker and its other popovers, at the panel's own
// width: each opens whole inside the window's content, below the header
// bar, with nothing in it overlapping or spilling out of its box.

import { expect, test, type Locator, type Page } from "@playwright/test";

type Box = { x: number; y: number; width: number; height: number };

async function box(locator: Locator): Promise<Box> {
  const found = await locator.boundingBox();
  expect(found, "the element has a box").not.toBeNull();
  return found as Box;
}

/** `inner` lies within `outer`, give or take a pixel of rounding. */
function expectInside(inner: Box, outer: Box, what: string) {
  expect(inner.x, `${what}: start`).toBeGreaterThanOrEqual(outer.x - 1);
  expect(inner.y, `${what}: top`).toBeGreaterThanOrEqual(outer.y - 1);
  expect(inner.x + inner.width, `${what}: end`).toBeLessThanOrEqual(outer.x + outer.width + 1);
  expect(inner.y + inner.height, `${what}: bottom`).toBeLessThanOrEqual(outer.y + outer.height + 1);
}

function expectApart(a: Box, b: Box, what: string) {
  const overlap = a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
  expect(overlap, what).toBe(false);
}

/** The popover holding `dialog` lies inside the content area, and so inside the window. */
async function expectFits(page: Page, dialog: Locator) {
  const popover = dialog.locator("xpath=..");
  const outer = await box(popover);
  const content = await box(page.locator('[data-region="content"]'));
  const viewport = page.viewportSize() ?? { width: 0, height: 0 };
  expectInside(outer, { x: 0, y: 0, width: viewport.width, height: viewport.height }, "popover in the window");
  expectInside(outer, content, "popover below the header bar");
  return outer;
}

async function openPanel(page: Page) {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).first().click();
  await page.locator("section[data-lane] [data-card]").first().dblclick();
  const panel = page.getByRole("complementary", { name: /^Task / });
  await expect(panel).toBeVisible();
  return panel;
}

test("Start and Due are edited in words and on the calendar, in a picker that fits", async ({ page }) => {
  const panel = await openPanel(page);
  const start = panel.getByRole("button", { name: /^\W*Start\W*:/ });

  // Typed in words: the picker shows the day they mean, Enter keeps it.
  await start.click();
  const picker = page.getByRole("dialog", { name: "Start" });
  const words = picker.getByRole("textbox", { name: /Start\W* in words/ });
  await expect(words).toBeFocused();
  const outer = await expectFits(page, picker);
  const grid = picker.getByTestId("day-picker-grid");
  const field = await box(words);
  const month = await box(grid);
  const clear = await box(picker.getByRole("button", { name: "Clear" }));
  for (const [part, at] of [["words", field], ["calendar", month], ["Clear", clear]] as const) {
    expectInside(at, outer, `${part} inside the picker`);
  }
  expectApart(field, month, "the words and the calendar don't overlap");
  expectApart(month, clear, "the calendar and Clear don't overlap");
  await expect(picker.locator('input[type="date"]')).toHaveCount(0);

  await words.fill("2031-12-01");
  await expect(picker.getByTestId("day-picker-preview")).toContainText("December 1, 2031");
  await words.press("Enter");
  await expect(picker).toBeHidden();
  await expect(start).toHaveAccessibleName(/Dec 1/);

  // Words that are no day keep the picker open and say so.
  await start.click();
  await words.fill("someday");
  await expect(picker.getByTestId("day-picker-preview")).toContainText("Not a date");
  await words.press("Enter");
  await expect(picker).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(picker).toBeHidden();
  await expect(start).toHaveAccessibleName(/Dec 1/);

  // Picked on the calendar with the mouse: the month after, the 15th.
  await start.click();
  await expect(picker.getByTestId("day-picker-grid").getByRole("button", { name: /December 1, 2031/ })).toBeVisible();
  await picker.getByRole("button", { name: "Next month" }).click();
  await picker.getByRole("button", { name: /January 15, 2032/ }).click();
  await expect(picker).toBeHidden();
  await expect(start).toHaveAccessibleName(/Jan 15/);

  // Due, by keyboard on the calendar: down into the month, a day on, Enter.
  const due = panel.getByRole("button", { name: /^\W*Due\W*:/ });
  await due.click();
  const duePicker = page.getByRole("dialog", { name: "Due" });
  await expectFits(page, duePicker);
  await duePicker.getByRole("textbox", { name: /Due\W* in words/ }).fill("2032-02-20");
  await page.keyboard.press("ArrowDown");
  await expect(duePicker.getByRole("button", { name: /February 20, 2032/ })).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(duePicker.getByRole("button", { name: /February 21, 2032/ })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(duePicker).toBeHidden();
  await expect(due).toHaveAccessibleName(/Feb 21/);

  // Start cleared; Due typed in words, then cleared.
  await start.click();
  await picker.getByRole("button", { name: "Clear" }).click();
  await expect(start).toHaveAccessibleName(/None/);
  await due.click();
  await duePicker.getByRole("textbox", { name: /Due\W* in words/ }).fill("+3d");
  await page.keyboard.press("Enter");
  await expect(due).toHaveAccessibleName(/in 3 days/);
  await due.click();
  await duePicker.getByRole("button", { name: "Clear" }).click();
  await expect(due).toHaveAccessibleName(/None/);
});

test("the Repeats editor keeps its dates and its buttons inside its box on a short window", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 640 });
  const panel = await openPanel(page);
  await panel.getByRole("button", { name: /^\W*Repeats\W*:/ }).click();
  const editor = page.getByRole("dialog", { name: "Repeat this task" });
  await expect(editor).toBeVisible();
  const outer = await expectFits(page, editor);
  const footer = await box(editor.getByTestId("repeat-footer"));
  expectInside(footer, outer, "the buttons inside the editor");
  expectInside(await box(editor.getByTestId("repeat-body")), outer, "the choices inside the editor");
  const viewport = page.viewportSize() ?? { width: 0, height: 0 };
  expectInside(footer, { x: 0, y: 0, width: viewport.width, height: viewport.height }, "the buttons in the window");
  await expect(editor.getByRole("button", { name: "Save" })).toBeInViewport();

  // The next dates scroll inside the body; they never spill past the box.
  const dates = editor.getByRole("list", { name: /Next dates/ });
  if ((await dates.count()) > 0) {
    await dates.scrollIntoViewIfNeeded();
    expectInside(await box(dates), outer, "the next dates inside the editor");
  }
  await page.keyboard.press("Escape");
  await expect(editor).toBeHidden();
});

test("in a narrow panel a date and how far off it is stay whole inside the cell", async ({ page }) => {
  const panel = await openPanel(page);
  await panel.evaluate((element) => {
    (element as HTMLElement).style.flexBasis = "440px";
  });
  const due = panel.getByRole("button", { name: /^\W*Due\W*:/ });
  await due.click();
  await page.getByRole("textbox", { name: /Due\W* in words/ }).fill("-8d");
  await page.keyboard.press("Enter");
  await expect(due).toHaveAccessibleName(/8 days ago/);

  const cell = await box(due.locator("xpath=.."));
  // The value button reaches into the gutter before the cell for its hover; its text stays in the cell.
  const value = await box(due);
  expect(value.x + value.width, "the value ends inside its cell").toBeLessThanOrEqual(cell.x + cell.width + 1);
  const date = await box(due.locator("span").first());
  expectInside(date, cell, "the date inside the cell");
  const away = await box(due.getByText("8 days ago"));
  expectInside(away, cell, "the relative text inside the cell");
  // Each is one line: the relative text sits beside the date or wholly under it.
  expect(away.height).toBeLessThanOrEqual(date.height + 2);
  expect(away.height).toBeLessThan(24);
  expect(date.height).toBeLessThan(24);
  expectApart(date, away, "the date and the relative text don't overlap");
});

test("a list row's due date is picked in the same picker, anchored to its cell", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).first().click();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  const cell = page.getByRole("button", { name: /^\W*Due date of \W*TM-1\W*:/ });
  await cell.click();
  const picker = page.getByRole("dialog", { name: /Due date of \W*TM-1/ });
  await expectFits(page, picker);
  await picker.getByRole("textbox", { name: /in words/ }).fill("2031-12-01");
  await page.keyboard.press("Enter");
  await expect(picker).toBeHidden();
  await expect(cell).toHaveAccessibleName(/Dec 1, 2031|Dec 1/);
});
