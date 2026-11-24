// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { mkdirSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

// The main screens in the pseudo-locale, one picture each. Navigation goes by
// structure and by the sample data's names, which are not translated, never
// by the interface's own words.

const SHOTS = process.env["GK_SCREENSHOT_DIR"] ?? "test-results/pseudo";
const LOCALE = process.env["GK_PSEUDO_LOCALE"] ?? "en-XA";

/** Accented letters inside brackets: the pseudo-locale's mark on a message. */
const PSEUDO_TEXT = /\[[^\]]*[áéíóúýƀçđĝĥĵķĺñþŕšţṽŵẋž]/u;

async function shot(page: Page, name: string): Promise<void> {
  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, `${name}.png`), fullPage: false });
}

/**
 * Elements whose text is cut off: content wider than the box with hidden
 * overflow. Reported with the picture; a list to look at, not a failure.
 */
function clipped(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const found: string[] = [];
    for (const element of document.querySelectorAll("body *")) {
      if (!(element instanceof HTMLElement)) {
        continue;
      }
      const style = getComputedStyle(element);
      const text = element.innerText.trim();
      if (text === "" || element.children.length > 0 || style.overflowX === "visible") {
        continue;
      }
      if (element.scrollWidth > element.clientWidth + 1 && style.textOverflow !== "ellipsis") {
        found.push(text.slice(0, 60));
      }
    }
    return found;
  });
}

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.waitForLoadState("networkidle");
  const text = await page.locator("body").innerText();
  test.skip(!PSEUDO_TEXT.test(text), `the page doesn't show the pseudo-locale ${LOCALE}`);
});

test("the main screens in the pseudo-locale", async ({ page }, info) => {
  const report: Record<string, string[]> = {};

  await shot(page, "01-today");
  report["today"] = await clipped(page);

  // A project's board, from its row in the sample data.
  await page.getByRole("row", { name: /Task Manager/ }).first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
  await shot(page, "02-board");
  report["board"] = await clipped(page);

  // A task beside the board.
  await page.locator("section[data-lane] [data-card]").first().click();
  await page.waitForTimeout(300);
  await shot(page, "03-task");
  report["task"] = await clipped(page);
  await page.keyboard.press("Escape");

  // The list of the same project: the second of the header's views.
  await page.locator(".seg[role=group] button").nth(1).click();
  await expect(page.getByRole("grid").first()).toBeVisible();
  await shot(page, "04-list");
  report["list"] = await clipped(page);

  // The command palette.
  await page.keyboard.press("Control+k");
  await page.waitForTimeout(300);
  await shot(page, "05-palette");
  report["palette"] = await clipped(page);
  await page.keyboard.press("Escape");

  // Preferences, through their shortcut.
  await page.keyboard.press("Control+Comma");
  await expect(page.getByRole("dialog").first()).toBeVisible();
  await shot(page, "06-preferences");
  report["preferences"] = await clipped(page);
  await page.keyboard.press("Escape");

  await info.attach("clipped-text.json", { body: JSON.stringify(report, null, 2), contentType: "application/json" });
  for (const [screen, items] of Object.entries(report)) {
    if (items.length > 0) {
      console.log(`${screen}: ${String(items.length)} clipped: ${items.slice(0, 5).join(" | ")}`);
    }
  }
});
