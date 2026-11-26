// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// What the desktop asks for, through the web view's media queries and the
// locale: less motion, more contrast, forced colours, and a language written
// right to left.

const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

async function openBoard(page: Page): Promise<void> {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
}

/** The WCAG 2.2 AA findings on the page, as short lines. */
async function violations(page: Page): Promise<string[]> {
  const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
  // Calendar days aren't on these screens; nothing else is accepted.
  return results.violations.flatMap((violation) => violation.nodes.map((node) => `${violation.id}: ${node.target.join(" ")}`));
}

test.describe("less motion", () => {
  test.use({ reducedMotion: "reduce" });

  test("takes the durations to zero, so menus and popovers appear at once", async ({ page }) => {
    await openBoard(page);
    const root = page.locator("html");
    await expect.poll(() => root.evaluate((element) => element.style.getPropertyValue("--dur"))).toBe("0ms");
    await page.locator("button[class*=projectButton]").first().click();
    const menu = page.locator(".menuPopover").first();
    await expect(menu).toBeVisible();
    expect(await menu.evaluate((element) => parseFloat(getComputedStyle(element).animationDuration))).toBe(0);
  });
});

test.describe("more contrast", () => {
  test.use({ contrast: "more", colorScheme: "light" });

  test("switches to the contrast theme, which passes on the board and the list", async ({ page }) => {
    await openBoard(page);
    await expect(page.locator("html")).toHaveAttribute("data-theme", "contrast");
    expect(await violations(page)).toEqual([]);
    await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
    await expect(page.getByRole("grid").first()).toBeVisible();
    expect(await violations(page)).toEqual([]);
  });
});

test.describe("forced colours", () => {
  test.use({ forcedColors: "active" });

  test("keep the edges of fields, cards and checkboxes and the focus ring", async ({ page }) => {
    await openBoard(page);
    const card = page.locator("section[data-lane] [data-card]").first();
    expect(await card.evaluate((element) => getComputedStyle(element).borderTopStyle)).toBe("solid");
    const field = page.locator(".field").first();
    expect(await field.evaluate((element) => getComputedStyle(element).borderTopStyle)).toBe("solid");
    // A focused card shows an outline, not only a shadow, which forced colours drop.
    await card.focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowUp");
    const ring = await page.evaluate(() => {
      const element = document.activeElement as HTMLElement;
      const style = getComputedStyle(element);
      return { style: style.outlineStyle, width: parseFloat(style.outlineWidth) };
    });
    expect(ring.style).not.toBe("none");
    expect(ring.width).toBeGreaterThanOrEqual(2);
  });
});

test.describe("right to left", () => {
  test.use({ locale: "he-IL" });

  test("the page takes its direction from the locale and still passes", async ({ page }) => {
    await openBoard(page);
    const root = page.locator("html");
    await expect(root).toHaveAttribute("dir", "rtl");
    // No Hebrew messages yet: the text, and so the language, is English.
    await expect(root).toHaveAttribute("lang", "en");
    expect(await violations(page)).toEqual([]);
  });

  test("a language chosen in Preferences sets the direction too", async ({ page }) => {
    await page.addInitScript(() => {
      window.localStorage.setItem("glyphkeel.language", JSON.stringify("en"));
    });
    await openBoard(page);
    await expect(page.locator("html")).toHaveAttribute("dir", "ltr");
  });
});
