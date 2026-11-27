// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// The keyboard on the sample workspace: Tab reaches every part of a screen
// and never gets stuck, the focused control always shows a focus indicator,
// dialogs give the focus back to what opened them, and what can be dragged
// can be done with keys too.

const TODAY = "2026-11-11";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

interface Stop {
  /** A short description of the focused element. */
  what: string;
  /**
   * Whether the focus can be seen: something about the element, its
   * ::before or ::after, or the field or row around it (up to five levels)
   * looks different while it has the focus.
   */
  shown: boolean;
}

/** The focused element, and whether its focus can be seen. */
function stop(page: Page): Promise<Stop | null> {
  return page.evaluate(async () => {
    // Components mark the focus a moment after it moves.
    const settle = () => new Promise((resolve) => setTimeout(resolve, 50));
    const element = document.activeElement;
    if (!(element instanceof HTMLElement) || element === document.body) {
      return null;
    }
    const look = () => {
      const parts: string[] = [];
      let node: Element | null = element;
      for (let level = 0; node && level < 6; level += 1, node = node.parentElement) {
        const style = getComputedStyle(node);
        parts.push(style.outlineStyle, style.outlineWidth, style.outlineColor, style.boxShadow, style.backgroundColor, style.borderColor, style.textDecorationLine);
      }
      for (const pseudo of ["::before", "::after"]) {
        const style = getComputedStyle(element, pseudo);
        parts.push(style.backgroundColor, style.boxShadow, style.outlineStyle);
      }
      return parts.join("|");
    };
    const focused = look();
    element.blur();
    await settle();
    const blurred = look();
    element.focus();
    await settle();
    const name = element.getAttribute("aria-label") ?? element.textContent;
    const role = element.getAttribute("role");
    const what = `${element.tagName.toLowerCase()}${role ? `[role=${role}]` : ""} "${name.replace(/[⁨⁩]/g, "").trim().slice(0, 50)}"`;
    return { what, shown: focused !== blurred };
  });
}

/** Presses Tab `count` times and returns each stop. */
async function walk(page: Page, count: number): Promise<Stop[]> {
  const stops: Stop[] = [];
  for (let step = 0; step < count; step += 1) {
    await page.keyboard.press("Tab");
    const found = await stop(page);
    if (found) {
      stops.push(found);
    }
  }
  return stops;
}

async function start(page: Page, path = "/"): Promise<void> {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto(path);
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
}

async function openProject(page: Page): Promise<void> {
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
}

async function view(page: Page, name: string): Promise<void> {
  await openProject(page);
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name }).click();
  await page.waitForTimeout(300);
}

const SCREENS: { name: string; path?: string; open: (page: Page) => Promise<void> }[] = [
  { name: "today", open: () => Promise.resolve() },
  { name: "board", open: openProject },
  { name: "list", open: (page) => view(page, "List") },
  { name: "calendar", open: (page) => view(page, "Calendar") },
  { name: "gantt", open: (page) => view(page, "Gantt") },
  { name: "wiki", path: "/?wiki", open: (page) => view(page, "Wiki") },
  { name: "roadmap", open: (page) => view(page, "Roadmap") },
];

test.describe("Tab", () => {
  for (const screen of SCREENS) {
    test(`moves through the ${screen.name} without a trap, and every stop shows its focus`, async ({ page }) => {
      await start(page, screen.path);
      await screen.open(page);
      const stops = await walk(page, 120);
      // A trap holds the focus on one place, or on a few, press after press.
      const last = stops.slice(-40).map((found) => found.what);
      expect(new Set(last).size, `the last stops: ${last.join(" | ")}`).toBeGreaterThan(5);
      expect(new Set(stops.map((found) => found.what)).size).toBeGreaterThan(10);
      const unseen = [...new Set(stops.filter((found) => !found.shown).map((found) => found.what))];
      expect(unseen, "focused without a visible indicator").toEqual([]);
    });
  }
});

test.describe("a dialog", () => {
  test("gives the focus back to what opened it: Preferences", async ({ page }) => {
    await start(page);
    const button = page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "Preferences" });
    await button.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    // The focus stays in the dialog while it is open.
    for (const found of await walk(page, 30)) {
      expect(found.what).not.toBe("");
    }
    expect(await dialog.evaluate((element) => element.contains(document.activeElement))).toBe(true);
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(button).toBeFocused();
  });

  test("gives the focus back to what opened it: the command palette", async ({ page }) => {
    await start(page);
    await openProject(page);
    const card = page.locator("section[data-lane] [data-card]").first();
    await card.focus();
    await page.keyboard.press("Control+k");
    await expect(page.getByRole("dialog")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toBeHidden();
    await expect(card).toBeFocused();
  });

  test("gives the focus back to what opened it: project settings", async ({ page }) => {
    await start(page);
    await openProject(page);
    const menu = page.locator("button[class*=projectButton]").first();
    await menu.focus();
    await page.keyboard.press("Enter");
    await page.getByRole("menuitem", { name: /settings/i }).press("Enter");
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(menu).toBeFocused();
  });

  test("gives the focus back to what opened it: moving a task", async ({ page }) => {
    await start(page);
    await openProject(page);
    await page.locator("section[data-lane] [data-card]").first().dblclick();
    const panel = page.getByRole("complementary", { name: /^Task / });
    const menu = panel.getByRole("button", { name: "Task menu" });
    await menu.focus();
    await page.keyboard.press("Enter");
    await page.getByRole("menuitem", { name: /another project/ }).press("Enter");
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(menu).toBeFocused();
  });
});

test.describe("what can be dragged can be done with keys", () => {
  test("a card moves to the next column with Alt+Right", async ({ page }) => {
    await start(page);
    await openProject(page);
    const card = page.locator("section[data-lane]").first().locator("[data-card]").first();
    const key = (await card.getAttribute("data-card")) ?? "";
    await card.focus();
    await page.keyboard.press("Alt+ArrowRight");
    await expect(page.locator("section[data-lane]").nth(1).locator(`[data-card="${key}"]`)).toBeVisible();
  });

  test("a list column is resized with the arrow keys on its edge", async ({ page }) => {
    await start(page);
    await view(page, "List");
    const edge = page.getByRole("grid").first().getByRole("separator").first();
    const before = Number(await edge.getAttribute("aria-valuenow"));
    await edge.focus();
    await page.keyboard.press("ArrowRight");
    await expect.poll(async () => Number(await page.getByRole("grid").first().getByRole("separator").first().getAttribute("aria-valuenow"))).toBeGreaterThan(before);
  });

  test("a Gantt column and the Gantt table are resized with the arrow keys", async ({ page }) => {
    await start(page);
    await view(page, "Gantt");
    const column = page.getByRole("separator", { name: /Width of the column/ }).first();
    await expect(column).toHaveAttribute("aria-valuenow", /^\d+$/);
    const before = Number(await column.getAttribute("aria-valuenow"));
    await column.focus();
    await page.keyboard.press("ArrowRight");
    await expect.poll(async () => Number(await page.getByRole("separator", { name: /Width of the column/ }).first().getAttribute("aria-valuenow"))).toBeGreaterThan(before);
    const table = page.getByRole("separator", { name: "Width of the table" });
    const wide = Number(await table.getAttribute("aria-valuenow"));
    await table.focus();
    await page.keyboard.press("ArrowRight");
    await expect.poll(async () => Number(await page.getByRole("separator", { name: "Width of the table" }).getAttribute("aria-valuenow"))).toBeGreaterThan(wide);
  });

  test("a calendar task moves a day with Alt+Right", async ({ page }) => {
    await start(page);
    await view(page, "Calendar");
    const bar = page.locator("button[data-task-bar]").first();
    const key = (await bar.getAttribute("data-task-bar")) ?? "";
    const days = (await bar.getAttribute("data-days")) ?? "";
    await bar.focus();
    await page.keyboard.press("Alt+ArrowRight");
    await expect.poll(async () => page.locator(`button[data-task-bar="${key}"]`).first().getAttribute("data-days")).not.toBe(days);
  });

  test("a wiki page moves up with Alt+Up in the page tree", async ({ page }) => {
    await start(page, "/?wiki");
    await view(page, "Wiki");
    const tree = page.getByRole("treegrid", { name: "Pages" });
    await expect(tree.getByRole("row").nth(1)).toBeVisible();
    const second = plain((await tree.getByRole("row").nth(1).getAttribute("aria-label")) ?? (await tree.getByRole("row").nth(1).innerText()));
    await tree.getByRole("row").nth(1).focus();
    await page.keyboard.press("Alt+ArrowUp");
    await expect.poll(async () => plain((await tree.getByRole("row").first().getAttribute("aria-label")) ?? (await tree.getByRole("row").first().innerText()))).toBe(second);
  });
});

test.describe("what can be dragged can be done with one click", () => {
  /** A handle is at least 24 by 24 px; a click on it, without dragging, opens its menu, whose "Wider" widens. */
  async function widensByMenu(page: Page, handle: Locator): Promise<void> {
    const box = await handle.boundingBox();
    expect(box?.width).toBeGreaterThanOrEqual(24);
    expect(box?.height).toBeGreaterThanOrEqual(24);
    const name = (await handle.getAttribute("aria-label")) ?? "";
    const before = Number(await handle.getAttribute("aria-valuenow"));
    await handle.click();
    const menu = page.getByRole("menu", { name });
    await expect(menu).toBeVisible();
    await menu.getByRole("menuitem", { name: "Wider" }).click();
    await expect(menu).toBeHidden();
    await expect.poll(async () => Number(await handle.getAttribute("aria-valuenow"))).toBeGreaterThan(before);
    await expect(handle).toBeFocused();
  }

  /** A width of one's own goes back to the default with "Reset width" in the menu, and with a double-click on the handle. */
  async function resets(page: Page, handle: Locator): Promise<void> {
    const name = (await handle.getAttribute("aria-label")) ?? "";
    const width = async () => Number(await handle.getAttribute("aria-valuenow"));
    const initial = await width();
    // Nothing of one's own yet: nothing to reset.
    await handle.click();
    const menu = page.getByRole("menu", { name });
    await expect(menu.getByRole("menuitem", { name: "Reset width" })).toBeDisabled();
    await page.keyboard.press("Escape");
    await widensByMenu(page, handle);
    await handle.click();
    await menu.getByRole("menuitem", { name: "Reset width" }).click();
    await expect.poll(width).toBe(initial);
    await widensByMenu(page, handle);
    await handle.dblclick();
    await expect.poll(width).toBe(initial);
    await expect(menu).toBeHidden();
  }

  test("a list column's edge", async ({ page }) => {
    await start(page);
    await view(page, "List");
    const handle = page.getByRole("grid").first().getByRole("separator").first();
    await widensByMenu(page, handle);
    await handle.click();
    await page.getByRole("menuitem", { name: "Reset width" }).click();
    await resets(page, handle);
  });

  test("a Gantt column's edge and the Gantt table's edge", async ({ page }) => {
    await start(page);
    await view(page, "Gantt");
    const column = page.getByRole("separator", { name: /Width of the column/ }).first();
    await expect(column).toHaveAttribute("aria-valuenow", /^\d+$/);
    await resets(page, column);
    await resets(page, page.getByRole("separator", { name: "Width of the table" }));
  });

  test("the sidebar's edge", async ({ page }) => {
    await start(page);
    await resets(page, page.getByRole("separator", { name: "Sidebar width" }));
  });
});
