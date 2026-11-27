// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The task panel's property grid at narrow panel widths, in English, in the
// pseudo-locale (longer words) and right to left: no value breaks inside a
// phrase, nothing spills out of its cell or the panel, and axe finds nothing.

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

// Found by structure, not by name, so the same steps run in every language.
async function openPanel(page: Page): Promise<Locator> {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).first().click();
  await page.locator("section[data-lane] [data-card]").first().dblclick();
  const panel = page.locator("aside").filter({ has: page.locator("dl") });
  await expect(panel).toBeVisible();
  return panel;
}

/** The value button of the property grid's `index`th row. */
const value = (panel: Locator, index: number) => panel.locator(":scope dl").nth(index).locator("dd button").first();

const ESTIMATE = 4;
const VERSION = 6;
const SPRINT = 7;

/** Fills the values that run long: an estimate with time spent, a version, a sprint and labels. */
async function fillValues(page: Page, panel: Locator, english: boolean) {
  await value(panel, ESTIMATE).click();
  await page.keyboard.type("8h");
  await page.keyboard.press("Enter");
  if (english) {
    await panel.getByRole("tab", { name: "Time" }).click();
    await panel.getByRole("button", { name: "Log time" }).click();
    const form = panel.getByRole("group", { name: "Log time" });
    await form.getByRole("textbox", { name: "Duration" }).fill("30m");
    await form.getByRole("button", { name: "Log", exact: true }).click();
    await expect(panel.getByRole("list", { name: "Time entries" })).toBeVisible();
  }

  await value(panel, SPRINT).click();
  await page.getByRole("menuitemradio").filter({ hasText: "Sprint 2" }).click();
  await expect(value(panel, SPRINT)).toContainText("Sprint 2");
  await value(panel, VERSION).click();
  const version = page.getByRole("menuitemradio").first();
  if (await version.isVisible()) {
    await version.click();
  } else {
    await page.keyboard.press("Escape");
  }
  await panel.locator('[data-task-key="labels"]').click();
  const options = page.getByRole("menuitemcheckbox");
  await expect(options.first()).toBeVisible();
  const count = Math.min(3, await options.count());
  for (let index = 0; index < count; index += 1) {
    await options.nth(index).click();
  }
  await page.keyboard.press("Escape");
}

interface Problem {
  what: string;
  text: string;
}

/**
 * Every phrase in the property grid that breaks over lines, and every value
 * that spills out of its cell or the panel, or makes the grid scroll sideways.
 */
async function layoutProblems(panel: Locator): Promise<Problem[]> {
  return panel.evaluate((root) => {
    const problems: { what: string; text: string }[] = [];
    const grid = root.querySelector("dl")?.parentElement;
    if (!grid) {
      return [{ what: "no property grid", text: "" }];
    }
    const outer = root.getBoundingClientRect();
    if (grid.scrollWidth > grid.clientWidth + 1) {
      problems.push({ what: "the grid scrolls sideways", text: "" });
    }
    for (const cell of grid.querySelectorAll("dl > dt, dl > dd")) {
      const box = cell.getBoundingClientRect();
      if (box.left < outer.left - 1 || box.right > outer.right + 1) {
        problems.push({ what: "a cell spills out of the panel", text: cell.textContent });
      }
      // Each run of text is one phrase: on one line, inside its cell.
      const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
        const text = (node.textContent ?? "").trim();
        if (text === "") {
          continue;
        }
        const range = document.createRange();
        range.selectNodeContents(node);
        const rects = [...range.getClientRects()].filter((rect) => rect.width > 0);
        const tops = new Set(rects.map((rect) => Math.round(rect.top)));
        // A label may wrap between its words; a value's phrase never does.
        if (cell.tagName === "DD" && tops.size > 1) {
          problems.push({ what: "a phrase breaks over lines", text });
        }
        const clip = (node.parentElement ?? cell).closest(".truncate") ?? cell;
        const bounds = clip.getBoundingClientRect();
        const cellBox = cell.getBoundingClientRect();
        if (bounds.left < cellBox.left - 8 || bounds.right > cellBox.right + 1) {
          problems.push({ what: "a phrase spills out of its cell", text });
        }
      }
    }
    return problems;
  });
}

for (const { name, locale } of [
  { name: "English", locale: "en-US" },
  { name: "the pseudo-locale", locale: "en-XA" },
  { name: "right to left", locale: "ar" },
]) {
  test.describe(name, () => {
    test.use({ locale });

    for (const width of [400, 550]) {
      test(`at ${String(width)} px the property values stay whole inside their cells`, async ({ page }) => {
        const panel = await openPanel(page);
        await panel.evaluate((element, basis) => {
          (element as HTMLElement).style.flexBasis = `${String(basis)}px`;
        }, width);
        await expect.poll(async () => (await panel.boundingBox())?.width).toBeLessThanOrEqual(width + 1);
        await fillValues(page, panel, locale === "en-US");
        expect(await layoutProblems(panel)).toEqual([]);

        const results = await new AxeBuilder({ page }).include("aside").withTags(TAGS).analyze();
        expect(results.violations.flatMap((violation) => violation.nodes.map((node) => `${violation.id}: ${node.target.join(" ")}`))).toEqual([]);
      });
    }
  });
}
