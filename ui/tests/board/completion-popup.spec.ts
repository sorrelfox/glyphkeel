// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

// The editor's completion list in the comment composer at the bottom of a
// small window: it opens above the cursor, inside the window, for "[[" and
// for "#", and it looks like the app's other menus.

test.use({ viewport: { width: 1000, height: 640 } });

const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

function lane(page: Page, name: string): Locator {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

/** A new task in "To do", open in its panel, with the comment composer focused. */
async function openComposer(page: Page, title: string): Promise<{ panel: Locator; composer: Locator }> {
  await page.goto("/?wiki");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await lane(page, "To do").getByRole("button", { name: /Add task to/ }).click();
  await page.keyboard.type(title);
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: title });
  const key = (await card.getAttribute("data-card")) ?? "";
  await card.dblclick();
  const panel = page.getByRole("complementary", { name: new RegExp(`^Task \\W*${key}\\W*$`) });
  const composer = panel.getByRole("textbox", { name: "Write a comment" });
  await composer.scrollIntoViewIfNeeded();
  await composer.click();
  return { panel, composer };
}

interface Box {
  top: number;
  bottom: number;
  left: number;
  right: number;
}

/** The completion list's box and the cursor's, in viewport coordinates. */
async function boxes(page: Page): Promise<{ list: Box; cursor: Box; viewport: { width: number; height: number } }> {
  return page.evaluate(() => {
    const box = (element: Element | null): Box => {
      const rect = element?.getBoundingClientRect();
      return { top: rect?.top ?? 0, bottom: rect?.bottom ?? 0, left: rect?.left ?? 0, right: rect?.right ?? 0 };
    };
    const selection = window.getSelection();
    const range = selection && selection.rangeCount > 0 ? selection.getRangeAt(0) : null;
    const rect = range?.getBoundingClientRect();
    return {
      list: box(document.querySelector(".cm-tooltip-autocomplete")),
      cursor: { top: rect?.top ?? 0, bottom: rect?.bottom ?? 0, left: rect?.left ?? 0, right: rect?.right ?? 0 },
      viewport: { width: window.innerWidth, height: window.innerHeight },
    };
  });
}

/** Whether nothing between the list and the page clips it: the topmost element at each corner is the list. */
async function unclipped(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const list = document.querySelector(".cm-tooltip-autocomplete");
    if (!list) {
      return false;
    }
    const rect = list.getBoundingClientRect();
    const inset = 3;
    const points = [
      [rect.left + inset, rect.top + inset],
      [rect.right - inset, rect.top + inset],
      [rect.left + inset, rect.bottom - inset],
      [rect.right - inset, rect.bottom - inset],
    ] as const;
    return points.every(([x, y]) => list.contains(document.elementFromPoint(x, y)));
  });
}

async function expectAboveAndInside(page: Page): Promise<void> {
  const list = page.getByRole("listbox");
  await expect(list.getByRole("option").first()).toBeVisible();
  // The list settles after it measures its place.
  await expect
    .poll(async () => {
      const { list: box, cursor, viewport } = await boxes(page);
      return box.top >= 0 && box.left >= 0 && box.bottom <= viewport.height && box.right <= viewport.width && box.bottom <= cursor.top + 1;
    })
    .toBe(true);
  expect(await unclipped(page)).toBe(true);
}

test("in the composer at the bottom of the window, the [[ list opens above the cursor, inside the window", async ({ page }) => {
  const { composer } = await openComposer(page, "Plan the launch");
  // A single page: a short list that would fit below the cursor in the
  // window still keeps clear of the status bar.
  await page.keyboard.insertText("Hi [[Stor");
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(1);
  await expectAboveAndInside(page);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Backspace");
  // Every page.
  await page.keyboard.type("Hi [[");
  await expect(page.getByRole("listbox").getByRole("option")).not.toHaveCount(1);
  await expectAboveAndInside(page);
  // A named list box, owned by the text field through its active option.
  await expect(page.getByRole("listbox", { name: "Suggestions" })).toBeVisible();
  const active = await composer.getAttribute("aria-activedescendant");
  expect(active).toBeTruthy();
  await expect(page.locator(`[id="${active ?? ""}"]`)).toHaveAttribute("role", "option");
  const results = await new AxeBuilder({ page }).withTags(TAGS).include(".cm-tooltip-autocomplete").analyze();
  expect(results.violations).toEqual([]);

  // It looks like the app's menus: the interface font, the popover colour and its shadow.
  const style = await page.locator(".cm-tooltip-autocomplete").evaluate((element) => {
    const list = getComputedStyle(element.querySelector("ul") ?? element);
    const box = getComputedStyle(element);
    const probe = document.body.appendChild(document.createElement("span"));
    probe.style.fontFamily = "var(--font-ui)";
    const ui = getComputedStyle(probe).fontFamily;
    probe.remove();
    return { font: list.fontFamily, shadow: box.boxShadow, ui };
  });
  expect(style.font).toBe(style.ui);
  expect(style.font).not.toMatch(/^monospace/);
  expect(style.shadow).not.toBe("none");

  // Picking a page with the keyboard still works.
  const first = page.getByRole("listbox").getByRole("option").first();
  await expect(first).toHaveAttribute("aria-selected", "true");
  const label = (await first.locator(".cm-completionLabel").textContent()) ?? "";
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("Enter");
  await expect(composer).toContainText(`Hi [[${label}]]`);
});

test("in the composer at the bottom of the window, the # list opens above the cursor, inside the window", async ({ page }) => {
  const { composer } = await openComposer(page, "Feed the zebras");
  // One task, found by a word of its title.
  await page.keyboard.insertText("See #zebras");
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(1);
  await expectAboveAndInside(page);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("listbox")).toHaveCount(0);
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Backspace");
  // The recent tasks.
  await page.keyboard.type("See #");
  await expect(page.getByRole("listbox").getByRole("option")).not.toHaveCount(1);
  await expectAboveAndInside(page);
  // A task key alone completes too.
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Backspace");
  await page.keyboard.insertText("See TM-1");
  await expectAboveAndInside(page);
  await page.keyboard.press("Enter");
  await expect(composer).toContainText(/See TM-1\d*/);
});
