// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

import { PAGE, day, gk, plain, project } from "../planning/engine";

// The deeper planning screens on the engine. Each flow makes its project
// with the command line, works in the page, and reads the result back from
// the engine as another process.

interface Task {
  key: string;
  title: string;
  status: { name: string };
  type?: { name: string };
  fix_version?: { name: string };
  fields?: Record<string, unknown>;
}

function show(key: string): Task {
  const shown = gk("task", "show", key) as { items: Task[] };
  const task = shown.items[0];
  if (!task) {
    throw new Error(`no task ${key}`);
  }
  return task;
}

function list(query: string): Task[] {
  return (gk("task", "list", "--query", query) as { items: Task[] }).items;
}

/** Opens a project's board, chosen from the sidebar, in a fresh page. */
async function openBoard(page: Page, projectName: string): Promise<void> {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await page.getByRole("navigation", { name: "Workspace" }).getByText(projectName, { exact: true }).first().click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
}

const card = (page: Page, title: string) => page.locator("section[data-lane] [data-card]", { hasText: title });
const panelOf = (page: Page) => page.getByRole("complementary", { name: /^Task / });

async function openPanel(page: Page, title: string): Promise<Locator> {
  await card(page, title).dblclick();
  const panel = panelOf(page);
  await expect(panel).toBeVisible();
  return panel;
}

test("the roadmap shows a version made elsewhere, and a task planned in the panel reaches the engine", async ({ page }) => {
  project("Roadmap", "RM");
  gk("version", "create", "--project", "RM", "--name", "1.0", "--due-date", day(14));
  gk("version", "create", "--project", "RM", "--name", "2.0");
  gk("task", "create", "--project", "RM", "--title", "Ship the parser");
  gk("task", "create", "--project", "RM", "--title", "Write the docs");
  gk("task", "edit", "RM-1", "--fix-version", "1.0");
  gk("task", "move", "RM-1", "--status", "Done");
  gk("task", "edit", "RM-2", "--fix-version", "1.0");

  await openBoard(page, "Roadmap");
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Roadmap" }).click();
  const version = (name: string) => page.getByRole("article", { name, exact: true });
  await expect(version("1.0")).toBeVisible();
  await expect.poll(async () => plain(await version("1.0").innerText())).toContain("1 of 2 tasks closed");

  // Planned for 2.0 in the panel: the engine has it, the roadmap moves it.
  await version("1.0").getByRole("button", { name: /^RM-2 / }).click();
  const planned = page.getByRole("button", { name: /^Version: / });
  await planned.click();
  await page.getByRole("menuitemradio", { name: "2.0" }).click();
  await expect.poll(() => show("RM-2").fix_version?.name).toBe("2.0");
  await expect(version("2.0").getByRole("button", { name: /^RM-2 / })).toBeVisible();

  // Another process closes 2.0 with its open task moved back to 1.0: the
  // roadmap follows without a reload.
  gk("version", "edit", "2.0", "--project", "RM", "--status", "closed", "--move-open-tasks-to", "1.0");
  await expect(version("2.0")).toHaveCount(0);
  await expect(version("1.0").getByRole("button", { name: /^RM-2 / })).toBeVisible();
  expect(show("RM-2").fix_version?.name).toBe("1.0");
});

test("a custom field made from the command line is filled in the panel and shown and filtered in the list", async ({ page }) => {
  project("Fields", "FD");
  gk("field", "add", "--project", "FD", "--name", "Story points", "--kind", "integer");
  gk("task", "create", "--project", "FD", "--title", "Estimate me");
  gk("task", "create", "--project", "FD", "--title", "Leave me");

  await openBoard(page, "Fields");
  const panel = await openPanel(page, "Estimate me");
  const input = panel.getByRole("region", { name: "Fields" }).getByRole("spinbutton", { name: /Change.*Story points/ });
  await input.fill("8");
  await input.press("Enter");
  await expect.poll(() => JSON.stringify(show("FD-1").fields ?? {})).toContain("8");
  expect(list("cf.story-points>=5").map((t) => t.key)).toEqual(["FD-1"]);
  await page.keyboard.press("Escape");

  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  await page.getByRole("button", { name: "Columns" }).click();
  await page.getByRole("menuitemcheckbox", { name: "Story points" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("columnheader", { name: /Story points/ })).toBeVisible();
  const query = page.getByRole("textbox", { name: "Filter query" });
  await query.click();
  await page.keyboard.type("cf.story-points>=5");
  await page.keyboard.press("Enter");
  await expect.poll(async () => plain(await page.getByTestId("list-totals").innerText())).toMatch(/^1 task/);
  await expect(page.getByRole("row", { name: /Estimate me/ })).toBeVisible();
  await expect(page.getByRole("row", { name: /Leave me/ })).toHaveCount(0);
});

test("a status rule set from the command line refuses a move in the board, and the engine keeps the task where it was", async ({ page }) => {
  project("Rules", "RU");
  gk("project", "rules", "RU", "--statuses", JSON.stringify([{ status: "Done", rules: { required: ["estimate"] } }]));
  gk("task", "create", "--project", "RU", "--title", "Needs an estimate");

  await openBoard(page, "Rules");
  await card(page, "Needs an estimate").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Move to", exact: true }).click();
  const done = page.getByRole("menuitem", { name: /^Done/ });
  await expect(done).toBeDisabled();
  await expect(done).toContainText(/Fill in first: .?Estimate/);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  expect(show("RU-1").status.name).toBe("Backlog");

  // With an estimate from another process the move is offered and made.
  gk("task", "edit", "RU-1", "--estimate", "2h");
  const panel = await openPanel(page, "Needs an estimate");
  await panel.getByRole("button", { name: /^Status:/ }).click();
  await expect(page.getByRole("menuitem", { name: /^Done/ })).toBeEnabled();
  await page.getByRole("menuitem", { name: /^Done/ }).click();
  await expect.poll(() => show("RU-1").status.name).toBe("Done");
});

test("closing a repeating task in the panel makes the next one in the engine", async ({ page }) => {
  project("Chores", "CH");
  gk("task", "create", "--project", "CH", "--title", "Water the plants", "--due-date", day(0));
  gk("recurrence", "set", "CH-1", "--every", "day");

  await openBoard(page, "Chores");
  const panel = await openPanel(page, "Water the plants");
  await expect(panel.getByTestId("recurrence-note")).toBeVisible();
  await panel.getByRole("button", { name: /^Status:/ }).click();
  await page.getByRole("menuitemradio", { name: "Done" }).click();

  await expect.poll(() => list('project:CH title:"Water the plants" is:open').length).toBe(1);
  const next = list('project:CH title:"Water the plants" is:open')[0];
  expect(next?.key).toBe("CH-2");
  expect(show("CH-1").status.name).toBe("Done");
  // The board shows the next one without a reload.
  await expect(page.locator('section[data-lane] [data-card="CH-2"]')).toBeVisible();
});

test("a small CSV file in the exports folder is checked and imported, and the engine has its tasks", async ({ page }) => {
  project("Imports", "IM");
  const exports = join(process.env.GK_BRIDGE_WORKSPACE ?? "", "exports");
  mkdirSync(exports, { recursive: true });
  writeFileSync(join(exports, "small.csv"), "Title,Status,Priority\r\nFirst import,To do,high\r\nSecond import,In progress,low\r\n");

  await openBoard(page, "Imports");
  await page.locator("button[class*=projectButton]").first().click();
  await page.getByRole("menuitem", { name: /Import from CSV/ }).click();
  const dialog = page.getByRole("dialog");
  // The app chooses the file in the system's dialog; the bridge's page
  // names it, and the person's paths are taken as given.
  await dialog.getByRole("textbox", { name: "CSV file in the exports folder" }).fill(join(exports, "small.csv"));
  await dialog.getByRole("button", { name: "Check" }).click();
  await expect.poll(async () => plain(await dialog.getByTestId("csv-summary").innerText())).toContain("2 new tasks");
  await dialog.getByRole("button", { name: /Import .?2.? tasks/ }).click();
  await expect(dialog).toBeHidden();

  await expect.poll(() => list("project:IM").length).toBe(2);
  const tasks = list("project:IM sort:title");
  expect(tasks.map((t) => [t.title, t.status.name])).toEqual([
    ["First import", "To do"],
    ["Second import", "In progress"],
  ]);
  await expect(card(page, "First import")).toBeVisible();
});

test("the reports count tasks made and closed by another process", async ({ page }) => {
  project("Counted", "CT");
  for (const title of ["One", "Two", "Three"]) {
    gk("task", "create", "--project", "CT", "--title", title);
  }
  gk("task", "move", "CT-1", "--status", "Done");
  const engine = gk("report", "created-closed", "--project", "CT", "--bucket", "day", "--from", day(-29)) as {
    items: { created: number; closed: number }[];
  };
  const created = engine.items.reduce((sum, item) => sum + item.created, 0);
  const closed = engine.items.reduce((sum, item) => sum + item.closed, 0);
  expect([created, closed]).toEqual([3, 1]);

  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await page.getByRole("navigation").getByText("Reports", { exact: true }).click();
  await expect(page.getByRole("region", { name: "Reports" })).toBeVisible();
  await page.getByRole("button", { name: "Last 30 days" }).click();
  await page.getByRole("button", { name: /^Project/ }).click();
  await page.getByRole("listbox").getByRole("option", { name: "Counted", exact: true }).click();
  await page.getByText("Show as a table", { exact: true }).click();
  const totals = page.locator("[data-flow-table=created-closed] tfoot td");
  await expect.poll(async () => plain(await totals.first().textContent()).replace(/\D/g, "")).toBe(String(created));
  await expect.poll(async () => plain(await totals.nth(1).textContent()).replace(/\D/g, "")).toBe(String(closed));
});
