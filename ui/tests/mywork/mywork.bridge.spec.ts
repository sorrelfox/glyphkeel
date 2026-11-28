// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

import { PAGE, gk, plain, project } from "../planning/engine";

// My work on the engine: cards of two projects in the columns of their
// categories, each with its real status; a drop into In progress, where
// the project has two statuses, asks which, and one Ctrl+Z takes the move
// back; a status rule refuses a move with its message, and the engine
// keeps the task where it was; a saved view shows in the sidebar and stays
// there after a reload.

interface Task {
  key: string;
  status: { name: string };
}

function statusOf(key: string): string {
  const shown = gk("task", "show", key) as { items: Task[] };
  return shown.items[0]?.status.name ?? "";
}

/** Opens My work from the sidebar, in a fresh page. */
async function openMyWork(page: Page): Promise<void> {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "My work", exact: true }).click();
  await expect(page.getByRole("group", { name: "My work across projects" })).toBeVisible();
}

const card = (page: Page, key: string): Locator => page.locator(`[data-lane] [data-card="${key}"]`);

/** The cell a card is in: its lane and its column. */
async function cellOf(locator: Locator): Promise<string> {
  return locator.evaluate((element) => element.closest("[data-lane]")?.getAttribute("data-lane")?.replace("\u001f", "/") ?? "");
}

/** Drags a card onto a cell, as a pointer does. */
async function dragTo(page: Page, from: Locator, to: Locator): Promise<void> {
  const a = await from.boundingBox();
  const b = await to.boundingBox();
  if (!a || !b) {
    throw new Error("the card or the target isn't on screen");
  }
  await page.mouse.move(a.x + a.width / 2, a.y + 12);
  await page.mouse.down();
  await page.mouse.move(a.x + a.width / 2 + 10, a.y + 20, { steps: 4 });
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 20 });
  await page.mouse.up();
}

test.beforeAll(() => {
  // Once per workspace, also when a failed flow starts the worker again.
  const projects = gk("project", "list") as { items: { key: string }[] };
  if (projects.items.some((each) => each.key === "WK")) {
    return;
  }
  project("Work", "WK");
  project("Home", "HM");
  gk("task", "create", "--project", "WK", "--title", "Write the report");
  gk("task", "create", "--project", "WK", "--title", "Review the budget", "--status", "Review");
  gk("task", "create", "--project", "HM", "--title", "Paint the door", "--priority", "high");
});

test("cards of both projects sit in their categories' columns, and a drop with two statuses asks which; one Undo takes it back", async ({ page }) => {
  await openMyWork(page);
  await expect(page.locator("[data-column] h2")).toHaveText(["To do", "In progress", "Done"]);
  // Lanes by project, each card with its real status.
  await expect(page.locator("section[data-swimlane]")).toHaveCount(2);
  expect(await cellOf(card(page, "WK-1"))).toBe("WK/todo");
  expect(await cellOf(card(page, "WK-2"))).toBe("WK/in_progress");
  expect(await cellOf(card(page, "HM-1"))).toBe("HM/todo");
  await expect(card(page, "WK-2").locator("[data-status]")).toHaveText(/Review/);
  await expect(card(page, "WK-1").locator("[data-status]")).toHaveText(/Backlog/);

  // Into In progress: Work has In progress and Review, so a menu asks.
  await dragTo(page, card(page, "WK-1"), page.locator('[data-lane="WK\u001fin_progress"]'));
  const review = page.getByRole("menuitem", { name: "Review", exact: true });
  await expect(page.getByRole("menuitem", { name: "In progress", exact: true })).toBeVisible();
  await review.click();
  await expect.poll(() => statusOf("WK-1")).toBe("Review");
  await expect.poll(() => cellOf(card(page, "WK-1"))).toBe("WK/in_progress");
  await expect(card(page, "WK-1").locator("[data-status]")).toHaveText(/Review/);

  // One Ctrl+Z: back in Backlog.
  await page.locator("body").click({ position: { x: 1, y: 1 } });
  await page.keyboard.press("Control+z");
  await expect.poll(() => statusOf("WK-1")).toBe("Backlog");
  await expect.poll(() => cellOf(card(page, "WK-1"))).toBe("WK/todo");
});

test("a status rule that needs an estimate refuses the move with its message, and the engine keeps the task", async ({ page }) => {
  const required = { required: ["estimate"] };
  gk("project", "rules", "HM", "--statuses", JSON.stringify([{ status: "In progress", rules: required }, { status: "Review", rules: required }]));
  await openMyWork(page);
  await card(page, "HM-1").click();
  await page.keyboard.press("Alt+ArrowRight");
  await expect(page.getByRole("alert").filter({ hasText: /Fill in first: .?Estimate/ })).toBeVisible();
  expect(statusOf("HM-1")).toBe("Backlog");
  expect(await cellOf(card(page, "HM-1"))).toBe("HM/todo");

  // With an estimate from another process, the move is made.
  gk("task", "edit", "HM-1", "--estimate", "1h");
  await openMyWork(page);
  await card(page, "HM-1").click();
  await page.keyboard.press("Alt+ArrowRight");
  await page.getByRole("menuitem", { name: "In progress", exact: true }).click();
  await expect.poll(() => statusOf("HM-1")).toBe("In progress");
});

test("a saved view of My work shows in the sidebar with its query, and stays after a reload", async ({ page }) => {
  await openMyWork(page);
  const query = page.getByRole("textbox", { name: "Filter My work with a query" });
  await query.click();
  await page.keyboard.type("project:WK");
  await page.keyboard.press("Enter");
  await expect(card(page, "HM-1")).toHaveCount(0);
  await expect(card(page, "WK-1")).toBeVisible();
  await page.getByRole("button", { name: "Save as…" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("textbox", { name: "Name" }).fill("Work only");
  await dialog.getByRole("button", { name: "Save view" }).click();
  await expect(dialog).toBeHidden();
  const views = page.getByRole("list", { name: "My work views" });
  await expect(views.getByRole("button", { name: "Work only" })).toBeVisible();
  const saved = gk("view", "list", "--kind", "board_across") as { items: { name: string; query: string }[] };
  expect(saved.items.map((view) => [view.name, view.query])).toEqual([["Work only", "project:WK"]]);

  // After a reload, as after a restart: still there, and it opens filtered.
  await page.reload();
  await page.getByRole("list", { name: "My work views" }).getByRole("button", { name: "Work only" }).click();
  await expect(card(page, "WK-1")).toBeVisible();
  await expect(card(page, "HM-1")).toHaveCount(0);
  expect(plain(await page.getByRole("textbox", { name: "Filter My work with a query" }).innerText())).toContain("project:WK");
});
