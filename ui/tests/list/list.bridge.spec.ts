// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { readFileSync } from "node:fs";

import { expect, test, type Page } from "@playwright/test";

const BASE = process.env.GK_LIST_UI_URL ?? "";
const PAGE = `${BASE}/?bridge=${process.env.GK_LIST_BRIDGE_PORT ?? ""}&token=${process.env.GK_BRIDGE_TOKEN ?? ""}`;

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

function rows(page: Page) {
  return page.locator("[data-list-scroll] [data-row]");
}

/** Replaces the query field's text, as typed. */
async function typeQuery(page: Page, text: string) {
  const field = page.getByRole("textbox", { name: "Filter query" });
  await field.click();
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Backspace");
  await page.keyboard.type(text);
  // Completion may be open; the query runs without it.
  await page.keyboard.press("Escape");
}

test("the list on the engine: a query with a mistake, a saved view, a bulk change with Undo, CSV export", async ({ page }) => {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");

  // A project with four tasks, made on its board.
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Shed");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  for (const title of ["Buy timber", "Cut boards", "Paint walls", "Fix the roof"]) {
    await page.keyboard.type(title);
    await page.keyboard.press("Enter");
  }
  await page.keyboard.press("Escape");
  await expect(page.locator("section[data-lane] [data-card]")).toHaveCount(4);

  // The list shows them, with totals.
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  await expect(rows(page)).toHaveCount(4);
  await expect.poll(async () => plain(await page.getByTestId("list-totals").innerText())).toContain("4 tasks, 4 open");

  // A query with a mistake: underlined, explained, and the rows stay.
  await typeQuery(page, "stauts:Backlog");
  const field = page.getByRole("textbox", { name: "Filter query" });
  await expect.poll(async () => plain(await page.getByRole("alert").innerText())).toBe('Unknown field "stauts". Did you mean "status"?');
  await expect(field).toHaveAttribute("aria-invalid", "true");
  await expect(page.locator(".cm-q-problem")).toHaveText("stauts");
  await expect(rows(page)).toHaveCount(4);

  // Fixed: the error goes, the query runs, its conditions show as chips.
  await typeQuery(page, "status:Backlog -title:roof");
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(field).not.toHaveAttribute("aria-invalid", "true");
  await expect(rows(page)).toHaveCount(3);
  const conditions = page.getByRole("group", { name: "Conditions" });
  await expect.poll(async () => plain(await conditions.innerText())).toMatch(/Status is Backlog\s+Title is not roof/);
  await expect.poll(async () => plain(await page.getByTestId("list-banner").innerText())).toContain("Filtered: 3 of 4 tasks");

  // Saved as this project's default view: it opens first, before the list draws.
  await page.getByRole("button", { name: "Save as…" }).click();
  const save = page.getByRole("dialog", { name: "Save view" });
  await save.getByRole("textbox", { name: "Name" }).fill("Open shed work");
  await save.getByText("Show it first when the list opens").click();
  await save.getByRole("button", { name: "Save view" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Open shed work" })).toBeVisible();
  await expect(page.getByRole("button", { name: /Saved view: .*Open shed work/ })).toBeVisible();
  await expect(page.getByTestId("list-changed")).toHaveCount(0);
  await page.reload();
  await page.getByRole("row", { name: /Shed/ }).click();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  await expect(page.getByRole("button", { name: /Saved view: .*Open shed work/ })).toBeVisible();
  await expect(field).toHaveText("status:Backlog -title:roof");
  await expect(rows(page)).toHaveCount(3);

  // Bulk change: select the three, preview, change, then Undo as one step.
  await page.getByRole("checkbox", { name: "Select all" }).click();
  await expect(page.getByRole("checkbox", { name: "Select all" })).toBeChecked();
  const bar = page.getByTestId("bulk-bar");
  await expect.poll(async () => plain(await bar.innerText())).toContain("3 selected");
  await bar.getByRole("button", { name: /Priority…/ }).click();
  await page.getByRole("option", { name: "Urgent" }).click();
  await expect.poll(async () => plain(await bar.getByRole("status").innerText())).toBe("Priority: Urgent · 3 of 3 tasks change");
  await bar.getByRole("button", { name: /^Change \W*3\W* tasks$/ }).click();
  await expect(page.locator("[data-row]").getByRole("button", { name: /^Priority of .*Urgent/ })).toHaveCount(3);
  await page.getByRole("status").filter({ hasText: /3\W* tasks changed/ }).getByRole("button", { name: "Undo" }).click();
  await expect(page.locator("[data-row]").getByRole("button", { name: /^Priority of .*Urgent/ })).toHaveCount(0);

  // CSV export of the query: every matching task, with a header row.
  await page.getByRole("button", { name: "Clear selection" }).click();
  await page.getByRole("button", { name: "Export" }).click();
  const downloading = page.waitForEvent("download");
  await page.getByRole("menuitem", { name: "Export as CSV" }).click();
  const download = await downloading;
  expect(download.suggestedFilename()).toMatch(/^open-shed-work-\d{4}-\d{2}-\d{2}\.csv$/);
  const csv = readFileSync(await download.path(), "utf8").trim().split("\r\n");
  expect(csv[0]).toBe("key,title,project,status,type,priority,labels,parent,start,due,estimate,spent,remaining,estimate_minutes,spent_minutes,progress,created,updated,closed");
  expect(csv).toHaveLength(4);
  expect(csv.slice(1).map((line) => line.split(",")[1]).sort()).toEqual(["Buy timber", "Cut boards", "Paint walls"]);
});

/** Where the flow saves its screenshots of grouping and resizing, when asked. */
const SHOTS = process.env.GK_LIST_SHOT_DIR;

/** The titles of the rows shown, in order. */
function titles(page: Page): Promise<string[]> {
  return rows(page).evaluateAll((all) =>
    [...all]
      .sort((a, b) => Number((a as HTMLElement).dataset.index) - Number((b as HTMLElement).dataset.index))
      .map((row) => row.querySelector("[class*='title'] .truncate")?.textContent ?? ""),
  );
}

/** The columns of the header, in order. */
function headerColumns(page: Page): Promise<string[]> {
  return page.locator("[data-list-scroll] [data-column]").evaluateAll((all) => all.map((cell) => (cell as HTMLElement).dataset.column ?? ""));
}

test("the list on the engine: groups, a second sort key, columns dragged and resized and kept after a restart, the default view's first query, JSON export", async ({ page }) => {
  // Every call the page sends to the engine.
  const sent: { op: string; input: Record<string, unknown> }[] = [];
  page.on("websocket", (socket) => {
    socket.on("framesent", (frame) => {
      try {
        const message = JSON.parse(String(frame.payload)) as { op?: string; input?: Record<string, unknown> };
        if (message.op !== undefined) {
          sent.push({ op: message.op, input: message.input ?? {} });
        }
      } catch {
        // Not a call.
      }
    });
  });
  const searches = () => sent.filter((each) => each.op === "task.search" && each.input.limit === 200).map((each) => each.input);

  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Garden");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  for (const title of ["Dig beds", "Plant beans", "Water roses", "Prune hedge"]) {
    await page.keyboard.type(title);
    await page.keyboard.press("Enter");
  }
  await page.keyboard.press("Escape");
  await expect(page.locator("section[data-lane] [data-card]")).toHaveCount(4);
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  await expect(rows(page)).toHaveCount(4);

  // Two urgent tasks, and one moved on to To do.
  for (const title of ["Water roses", "Dig beds"]) {
    await rows(page).filter({ hasText: title }).getByRole("button", { name: /^Priority of / }).click();
    await page.getByRole("menuitemradio", { name: /Urgent/ }).click();
    await expect(rows(page).filter({ hasText: title }).getByRole("button", { name: /^Priority of .*Urgent/ })).toBeVisible();
  }
  await rows(page).filter({ hasText: "Plant beans" }).getByRole("button", { name: /^Status of / }).click();
  await page.getByRole("menuitemradio", { name: /To do/ }).click();
  await expect(rows(page).filter({ hasText: "Plant beans" }).getByRole("button", { name: /^Status of .*To do/ })).toBeVisible();

  // Grouped by status: a header per status with its count; a group collapses.
  await page.getByRole("button", { name: /^Group by: \W*None/ }).click();
  await page.getByRole("menuitemradio", { name: "Status" }).click();
  const groups = page.locator("[data-list-scroll] [data-group]");
  await expect(groups).toHaveCount(2);
  expect(await groups.evaluateAll((all) => all.map((group) => (group as HTMLElement).dataset.group))).toEqual(["Backlog", "To do"]);
  await expect.poll(async () => plain(await groups.first().innerText()).replace(/\s+/g, " ")).toBe("Backlog 3");
  expect(searches().at(-1)?.group_by).toBe("status");
  if (SHOTS) {
    await page.screenshot({ path: `${SHOTS}/list-grouped-by-status.png` });
  }
  await groups.first().getByRole("button").click();
  await expect(groups.first().getByRole("button")).toHaveAttribute("aria-expanded", "false");
  await expect(rows(page)).toHaveCount(1);
  await groups.first().getByRole("button").click();
  await expect(rows(page)).toHaveCount(4);
  await page.getByRole("button", { name: /^Group by: \W*Status/ }).click();
  await page.getByRole("menuitemradio", { name: "None" }).click();
  await expect(groups).toHaveCount(0);

  // Priority, highest first, then by title: Shift-click adds the second key.
  const priority = page.getByRole("button", { name: /^Sort by \W*Priority\W*$/ });
  await priority.click();
  await priority.click();
  await expect.poll(() => searches().at(-1)?.sort).toBe("-priority");
  await page.getByRole("button", { name: /^Sort by \W*Title\W*$/ }).click({ modifiers: ["Shift"] });
  await expect.poll(() => searches().at(-1)?.sort).toBe("-priority,title");
  await expect.poll(() => titles(page)).toEqual(["Dig beds", "Water roses", "Plant beans", "Prune hedge"]);
  await expect(page.getByRole("button", { name: /^Sort by \W*Title\W*$/ })).toHaveAttribute("aria-description", /Sort key \W*2\W*, ascending/);

  // The due date column is dragged to the front.
  const before = await headerColumns(page);
  const due = page.locator("[data-list-scroll] [data-column='due']");
  const key = page.locator("[data-list-scroll] [data-column='key']");
  const from = await due.boundingBox();
  const to = await key.boundingBox();
  if (!from || !to) {
    throw new Error("no header");
  }
  await page.mouse.move(from.x + from.width / 3, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(to.x + 4, to.y + to.height / 2, { steps: 8 });
  await page.mouse.up();
  await expect.poll(() => headerColumns(page)).toEqual(["due", ...before.filter((column) => column !== "due")]);

  // The title column is made 120 pixels wider at its edge.
  const edge = page.getByRole("separator", { name: /^Width of \W*Title/ });
  const titleWidth = Number(await edge.getAttribute("aria-valuenow"));
  const handle = await edge.boundingBox();
  if (!handle) {
    throw new Error("no column edge");
  }
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(handle.x + handle.width / 2 + 120, handle.y + handle.height / 2, { steps: 8 });
  if (SHOTS) {
    await page.screenshot({ path: `${SHOTS}/list-column-resizing.png` });
  }
  await page.mouse.up();
  await expect.poll(async () => Number(await edge.getAttribute("aria-valuenow"))).toBeGreaterThanOrEqual(titleWidth + 115);
  const wider = Number(await edge.getAttribute("aria-valuenow"));
  expect(sent.filter((each) => each.op === "list.layout.update").length).toBeGreaterThanOrEqual(2);

  // After a restart the list has the same columns, in the same order and widths.
  await page.reload();
  await page.getByRole("row", { name: /Garden/ }).click();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  await expect(rows(page)).toHaveCount(4);
  expect(await headerColumns(page)).toEqual(["due", ...before.filter((column) => column !== "due")]);
  await expect(page.getByRole("separator", { name: /^Width of \W*Title/ })).toHaveAttribute("aria-valuenow", String(wider));
  if (SHOTS) {
    await page.screenshot({ path: `${SHOTS}/list-columns-after-restart.png` });
  }

  // A default view: after the next restart, the very first query is its own.
  await typeQuery(page, "-title:hedge");
  await expect(rows(page)).toHaveCount(3);
  await page.getByRole("button", { name: "Save as…" }).click();
  const save = page.getByRole("dialog", { name: "Save view" });
  await save.getByRole("textbox", { name: "Name" }).fill("Garden work");
  await save.getByText("Show it first when the list opens").click();
  await save.getByRole("button", { name: "Save view" }).click();
  await expect(page.getByRole("button", { name: /Saved view: .*Garden work/ })).toBeVisible();
  await page.reload();
  sent.length = 0;
  await page.getByRole("row", { name: /Garden/ }).click();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  await expect(rows(page)).toHaveCount(3);
  const project = searches()[0]?.project;
  expect(project).toBeDefined();
  const listed = searches().filter((input) => input.project === project);
  expect(listed[0]?.query).toBe("-title:hedge");
  expect(listed.every((input) => input.query === "-title:hedge")).toBe(true);

  // JSON export of the view: the engine's records of every matching task, with the query.
  await page.getByRole("button", { name: "Export" }).click();
  const downloading = page.waitForEvent("download");
  await page.getByRole("menuitem", { name: "Export as JSON" }).click();
  const download = await downloading;
  expect(download.suggestedFilename()).toMatch(/^garden-work-\d{4}-\d{2}-\d{2}\.json$/);
  const exported = JSON.parse(readFileSync(await download.path(), "utf8")) as { query: string; project?: string; total: number; items: { key: string; title: string; priority: string }[] };
  expect(exported.query).toBe("-title:hedge");
  expect(exported.project).toBe(project);
  expect(exported.total).toBe(3);
  expect(exported.items.map((item) => item.title).sort()).toEqual(["Dig beds", "Plant beans", "Water roses"]);
  expect(exported.items.find((item) => item.title === "Dig beds")?.priority).toBe("urgent");
});
