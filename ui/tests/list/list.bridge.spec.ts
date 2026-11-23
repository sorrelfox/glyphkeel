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
