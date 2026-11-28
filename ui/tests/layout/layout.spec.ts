// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// Every main screen at the window's smallest size and at a medium one, with
// the sidebar open and closed: the page never scrolls sideways (lists,
// boards and charts scroll inside their own areas), the header bar and the
// toolbars show every control whole and none on top of another, and axe
// finds nothing at the smallest size.

const TODAY = "2026-11-11";

/** The main window's smallest size, as the desktop app sets it. */
const SMALLEST = { width: 760, height: 520 };
const MEDIUM = { width: 1024, height: 700 };

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

interface Screen {
  name: string;
  /** Opens the screen from Today. */
  open: (page: Page) => Promise<void>;
  path?: string;
}

async function openProject(page: Page): Promise<void> {
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
}

/** Opens a view of the project: from the header's views, or its overflow menu when they don't fit. */
async function view(page: Page, name: string): Promise<void> {
  await openProject(page);
  const views = page.getByRole("group", { name: "Views" });
  const button = views.getByRole("button", { name, exact: true });
  if (await button.isVisible()) {
    await button.click();
  } else {
    await views.getByRole("button", { name: "More views" }).click();
    await page.getByRole("menuitemradio", { name, exact: true }).click();
  }
}

/** Shows the sidebar: some sizes start without it. */
async function withSidebar(page: Page): Promise<void> {
  const nav = page.getByRole("navigation", { name: "Workspace" });
  if (!(await nav.isVisible())) {
    await page.keyboard.press("F9");
  }
  await expect(nav).toBeVisible();
}

async function sidebar(page: Page, name: string): Promise<void> {
  await withSidebar(page);
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: new RegExp(`^${name}\\b`) }).first().click();
}

async function openTask(page: Page, from: (page: Page) => Promise<void>): Promise<void> {
  await from(page);
  await page.locator("section[data-lane] [data-card]").first().dblclick();
  await expect(page.getByRole("complementary", { name: /^Task / }).getByRole("button", { name: "Task menu" })).toBeVisible();
}

const SCREENS: Screen[] = [
  { name: "today", open: async (page) => { await expect(page.getByRole("main")).toBeVisible(); } },
  { name: "board", open: async (page) => { await withSidebar(page); await openProject(page); } },
  { name: "board with a task open", open: async (page) => { await withSidebar(page); await openTask(page, openProject); } },
  {
    name: "board with swimlanes",
    open: async (page) => {
      await withSidebar(page);
      await openProject(page);
      await page.getByRole("button", { name: /^Swimlanes: / }).click();
      await page.getByRole("menuitemradio", { name: "Priority", exact: true }).click();
      await expect(page.locator("section[data-swimlane]").first()).toBeVisible();
    },
  },
  {
    name: "my work",
    open: async (page) => {
      await sidebar(page, "My work");
      await expect(page.getByRole("group", { name: "My work across projects" })).toBeVisible();
    },
  },
  { name: "list", open: async (page) => { await withSidebar(page); await view(page, "List"); await expect(page.getByRole("grid").first()).toBeVisible(); } },
  { name: "calendar", open: async (page) => { await withSidebar(page); await view(page, "Calendar"); await page.waitForTimeout(300); } },
  { name: "gantt", open: async (page) => { await withSidebar(page); await view(page, "Gantt"); await page.waitForTimeout(300); } },
  { name: "roadmap", open: async (page) => { await withSidebar(page); await view(page, "Roadmap"); await expect(page.getByRole("article").first()).toBeVisible(); } },
  { name: "sprints", open: async (page) => { await withSidebar(page); await view(page, "Sprints"); await expect(page.locator("[data-chart]").first()).toBeVisible(); } },
  {
    name: "wiki",
    path: "/?wiki",
    open: async (page) => {
      await withSidebar(page);
      await view(page, "Wiki");
      await page.getByRole("treegrid", { name: "Pages" }).getByRole("row").first().click();
      await expect(page.getByRole("region", { name: /Wiki page/ })).toBeVisible();
    },
  },
  { name: "reports", open: async (page) => { await sidebar(page, "Reports"); await expect(page.locator("[data-chart]").first()).toBeVisible(); } },
  { name: "time report", open: async (page) => { await sidebar(page, "Time report"); await page.waitForTimeout(300); } },
  {
    name: "dashboards",
    open: async (page) => {
      await sidebar(page, "Dashboards");
      await page.getByRole("button", { name: "Create a starter dashboard" }).click();
      await expect(page.locator("[data-chart]").first()).toBeVisible();
    },
  },
  { name: "all tasks", open: async (page) => { await sidebar(page, "All tasks"); await expect(page.getByRole("grid").first()).toBeVisible(); } },
  { name: "activity", open: async (page) => { await sidebar(page, "Activity"); await page.waitForTimeout(300); } },
  { name: "trash", open: async (page) => { await sidebar(page, "Trash"); await page.waitForTimeout(300); } },
  {
    name: "search",
    open: async (page) => {
      await withSidebar(page);
      const field = page.getByRole("navigation", { name: "Workspace" }).getByRole("combobox", { name: "Search the workspace" });
      await field.fill("query");
      await field.press("Enter");
      await page.waitForTimeout(500);
    },
  },
  {
    name: "preferences",
    open: async (page) => {
      await page.keyboard.press("Control+Comma");
      await expect(page.getByRole("dialog")).toBeVisible();
    },
  },
];

interface Problem {
  what: string;
  detail: string;
}

/**
 * What is wrong with the layout: a page or a main area that scrolls
 * sideways, a control of the header bar, a toolbar or the status bar cut off
 * by its bar or the window, or two of them on top of each other.
 */
function problems(page: Page): Promise<Problem[]> {
  return page.evaluate(() => {
    const found: { what: string; detail: string }[] = [];
    const name = (element: Element) => {
      const label = element.getAttribute("aria-label") ?? element.textContent;
      const kind = element.getAttribute("role") ?? element.tagName.toLowerCase();
      return `${kind} "${label.replace(/[⁨⁩]/g, "").trim().slice(0, 40)}"`;
    };
    const sideways = (element: Element | null, what: string) => {
      if (element && element.scrollWidth > element.clientWidth + 1) {
        found.push({ what, detail: `${String(element.scrollWidth)} wide in ${String(element.clientWidth)}` });
      }
    };
    sideways(document.documentElement, "the page scrolls sideways");
    sideways(document.body, "the body scrolls sideways");
    sideways(document.querySelector("#root"), "the app scrolls sideways");
    const main = document.querySelector("main[data-region=content]");
    sideways(main, "the content area scrolls sideways");
    for (const child of main ? [...main.children] : []) {
      sideways(child, `the screen ${child.className.slice(0, 30)} scrolls sideways`);
    }
    const viewport = { left: 0, right: window.innerWidth, top: 0, bottom: window.innerHeight };
    const shown = (element: Element) => {
      const rect = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.visibility !== "hidden" && style.display !== "contents";
    };
    const controls = (bar: Element) =>
      [...bar.querySelectorAll("button, input, [role=button], [data-query-field], [role=switch], [role=combobox], [role=radio], select, a[href]")].filter(
        (control) =>
          shown(control) &&
          // A form's hidden copy of a list box, not something on screen.
          control.closest("[aria-hidden=true]") === null &&
          control.closest("[role=dialog], .pop") === bar.closest("[role=dialog], .pop"),
      );
    const bars = [...document.querySelectorAll("[data-region=header], [data-region=status], [role=toolbar], .toolbar")].filter(shown);
    for (const bar of bars) {
      const box = bar.getBoundingClientRect();
      const limit = { left: Math.max(box.left, viewport.left), right: Math.min(box.right, viewport.right), top: Math.max(box.top, viewport.top), bottom: Math.min(box.bottom, viewport.bottom) };
      const items = controls(bar).filter((control) => !controls(bar).some((other) => other !== control && other.contains(control)));
      const barName = name(bar).slice(0, 40);
      for (const item of items) {
        const rect = item.getBoundingClientRect();
        if (rect.left < limit.left - 1 || rect.right > limit.right + 1 || rect.top < limit.top - 1 || rect.bottom > limit.bottom + 1) {
          found.push({ what: `${name(item)} is cut off in ${barName}`, detail: `${String(Math.round(rect.left))}–${String(Math.round(rect.right))} in ${String(Math.round(limit.left))}–${String(Math.round(limit.right))}` });
        }
      }
      for (let i = 0; i < items.length; i += 1) {
        for (let j = i + 1; j < items.length; j += 1) {
          const a = items[i]?.getBoundingClientRect();
          const b = items[j]?.getBoundingClientRect();
          if (a && b && Math.min(a.right, b.right) - Math.max(a.left, b.left) > 2 && Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 2) {
            found.push({ what: `${name(items[i] as Element)} and ${name(items[j] as Element)} overlap in ${barName}`, detail: "" });
          }
        }
      }
    }
    return found;
  });
}

for (const size of [SMALLEST, MEDIUM]) {
  for (const sidebarOpen of [true, false]) {
    const look = `${String(size.width)}×${String(size.height)}, sidebar ${sidebarOpen ? "open" : "closed"}`;
    test.describe(look, () => {
      test.use({ viewport: size, locale: "en-US" });

      for (const screen of SCREENS) {
        test(`${screen.name} fits`, async ({ page }) => {
          await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
          await page.goto(screen.path ?? "/");
          await expect(page.getByRole("main")).toBeVisible();
          await screen.open(page);
          const nav = page.getByRole("navigation", { name: "Workspace" });
          if ((await nav.isVisible()) !== sidebarOpen) {
            await page.keyboard.press("F9");
            await expect(nav).toBeVisible({ visible: sidebarOpen });
          }
          await page.waitForTimeout(250);
          expect(await problems(page)).toEqual([]);
          if (size === SMALLEST) {
            const results = await new AxeBuilder({ page }).options({ rules: { "target-size": { enabled: true } } }).withTags(TAGS).analyze();
            const accepted = (rule: string, target: string) => rule === "label-content-name-mismatch" && target.includes("[data-day=");
            const violations = results.violations.flatMap((violation) =>
              violation.nodes.filter((node) => !accepted(violation.id, node.target.join(" "))).map((node) => `${violation.id}: ${node.target.join(" ")}`),
            );
            expect(violations).toEqual([]);
          }
        });
      }
    });
  }
}

test.describe("dark theme", () => {
  test.use({ viewport: SMALLEST, locale: "en-US", colorScheme: "dark" });

  for (const screen of SCREENS) {
    test(`${screen.name} fits and passes axe`, async ({ page }) => {
      await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
      await page.goto(screen.path ?? "/");
      await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
      await screen.open(page);
      await page.waitForTimeout(250);
      expect(await problems(page)).toEqual([]);
      const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
      const violations = results.violations.flatMap((violation) =>
        violation.nodes.filter((node) => !(violation.id === "label-content-name-mismatch" && node.target.join(" ").includes("[data-day="))).map((node) => `${violation.id}: ${node.target.join(" ")}`),
      );
      expect(violations).toEqual([]);
    });
  }
});

test.describe("right to left", () => {
  test.use({ viewport: SMALLEST, locale: "he-IL" });

  for (const screen of SCREENS.filter((each) => ["today", "board", "board with swimlanes", "my work", "list", "dashboards"].includes(each.name))) {
    test(`${screen.name} fits`, async ({ page }) => {
      await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
      await page.goto(screen.path ?? "/");
      await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
      await expect(page.getByRole("main")).toBeVisible();
      await screen.open(page);
      await page.waitForTimeout(250);
      expect(await problems(page)).toEqual([]);
    });
  }
});
