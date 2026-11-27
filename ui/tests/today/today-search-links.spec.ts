// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

// Today, the search page and links to tasks and wiki pages in the editor,
// on the real engine. Tasks, comments and wiki pages for the set-up are made
// through the same operations the command line and MCP use, over the
// bridge. With GK_SCREENSHOT_DIR set, each flow saves a screenshot there.

const token = process.env.GK_BRIDGE_TOKEN ?? "";
const port = process.env.GK_BRIDGE_PORT ?? "";
const PAGE = `/?bridge=${port}&token=${token}`;

test.use({ viewport: { width: 1280, height: 800 } });

async function shot(page: Page, name: string, clip?: Locator) {
  const dir = process.env.GK_SCREENSHOT_DIR;
  if (!dir) {
    return;
  }
  await page.waitForTimeout(300);
  const box = clip ? await clip.boundingBox() : null;
  const margin = 12;
  await page.screenshot({
    path: join(dir, name),
    ...(box ? { clip: { x: Math.max(0, box.x - margin), y: Math.max(0, box.y - margin), width: box.width + 2 * margin, height: box.height + 2 * margin } } : {}),
  });
}

/** Runs one operation on the bridge's engine, from the page (the bridge takes the page's origin only). */
async function op<T>(page: Page, name: string, input: unknown): Promise<T> {
  return page.evaluate(
    ({ port, token, name, input }) =>
      new Promise<T>((resolve, reject) => {
        const socket = new WebSocket(`ws://127.0.0.1:${port}/?token=${encodeURIComponent(token)}`);
        socket.onopen = () => {
          socket.send(JSON.stringify({ id: 1, op: name, input }));
        };
        socket.onmessage = (message) => {
          const reply = JSON.parse(String(message.data)) as { id?: number; ok?: T; err?: unknown };
          if (reply.id !== 1) {
            return;
          }
          socket.close();
          if ("ok" in reply) {
            resolve(reply.ok as T);
          } else {
            reject(new Error(JSON.stringify(reply.err)));
          }
        };
        socket.onerror = () => {
          reject(new Error("the bridge refused the connection"));
        };
      }),
    { port, token, name, input },
  );
}

/** Opens the page and makes a project with one task. */
async function setUp(page: Page, project: { name: string; key: string }, title: string): Promise<string> {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).not.toHaveText("Workspace unavailable");
  await op(page, "project.create", project);
  const created = await op<{ task: { key: string } }>(page, "task.create", { project: project.key, title });
  return created.task.key;
}

const sidebar = (page: Page) => page.getByRole("navigation", { name: "Workspace" });

test("Today shows an overdue task, counts it in the sidebar, and a due date can be typed in words", async ({ page }) => {
  const key = await setUp(page, { name: "Household", key: "HOME" }, "Renew the passport");
  // A date in words, read by the engine.
  await op(page, "task.update", { key, due_date: "yesterday" });
  const soon = await op<{ task: { key: string } }>(page, "task.create", { project: "HOME", title: "Book the car service" });
  await op(page, "task.update", { key: soon.task.key, due_date: "today" });

  await sidebar(page).getByRole("button", { name: /^Today/ }).click();
  const overdue = page.getByRole("list", { name: "Overdue" });
  await expect(overdue.getByRole("button", { name: new RegExp(`${key}.*Renew the passport`) })).toBeVisible();
  await expect(page.getByRole("list", { name: "Due today" })).toContainText("Book the car service");
  await expect(sidebar(page).locator("[data-count=today]")).not.toHaveText("0");
  await shot(page, "today.png");

  // The row opens the task beside the lists; its due date takes words.
  await overdue.getByRole("button", { name: new RegExp(key) }).click();
  const panel = page.getByRole("complementary", { name: new RegExp(`^Task \\W*${key}\\W*$`) });
  // Fluent isolates placeables with invisible marks.
  await panel.getByRole("button", { name: /^\W*Due\W*:/ }).click();
  const words = page.getByRole("textbox", { name: /Due\W* in words/ });
  await words.fill("in 3d");
  await words.press("Enter");
  await expect(overdue).toBeHidden();
});

test("the search page finds a comment, marks the match and filters by kind", async ({ page }) => {
  const key = await setUp(page, { name: "Travel", key: "TRIP" }, "Plan the trip");
  await op(page, "comment.add", { key, body: "The ferry leaves at noon from pier four." });
  await op(page, "wiki.create", { project: "TRIP", title: "Ferry timetable", body: "# Ferry timetable\n\nBoats run every hour." });

  const field = sidebar(page).getByRole("combobox", { name: "Search the workspace" });
  // Typing suggests the task by its title; the first row searches everything.
  await field.fill("trip");
  const suggestions = page.getByRole("listbox", { name: "Suggestions" });
  await expect(suggestions.getByRole("option").first()).toContainText("Search everything");
  await expect(suggestions.getByRole("option", { name: new RegExp(key) })).toBeVisible();
  await field.fill("ferry");
  await field.press("Enter");
  const results = page.getByRole("list", { name: "Search results" });
  const comment = results.locator("[data-hit=comment]");
  await expect(comment).toContainText(key);
  await expect(comment.locator("mark").first()).toHaveText(/ferry/i);
  await expect(results.locator("[data-hit=wiki]")).toContainText("Ferry timetable");
  await shot(page, "search.png");

  await page.getByRole("radio", { name: "Comments" }).click();
  await expect(results.locator("[data-hit=wiki]")).toHaveCount(0);
  await expect(results.locator("[data-hit=comment]")).toHaveCount(1);
  await results.locator("[data-hit=comment]").click();
  await expect(page.getByRole("complementary", { name: new RegExp(`^Task \\W*${key}\\W*$`) })).toBeVisible();
});

test("# inserts a task key and [[ a wiki link, also from the toolbar; the link opens its page", async ({ page }) => {
  const key = await setUp(page, { name: "Garden", key: "GDN" }, "Order the compost");
  const other = await op<{ task: { key: string } }>(page, "task.create", { project: "GDN", title: "Repair the greenhouse door" });
  await op(page, "wiki.create", { project: "GDN", title: "Planting guide", body: "# Planting guide\n\n## Tomatoes\n\nSow in March.\n\n## Beans\n\nSow in May." });

  await page.goto(PAGE);
  await sidebar(page).getByRole("row", { name: /Garden/ }).click();
  await page.locator(`[data-card="${key}"]`).dblclick();
  const panel = page.getByRole("complementary", { name: new RegExp(`^Task \\W*${key}\\W*$`) });
  const description = panel.getByRole("region", { name: "Description" });
  await description.getByRole("button", { name: "Edit" }).click();
  const field = description.getByRole("textbox", { name: "Description" });
  await field.click();

  // "#" and a word of the title: the key replaces the search.
  await page.keyboard.type("After #greenh");
  const options = page.getByRole("listbox");
  await expect(options.getByRole("option", { name: new RegExp(other.task.key) })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(field).toContainText(`After ${other.task.key}`);

  // "[[" with a heading.
  await page.keyboard.type(", see [[Planting guide#Bea");
  await expect(options.getByRole("option", { name: /Planting guide#Beans/ })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(field).toContainText("[[Planting guide#Beans]]");

  // The toolbar's button opens the same search.
  await page.keyboard.type(" and ");
  await description.getByRole("toolbar", { name: "Formatting" }).getByRole("button", { name: "Link wiki page" }).click();
  await page.keyboard.type("plant");
  const guide = options.getByRole("option", { name: /^Planting guide\s*GDN$/ });
  await expect(guide).toBeVisible();
  await shot(page, "links.png", panel);
  // Enter right after the list changes doesn't count (the editor's guard
  // against accidental picks); a click picks it at once.
  await guide.click();
  await expect(field).toContainText("and [[Planting guide]]");
  await description.getByRole("button", { name: "Save" }).click();

  // The reader shows the links; a wiki link opens its page, read only, at the heading.
  await expect(description.locator(`a[data-ref="${other.task.key}"]`)).toBeVisible();
  await description.locator('a[data-wiki="Planting guide#Beans"]').click();
  const dialog = page.getByRole("dialog", { name: "Planting guide" });
  await expect(dialog.getByRole("heading", { name: "Beans" })).toBeVisible();
});
