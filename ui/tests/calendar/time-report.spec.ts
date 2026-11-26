// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { readFile } from "node:fs/promises";

import { expect, test, type Page } from "@playwright/test";

// The time report on the sample data, with the clock fixed so the logged
// days are the same on every run.

const TODAY = "2026-11-11";

async function openReport(page: Page) {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("navigation").getByText("Time report", { exact: true }).click();
  await expect(page.getByRole("region", { name: "Time report" })).toBeVisible();
}

async function groupBy(page: Page, name: string) {
  await page.getByRole("button", { name: /Group by/ }).click();
  await page.getByRole("listbox").getByRole("option", { name, exact: true }).click();
  await expect(page.getByRole("columnheader", { name, exact: true })).toBeVisible();
}

const rows = (page: Page) => page.locator("[data-report-row]");
const total = (page: Page) => page.locator("[data-report-total]");

test("the year by month, week and person, with totals", async ({ page }) => {
  await openReport(page);
  await page.getByRole("button", { name: "This year" }).click();
  await expect(page.getByLabel("First day")).toHaveValue("2026-01-01");
  await expect(total(page)).toBeVisible();
  const yearTotal = await total(page).textContent();

  await groupBy(page, "Month");
  await expect(rows(page).first()).toHaveAttribute("data-report-row", /^2026-\d\d$/);
  await expect(page.getByRole("row", { name: /November 2026/ })).toBeVisible();
  await expect(total(page)).toHaveText(yearTotal ?? "");

  await groupBy(page, "Week");
  await expect(rows(page).first()).toHaveAttribute("data-report-row", /^2026-\d\d-\d\d$/);

  await groupBy(page, "Person");
  await expect(rows(page)).toHaveCount(1);
  await expect(rows(page).first()).toHaveAttribute("data-report-row", "user:You");
  await expect(total(page)).toHaveText(yearTotal ?? "");
});

test("one project with or without its sub-projects, saved as CSV", async ({ page }) => {
  await openReport(page);
  await page.getByRole("button", { name: "This year" }).click();
  await groupBy(page, "Project");
  const projects = await rows(page).evaluateAll((all) => all.map((row) => (row as HTMLElement).dataset.reportRow));
  expect(projects.length).toBeGreaterThan(1);

  await page.getByRole("button", { name: /^Project/ }).click();
  await page.getByRole("listbox").getByRole("option", { name: "Task Manager", exact: true }).click();
  const subprojects = page.getByRole("switch", { name: "Include sub-projects" });
  await expect(subprojects).toBeChecked();
  await expect.poll(() => rows(page).count()).toBeGreaterThan(1);
  await page.getByText("Include sub-projects", { exact: true }).click();
  await expect(subprojects).not.toBeChecked();
  await expect(rows(page)).toHaveCount(1);
  await expect(rows(page).first()).toHaveAttribute("data-report-row", "TM");

  await page.getByRole("button", { name: "Export" }).click();
  const download = page.waitForEvent("download");
  await page.getByRole("menuitem", { name: "Export the entries as CSV" }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe("time-entries-2026-01-01-2026-12-31.csv");
  const text = await readFile(await file.path(), "utf8");
  const lines = text.split("\r\n").filter((line) => line !== "");
  expect(lines[0]).toBe("date,person_kind,person,project,task,title,minutes,duration,note");
  expect(lines.length).toBeGreaterThan(1);
  expect(lines.slice(1).every((line) => line.split(",")[3] === "TM")).toBe(true);
  await expect(page.getByText("Time report exported")).toBeVisible();
});
