// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// The list's sideways scrolling, with scroll bars that take room as the
// desktop's web view draws them (headless Chromium hides them unless told
// not to): when the columns fit there is no sideways scroll bar at all, and
// when they don't, the scroll bar sits above the totals row, never over it.
// The same for a project's list, a list opened from a dashboard, and a
// dashboard's task lists.

test.use({ locale: "en-US", launchOptions: { ignoreDefaultArgs: ["--hide-scrollbars"] } });

const TODAY = "2026-11-11";

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

interface Measure {
  scrollWidth: number;
  clientWidth: number;
  /** The scroller's box bottom, past any scroll bar it draws. */
  scrollerBottom: number;
  /** Where the visible rows end: above a sideways scroll bar. */
  rowsBottom: number;
  totalsTop: number;
  totalsBottom: number;
  /** Whether the totals row is inside the scrolling area. */
  totalsInside: boolean;
  /** The title cell's start in the header and in the totals row. */
  headTitle: number;
  totalsTitle: number;
  viewportBottom: number;
}

async function measure(page: Page): Promise<Measure> {
  await expect(page.getByTestId("list-totals")).toBeVisible();
  return page.evaluate(() => {
    const scroller = document.querySelector<HTMLElement>("[data-list-scroll]");
    const totals = document.querySelector<HTMLElement>("[data-testid=list-totals]");
    if (!scroller || !totals) {
      throw new Error("no list");
    }
    const box = scroller.getBoundingClientRect();
    const totalsBox = totals.getBoundingClientRect();
    const title = (row: Element | null) => (row?.children[2] as HTMLElement | undefined)?.getBoundingClientRect().left ?? Number.NaN;
    return {
      scrollWidth: scroller.scrollWidth,
      clientWidth: scroller.clientWidth,
      scrollerBottom: box.bottom,
      rowsBottom: box.top + scroller.clientTop + scroller.clientHeight,
      totalsTop: totalsBox.top,
      totalsBottom: totalsBox.bottom,
      totalsInside: scroller.contains(totals),
      headTitle: title(scroller.querySelector(".head")),
      totalsTitle: title(totals),
      viewportBottom: window.innerHeight,
    };
  });
}

/** The totals row shows whole, under the rows and their scroll bars. */
function expectTotalsClear(m: Measure): void {
  expect(m.totalsInside).toBe(false);
  expect(m.totalsTop).toBeGreaterThanOrEqual(m.scrollerBottom - 0.5);
  expect(m.totalsTop).toBeGreaterThanOrEqual(m.rowsBottom - 0.5);
  expect(m.totalsBottom).toBeLessThanOrEqual(m.viewportBottom + 0.5);
  expect(Math.abs(m.headTitle - m.totalsTitle)).toBeLessThanOrEqual(1);
}

async function start(page: Page): Promise<void> {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await expect(page.getByRole("main")).toBeVisible();
  const nav = page.getByRole("navigation", { name: "Workspace" });
  if (!(await nav.isVisible())) {
    await page.keyboard.press("F9");
  }
  await expect(nav).toBeVisible();
}

async function allTasks(page: Page): Promise<void> {
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /^All tasks\b/ }).first().click();
  await expect(page.getByRole("grid").first()).toBeVisible();
}

async function projectList(page: Page): Promise<void> {
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
  const views = page.getByRole("group", { name: "Views" });
  const button = views.getByRole("button", { name: "List", exact: true });
  if (await button.isVisible()) {
    await button.click();
  } else {
    await views.getByRole("button", { name: "More views" }).click();
    await page.getByRole("menuitemradio", { name: "List", exact: true }).click();
  }
  await expect(page.getByRole("grid").first()).toBeVisible();
}

async function starterDashboard(page: Page): Promise<void> {
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /^Dashboards\b/ }).first().click();
  await page.getByRole("button", { name: "Create a starter dashboard" }).click();
  await expect(page.locator("[data-widget]").first()).toBeVisible();
}

/**
 * The default columns need about 1,340 px: they fit a wide window, and in a
 * medium one (1024 px, with the sidebar) they scroll sideways by design.
 */
function expectSideways(m: Measure, fits: boolean): void {
  if (fits) {
    expect(m.scrollWidth).toBeLessThanOrEqual(m.clientWidth);
    expect(m.rowsBottom).toBeCloseTo(m.scrollerBottom, 0);
  } else {
    expect(m.scrollWidth).toBeGreaterThan(m.clientWidth);
    expect(m.rowsBottom).toBeLessThan(m.scrollerBottom);
  }
}

for (const { size, fits } of [
  { size: { width: 1600, height: 1000 }, fits: true },
  { size: { width: 1024, height: 700 }, fits: false },
]) {
  test.describe(`${String(size.width)}×${String(size.height)}`, () => {
    test.use({ viewport: size });

    test("all tasks with the default columns scrolls sideways only when they don't fit, and the totals show whole", async ({ page }) => {
      await start(page);
      await allTasks(page);
      const m = await measure(page);
      expectSideways(m, fits);
      expectTotalsClear(m);
      const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
      expect(results.violations.map((violation) => `${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`)).toEqual([]);
    });

    test("a project's list scrolls sideways only when its columns don't fit", async ({ page }) => {
      await start(page);
      await projectList(page);
      const m = await measure(page);
      expectSideways(m, fits);
      expectTotalsClear(m);
    });

    test("a dashboard's task lists don't scroll sideways, nor a list opened from it when its columns fit", async ({ page }) => {
      await start(page);
      await starterDashboard(page);
      const bodies = await page.locator("[data-widget]").evaluateAll((widgets) =>
        widgets.map((widget) => {
          const body = [...widget.querySelectorAll<HTMLElement>("*")].find((element) => getComputedStyle(element).overflowX === "auto");
          return { kind: widget.getAttribute("data-widget"), scrollWidth: body?.scrollWidth ?? 0, clientWidth: body?.clientWidth ?? 0 };
        }),
      );
      for (const body of bodies.filter((each) => each.kind === "overdue" || each.kind === "due_soon")) {
        expect(body.scrollWidth, String(body.kind)).toBeLessThanOrEqual(body.clientWidth);
      }
      await page.getByRole("article", { name: "Open tasks" }).getByRole("button", { name: "Open as a list" }).click();
      const m = await measure(page);
      expectSideways(m, fits);
      expectTotalsClear(m);
    });
  });
}

test.describe("columns that don't fit", () => {
  test.use({ viewport: { width: 1600, height: 1000 } });

  test("many columns scroll sideways above the totals, which follow the rows", async ({ page }) => {
    await start(page);
    await allTasks(page);
    await page.getByRole("button", { name: "Columns" }).click();
    const unchecked = page.getByRole("menuitemcheckbox", { checked: false });
    while ((await unchecked.count()) > 0) {
      await unchecked.first().click();
      if (!(await page.getByRole("menuitemcheckbox").first().isVisible())) {
        await page.getByRole("button", { name: "Columns" }).click();
      }
    }
    await page.keyboard.press("Escape");
    const m = await measure(page);
    expect(m.scrollWidth).toBeGreaterThan(m.clientWidth);
    // The sideways scroll bar takes room under the rows, and the totals sit below it.
    expect(m.rowsBottom).toBeLessThan(m.scrollerBottom);
    expectTotalsClear(m);

    await page.locator("[data-list-scroll]").evaluate((scroller) => {
      scroller.scrollLeft = 300;
    });
    await expect.poll(async () => {
      const now = await measure(page);
      return Math.abs(now.headTitle - now.totalsTitle);
    }).toBeLessThanOrEqual(1);
    const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
    expect(results.violations.map((violation) => `${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`)).toEqual([]);
  });
});

test.describe("a narrow window", () => {
  test.use({ viewport: { width: 760, height: 520 } });

  test("the default columns scroll sideways above the totals", async ({ page }) => {
    await start(page);
    await allTasks(page);
    const m = await measure(page);
    expect(m.scrollWidth).toBeGreaterThan(m.clientWidth);
    expect(m.rowsBottom).toBeLessThan(m.scrollerBottom);
    expectTotalsClear(m);
  });
});
