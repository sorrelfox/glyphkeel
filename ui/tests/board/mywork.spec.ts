// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

// My work on sample data: the sidebar entry and the command palette open
// it; a column for each status category, lanes by project, each card with
// its project's colour and its real status; the quick toggles narrow the
// board and press; a card dropped into In progress, where its project has
// two statuses, opens a chooser; axe finds nothing in light, dark and
// right to left.

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

const board = (page: Page): Locator => page.getByRole("group", { name: "My work across projects" });

async function openMyWork(page: Page): Promise<void> {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "My work", exact: true }).click();
  await expect(board(page)).toBeVisible();
  await expect(page.locator("section[data-swimlane]").first()).toBeVisible();
}

const cardCount = (page: Page) => page.locator("[data-lane] [data-card]").count();

async function axe(page: Page): Promise<void> {
  const results = await new AxeBuilder({ page }).withTags(TAGS).include("[role=group][aria-label='My work across projects']").include("[role=toolbar]").analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`)).toEqual([]);
}

test("My work shows every active project's cards by category, in lanes by project, each with its status", async ({ page }) => {
  await openMyWork(page);
  await expect(page.locator("[data-column] h2")).toHaveText(["To do", "In progress", "Done"]);
  const lanes = await page.locator("section[data-swimlane]").evaluateAll((all) => all.map((each) => each.getAttribute("data-swimlane")));
  expect(lanes).toEqual(["CLI", "DOC", "LAB", "PER", "TM"]);
  // The archived blog is not there.
  await expect(page.locator('[data-card^="BLOG-"]')).toHaveCount(0);
  const first = page.locator("[data-lane] [data-card]").first();
  await expect(first.locator("[data-status]")).toBeVisible();
  await expect(first.locator("[data-mark]")).toHaveAttribute("aria-label", /^Project /);

  // The palette opens it too.
  await page.goto("/");
  await page.keyboard.press("Control+k");
  await page.keyboard.type("My work");
  await page.keyboard.press("Enter");
  await expect(board(page)).toBeVisible();
});

test("the quick toggles narrow the board and show as pressed", async ({ page }) => {
  await openMyWork(page);
  const all = await cardCount(page);
  const high = page.getByRole("button", { name: "High priority" });
  await expect(high).toHaveAttribute("aria-pressed", "false");
  await high.click();
  await expect(high).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => cardCount(page)).toBeLessThan(all);
  await expect(page.locator("[data-lane] [data-card]").first()).toBeVisible();
  await high.click();
  await expect.poll(() => cardCount(page)).toBe(all);
});

test("a card dropped into In progress asks which of its project's two statuses", async ({ page }) => {
  await openMyWork(page);
  const card = page.locator('[data-lane^="TM\u001ftodo"] [data-card]').first();
  const key = (await card.getAttribute("data-card")) ?? "";
  const target = page.locator('[data-lane="TM\u001fin_progress"]');
  await card.scrollIntoViewIfNeeded();
  const from = await card.boundingBox();
  const to = await target.boundingBox();
  if (!from || !to) {
    throw new Error("the card or the target isn't on screen");
  }
  await page.mouse.move(from.x + from.width / 2, from.y + 12);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2 + 10, from.y + 20, { steps: 4 });
  await page.mouse.move(to.x + to.width / 2, to.y + 30, { steps: 20 });
  await page.mouse.up();
  await expect(page.getByRole("menuitem", { name: "In progress", exact: true })).toBeVisible();
  await page.getByRole("menuitem", { name: "Review", exact: true }).click();
  const moved = page.locator(`[data-lane="TM\u001fin_progress"] [data-card="${key}"]`);
  await expect(moved).toBeVisible();
  await expect(moved.locator("[data-status]")).toHaveText(/Review/);
});

for (const scheme of ["light", "dark"] as const) {
  test.describe(`${scheme} theme`, () => {
    test.use({ colorScheme: scheme });

    test(`My work passes axe in the ${scheme} theme`, async ({ page }) => {
      await openMyWork(page);
      await axe(page);
    });
  });
}

test.describe("right to left", () => {
  test.use({ locale: "he-IL" });

  test("the first column is on the right, and axe finds nothing", async ({ page }) => {
    await openMyWork(page);
    await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
    const heads = page.locator("[data-column]");
    const first = await heads.first().boundingBox();
    const second = await heads.nth(1).boundingBox();
    if (!first || !second) {
      throw new Error("the column headers aren't on screen");
    }
    expect(first.x).toBeGreaterThan(second.x);
    await axe(page);
  });
});
