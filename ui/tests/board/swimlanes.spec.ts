// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

// Swimlanes on sample data: the toolbar groups the board by priority; each
// lane is a labelled region with a collapse button that names its count;
// dropping a card into another lane and column changes both, announced with
// the lane, and one Ctrl+Z takes both back; the keyboard's Move to lane
// moves it too, and refuses a lane that can't take it; collapsing a lane
// hides its cells; axe finds nothing in light, dark and right to left.

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");

/** Where Fluent may put an isolation mark, in a pattern. */
const I = "[\u2068\u2069]*";

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

async function openBoard(page: Page): Promise<void> {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
}

async function lanesBy(page: Page, by: string): Promise<void> {
  await page.getByRole("button", { name: /^Swimlanes: / }).click();
  await page.getByRole("menuitemradio", { name: by, exact: true }).click();
  if (by !== "None") {
    await expect(page.locator("section[data-swimlane]").first()).toBeVisible();
  }
}

const swimlane = (page: Page, name: string): Locator => page.getByRole("region", { name: new RegExp(`^Lane ${I}${name}${I}, ${I}\\d+${I} tasks?$`) });

/** The cell a card is in: its lane's ID and its column's. */
async function cellOf(card: Locator): Promise<string> {
  return card.evaluate((element) => element.closest("[data-lane]")?.getAttribute("data-lane") ?? "");
}

async function columnNames(page: Page): Promise<string[]> {
  return page.locator("[data-column] h2").allInnerTexts();
}

test("cards group into swimlanes by priority, and a drop into another lane and column changes both, undone at once", async ({ page }) => {
  await openBoard(page);
  await lanesBy(page, "Priority");
  for (const name of ["Urgent", "High", "Normal", "Low", "No priority"]) {
    await expect(swimlane(page, name)).toBeVisible();
  }
  // The lane without a value comes last.
  const regions = page.locator("section[data-swimlane]");
  await expect(regions.last()).toHaveAttribute("data-swimlane", "none");

  // A high card, dragged into the urgent lane's cell of another column.
  const columns = await columnNames(page);
  const card = swimlane(page, "High").locator("[data-card]").first();
  await card.scrollIntoViewIfNeeded();
  const key = (await card.getAttribute("data-card")) ?? "";
  const home = await cellOf(card);
  const fromColumn = await page.locator(`[data-lane="${home.replace(/"/g, '\\"')}"]`).getAttribute("aria-label");
  const columnName = columns.find((name) => !plain(fromColumn).includes(`, ${name}:`)) ?? "";
  const target = swimlane(page, "Urgent").getByRole("group", { name: new RegExp(`^${I}Urgent${I}, ${I}${columnName}${I}: `) });
  await target.scrollIntoViewIfNeeded();
  const from = await card.boundingBox();
  const to = await target.boundingBox();
  if (!from || !to) {
    throw new Error("the card or the target cell isn't on screen");
  }
  await page.mouse.move(from.x + from.width / 2, from.y + 12);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2 + 10, from.y + 20, { steps: 4 });
  await page.mouse.move(to.x + to.width / 2, to.y + to.height - 8, { steps: 20 });
  await page.mouse.up();
  const moved = page.locator(`[data-lane] [data-card="${key}"]`);
  await expect.poll(async () => (await cellOf(moved)).startsWith("urgent")).toBe(true);
  await expect.poll(async () => plain(await page.getByTestId("status-note").innerText())).toBe(`${key} moved to ${columnName} in lane Urgent`);
  await expect(target.locator(`[data-card="${key}"]`)).toBeVisible();

  // One Ctrl+Z: back in its own lane and column.
  await page.locator("body").click({ position: { x: 1, y: 1 } });
  await page.keyboard.press("Control+z");
  await expect.poll(() => cellOf(moved)).toBe(home);

  // Move to lane, from the card's menu: the same column, another lane.
  await moved.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Move to lane" }).click();
  await page.getByRole("menuitem", { name: "Low" }).click();
  await expect.poll(async () => (await cellOf(moved)).startsWith("low")).toBe(true);
  expect((await cellOf(moved)).split("\u001f")[1]).toBe(home.split("\u001f")[1]);

  // Collapse: the button names the count; the lane's cells go, its header stays.
  const low = swimlane(page, "Low");
  const collapse = low.getByRole("button", { name: new RegExp(`^Collapse lane ${I}Low${I}, ${I}\\d+${I} tasks?$`) });
  await expect(collapse).toHaveAttribute("aria-expanded", "true");
  await collapse.click();
  await expect(low.getByRole("button", { name: new RegExp(`^Expand lane ${I}Low${I}, ${I}\\d+${I} tasks?$`) })).toHaveAttribute("aria-expanded", "false");
  await expect(low.locator("[data-card]")).toHaveCount(0);
  // Collapsed lanes are the person's own, kept per grouping: another grouping
  // opens its own lanes; back to priority, Low is still collapsed.
  await lanesBy(page, "Label");
  await expect(page.locator("section[data-swimlane]").last()).toHaveAttribute("data-swimlane", "none");
  await expect(page.getByRole("button", { name: /^Expand lane/ })).toHaveCount(0);
  await lanesBy(page, "Priority");
  await expect(low.getByRole("button", { name: new RegExp(`^Expand lane ${I}Low`) })).toBeVisible();
  // And after a reload (the sample data starts again; the person's settings stay).
  await page.reload();
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await lanesBy(page, "Priority");
  await expect(low.getByRole("button", { name: new RegExp(`^Expand lane ${I}Low`) })).toHaveAttribute("aria-expanded", "false");
  await lanesBy(page, "None");
  await expect(page.locator("section[data-swimlane]")).toHaveCount(0);
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
});

test("a parent's card can't go into its own lane, and the menu says why", async ({ page }) => {
  await openBoard(page);
  await lanesBy(page, "Epic or parent");
  await expect(page.locator("section[data-swimlane]").last()).toHaveAttribute("data-swimlane", "none");
  // A parent whose own card is on the board too.
  const names = (await page.locator("section[data-swimlane] h3").allInnerTexts()).map(plain);
  let parent = "";
  for (const name of names) {
    const key = name.split(" ")[0] ?? "";
    if (/^[A-Z]+-\d+$/.test(key) && (await page.locator(`[data-lane] [data-card="${key}"]`).count()) > 0) {
      parent = key;
      break;
    }
  }
  expect(parent, "the sample has a parent on its own board").not.toBe("");
  await page.locator(`[data-lane] [data-card="${parent}"]`).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Move to lane" }).click();
  const own = page.getByRole("menuitem", { name: new RegExp(`^${I}${parent}${I} `) });
  await expect(own).toBeDisabled();
  await expect(own).toContainText("its own lane");
  await page.keyboard.press("Escape");
});

for (const scheme of ["light", "dark"] as const) {
  test.describe(`${scheme} theme`, () => {
    test.use({ colorScheme: scheme });

    test(`swimlanes pass axe in the ${scheme} theme`, async ({ page }) => {
      await openBoard(page);
      await lanesBy(page, "Priority");
      await swimlane(page, "Low").getByRole("button", { name: new RegExp(`^Collapse lane ${I}Low`) }).click();
      const results = await new AxeBuilder({ page }).withTags(TAGS).include("[role=group][aria-label^='Board of']").analyze();
      expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`)).toEqual([]);
    });
  });
}

test.describe("right to left", () => {
  test.use({ locale: "he-IL" });

  test("lanes mirror: names at the inline start, the first column on the right, axe clean", async ({ page }) => {
    await openBoard(page);
    await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
    await lanesBy(page, "Priority");
    const header = swimlane(page, "Urgent").locator("header");
    const board = page.getByRole("group", { name: /^Board of/ });
    const [headerBox, boardBox] = [await header.boundingBox(), await board.boundingBox()];
    if (!headerBox || !boardBox) {
      throw new Error("the lane header isn't on screen");
    }
    // The lane's name starts at the board's right edge.
    expect(boardBox.x + boardBox.width - (headerBox.x + headerBox.width)).toBeLessThan(40);
    const heads = page.locator("[data-column]");
    const first = await heads.first().boundingBox();
    const second = await heads.nth(1).boundingBox();
    if (!first || !second) {
      throw new Error("the column headers aren't on screen");
    }
    expect(first.x).toBeGreaterThan(second.x);
    const results = await new AxeBuilder({ page }).withTags(TAGS).include("[role=group][aria-label^='Board of']").analyze();
    expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`)).toEqual([]);
  });
});
