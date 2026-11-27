// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

async function openAutomation(page: Page) {
  await page.getByRole("button", { name: /Task Manager.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Automation" }).click();
  return settings;
}

async function axeClean(page: Page) {
  const results = await new AxeBuilder({ page }).withTags(TAGS).include("[role=dialog]").analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`)).toEqual([]);
}

test("a rule is written, tried, saved, acts on a new task, logs its run and can be switched off", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();

  let settings = await openAutomation(page);
  await expect(settings.getByText("No rules yet.")).toBeVisible();
  await axeClean(page);
  await settings.getByRole("button", { name: "New rule" }).click();
  const editor = settings.getByRole("form", { name: "New rule" });
  await editor.getByRole("textbox", { name: "Name" }).fill("Triage crashes");
  await editor.getByRole("textbox", { name: "Only for tasks that match" }).fill("title:crash");
  // The first action adds the project's first label; a second one comments.
  await editor.getByRole("button", { name: "Add an action" }).click();
  await editor.getByRole("group", { name: /^Action .?2.?$/ }).getByRole("textbox", { name: "Text" }).fill("Triaged {key} by {rule}");
  await axeClean(page);

  // The dry run on a task the condition leaves out changes nothing.
  await editor.getByRole("textbox", { name: "Task key" }).fill("TM-1");
  await editor.getByRole("button", { name: "Try", exact: true }).click();
  await expect(editor.getByRole("status")).toContainText("doesn't match the condition");

  await editor.getByRole("button", { name: "Save rule" }).click();
  const row = settings.getByTestId("automation-Triage crashes");
  await expect(row).toBeVisible();
  await expect(row.getByRole("switch", { name: /^Run .?Triage crashes.?$/ })).toBeChecked();
  await page.keyboard.press("Escape");
  await expect(settings).toBeHidden();

  // A new task that matches gets the label and the comment.
  const lane = page.locator("section[data-lane]").first();
  await lane.getByRole("button", { name: "Add task", exact: true }).click();
  await lane.getByRole("textbox", { name: /Title of the new task/ }).fill("Login crash");
  await page.keyboard.press("Enter");
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Login crash" });
  await expect(card).toBeVisible();
  await expect(card).toContainText("board");
  await card.dblclick();
  const panel = page.getByRole("complementary", { name: /^Task / });
  await expect(panel.getByText(/Triaged TM-\d+ by Triage crashes/)).toBeVisible();
  await page.keyboard.press("Escape");

  // The run log shows the run; the switch turns the rule off.
  settings = await openAutomation(page);
  const saved = settings.getByTestId("automation-Triage crashes");
  await expect(saved).toContainText("Done");
  await saved.getByRole("button", { name: /^Runs of .?Triage crashes.?$/ }).click();
  await expect(saved.getByRole("list", { name: /^Runs of .?Triage crashes.?$/ }).getByRole("listitem")).toHaveCount(1);
  await axeClean(page);
  // The switch's own drawing takes the pointer, as everywhere: the keyboard turns it.
  await saved.getByRole("switch", { name: /^Run .?Triage crashes.?$/ }).focus();
  await page.keyboard.press("Space");
  await expect(saved.getByRole("switch", { name: /^Run .?Triage crashes.?$/ })).not.toBeChecked();
});
