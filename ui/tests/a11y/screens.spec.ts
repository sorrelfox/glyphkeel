// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { appendFileSync } from "node:fs";

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// axe-core on every screen and main dialog of the sample workspace, in every
// theme and with more than the default accents, against the WCAG 2.2 A and
// AA rules. A finding
// fails the test unless it is one of the accepted exceptions below, each with
// its reason. GK_A11Y_LOG names a file that gets every finding and every item
// axe leaves for review, one JSON line each.

const TODAY = "2026-11-11";

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

interface Exception {
  rule: string;
  /** A part of the element's selector. */
  target: string;
  reason: string;
}

/** Findings accepted on purpose. */
const EXCEPTIONS: Exception[] = [
  {
    rule: "label-content-name-mismatch",
    target: "[data-day=",
    reason:
      "A calendar day is a grid cell that holds its tasks, each a button with a name of its own, and its day number. " +
      "Its name is the full date and the number of tasks; the text inside belongs to the tasks, not to the day.",
  },
];

interface Screen {
  name: string;
  /** Opens the screen or dialog from Today. */
  open: (page: Page) => Promise<void>;
  /** Where to start: Today unless given. */
  path?: string;
}

async function openProject(page: Page): Promise<void> {
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
}

async function view(page: Page, name: string): Promise<void> {
  await openProject(page);
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name }).click();
}

async function sidebar(page: Page, name: string): Promise<void> {
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: new RegExp(`^${name}\\b`) }).first().click();
}

async function projectMenu(page: Page, item: RegExp): Promise<void> {
  await openProject(page);
  await page.locator("button[class*=projectButton]").first().click();
  await page.getByRole("menuitem", { name: item }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
}

async function openTask(page: Page): Promise<void> {
  await openProject(page);
  await page.locator("section[data-lane] [data-card]").first().dblclick();
  await expect(page.getByRole("complementary", { name: /^Task / }).getByRole("button", { name: "Task menu" })).toBeVisible();
}

const SETTINGS_TABS = ["General", "Statuses", "Task types", "Labels", "Fields", "Rules", "Automation", "Templates"];
const PREFERENCE_PAGES = ["Appearance", "Language", "Time", "Notifications", "Quick add", "Keyboard shortcuts", "Agents & CLI", "Workspace", "Data and backups"];

const SCREENS: Screen[] = [
  { name: "today", open: async (page) => { await expect(page.getByRole("main")).toBeVisible(); } },
  { name: "board", open: openProject },
  { name: "task panel", open: openTask },
  {
    // The three kinds of "Linked from" row: a wiki page, a description, a comment.
    name: "task panel with links to it",
    path: "/?wiki",
    open: async (page) => {
      await openProject(page);
      await page.locator('section[data-lane] [data-card="TM-3"]').dblclick();
      const section = page.getByRole("complementary", { name: /^Task / }).getByRole("region", { name: "Linked from" });
      await expect(section.getByRole("button")).toHaveCount(3);
    },
  },
  {
    name: "move dialog",
    open: async (page) => {
      await openTask(page);
      await page.getByRole("button", { name: "Task menu" }).click();
      await page.getByRole("menuitem", { name: /another project/ }).click();
      await expect(page.getByRole("dialog")).toBeVisible();
    },
  },
  { name: "list", open: async (page) => { await view(page, "List"); await expect(page.getByRole("grid").first()).toBeVisible(); } },
  { name: "calendar", open: async (page) => { await view(page, "Calendar"); await page.waitForTimeout(300); } },
  { name: "gantt", open: async (page) => { await view(page, "Gantt"); await page.waitForTimeout(300); } },
  {
    name: "wiki",
    path: "/?wiki",
    open: async (page) => {
      await view(page, "Wiki");
      await expect(page.getByRole("treegrid", { name: "Pages" }).getByRole("row").first()).toBeVisible();
      await page.getByRole("treegrid", { name: "Pages" }).getByRole("row").first().click();
      await expect(page.getByRole("region", { name: /Wiki page/ })).toBeVisible();
    },
  },
  {
    // A page that other pages and tasks link to, and that links to a task.
    name: "wiki page with links",
    path: "/?wiki",
    open: async (page) => {
      await view(page, "Wiki");
      await page.getByRole("treegrid", { name: "Pages" }).getByRole("row", { name: "Query language" }).click();
      await expect(page.getByRole("region", { name: "Tasks that link here" }).getByRole("button").first()).toBeVisible();
    },
  },
  { name: "roadmap", open: async (page) => { await view(page, "Roadmap"); await expect(page.getByRole("article").first()).toBeVisible(); } },
  { name: "sprints", open: async (page) => { await view(page, "Sprints"); await expect(page.locator("[data-chart]").first()).toBeVisible(); } },
  { name: "reports", open: async (page) => { await sidebar(page, "Reports"); await expect(page.locator("[data-chart]").first()).toBeVisible(); } },
  { name: "time report", open: async (page) => { await sidebar(page, "Time report"); await page.waitForTimeout(300); } },
  { name: "all tasks", open: async (page) => { await sidebar(page, "All tasks"); await expect(page.getByRole("grid").first()).toBeVisible(); } },
  { name: "activity", open: async (page) => { await sidebar(page, "Activity"); await page.waitForTimeout(300); } },
  { name: "trash", open: async (page) => { await sidebar(page, "Trash"); await page.waitForTimeout(300); } },
  {
    name: "search",
    open: async (page) => {
      const field = page.getByRole("navigation", { name: "Workspace" }).getByRole("combobox", { name: "Search the workspace" });
      await field.fill("query");
      await field.press("Enter");
      await page.waitForTimeout(500);
    },
  },
  {
    name: "palette",
    open: async (page) => {
      await page.keyboard.press("Control+k");
      await expect(page.getByRole("dialog")).toBeVisible();
      await page.keyboard.type("TM");
      await expect(page.getByRole("option").first()).toBeVisible();
    },
  },
  { name: "quick add", path: "/quick-add", open: async (page) => { await expect(page.getByRole("textbox").first()).toBeVisible(); } },
  { name: "first run", path: "/?first-run", open: async (page) => { await expect(page.getByRole("dialog")).toBeVisible(); } },
  {
    name: "CSV import",
    open: async (page) => {
      await projectMenu(page, /CSV/);
      const dialog = page.getByRole("dialog");
      await dialog.getByRole("textbox").first().fill("tasks.csv");
      await dialog.getByRole("button", { name: "Check" }).click();
      await expect(dialog.getByTestId("csv-summary")).toBeVisible();
    },
  },
  ...SETTINGS_TABS.map((tab) => ({
    name: `project settings: ${tab}`,
    open: async (page: Page) => {
      await projectMenu(page, /settings/i);
      await page.getByRole("dialog").getByRole("tab", { name: tab, exact: true }).click();
      await page.waitForTimeout(200);
    },
  })),
  ...PREFERENCE_PAGES.map((name) => ({
    name: `preferences: ${name}`,
    open: async (page: Page) => {
      await page.keyboard.press("Control+Comma");
      const dialog = page.getByRole("dialog");
      await expect(dialog).toBeVisible();
      await dialog.getByRole("tab", { name, exact: true }).click();
      await page.waitForTimeout(200);
    },
  })),
];

/** Appends one record to the GK_A11Y_LOG file, when there is one. */
function log(record: Record<string, unknown>): void {
  const file = process.env["GK_A11Y_LOG"];
  if (file !== undefined && file !== "") {
    appendFileSync(file, `${JSON.stringify(record)}\n`);
  }
}

function accepted(rule: string, target: string): boolean {
  return EXCEPTIONS.some((exception) => exception.rule === rule && target.includes(exception.target));
}

interface Look {
  name: string;
  scheme: "light" | "dark";
  /** What the appearance settings hold; nothing: follow the desktop. */
  appearance?: { theme: string; accent?: string };
  theme: string;
  accent: string;
}

/**
 * The themes, each with the accent it suggests, and two more accents: the
 * darkest fill on a light theme, and one on a dark theme, where accent text
 * sits on an accent-tinted selection.
 */
const LOOKS: Look[] = [
  { name: "light theme", scheme: "light", theme: "light", accent: "blue" },
  { name: "dark theme", scheme: "dark", theme: "dark", accent: "blue" },
  { name: "sand theme", scheme: "light", appearance: { theme: "sand" }, theme: "sand", accent: "orange" },
  { name: "slate theme", scheme: "dark", appearance: { theme: "slate" }, theme: "slate", accent: "teal" },
  { name: "light theme, amber accent", scheme: "light", appearance: { theme: "light", accent: "amber" }, theme: "light", accent: "amber" },
  { name: "dark theme, purple accent", scheme: "dark", appearance: { theme: "dark", accent: "purple" }, theme: "dark", accent: "purple" },
];

for (const look of LOOKS) {
  test.describe(look.name, () => {
    test.use({ colorScheme: look.scheme, locale: "en-US" });

    for (const screen of SCREENS) {
      test(`${screen.name} has no WCAG 2.2 AA violations`, async ({ page }) => {
        await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
        if (look.appearance) {
          await page.addInitScript((appearance) => {
            window.localStorage.setItem("glyphkeel.appearance", JSON.stringify(appearance));
          }, look.appearance);
        }
        await page.goto(screen.path ?? "/");
        await expect(page.locator("html")).toHaveAttribute("data-theme", look.theme);
        await expect(page.locator("html")).toHaveAttribute("data-accent", look.accent);
        await screen.open(page);
        // Let transitions settle, so colours are measured at rest.
        await page.waitForTimeout(250);
        // Target size (2.5.8) is off in axe by default; it is AA in WCAG 2.2.
        const results = await new AxeBuilder({ page }).options({ rules: { "target-size": { enabled: true } } }).withTags(TAGS).analyze();
        const ran = [...results.passes, ...results.violations, ...results.incomplete, ...results.inapplicable].map((rule) => rule.id);
        expect(ran).toContain("target-size");
        for (const review of results.incomplete) {
          for (const node of review.nodes) {
            log({ screen: screen.name, look: look.name, review: review.id, target: node.target.join(" "), html: node.html.slice(0, 200), summary: node.failureSummary });
          }
        }
        const found: string[] = [];
        for (const violation of results.violations) {
          for (const node of violation.nodes) {
            const target = node.target.join(" ");
            if (!accepted(violation.id, target)) {
              log({ screen: screen.name, look: look.name, rule: violation.id, impact: violation.impact, target, html: node.html.slice(0, 300), summary: node.failureSummary });
              found.push(`[${violation.impact ?? "?"}] ${violation.id}: ${target} — ${node.failureSummary?.split("\n").slice(1, 3).join(" ") ?? ""}`);
            }
          }
        }
        expect(found).toEqual([]);
      });
    }
  });
}
